//! The preview of the inference in the differential of a changeset (feature `pg`; spec
//! 010, REQ-010-55, S36; design « Aperçu de l'inférence »).
//!
//! When a changeset writes a rule of inference (creation, modification, closure), the
//! contribution of the module adds to its section, simulated or applied, one row
//! `policy.inference.summary` per axis a touched rule aims at, and one row
//! `policy.inference.node` per node whose expectation or deviation changes between the
//! state before the changeset and the state after it. The scope is the subtrees of the
//! nodes a touched root matches, before or after; the criteria an index serves (domain,
//! prefix) bound it in SQL, the others are evaluated in memory. Beyond [`SCOPE_MAX`]
//! nodes, the summary gives the count of the scope only. No anomaly is raised or closed
//! here: the background task `policy.inference_check` keeps doing it after application.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use kartenn_service::ObjectKind;
use kartenn_service::module::{DiffContext, DiffRow};
use kartenn_service::port::{ObjectView, StoreError};
use serde_json::{Value, json};
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgConnection, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::infer::{chains, rules_in, tested};
use crate::inference::{Criterion, Expectation, InferenceRule, Node, expected, mismatch};
use crate::load::State;
use crate::model::{AxisKey, AxisValue, Key};
use crate::module::INFERENCE_RULE;
use crate::pg::store;
use crate::rules::INFERENCE_MISMATCH;

/// The kinds of the rows of the preview, in the section `policy.combinations`.
pub(crate) const SUMMARY: &str = "policy.inference.summary";
pub(crate) const NODE: &str = "policy.inference.node";

/// Most nodes in the scope of a preview (REQ-010-55): beyond, the count alone.
pub const SCOPE_MAX: u64 = 100_000;

static LIMIT: AtomicU64 = AtomicU64::new(SCOPE_MAX);

/// Lowers the bound of the scope, for the tests of the truncation (a test runs in its own
/// process).
#[doc(hidden)]
pub fn set_scope_limit(limit: u64) {
    LIMIT.store(limit, Ordering::Relaxed);
}

/// What a node shows on a side of the changeset.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Side {
    /// The single expected set.
    expected: Option<Vec<String>>,
    /// The sets at a tie (REQ-010-45).
    candidates: Vec<Vec<String>>,
    /// The rule kept: the deepest exception applied, else the root.
    rule: Option<String>,
    /// The declared set the node resolves, and its origin.
    declared: Option<Vec<String>>,
    origin: Option<Uuid>,
    /// The deviation of REQ-010-47.
    deviates: bool,
}

fn values(set: &BTreeSet<AxisValue>) -> Vec<String> {
    set.iter().map(|v| v.as_str().to_owned()).collect()
}

fn side(expectation: &Expectation, declared: Option<&(Option<Uuid>, Vec<String>)>) -> Side {
    let (expected, candidates, rule) = match expectation {
        Expectation::None => (None, Vec::new(), None),
        Expectation::Single(e) => (
            Some(values(&e.values)),
            Vec::new(),
            Some(e.exceptions.last().unwrap_or(&e.rule).as_str().to_owned()),
        ),
        Expectation::Multiple(all) => (None, all.iter().map(|e| values(&e.values)).collect(), None),
    };
    let (origin, set) = declared.map_or((None, None), |(origin, set)| {
        (*origin, (!set.is_empty()).then(|| set.clone()))
    });
    let deviates = set.as_ref().is_some_and(|set| {
        let set: BTreeSet<AxisValue> = set.iter().map(|v| AxisValue::new(v.as_str())).collect();
        mismatch(&set, expectation)
    });
    Side {
        expected,
        candidates,
        rule,
        declared: set,
        origin,
        deviates,
    }
}

fn shown(axis: &AxisKey, revision: i64, side: &Side) -> Value {
    json!({
        "axis": axis.to_string(),
        "revision": revision,
        "expected": side.expected,
        "candidates": side.candidates,
        "rule": side.rule,
        "declared": side.declared,
        "origin": side.origin,
        "deviates": side.deviates,
    })
}

/// The rules of inference the changeset writes, by identifier.
fn touched(context: &DiffContext<'_>) -> Vec<Uuid> {
    context
        .touched
        .iter()
        .filter_map(|(kind, id)| match kind {
            ObjectKind::Module(k) if k.as_str() == INFERENCE_RULE => Some(*id),
            _ => None,
        })
        .collect()
}

