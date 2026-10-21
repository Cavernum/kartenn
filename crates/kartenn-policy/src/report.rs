//! The audit report of the policy (REQ-010-22, 23; design « Rapport d'audit »): asked for
//! a date by [`request`], produced by the background task `policy.report` by batches, read
//! back by rubric ([`report`], [`items`]) and exported in JSON ([`export`]).
//!
//! The rubrics that range over the referential (unclassified nodes, transverse values,
//! anomalies) go by windows of [`BATCH`] objects resumed at the last key: a run never reads
//! the whole referential in one write transaction. The rubrics over the policy itself
//! (bounded by its size, REQ-010-28) are produced in one step each. The rubrics over the
//! declared flows read the evaluations the 011 materialises; without them they are not
//! answered (`null` count).

use std::collections::BTreeMap;

use kartenn_service::module::ModuleTx;
use kartenn_service::port::StoreError;
use serde_json::{Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::pg::store;
use crate::rules::{INFERENCE_MISMATCH, MACHINE_ENVIRONMENT_MISMATCH};

/// The rubrics of the report, in the order the task produces them (REQ-010-23).
pub const RUBRICS: [&str; 10] = [
    // Nodes without a value on an axis cited by the policy, by axis and domain.
    "unclassified",
    // Nodes whose resolved set holds a transverse value, with the origin node.
    "transverse",
    // Authorisations that only hold through a transverse value.
    "transverse_allow",
    // Values of an axis no rule cites.
    "uncited_values",
    // Rules citing a value no node carries.
    "unborne_values",
    // `allow` rules entirely masked by a `deny` rule.
    "masked_allow",
    // Rules never decisive for a declared flow (011).
    "rules_never_applied",
    // Authorised combinations (rule, service) without any declared flow (011).
    "combinations_without_flow",
    // Deviations of the inference (REQ-010-48).
    "inference_mismatch",
    // Anomalies of machines (REQ-010-17).
    "machine_anomalies",
];

/// The objects a window reads, at most.
pub const BATCH: i64 = 1_000;

/// The steps of one run of the task, at most.
pub const STEPS: usize = 20;

/// Days a report is kept after it was asked for (design « Rapport d'audit »).
pub const RETENTION_DAYS: i32 = 30;

/// The tables of the addressing tree a window goes over, with the domain of their rows.
const TREE: [(&str, &str, &str); 3] = [
    ("addressing_domain", "id", "domain"),
    ("prefix", "domain_id", "prefix"),
    ("address", "domain_id", "address"),
];

/// The answer to [`request`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requested {
    /// A report is asked for: its identifier.
    Started(Uuid),
    /// A report is already being produced (one at a time): its identifier.
    Busy(Uuid),
}

/// A report as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Identifier.
    pub id: Uuid,
    /// The date the report describes.
    pub valid_at: OffsetDateTime,
    /// Who asked for it.
    pub requested_by: Option<Uuid>,
    /// When it was asked for.
    pub requested_at: OffsetDateTime,
    /// `pending`, `running` or `done`.
    pub state: String,
    /// When it was over.
    pub done_at: Option<OffsetDateTime>,
    /// The number of items of each rubric produced so far; `None` for a rubric the build
    /// cannot answer.
    pub counts: BTreeMap<String, Option<i64>>,
}

/// An item of a rubric.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Rubric.
    pub rubric: String,
    /// Rank within the rubric, from 1.
    pub seq: i64,
    /// The object the item is about, if one.
    pub subject: Option<Uuid>,
    /// What the item says.
    pub detail: Value,
}

/// Asks for a report at `valid_at`, unless one is being produced (REQ-010-23).
///
/// # Errors
/// A failure of the database.
pub async fn request(
    conn: &mut PgConnection,
    valid_at: OffsetDateTime,
    requested_by: Option<Uuid>,
) -> Result<Requested, StoreError> {
    let id = Uuid::now_v7();
    let inserted = sqlx::query(
        "INSERT INTO policy.policy_report (id, valid_at, requested_by) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(valid_at)
    .bind(requested_by)
    .execute(&mut *conn)
    .await
    .map_err(|e| store(&e))?
    .rows_affected();
    if inserted == 1 {
        return Ok(Requested::Started(id));
    }
    let busy: Uuid = sqlx::query_scalar(
        "SELECT id FROM policy.policy_report WHERE state <> 'done' ORDER BY requested_at LIMIT 1",
    )
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(Requested::Busy(busy))
}

type ReportRow = (
    Uuid,
    OffsetDateTime,
    Option<Uuid>,
    OffsetDateTime,
    String,
    Option<OffsetDateTime>,
    Value,
);

/// The report `id`, `None` when it does not exist (or was purged).
///
/// # Errors
/// A failure of the database.
pub async fn report(conn: &mut PgConnection, id: Uuid) -> Result<Option<Report>, StoreError> {
    let row: Option<ReportRow> = sqlx::query_as(
        "SELECT id, valid_at, requested_by, requested_at, state, done_at, counts
           FROM policy.policy_report WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(row.map(
        |(id, valid_at, requested_by, requested_at, state, done_at, counts)| Report {
            id,
            valid_at,
            requested_by,
            requested_at,
            state,
            done_at,
            counts: counts
                .as_object()
                .map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_i64())).collect())
                .unwrap_or_default(),
        },
    ))
}

