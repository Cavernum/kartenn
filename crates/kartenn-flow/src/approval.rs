//! What the module gives the approval policies and the anomalies (task 011-6; REQ-011-6,
//! 12, 25): the measures `created`, `nonconforming` and `waiver` of a changeset, the
//! policy `flow.nonconforming ≥ 1` installed with the module, and the anomaly `flow.verdict.denied` of
//! the registered flows the changeset makes noncompliant, closed once they comply again.

use kartenn_core::rules::Violation;
use kartenn_service::module::{CheckContext, DefaultPolicy, Findings};
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::pg;
use crate::reevaluation::{FLOW_ROW, SECTION, SUMMARY_ROW};
use crate::rules::VERDICT_DENIED;

/// The measures of the module (REQ-011-6): flows created or prolonged, flows that become
/// non-conforming to the flow matrix (denied), waivers granted or prolonged.
pub const MEASURES: &[&str] = &["created", "nonconforming", "waiver"];

/// The policy the module installs once (REQ-011-6, F8): a changeset that makes at least
/// one registered flow non-conforming waits for an approval; an administrator may disable
/// it.
pub static DEFAULT_POLICIES: &[DefaultPolicy] = &[DefaultPolicy {
    name: "flow.nonconforming",
    triggers: &[("nonconforming", 1)],
}];

/// The values of [`MEASURES`] for the changeset of `context`: the operations it wrote and
/// the counts of its section `flow.flows`.
///
/// # Errors
///
/// A failure of the database.
pub(crate) async fn measures(
    conn: &mut PgConnection,
    context: &CheckContext<'_>,
) -> Result<Vec<(&'static str, u64)>, StoreError> {
    let (created, waiver) = pg::written_counts(conn).await?;
    let nonconforming = context
        .section(SECTION)
        .and_then(|s| s.rows.iter().find(|r| r.kind == SUMMARY_ROW))
        .and_then(|r| r.after.as_ref()?.get("nonconforming")?.as_u64())
        .unwrap_or(0);
    Ok(vec![
        ("created", created),
        ("nonconforming", nonconforming),
        ("waiver", waiver),
    ])
}

/// The anomalies of the flows the changeset re-evaluated (REQ-011-12): each flow whose
/// evaluation flipped is evaluated anew under `flow.verdict.denied`, a warning when it is
/// denied now and no waiver covers now, so that the anomaly opens on a flow that becomes
/// non-conforming and closes on one that conforms again. A flow the changeset creates has its triage instead.
///
/// # Errors
///
/// A failure of the database.
pub(crate) async fn anomalies(
    conn: &mut PgConnection,
    context: &CheckContext<'_>,
    now: OffsetDateTime,
) -> Result<Findings, StoreError> {
    let mut findings = Findings::default();
    let Some(section) = context.section(SECTION) else {
        return Ok(findings);
    };
    for row in section.rows.iter().filter(|r| r.kind == FLOW_ROW) {
        findings.reevaluated.push((row.object, VERDICT_DENIED.id));
        let denied = row
            .after
            .as_ref()
            .and_then(|a| a.get("summary"))
            .is_some_and(|s| s == "denied");
        if denied && pg::waiver_at(conn, row.object, now).await?.is_none() {
            findings.violations.push(noncompliant(row.object));
        }
    }
    Ok(findings)
}

/// The anomaly of a flow the policy denies without a valid waiver.
pub(crate) fn noncompliant(flow: Uuid) -> Violation {
    Violation::new(
        &VERDICT_DENIED,
        "the policy denies the registered flow and no waiver covers it: noncompliant",
    )
    .with_subjects([flow])
}