/// The date of the evaluation: the earliest start (or end, for a closure) the changeset
/// writes from now on, now by default (a retroactive write is evaluated now).
async fn valid_at(conn: &mut PgConnection, ids: &[Uuid]) -> Result<OffsetDateTime, StoreError> {
    sqlx::query_scalar(
        "SELECT coalesce(min(t), now()) FROM (
           SELECT lower(valid_period) AS t FROM policy.policy_inference_rule
            WHERE id = ANY($1) AND sys_start = now()
           UNION ALL
           SELECT upper(valid_period) FROM policy.policy_inference_rule
            WHERE id = ANY($1) AND sys_start = now() AND NOT upper_inf(valid_period)) x
          WHERE t >= now()",
    )
    .bind(ids)
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))
}

/// The keys of the rules `ids`, before or after the changeset.
async fn keys_of(conn: &mut PgConnection, ids: &[Uuid]) -> Result<Vec<Key>, StoreError> {
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT key FROM policy.policy_inference_rule WHERE id = ANY($1)
         UNION SELECT key FROM policy.policy_inference_rule_history WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(keys.iter().filter_map(|k| Key::new(k).ok()).collect())
}

/// The revisions of the rules of inference before and after the changeset.
async fn revisions(conn: &mut PgConnection, changeset: Uuid) -> Result<(i64, i64), StoreError> {
    let (after, by): (i64, Option<Uuid>) =
        sqlx::query_as("SELECT inference, inference_by FROM policy.policy_revision")
            .fetch_one(conn)
            .await
            .map_err(|e| store(&e))?;
    Ok((
        if by == Some(changeset) {
            after - 1
        } else {
            after
        },
        after,
    ))
}

/// The bound a root puts on its scope in SQL: its domains and its prefixes, `None` when
/// no criterion bounds it so.
type Bound = (Option<Vec<Uuid>>, Option<Vec<String>>);

fn bound(rule: &InferenceRule) -> Bound {
    let mut domains: Option<Vec<Uuid>> = None;
    let mut prefixes: Option<Vec<String>> = None;
    for criterion in &rule.criteria {
        match criterion {
            Criterion::Domain(ds) => domains = Some(ds.clone()),
            Criterion::Within(ps) | Criterion::Equals(ps) => {
                prefixes = Some(ps.iter().map(ToString::to_string).collect());
            }
            _ => {}
        }
    }
    (domains, prefixes)
}

/// The nodes of the addressing tree (domains, prefixes, addresses) valid at `at` that one
/// of `bounds` may reach, as `select` reads them.
fn candidates_sql(at: OffsetDateTime, bounds: &[Bound], select: &str) -> QueryBuilder<Postgres> {
    let mut q = QueryBuilder::<Postgres>::new(format!(
        "{select} FROM (
           SELECT id, 'domain' AS kind, id AS domain_id, NULL::text AS value, NULL::cidr AS p
             FROM addressing_domain WHERE valid_period @> "
    ));
    q.push_bind(at);
    q.push(
        " UNION ALL
           SELECT id, 'prefix', domain_id, prefix::text, prefix FROM prefix
            WHERE valid_period @> ",
    );
    q.push_bind(at);
    q.push(
        " UNION ALL
           SELECT id, 'address', domain_id, host(address),
                  set_masklen(address, CASE family(address) WHEN 4 THEN 32 ELSE 128 END)::cidr
             FROM address WHERE valid_period @> ",
    );
    q.push_bind(at);
    q.push(") n WHERE FALSE");
    for (domains, prefixes) in bounds {
        q.push(" OR (TRUE");
        if let Some(domains) = domains {
            q.push(" AND n.domain_id = ANY(");
            q.push_bind(domains.clone());
            q.push(")");
        }
        if let Some(prefixes) = prefixes {
            q.push(" AND EXISTS (SELECT FROM unnest(");
            q.push_bind(prefixes.clone());
            q.push("::cidr[]) w WHERE n.p <<= w)");
        }
        q.push(")");
    }
    q
}