/// The items of `rubric` of the report `id` after the rank `after`, `limit` at most, by
/// rank: the detail of a rubric, page by page.
///
/// # Errors
/// A failure of the database.
pub async fn items(
    conn: &mut PgConnection,
    id: Uuid,
    rubric: &str,
    after: i64,
    limit: i64,
) -> Result<Vec<Item>, StoreError> {
    let rows: Vec<(String, i64, Option<Uuid>, Value)> = sqlx::query_as(
        "SELECT rubric, seq, subject, detail FROM policy.policy_report_item
          WHERE report_id = $1 AND rubric = $2 AND seq > $3 ORDER BY seq LIMIT $4",
    )
    .bind(id)
    .bind(rubric)
    .bind(after)
    .bind(limit)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(rubric, seq, subject, detail)| Item {
            rubric,
            seq,
            subject,
            detail,
        })
        .collect())
}

/// The report `id` with every item, in JSON (REQ-010-23), `None` when it does not exist.
/// A rubric the build cannot answer has a `null` count and `"answered": false`.
///
/// # Errors
/// A failure of the database.
pub async fn export(conn: &mut PgConnection, id: Uuid) -> Result<Option<Value>, StoreError> {
    // The dates as PostgreSQL writes them in JSON (ISO 8601).
    let head: Option<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('id', id, 'valid_at', valid_at,
                                   'requested_by', requested_by, 'requested_at', requested_at,
                                   'state', state, 'done_at', done_at)
           FROM policy.policy_report WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let (Some(mut head), Some(r)) = (head, report(conn, id).await?) else {
        return Ok(None);
    };
    let mut rubrics = serde_json::Map::new();
    for rubric in RUBRICS {
        let count = r.counts.get(rubric).copied().flatten();
        let answered = r.counts.get(rubric).is_none_or(Option::is_some);
        let mut entries = Vec::new();
        let mut after = 0;
        loop {
            let chunk = items(conn, id, rubric, after, 1_000).await?;
            let Some(last) = chunk.last() else { break };
            after = last.seq;
            entries.extend(
                chunk
                    .into_iter()
                    .map(|i| json!({ "seq": i.seq, "subject": i.subject, "detail": i.detail })),
            );
        }
        rubrics.insert(
            rubric.to_owned(),
            json!({ "count": count, "answered": answered, "items": entries }),
        );
    }
    if let Some(o) = head.as_object_mut() {
        o.insert("rubrics".into(), Value::Object(rubrics));
    }
    Ok(Some(head))
}

/// The progress of the report being produced.
#[derive(Debug, Clone, sqlx::FromRow)]
struct Progress {
    id: Uuid,
    valid_at: OffsetDateTime,
    counts: Value,
    rubric: i32,
    part: i32,
    after_key: Option<Uuid>,
}

impl Progress {
    fn count(&self, rubric: &str) -> i64 {
        self.counts.get(rubric).and_then(Value::as_i64).unwrap_or(0)
    }

    fn add(&mut self, rubric: &str, n: usize) {
        let total = self.count(rubric) + i64::try_from(n).unwrap_or(i64::MAX);
        if let Some(o) = self.counts.as_object_mut() {
            o.insert(rubric.to_owned(), json!(total));
        }
    }

    fn unanswered(&mut self, rubric: &str) {
        if let Some(o) = self.counts.as_object_mut() {
            o.insert(rubric.to_owned(), Value::Null);
        }
    }

    fn next_rubric(&mut self) {
        self.rubric += 1;
        self.part = 0;
        self.after_key = None;
    }

    fn next_part(&mut self) {
        self.part += 1;
        self.after_key = None;
    }
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut::<PgConnection>()
        .ok_or_else(|| StoreError("the policy needs a PostgreSQL transaction".into()))
}

