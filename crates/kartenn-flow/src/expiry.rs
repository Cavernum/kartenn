//! Deadlines and events of the registry (task 011-8; REQ-011-7, 9; design 011
//! « Échéances »): the daily task `flow.expiry` announces the end of a dated flow or of a
//! waiver 30 and 7 days before it and at its term, once per end (`flow_notice`), and opens
//! the anomaly of a flow its expired waiver leaves denied; the changesets emit
//! `outcome_changed` when the state or the outcome of a flow flips, and `closed` when one
//! is closed.

use std::time::Duration;

use kartenn_service::module::{CheckContext, Findings, JobSpec, ModuleTx};
use kartenn_service::port::StoreError;
use serde_json::{Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::approval::noncompliant;
use crate::pg::store;
use crate::reevaluation::{FLOW_ROW, SECTION};
use crate::rules::VERDICT_DENIED;

/// The name of the task, within the module.
pub const EXPIRY: &str = "expiry";

/// The tasks of the module: the deadlines, once a day.
pub static JOBS: &[JobSpec] = &[JobSpec {
    name: EXPIRY,
    period: Duration::from_hours(24),
}];

/// The events of the module, `org.kartenn.flow.<name>` (REQ-011-7).
pub const EVENTS: &[&str] = &["expiring", "expired", "outcome_changed", "closed"];

/// The days before an end that a notice announces, the widest first.
const NOTICE_DAYS: [i64; 2] = [30, 7];

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

/// The ends to announce now, by the kind of their notice: the latest version of each
/// dated flow not closed before its end (`flow`), and the waivers (`waiver`), whose end
/// falls in the window of the notice — the narrowest window that holds it, so that a flow
/// found 5 days before its end is announced once, at 7 days — or is past (`expired`).
async fn due(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<Vec<(Uuid, String, OffsetDateTime)>, StoreError> {
    let (wide, narrow) = (NOTICE_DAYS[0], NOTICE_DAYS[1]);
    sqlx::query_as(
        "WITH ends AS (
            SELECT DISTINCT ON (f.id) f.id AS flow_id, '' AS prefix,
                   upper(f.valid_period) AS period_end
              FROM flow.flow f
             ORDER BY f.id, upper(f.valid_period) DESC NULLS FIRST),
          flows AS (
            SELECT e.* FROM ends e
             WHERE e.period_end IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM flow.flow_history h WHERE h.id = e.flow_id
                                AND (upper(h.valid_period) IS NULL
                                     OR upper(h.valid_period) > e.period_end))
            UNION ALL
            SELECT w.flow_id, 'waiver_', upper(w.valid_period)
              FROM flow.flow_waiver w),
          kinds AS (
            SELECT flow_id, prefix || CASE
                     WHEN period_end <= $1 THEN 'expired'
                     WHEN period_end <= $1 + make_interval(days => $3::int)
                       THEN 'expiring_' || $3::text
                     WHEN period_end <= $1 + make_interval(days => $2::int)
                       THEN 'expiring_' || $2::text
                   END AS kind, period_end
              FROM flows)
         INSERT INTO flow.flow_notice (flow_id, kind, period_end)
         SELECT flow_id, kind, period_end FROM kinds WHERE kind IS NOT NULL
         ON CONFLICT DO NOTHING
         RETURNING flow_id, kind, period_end",
    )
    .bind(now)
    .bind(wide)
    .bind(narrow)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// Whether `flow` is denied now, by its current summary.
async fn denied_now(
    conn: &mut PgConnection,
    flow: Uuid,
    now: OffsetDateTime,
) -> Result<bool, StoreError> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM flow.flow_summary
                         WHERE flow_id = $1 AND valid_period @> $2 AND summary = 'denied')
            AND NOT EXISTS (SELECT 1 FROM flow.flow_waiver
                             WHERE flow_id = $1 AND valid_period @> $2)",
    )
    .bind(flow)
    .bind(now)
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))
}

/// Runs `flow.expiry`: the notices due, each one an event; a waiver that ends leaves its
/// flow noncompliant when the policy still denies it (REQ-011-9), an anomaly found by the
/// task (ADR 0026, amendment 4).
///
/// # Errors
///
/// A failure of the database.
pub(crate) async fn run(tx: &mut dyn ModuleTx) -> Result<Findings, StoreError> {
    let conn = connection(tx)?;
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    let due = due(conn, now).await?;
    let mut findings = Findings::default();
    let mut events = Vec::new();
    for (flow, kind, end) in due {
        let (subject, rest) = match kind.strip_prefix("waiver_") {
            Some(rest) => ("waiver", rest.to_owned()),
            None => ("flow", kind.clone()),
        };
        let (event, days) = match rest.strip_prefix("expiring_") {
            Some(days) => ("flow.expiring", days.parse::<i64>().ok()),
            None => ("flow.expired", None),
        };
        if subject == "waiver" && event == "flow.expired" {
            findings.reevaluated.push((flow, VERDICT_DENIED.id));
            if denied_now(conn, flow, now).await? {
                findings.violations.push(noncompliant(flow));
            }
        }
        events.push((
            event,
            flow,
            json!({ "flow": flow, "of": subject, "end": end.unix_timestamp(), "days": days }),
        ));
    }
    for (event, flow, data) in events {
        tx.emit(event, Some(flow.to_string()), data)?;
    }
    Ok(findings)
}

/// Emits `outcome_changed` for each flow of the changeset's section `flow.flows` whose
/// state or outcome flips (REQ-011-7, 12): the event reaches the outbox only when the
/// changeset is applied.
///
/// # Errors
///
/// An event the module does not declare.
pub(crate) fn outcome_changed(
    tx: &mut dyn ModuleTx,
    context: &CheckContext<'_>,
) -> Result<(), StoreError> {
    let Some(section) = context.section(SECTION) else {
        return Ok(());
    };
    for row in section.rows.iter().filter(|r| r.kind == FLOW_ROW) {
        let shown = |v: &Option<Value>, key: &str| {
            v.as_ref()
                .and_then(|v| v.get(key))
                .cloned()
                .unwrap_or(Value::Null)
        };
        let (before, after) = (&row.before, &row.after);
        if shown(before, "summary") == shown(after, "summary")
            && shown(before, "outcome") == shown(after, "outcome")
        {
            continue;
        }
        tx.emit(
            "flow.outcome_changed",
            Some(row.object.to_string()),
            json!({ "flow": row.object, "before": before, "after": after }),
        )?;
    }
    Ok(())
}