/// The declared set each of `ids` resolves on `axis` at `at`, with its origin, in `state`.
async fn declared(
    conn: &mut PgConnection,
    ids: &[Uuid],
    axis: &AxisKey,
    at: OffsetDateTime,
    state: State,
) -> Result<BTreeMap<Uuid, (Option<Uuid>, Vec<String>)>, StoreError> {
    let relation = |table: &str, columns: &str| match state {
        State::After => format!("(SELECT {columns} FROM {table})"),
        State::Before => format!(
            "(SELECT {columns} FROM {table} WHERE sys_start < now()
              UNION ALL
              SELECT {columns} FROM {table}_history WHERE sys_start < now() AND sys_end >= now())"
        ),
    };
    let origins = relation(
        "property_origin",
        "object_id, property_id, origin_object_id, valid_period",
    );
    let assignments = relation(
        "property_assignment",
        "object_id, property_id, kind, v_uuid, v_uuids, valid_period",
    );
    let sql = format!(
        "SELECT o.id, po.origin_object_id,
                coalesce(array_agg(e.key ORDER BY e.key) FILTER (WHERE e.key IS NOT NULL), '{{}}')
           FROM unnest($1::uuid[]) AS o(id)
           JOIN property_declaration d ON d.key = $2 AND d.valid_period @> $3
           JOIN {origins} po ON po.object_id = o.id AND po.property_id = d.id
                            AND po.valid_period @> $3
           LEFT JOIN {assignments} a ON a.object_id = po.origin_object_id
                            AND a.property_id = d.id AND a.kind = 'value'
                            AND a.valid_period @> $3
           LEFT JOIN property_enum_value e ON e.id = a.v_uuid OR e.id = ANY(a.v_uuids)
          GROUP BY o.id, po.origin_object_id"
    );
    let rows: Vec<(Uuid, Option<Uuid>, Vec<String>)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(ids)
        .bind(axis.to_string())
        .bind(at)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(id, origin, set)| (id, (origin, set)))
        .collect())
}

/// The views of `ids` (domains, prefixes, addresses) valid at `at`.
async fn views(
    conn: &mut PgConnection,
    ids: &[Uuid],
    at: OffsetDateTime,
) -> Result<Vec<ObjectView>, StoreError> {
    let rows: Vec<(Uuid, String, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id, 'domain', id, NULL::text FROM addressing_domain
          WHERE id = ANY($1) AND valid_period @> $2
         UNION ALL
         SELECT id, 'prefix', domain_id, prefix::text FROM prefix
          WHERE id = ANY($1) AND valid_period @> $2
         UNION ALL
         SELECT id, 'address', domain_id, host(address) FROM address
          WHERE id = ANY($1) AND valid_period @> $2",
    )
    .bind(ids)
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(id, kind, domain, value)| ObjectView {
            id,
            kind,
            domain,
            value,
            properties: BTreeMap::new(),
        })
        .collect())
}

/// The rules of `axis` among `rules`.
fn of_axis(rules: &[InferenceRule], axis: &AxisKey) -> Vec<InferenceRule> {
    rules.iter().filter(|r| &r.axis == axis).cloned().collect()
}

/// The chains of `views` at `at`, for the rules `rules`.
async fn chains_of(
    conn: &mut PgConnection,
    views: &[ObjectView],
    rules: &[InferenceRule],
    at: OffsetDateTime,
) -> Result<Vec<Vec<Node>>, StoreError> {
    chains(conn, views, &tested(rules), Some(at)).await
}

/// The origins among `origins` that carry an open `policy.inference.mismatch`.
async fn flagged(conn: &mut PgConnection, origins: &[Uuid]) -> Result<BTreeSet<Uuid>, StoreError> {
    let found: Vec<Uuid> = sqlx::query_scalar(
        "SELECT object_id FROM anomaly
          WHERE rule_id = $1 AND object_id = ANY($2) AND closed_at IS NULL",
    )
    .bind(INFERENCE_MISMATCH.id)
    .bind(origins)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(found.into_iter().collect())
}

/// The heirs of `origins` on `axis` after the changeset, at `at`.
async fn heirs(
    conn: &mut PgConnection,
    origins: &[Uuid],
    axis: &AxisKey,
    at: OffsetDateTime,
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT o.origin_object_id, o.object_id FROM property_origin o
           JOIN property_declaration d ON d.id = o.property_id AND d.key = $2
                                      AND d.valid_period @> $3
          WHERE o.origin_object_id = ANY($1) AND o.valid_period @> $3",
    )
    .bind(origins)
    .bind(axis.to_string())
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// What the preview reads for one axis.
struct Axis<'a> {
    key: &'a AxisKey,
    at: OffsetDateTime,
    before: Vec<InferenceRule>,
    after: Vec<InferenceRule>,
    /// The touched roots, before and after.
    roots: Vec<InferenceRule>,
    revisions: (i64, i64),
}