/// One run of the task `policy.report`: purges the old reports, then goes on producing
/// the report asked for, [`STEPS`] steps of [`BATCH`] objects at most.
///
/// # Errors
/// A failure of the database.
pub async fn run(tx: &mut dyn ModuleTx) -> Result<(), StoreError> {
    run_with(connection(tx)?, BATCH, STEPS).await
}

/// [`run`] with windows of `batch` objects and `steps` steps.
///
/// # Errors
/// A failure of the database.
pub async fn run_with(conn: &mut PgConnection, batch: i64, steps: usize) -> Result<(), StoreError> {
    sqlx::query(
        "DELETE FROM policy.policy_report
          WHERE requested_at < now() - make_interval(days => $1)",
    )
    .bind(RETENTION_DAYS)
    .execute(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let progress: Option<Progress> = sqlx::query_as(
        "SELECT id, valid_at, counts, rubric, part, after_key FROM policy.policy_report
          WHERE state <> 'done' ORDER BY requested_at LIMIT 1 FOR UPDATE SKIP LOCKED",
    )
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let Some(mut p) = progress else {
        return Ok(());
    };
    let batch = batch.max(1);
    let mut done = false;
    for _ in 0..steps.max(1) {
        let Some(rubric) = usize::try_from(p.rubric).ok().and_then(|i| RUBRICS.get(i)) else {
            done = true;
            break;
        };
        // Every rubric has a count, zero when it lists nothing.
        p.add(rubric, 0);
        step(conn, &mut p, rubric, batch).await?;
    }
    if usize::try_from(p.rubric).is_ok_and(|i| i >= RUBRICS.len()) {
        done = true;
    }
    sqlx::query(
        "UPDATE policy.policy_report
            SET counts = $2, rubric = $3, part = $4, after_key = $5,
                state = CASE WHEN $6 THEN 'done' ELSE 'running' END,
                done_at = CASE WHEN $6 THEN now() END
          WHERE id = $1",
    )
    .bind(p.id)
    .bind(&p.counts)
    .bind(p.rubric)
    .bind(p.part)
    .bind(p.after_key)
    .bind(done)
    .execute(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(())
}

/// Stores `rows` as the next items of `rubric`.
async fn insert(
    conn: &mut PgConnection,
    p: &mut Progress,
    rubric: &str,
    rows: Vec<(Option<Uuid>, Value)>,
) -> Result<(), StoreError> {
    let n = rows.len();
    if n > 0 {
        let (subjects, details): (Vec<Option<Uuid>>, Vec<Value>) = rows.into_iter().unzip();
        sqlx::query(
            "INSERT INTO policy.policy_report_item (report_id, rubric, seq, subject, detail)
             SELECT $1, $2, $3 + t.ord, t.s, t.d
               FROM unnest($4::uuid[], $5::jsonb[]) WITH ORDINALITY AS t(s, d, ord)",
        )
        .bind(p.id)
        .bind(rubric)
        .bind(p.count(rubric))
        .bind(subjects)
        .bind(details)
        .execute(conn)
        .await
        .map_err(|e| store(&e))?;
    }
    p.add(rubric, n);
    Ok(())
}

/// One step of the rubric `rubric`.
async fn step(
    conn: &mut PgConnection,
    p: &mut Progress,
    rubric: &str,
    batch: i64,
) -> Result<(), StoreError> {
    match rubric {
        "unclassified" => unclassified(conn, p, rubric, batch).await,
        "transverse" => transverse(conn, p, rubric, batch).await,
        "inference_mismatch" => anomalies(conn, p, rubric, INFERENCE_MISMATCH.id, batch).await,
        "machine_anomalies" => {
            anomalies(conn, p, rubric, MACHINE_ENVIRONMENT_MISMATCH.id, batch).await
        }
        "rules_never_applied" | "combinations_without_flow" if !flows(conn).await? => {
            p.unanswered(rubric);
            p.next_rubric();
            Ok(())
        }
        _ => {
            let rows = whole(conn, rubric, p.valid_at).await?;
            insert(conn, p, rubric, rows).await?;
            p.next_rubric();
            Ok(())
        }
    }
}

/// Whether the evaluations of the declared flows (011) are there.
async fn flows(conn: &mut PgConnection) -> Result<bool, StoreError> {
    sqlx::query_scalar(
        "SELECT to_regclass('flow.flow_evaluation') IS NOT NULL
            AND to_regclass('flow.flow') IS NOT NULL",
    )
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))
}

