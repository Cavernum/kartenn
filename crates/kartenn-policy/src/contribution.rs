//! The contribution of the module to the differential of a changeset (feature `pg`;
//! REQ-010-18, 19; design 010 « Différentiel de politique ») and the measures its
//! approval triggers read (REQ-010-20).
//!
//! The contract gives a module one section: `policy.combinations` carries, by the kind of
//! its rows, the summary of the policy differential (`policy.summary`), the combinations
//! whose verdict of principle flips (`policy.combination`) and the nodes reclassified on
//! an axis with their heirs (`policy.reclassified`).

use std::collections::{BTreeMap, BTreeSet};

use kartenn_service::ObjectKind;
use kartenn_service::module::{CheckContext, DiffContext, DiffRow, DiffSection};
use kartenn_service::port::StoreError;
use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::diff::{Candidate, Flip, PolicyDiff, Slot, diff};
use crate::evaluate::{Decisive, Side, Verdict};
use crate::load::{self, State};
use crate::module::ENVIRONMENT;
use crate::pg::store;
use crate::snapshot::PolicySnapshot;

/// The section of the module.
pub(crate) const SECTION: &str = "policy.combinations";
/// The kinds of its rows.
const SUMMARY: &str = "policy.summary";
const COMBINATION: &str = "policy.combination";
const RECLASSIFIED: &str = "policy.reclassified";

/// The measures of the module (REQ-010-20), qualified `policy.<name>` by the core.
pub(crate) const WRITE: &str = "write";
pub(crate) const OPENS: &str = "opens";
pub(crate) const CLOSES: &str = "closes";
pub(crate) const RECLASSIFY: &str = "reclassify";

fn verdict(v: Verdict) -> &'static str {
    match v {
        Verdict::Allowed => "allowed",
        Verdict::AllowedPending => "allowed_pending",
        Verdict::Denied => "denied",
    }
}

fn side(s: Side) -> &'static str {
    match s {
        Side::Source => "source",
        Side::Destination => "destination",
    }
}

fn decisive(d: &Decisive) -> Value {
    match d {
        Decisive::Unclassified { side: s, axis } => {
            json!({ "unclassified": { "side": side(*s), "axis": axis.to_string() } })
        }
        Decisive::Rule(rule) => json!({ "rule": rule.as_str() }),
        Decisive::Pattern { pattern, rule } => {
            json!({ "pattern": pattern.as_str(), "rule": rule.as_str() })
        }
        Decisive::Pending { rule, expected } => json!({
            "pending": rule.as_str(),
            "expected": expected.iter().map(crate::model::Key::as_str).collect::<Vec<_>>(),
        }),
        Decisive::NoRule => json!("no_rule"),
        Decisive::UnknownService => json!("unknown_service"),
    }
}

/// A side of a combination: per axis, the classes of its values (`*other`: a value no
/// rule cites; `*same`: one uncited value both sides share).
fn classes(side: &BTreeMap<kartenn_core::properties::PropertyKey, BTreeSet<Slot>>) -> Value {
    side.iter()
        .map(|(axis, slots)| {
            let slots: Vec<Value> = slots
                .iter()
                .map(|s| match s {
                    Slot::Value(v) => v.as_str().into(),
                    Slot::Other => "*other".into(),
                    Slot::Shared => "*same".into(),
                })
                .collect();
            (axis.to_string(), Value::Array(slots))
        })
        .collect::<Map<_, _>>()
        .into()
}

fn combination(candidate: &Candidate, principle: &(Verdict, Decisive)) -> Value {
    json!({
        "service": candidate.service.as_str(),
        "source": classes(&candidate.source),
        "destination": classes(&candidate.destination),
        "verdict": verdict(principle.0),
        "decisive": decisive(&principle.1),
    })
}

fn flip_row(flip: &Flip) -> DiffRow {
    DiffRow {
        object: Uuid::nil(),
        kind: COMBINATION.into(),
        before: Some(combination(&flip.candidate, &flip.before)),
        after: Some(combination(&flip.candidate, &flip.after)),
        period: None,
    }
}

fn summary_row(policy: &PolicyDiff) -> DiffRow {
    DiffRow {
        object: Uuid::nil(),
        kind: SUMMARY.into(),
        before: None,
        after: Some(json!({
            "candidates": policy.candidates,
            "truncated": policy.truncated,
            "opens": policy.opens(),
            "closes": policy.closes(),
            "pending": policy.pending(),
        })),
        period: None,
    }
}