impl Axis<'_> {
    fn summary(&self, members: Value) -> DiffRow {
        let mut after = json!({
            "axis": self.key.to_string(),
            "valid_at": kartenn_core::audit::rfc3339(kartenn_core::period::Instant(
                i64::try_from(self.at.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MAX),
            )),
        });
        if let (Value::Object(after), Value::Object(members)) = (&mut after, members) {
            after.extend(members);
        }
        DiffRow {
            object: Uuid::nil(),
            kind: SUMMARY.into(),
            before: None,
            after: Some(after),
            period: None,
        }
    }

    /// The rows of the axis.
    async fn rows(&self, conn: &mut PgConnection) -> Result<Vec<DiffRow>, StoreError> {
        let limit = LIMIT.load(Ordering::Relaxed);
        let bounds: Vec<_> = self.roots.iter().map(bound).collect();
        let scoped = i64::try_from(limit).unwrap_or(i64::MAX).saturating_add(1);
        let mut count = candidates_sql(self.at, &bounds, "SELECT count(*) FROM (SELECT n.id");
        count.push(" LIMIT ");
        count.push_bind(scoped);
        count.push(") c");
        let counted: i64 = count
            .build_query_scalar()
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
        if u64::try_from(counted).unwrap_or(u64::MAX) > limit {
            return Ok(vec![self.summary(json!({
                "scope": counted, "scope_exact": false, "truncated": true,
                "changed": null, "opens": null, "closes": null,
                "origins_opened": null, "origins_closed": null,
            }))]);
        }
        let mut list = candidates_sql(self.at, &bounds, "SELECT n.id");
        list.push(" ORDER BY n.domain_id, n.p NULLS FIRST, n.id");
        let ids: Vec<Uuid> = list
            .build_query_scalar()
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
        let order: BTreeMap<Uuid, usize> = ids.iter().enumerate().map(|(n, id)| (*id, n)).collect();
        let found = views(conn, &ids, self.at).await?;
        let mut every = self.before.clone();
        every.extend(self.after.iter().cloned());
        let chained = chains_of(conn, &found, &every, self.at).await?;
        let scope: Vec<(Uuid, &Vec<Node>)> = found
            .iter()
            .zip(&chained)
            .filter(|(_, chain)| !chain.is_empty() && self.roots.iter().any(|r| r.reaches(chain)))
            .map(|(view, chain)| (view.id, chain))
            .collect();
        let in_scope: Vec<Uuid> = scope.iter().map(|(id, _)| *id).collect();
        let before_declared = declared(conn, &in_scope, self.key, self.at, State::Before).await?;
        let after_declared = declared(conn, &in_scope, self.key, self.at, State::After).await?;
        let mut changed: Vec<(Uuid, Side, Side)> = Vec::new();
        for (id, chain) in &scope {
            let before = side(
                &expected(&self.before, self.key, chain),
                before_declared.get(id),
            );
            let after = side(
                &expected(&self.after, self.key, chain),
                after_declared.get(id),
            );
            let same = before.expected == after.expected
                && before.candidates == after.candidates
                && before.deviates == after.deviates;
            if !same {
                changed.push((*id, before, after));
            }
        }
        let opening: Vec<&(Uuid, Side, Side)> = changed
            .iter()
            .filter(|(_, b, a)| !b.deviates && a.deviates)
            .collect();
        let closing: Vec<&(Uuid, Side, Side)> = changed
            .iter()
            .filter(|(_, b, a)| b.deviates && !a.deviates)
            .collect();
        let (origins_opened, origins_closed) = self
            .origins(conn, &opening, &closing, &after_declared)
            .await?;
        let mut rows = vec![self.summary(json!({
            "scope": scope.len(), "scope_exact": true, "truncated": false,
            "changed": changed.len(), "opens": opening.len(), "closes": closing.len(),
            "origins_opened": origins_opened, "origins_closed": origins_closed,
        }))];
        let rank = |(_, b, a): &&(Uuid, Side, Side)| match (b.deviates, a.deviates) {
            (false, true) => 0,
            (true, false) => 1,
            _ => 2,
        };
        let mut sorted: Vec<&(Uuid, Side, Side)> = changed.iter().collect();
        sorted.sort_by_key(|row| (rank(row), order.get(&row.0).copied().unwrap_or(usize::MAX)));
        rows.extend(sorted.into_iter().map(|(id, before, after)| DiffRow {
            object: *id,
            kind: NODE.into(),
            before: Some(shown(self.key, self.revisions.0, before)),
            after: Some(shown(self.key, self.revisions.1, after)),
            period: None,
        }));
        Ok(rows)
    }

    /// The origins that gain an heir in deviation without an open anomaly, and those
    /// whose open anomaly loses every heir in deviation.
    async fn origins(
        &self,
        conn: &mut PgConnection,
        opening: &[&(Uuid, Side, Side)],
        closing: &[&(Uuid, Side, Side)],
        after_declared: &BTreeMap<Uuid, (Option<Uuid>, Vec<String>)>,
    ) -> Result<(usize, usize), StoreError> {
        let gaining: BTreeSet<Uuid> = opening.iter().filter_map(|(_, _, a)| a.origin).collect();
        let losing: BTreeSet<Uuid> = closing.iter().filter_map(|(_, b, _)| b.origin).collect();
        let all: Vec<Uuid> = gaining.union(&losing).copied().collect();
        if all.is_empty() {
            return Ok((0, 0));
        }
        let flagged = flagged(conn, &all).await?;
        let opened = gaining.iter().filter(|o| !flagged.contains(o)).count();
        let candidates: Vec<Uuid> = losing
            .iter()
            .filter(|o| flagged.contains(o))
            .copied()
            .collect();
        if candidates.is_empty() {
            return Ok((opened, 0));
        }
        // Every heir of these origins after the changeset, in the scope or not.
        let pairs = heirs(conn, &candidates, self.key, self.at).await?;
        let ids: Vec<Uuid> = pairs.iter().map(|(_, heir)| *heir).collect();
        let found = views(conn, &ids, self.at).await?;
        let chained = chains_of(conn, &found, &self.after, self.at).await?;
        let mut declared_after = declared(conn, &ids, self.key, self.at, State::After).await?;
        declared_after.extend(after_declared.iter().map(|(k, v)| (*k, v.clone())));
        let mut deviating: BTreeSet<Uuid> = BTreeSet::new();
        for (view, chain) in found.iter().zip(&chained) {
            if chain.is_empty() {
                continue;
            }
            let after = side(
                &expected(&self.after, self.key, chain),
                declared_after.get(&view.id),
            );
            if after.deviates
                && let Some(origin) = after.origin
            {
                deviating.insert(origin);
            }
        }
        let closed = candidates.iter().filter(|o| !deviating.contains(o)).count();
        Ok((opened, closed))
    }
}