/// The axes of the tree cited by a rule at `at`: their key and declaration, by key.
async fn cited_tree_axes(
    conn: &mut PgConnection,
    at: OffsetDateTime,
) -> Result<Vec<(String, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT a.property_key, d.id
           FROM (SELECT property_key, carrier FROM policy.policy_axis
                  WHERE valid_period @> $1
                 UNION SELECT 'policy.environment', 'tree') AS a
           JOIN property_declaration d ON d.key = a.property_key AND d.valid_period @> $1
          WHERE a.carrier = 'tree'
            AND EXISTS (SELECT 1 FROM policy.policy_rule r
                         WHERE r.valid_period @> $1
                           AND (r.source ? a.property_key OR r.destination ? a.property_key
                                OR a.property_key = ANY(r.same)))
          ORDER BY a.property_key",
    )
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// Unclassified nodes (REQ-010-22): part = axis × table of the tree; a window of nodes by
/// identifier, those without a resolved value kept.
async fn unclassified(
    conn: &mut PgConnection,
    p: &mut Progress,
    rubric: &str,
    batch: i64,
) -> Result<(), StoreError> {
    let cited = cited_tree_axes(conn, p.valid_at).await?;
    let part = usize::try_from(p.part).unwrap_or(usize::MAX);
    let Some((axis, property)) = cited.get(part / TREE.len()) else {
        p.next_rubric();
        return Ok(());
    };
    let (table, domain, kind) = TREE[part % TREE.len()];
    // `table` and `domain` come from `TREE`, never from an input.
    let window: Vec<(Uuid, Uuid, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH w AS (
             SELECT id, {domain} AS domain FROM {table}
              WHERE valid_period @> $1 AND ($2::uuid IS NULL OR id > $2)
              ORDER BY id LIMIT $3)
         SELECT w.id, w.domain,
                NOT EXISTS (SELECT 1 FROM property_origin o
                             WHERE o.object_id = w.id AND o.property_id = $4
                               AND o.valid_period @> $1 AND o.origin_object_id IS NOT NULL)
           FROM w ORDER BY w.id"
    )))
    .bind(p.valid_at)
    .bind(p.after_key)
    .bind(batch)
    .bind(property)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let read = i64::try_from(window.len()).unwrap_or(i64::MAX);
    let last = window.last().map(|w| w.0);
    let rows = window
        .into_iter()
        .filter(|w| w.2)
        .map(|(id, domain, _)| {
            (
                Some(id),
                json!({ "axis": axis, "domain": domain, "kind": kind }),
            )
        })
        .collect();
    insert(conn, p, rubric, rows).await?;
    if read < batch {
        p.next_part();
    } else {
        p.after_key = last;
    }
    Ok(())
}

/// Nodes holding a transverse value (REQ-010-23): part = axis of the tree with transverse
/// values; a window of the resolved origins by object, those whose origin carries one kept.
async fn transverse(
    conn: &mut PgConnection,
    p: &mut Progress,
    rubric: &str,
    batch: i64,
) -> Result<(), StoreError> {
    let carrying: Vec<(String, Uuid, Vec<String>)> = sqlx::query_as(
        "SELECT a.property_key, d.id, array_agg(v.value_key ORDER BY v.value_key)
           FROM (SELECT property_key, carrier FROM policy.policy_axis
                  WHERE valid_period @> $1
                 UNION SELECT 'policy.environment', 'tree') AS a
           JOIN property_declaration d ON d.key = a.property_key AND d.valid_period @> $1
           JOIN policy.policy_value v ON v.property_key = a.property_key AND v.transverse
                                     AND v.valid_period @> $1
          WHERE a.carrier = 'tree'
          GROUP BY a.property_key, d.id ORDER BY a.property_key",
    )
    .bind(p.valid_at)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let Some((axis, property, values)) = usize::try_from(p.part).ok().and_then(|i| carrying.get(i))
    else {
        p.next_rubric();
        return Ok(());
    };
    let window: Vec<(Uuid, Option<Uuid>, Option<Vec<String>>)> = sqlx::query_as(
        "WITH w AS (
             SELECT object_id, origin_object_id FROM property_origin
              WHERE property_id = $4 AND valid_period @> $1
                AND ($2::uuid IS NULL OR object_id > $2)
              ORDER BY object_id LIMIT $3)
         SELECT w.object_id, w.origin_object_id,
                (SELECT array_agg(DISTINCT e.key ORDER BY e.key)
                   FROM property_assignment s
                   JOIN property_enum_value e ON e.id = s.v_uuid OR e.id = ANY(s.v_uuids)
                  WHERE s.object_id = w.origin_object_id AND s.property_id = $4
                    AND s.kind = 'value' AND s.valid_period @> $1 AND e.key = ANY($5))
           FROM w ORDER BY w.object_id",
    )
    .bind(p.valid_at)
    .bind(p.after_key)
    .bind(batch)
    .bind(property)
    .bind(values)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let read = i64::try_from(window.len()).unwrap_or(i64::MAX);
    let last = window.last().map(|w| w.0);
    let axis = axis.clone();
    let rows = window
        .into_iter()
        .filter_map(|(id, origin, found)| {
            let found = found?;
            Some((
                Some(id),
                json!({ "axis": axis, "values": found, "origin": origin }),
            ))
        })
        .collect();
    insert(conn, p, rubric, rows).await?;
    if read < batch {
        p.next_part();
    } else {
        p.after_key = last;
    }
    Ok(())
}