/// Whether the changeset writes an object of the policy.
fn writes_policy(touched: &[(ObjectKind, Uuid)]) -> bool {
    touched
        .iter()
        .any(|(kind, _)| matches!(kind, ObjectKind::Module(k) if k.module() == "policy"))
}

/// The axes of the policy, designated, at `at` with the changeset's writes.
async fn axes(conn: &mut PgConnection, at: OffsetDateTime) -> Result<Vec<String>, StoreError> {
    let mut keys: Vec<String> =
        sqlx::query_scalar("SELECT property_key FROM policy.policy_axis WHERE valid_period @> $1")
            .bind(at)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
    if !keys.iter().any(|k| k == ENVIRONMENT) {
        keys.push(ENVIRONMENT.to_owned());
    }
    Ok(keys)
}

/// The nodes reclassified on an axis by the changeset, with the number of heirs whose
/// resolved value changes (REQ-010-19): the objects of the property differential grouped
/// by the origin of their value after the changeset.
async fn reclassified(
    conn: &mut PgConnection,
    at: OffsetDateTime,
) -> Result<Vec<DiffRow>, StoreError> {
    let detail: bool = sqlx::query_scalar("SELECT to_regclass('pg_temp.diff_detail') IS NOT NULL")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    if !detail {
        return Ok(Vec::new());
    }
    let axes = axes(conn, at).await?;
    let found: Vec<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT o.origin_object_id, d.property_key,
                count(*) FILTER (WHERE d.object_id <> o.origin_object_id)
           FROM pg_temp.diff_detail d
           JOIN property_declaration p ON p.key = d.property_key
           JOIN property_origin o ON o.object_id = d.object_id AND o.property_id = p.id
                                 AND o.valid_period @> $1
          WHERE d.property_key = ANY($2) AND o.origin_object_id IS NOT NULL
          GROUP BY 1, 2
          ORDER BY 2, 1",
    )
    .bind(at)
    .bind(&axes)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(found
        .into_iter()
        .map(|(node, axis, heirs)| DiffRow {
            object: node,
            kind: RECLASSIFIED.into(),
            before: None,
            after: Some(json!({ "axis": axis, "heirs": heirs })),
            period: None,
        })
        .collect())
}

/// The section of the module for the changeset of `context`, if anything changes.
pub(crate) async fn section(
    conn: &mut PgConnection,
    context: &DiffContext<'_>,
) -> Result<Option<DiffSection>, StoreError> {
    // The window of the changeset: its verdicts of principle now, as the core's
    // differential of the properties reads its own window from now on.
    let at: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    let mut rows = Vec::new();
    if writes_policy(context.touched) {
        let before = PolicySnapshot::new(load::parts(conn, at, State::Before).await?);
        let after = PolicySnapshot::new(load::parts(conn, at, State::After).await?);
        let policy = diff(&before, &after);
        rows.push(summary_row(&policy));
        rows.extend(policy.flips.iter().map(flip_row));
    }
    rows.extend(reclassified(conn, at).await?);
    Ok((!rows.is_empty()).then(|| DiffSection {
        name: SECTION.into(),
        rows,
    }))
}

/// The measures of the changeset of `context` (REQ-010-20): objects of the policy
/// written; combinations opened and closed; most heirs reclassified on one axis.
pub(crate) fn measures(context: &CheckContext<'_>) -> Vec<(&'static str, u64)> {
    let mut measured = vec![(WRITE, context.touched.len() as u64)];
    let Some(section) = context.sections.iter().find(|s| s.name == SECTION) else {
        return measured;
    };
    let count = |field: &str| {
        section
            .rows
            .iter()
            .find(|r| r.kind == SUMMARY)
            .and_then(|r| r.after.as_ref())
            .and_then(|a| a.get(field))
            .and_then(Value::as_u64)
            .unwrap_or_default()
    };
    measured.push((OPENS, count("opens")));
    measured.push((CLOSES, count("closes")));
    let mut heirs: BTreeMap<&str, u64> = BTreeMap::new();
    for row in section.rows.iter().filter(|r| r.kind == RECLASSIFIED) {
        if let Some(after) = &row.after
            && let (Some(axis), Some(n)) = (
                after.get("axis").and_then(Value::as_str),
                after.get("heirs").and_then(Value::as_u64),
            )
        {
            *heirs.entry(axis).or_default() += n;
        }
    }
    measured.push((
        RECLASSIFY,
        heirs.values().copied().max().unwrap_or_default(),
    ));
    measured
}