/// The rows of the preview for the changeset of `context`: none unless it writes a rule
/// of inference.
///
/// # Errors
/// A failure of the database.
pub(crate) async fn rows(
    conn: &mut PgConnection,
    context: &DiffContext<'_>,
) -> Result<Vec<DiffRow>, StoreError> {
    let ids = touched(context);
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let at = valid_at(conn, &ids).await?;
    let keys = keys_of(conn, &ids).await?;
    let before = rules_in(conn, Some(at), State::Before).await?;
    let after = rules_in(conn, Some(at), State::After).await?;
    let roots: Vec<&InferenceRule> = before
        .iter()
        .chain(&after)
        .filter(|r| keys.iter().any(|k| r.holds(k)))
        .collect();
    let axes: BTreeSet<AxisKey> = roots.iter().map(|r| r.axis.clone()).collect();
    let revisions = revisions(conn, context.changeset).await?;
    let mut rows = Vec::new();
    for axis in &axes {
        let preview = Axis {
            key: axis,
            at,
            before: of_axis(&before, axis),
            after: of_axis(&after, axis),
            roots: roots
                .iter()
                .filter(|r| &r.axis == axis)
                .map(|r| (*r).clone())
                .collect(),
            revisions,
        };
        rows.extend(preview.rows(conn).await?);
    }
    // The summaries first, then the nodes (design « Aperçu de l'inférence »).
    rows.sort_by_key(|r| r.kind != SUMMARY);
    Ok(rows)
}