/// The open anomalies of `rule` (current state), by window of objects.
async fn anomalies(
    conn: &mut PgConnection,
    p: &mut Progress,
    rubric: &str,
    rule: &str,
    batch: i64,
) -> Result<(), StoreError> {
    let window: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT object_id, message FROM anomaly
          WHERE rule_id = $1 AND closed_at IS NULL AND ($2::uuid IS NULL OR object_id > $2)
          ORDER BY object_id LIMIT $3",
    )
    .bind(rule)
    .bind(p.after_key)
    .bind(batch)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let read = i64::try_from(window.len()).unwrap_or(i64::MAX);
    let last = window.last().map(|w| w.0);
    let rows = window
        .into_iter()
        .map(|(id, message)| (Some(id), json!({ "rule": rule, "message": message })))
        .collect();
    insert(conn, p, rubric, rows).await?;
    if read < batch {
        p.next_rubric();
    } else {
        p.after_key = last;
    }
    Ok(())
}

/// A rubric over the policy, produced whole: its items.
#[allow(clippy::too_many_lines)] // one query per rubric, kept side by side
async fn whole(
    conn: &mut PgConnection,
    rubric: &str,
    at: OffsetDateTime,
) -> Result<Vec<(Option<Uuid>, Value)>, StoreError> {
    let sql = match rubric {
        // An `allow` rule one of whose terms lists transverse values only.
        "transverse_allow" => {
            "SELECT r.id, jsonb_build_object('rule', r.key, 'side', s.side, 'axis', t.key,
                                             'values', t.vals)
               FROM policy.policy_rule r
               CROSS JOIN LATERAL (VALUES ('source', r.source), ('destination', r.destination))
                    AS s(side, sel)
               CROSS JOIN LATERAL jsonb_each(s.sel) AS t(key, vals)
              WHERE r.valid_period @> $1 AND r.effect = 'allow'
                AND jsonb_array_length(t.vals) > 0
                AND NOT EXISTS (
                    SELECT 1 FROM jsonb_array_elements_text(t.vals) AS x(v)
                     WHERE NOT EXISTS (
                         SELECT 1 FROM policy.policy_value pv
                          WHERE pv.property_key = t.key AND pv.value_key = x.v
                            AND pv.transverse AND pv.valid_period @> $1))
              ORDER BY r.key, s.side, t.key"
        }
        // A value of an axis (its list, or the list it borrows) no rule cites.
        "uncited_values" => {
            "SELECT e.id, jsonb_build_object('axis', a.property_key, 'value', e.key)
               FROM (SELECT property_key FROM policy.policy_axis WHERE valid_period @> $1
                     UNION SELECT 'policy.environment') AS a
               JOIN property_declaration d ON d.key = a.property_key AND d.valid_period @> $1
               JOIN property_declaration l
                 ON l.key = coalesce(d.type_params->>'values_of', d.key) AND l.valid_period @> $1
               JOIN property_enum_value e ON e.property_id = l.id AND e.valid_period @> $1
              WHERE NOT EXISTS (
                    SELECT 1 FROM policy.policy_rule r
                     WHERE r.valid_period @> $1
                       AND (coalesce(r.source -> a.property_key, '[]') ? e.key
                            OR coalesce(r.destination -> a.property_key, '[]') ? e.key))
              ORDER BY a.property_key, e.key"
        }
        // A value cited by a rule that no object carries.
        "unborne_values" => {
            "SELECT r.id, jsonb_build_object('rule', r.key, 'side', s.side, 'axis', t.key,
                                             'value', x.v)
               FROM policy.policy_rule r
               CROSS JOIN LATERAL (VALUES ('source', r.source), ('destination', r.destination))
                    AS s(side, sel)
               CROSS JOIN LATERAL jsonb_each(s.sel) AS t(key, vals)
               CROSS JOIN LATERAL jsonb_array_elements_text(t.vals) AS x(v)
              WHERE r.valid_period @> $1
                AND NOT EXISTS (
                    SELECT 1 FROM property_declaration d
                      JOIN property_declaration l
                        ON l.key = coalesce(d.type_params->>'values_of', d.key)
                       AND l.valid_period @> $1
                      JOIN property_enum_value e
                        ON e.property_id = l.id AND e.key = x.v AND e.valid_period @> $1
                      JOIN property_assignment a
                        ON a.property_id = d.id AND a.kind = 'value' AND a.valid_period @> $1
                       AND (a.v_uuid = e.id OR e.id = ANY(a.v_uuids))
                     WHERE d.key = t.key AND d.valid_period @> $1)
              ORDER BY r.key, s.side, t.key, x.v"
        }
        // An `allow` rule every flow of which a `deny` rule covers: the deny's terms are
        // looser on both sides, its « same value » axes among the allow's, its flows wider.
        "masked_allow" => {
            "SELECT DISTINCT ON (a.key) a.id,
                    jsonb_build_object('rule', a.key, 'masked_by', d.key)
               FROM policy.policy_rule a
               JOIN policy.policy_rule d ON d.effect = 'deny' AND d.valid_period @> $1
              WHERE a.effect = 'allow' AND a.valid_period @> $1
                AND NOT EXISTS (SELECT 1 FROM jsonb_each(d.source) AS t(key, vals)
                                 WHERE NOT (a.source ? t.key AND (a.source -> t.key) <@ t.vals))
                AND NOT EXISTS (SELECT 1 FROM jsonb_each(d.destination) AS t(key, vals)
                                 WHERE NOT (a.destination ? t.key
                                            AND (a.destination -> t.key) <@ t.vals))
                AND d.same <@ a.same
                AND a.flow_type_ids <@ d.flow_type_ids
                AND NOT EXISTS (
                    SELECT 1 FROM unnest(a.service_ids) AS sv(id)
                     WHERE sv.id <> ALL(d.service_ids)
                       AND NOT EXISTS (SELECT 1 FROM policy.policy_service ps
                                        WHERE ps.id = sv.id AND ps.valid_period @> $1
                                          AND ps.flow_type_id = ANY(d.flow_type_ids)))
              ORDER BY a.key, d.key"
        }
        // A rule no evaluation of a declared flow names as decisive.
        "rules_never_applied" => {
            "WITH used AS (
                 SELECT DISTINCT decisive_id FROM flow.flow_evaluation
                  WHERE valid_period @> $1 AND decisive_id IS NOT NULL)
             SELECT r.id, jsonb_build_object('rule', r.key, 'effect', r.effect)
               FROM policy.policy_rule r
              WHERE r.valid_period @> $1
                AND NOT EXISTS (SELECT 1 FROM used u WHERE u.decisive_id = r.id)
              ORDER BY r.key"
        }
        // A service an `allow` rule covers with no declared flow it allows.
        "combinations_without_flow" => {
            "WITH used AS (
                 SELECT DISTINCT e.decisive_id, f.service_id
                   FROM flow.flow_evaluation e
                   JOIN flow.flow f ON f.id = e.flow_id AND f.valid_period @> $1
                  WHERE e.valid_period @> $1 AND e.verdict = 'allowed')
             SELECT r.id, jsonb_build_object('rule', r.key, 'service', s.key)
               FROM policy.policy_rule r
               JOIN policy.policy_service s
                 ON s.valid_period @> $1
                AND (s.id = ANY(r.service_ids) OR s.flow_type_id = ANY(r.flow_type_ids))
              WHERE r.valid_period @> $1 AND r.effect = 'allow'
                AND NOT EXISTS (SELECT 1 FROM used u
                                 WHERE u.decisive_id = r.id AND u.service_id = s.id)
              ORDER BY r.key, s.key"
        }
        _ => return Ok(Vec::new()),
    };
    let rows: Vec<(Uuid, Value)> = sqlx::query_as(sql)
        .bind(at)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))?;
    Ok(rows.into_iter().map(|(id, d)| (Some(id), d)).collect())
}
