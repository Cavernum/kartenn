//! The re-evaluation of the flows a changeset touches, in its transaction (REQ-011-11 to
//! 13; design 011 « Réévaluation »), and the section `flow.flows` of its differential.
//!
//! A flow is touched when the changeset writes it, its service or an endpoint; changes the
//! resolved value of a property on an endpoint, on one of its addresses or on a machine
//! holding one (the differential of the properties); attaches or detaches an address of
//! an endpoint (`machines.attachments`); or writes the policy, save its inference rules,
//! which never decide a verdict (D23). Each touched flow is evaluated anew from now to
//! its end, cut wherever the policy, an attachment, an origin or a value its endpoints
//! read starts or ends; the evaluation is rewritten only when it differs from the stored
//! one, contiguous equal segments merged.

use std::collections::BTreeSet;

use kartenn_service::ObjectKind;
use kartenn_service::module::{DiffContext, DiffRow, DiffSection};
use kartenn_service::port::StoreError;
use serde_json::{Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::evaluation::evaluate_flow;
use crate::model::Flow;
use crate::pg::{self, store};

/// The section of the module in the differential (REQ-011-13).
pub const SECTION: &str = "flow.flows";
/// The kind of a row of a flow whose summary, a verdict or the outcome flips.
const FLOW_ROW: &str = "flow.flow";
/// The kind of the counted row, first in the section.
const SUMMARY_ROW: &str = "flow.summary";

/// The versioned tables of the policy a verdict reads: the inference rules are left out.
const POLICY_TABLES: &[&str] = &[
    "policy.policy_axis",
    "policy.policy_value",
    "policy.policy_flow_type",
    "policy.policy_service",
    "policy.policy_pattern",
    "policy.policy_rule",
    "policy.policy_condition",
    "policy.policy_signal",
    "policy.policy_exempt_role",
];

/// The evaluation of a pair of addresses, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) source: Uuid,
    pub(crate) destination: Uuid,
    pub(crate) verdict: String,
    pub(crate) pending: bool,
    pub(crate) outcome: String,
    pub(crate) reason: String,
    pub(crate) decisive: Option<Uuid>,
    pub(crate) combination: Value,
    pub(crate) unmet: Vec<Uuid>,
    pub(crate) score: Option<i32>,
}

/// The summary of a flow's evaluations, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Summary {
    pub(crate) verdict: String,
    pub(crate) outcome: String,
    pub(crate) allowed: i32,
    pub(crate) denied: i32,
}

/// What flips in the differential: the summary and outcome, the verdict of each pair.
type Flips<'a> = (Option<(&'a str, &'a str)>, Vec<(Uuid, Uuid, &'a str, bool)>);

/// The evaluation of a flow over a segment of time: its addresses (sorted), its rows (by
/// pair) and its summary. The revision of the policy is not part of it: a new revision
/// that changes nothing of a flow does not rewrite it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) sources: Vec<Uuid>,
    pub(crate) destinations: Vec<Uuid>,
    pub(crate) rows: Vec<Row>,
    pub(crate) summary: Option<Summary>,
}

impl State {
    pub(crate) fn is_empty(&self) -> bool {
        self.sources.is_empty()
            && self.destinations.is_empty()
            && self.rows.is_empty()
            && self.summary.is_none()
    }

    /// What flips in the differential: the summary, the outcome, a verdict.
    fn verdicts(&self) -> Flips<'_> {
        (
            self.summary
                .as_ref()
                .map(|s| (s.verdict.as_str(), s.outcome.as_str())),
            self.rows
                .iter()
                .map(|r| (r.source, r.destination, r.verdict.as_str(), r.pending))
                .collect(),
        )
    }

    /// Denied: no pair of addresses, or none allowed (REQ-011-5).
    fn denied(&self) -> bool {
        self.summary.as_ref().is_none_or(|s| s.verdict == "denied")
    }

    fn outcome(&self) -> Option<&str> {
        self.summary.as_ref().map(|s| s.outcome.as_str())
    }

    fn shown(&self) -> Value {
        let decisive = self
            .rows
            .iter()
            .find(|r| r.verdict == "denied")
            .or_else(|| self.rows.first())
            .map(|r| r.reason.as_str());
        json!({
            "summary": self.summary.as_ref().map(|s| s.verdict.as_str()),
            "outcome": self.outcome(),
            "allowed": self.summary.as_ref().map_or(0, |s| s.allowed),
            "denied": self.summary.as_ref().map_or(0, |s| s.denied),
            "decisive": decisive,
        })
    }
}

/// A segment of the evaluation of a flow: [`from`, `to`).
#[derive(Debug, Clone)]
pub(crate) struct Segment {
    pub(crate) from: OffsetDateTime,
    pub(crate) to: Option<OffsetDateTime>,
    pub(crate) state: State,
    /// The revision of the policy it was evaluated on; 0 when read back.
    pub(crate) revision: i64,
}

/// `segments` in order, contiguous equal states merged, empty ones left out.
fn merged(segments: Vec<Segment>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for segment in segments {
        if let Some(last) = out.last_mut()
            && last.state == segment.state
            && last.to == Some(segment.from)
        {
            last.to = segment.to;
            continue;
        }
        out.push(segment);
    }
    out.retain(|s| !s.state.is_empty());
    out
}

/// Whether two merged evaluations hold the same states over the same periods.
fn same(a: &[Segment], b: &[Segment]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.from == y.from && x.to == y.to && x.state == y.state)
}

/// The state that holds at `at`, empty if none.
fn state_at(segments: &[Segment], at: OffsetDateTime) -> State {
    segments
        .iter()
        .find(|s| s.from <= at && s.to.is_none_or(|t| t > at))
        .map(|s| s.state.clone())
        .unwrap_or_default()
}

/// Whether the changeset writes the policy a verdict reads.
fn writes_policy(touched: &[(ObjectKind, Uuid)]) -> bool {
    touched.iter().any(|(kind, _)| {
        matches!(kind, ObjectKind::Module(k)
            if k.module() == "policy" && !k.as_str().contains("inference"))
    })
}

/// The living flows the changeset of `context` touches, in the order of their
/// identifiers (F13).
async fn touched(
    conn: &mut PgConnection,
    context: &DiffContext<'_>,
) -> Result<Vec<Uuid>, StoreError> {
    let mut written: BTreeSet<Uuid> = BTreeSet::new();
    let mut objects: BTreeSet<Uuid> = BTreeSet::new();
    for (kind, id) in context.touched {
        if matches!(kind, ObjectKind::Module(k) if k.as_str() == crate::module::FLOW) {
            written.insert(*id);
        } else {
            objects.insert(*id);
        }
    }
    let marks: bool = sqlx::query_scalar("SELECT to_regclass('pg_temp.flow_written') IS NOT NULL")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    if marks {
        let found: Vec<Uuid> =
            sqlx::query_scalar("SELECT DISTINCT flow_id FROM pg_temp.flow_written")
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| store(&e))?;
        written.extend(found);
    }
    let detail: bool = sqlx::query_scalar("SELECT to_regclass('pg_temp.diff_detail') IS NOT NULL")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    if detail {
        let found: Vec<Uuid> =
            sqlx::query_scalar("SELECT DISTINCT object_id FROM pg_temp.diff_detail")
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| store(&e))?;
        objects.extend(found);
    }
    if let Some(section) = context.section(kartenn_machines::handler::ATTACHMENTS_SECTION) {
        for row in &section.rows {
            objects.insert(row.object);
            for side in [&row.before, &row.after].into_iter().flatten() {
                for key in ["machine", "address"] {
                    if let Some(id) = side.get(key).and_then(Value::as_str)
                        && let Ok(id) = id.parse()
                    {
                        objects.insert(id);
                    }
                }
            }
        }
    }
    let written: Vec<Uuid> = written.into_iter().collect();
    let objects: Vec<Uuid> = objects.into_iter().collect();
    sqlx::query_scalar(
        "SELECT DISTINCT f.id FROM flow.flow f
          WHERE (upper(f.valid_period) IS NULL OR upper(f.valid_period) > now())
            AND ($3 OR f.id = ANY($1)
                 OR f.source_id = ANY($2) OR f.destination_id = ANY($2) OR f.service_id = ANY($2)
                 OR EXISTS (SELECT 1 FROM flow.flow_endpoint_address e
                             WHERE e.flow_id = f.id AND e.address_id = ANY($2)
                               AND (upper(e.valid_period) IS NULL OR upper(e.valid_period) > now()))
                 OR EXISTS (SELECT 1 FROM flow.flow_endpoint_address e
                              JOIN machines.attachment a ON a.address_id = e.address_id
                              JOIN machines.interface i ON i.id = a.interface_id
                             WHERE e.flow_id = f.id AND i.machine_id = ANY($2)))
          ORDER BY f.id",
    )
    .bind(&written)
    .bind(&objects)
    .bind(writes_policy(context.touched))
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))
}

/// The instants in (`from`, `to`) where what the evaluation of `flow` reads may change:
/// a version of the policy, an attachment of an endpoint, an origin or a value of an
/// endpoint, of an address it develops into or of a machine holding one, a version of a
/// prefix endpoint (design 011 « Réévaluation », step 2).
async fn boundaries(
    conn: &mut PgConnection,
    flow: &Flow,
    from: OffsetDateTime,
    to: Option<OffsetDateTime>,
) -> Result<Vec<OffsetDateTime>, StoreError> {
    let policy: Vec<String> = POLICY_TABLES
        .iter()
        .map(|t| format!("UNION ALL SELECT valid_period FROM {t} WHERE valid_period && $2"))
        .collect();
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH ends AS (SELECT unnest($1::uuid[]) AS id),
              att AS (
                  SELECT a.valid_period, a.address_id, i.machine_id
                    FROM machines.attachment a
                    JOIN machines.interface i ON i.id = a.interface_id
                   WHERE a.valid_period && $2
                     AND (a.interface_id IN (SELECT id FROM ends)
                          OR i.machine_id IN (SELECT id FROM ends)
                          OR a.address_id IN (SELECT id FROM ends))),
              objs AS (SELECT id FROM ends UNION SELECT address_id FROM att
                       UNION SELECT machine_id FROM att),
              origins AS (
                  SELECT o.valid_period, o.origin_object_id, o.property_id
                    FROM public.property_origin o
                   WHERE o.object_id IN (SELECT id FROM objs) AND o.valid_period && $2),
              periods AS (
                  SELECT valid_period FROM att
                  UNION ALL SELECT valid_period FROM origins
                  UNION ALL SELECT a.valid_period FROM public.property_assignment a
                              JOIN origins o ON a.object_id = o.origin_object_id
                                            AND a.property_id = o.property_id
                             WHERE a.valid_period && $2
                  UNION ALL SELECT valid_period FROM public.prefix
                             WHERE id IN (SELECT id FROM ends) AND valid_period && $2
                  {})
         SELECT DISTINCT b FROM periods,
                LATERAL (VALUES (lower(valid_period)), (upper(valid_period))) v(b)
          WHERE b > lower($2) AND (upper($2) IS NULL OR b < upper($2))
          ORDER BY b",
        policy.join("\n                  ")
    )))
    .bind(vec![flow.source.id, flow.destination.id])
    .bind(sqlx::postgres::types::PgRange::from((
        std::ops::Bound::Included(from),
        to.map_or(std::ops::Bound::Unbounded, std::ops::Bound::Excluded),
    )))
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The evaluation of `flow` over [`from`, `to`), one segment per stretch where what it
/// reads holds still.
async fn evaluate_over(
    conn: &mut PgConnection,
    flow: &Flow,
    from: OffsetDateTime,
    to: Option<OffsetDateTime>,
) -> Result<Vec<Segment>, StoreError> {
    if to.is_some_and(|t| t <= from) {
        return Ok(Vec::new());
    }
    let mut cuts = vec![from];
    cuts.extend(boundaries(conn, flow, from, to).await?);
    let mut out = Vec::with_capacity(cuts.len());
    for (i, at) in cuts.iter().enumerate() {
        let evaluated = evaluate_flow(conn, flow, *at).await?;
        out.push(Segment {
            from: *at,
            to: cuts.get(i + 1).copied().or(to),
            state: pg::state_of(conn, &evaluated, *at).await?,
            revision: i64::try_from(evaluated.revision).unwrap_or(i64::MAX),
        });
    }
    Ok(out)
}

/// A flow whose evaluation the changeset rewrote: its state at the start of the window
/// before and after.
struct Rewritten {
    flow: Uuid,
    before: State,
    after: State,
}

/// Evaluates `flow` anew from `now` on and rewrites its evaluation if it differs from
/// the stored one (design 011 « Réévaluation », steps 2 and 3).
async fn reevaluate(
    conn: &mut PgConnection,
    id: Uuid,
    now: OffsetDateTime,
    changeset: Uuid,
) -> Result<Option<Rewritten>, StoreError> {
    let Some(stored) = pg::load(conn, id).await? else {
        return Ok(None);
    };
    let start: Option<OffsetDateTime> =
        sqlx::query_scalar("SELECT min(lower(valid_period)) FROM flow.flow WHERE id = $1")
            .bind(id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
    let from = start.map_or(now, |s| s.max(now));
    let to = stored.flow.period.end().map(pg::time);
    let wanted = merged(evaluate_over(conn, &stored.flow, from, to).await?);
    let held = merged(pg::read_states(conn, id, now).await?);
    if same(&held, &wanted) {
        return Ok(None);
    }
    pg::write_states(conn, id, now, &wanted, changeset).await?;
    Ok(Some(Rewritten {
        flow: id,
        before: state_at(&held, from),
        after: state_at(&wanted, from),
    }))
}

/// Re-evaluates every flow the changeset of `context` touches and gives the section
/// `flow.flows` (REQ-011-11, 13): first its counts (`breaks`: flows that become denied,
/// `heals`: denied flows that no longer are, `outcome`: flows whose outcome changes), then
/// each existing flow whose summary, a verdict or the outcome flips, before and after,
/// with its decisive element. A flow the changeset creates has nothing before: it is not
/// listed.
///
/// # Errors
///
/// A failure of the database.
pub(crate) async fn section(
    conn: &mut PgConnection,
    context: &DiffContext<'_>,
) -> Result<Option<DiffSection>, StoreError> {
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    let (mut breaks, mut heals, mut outcome) = (0_u64, 0_u64, 0_u64);
    let mut rows = Vec::new();
    for id in touched(conn, context).await? {
        let Some(r) = reevaluate(conn, id, now, context.changeset).await? else {
            continue;
        };
        if r.before.is_empty() || r.before.verdicts() == r.after.verdicts() {
            continue;
        }
        breaks += u64::from(!r.before.denied() && r.after.denied());
        heals += u64::from(r.before.denied() && !r.after.denied());
        outcome += u64::from(r.before.outcome() != r.after.outcome());
        rows.push(DiffRow {
            object: r.flow,
            kind: FLOW_ROW.into(),
            before: Some(r.before.shown()),
            after: Some(r.after.shown()),
            period: None,
        });
    }
    if rows.is_empty() {
        return Ok(None);
    }
    let counts = DiffRow {
        object: Uuid::nil(),
        kind: SUMMARY_ROW.into(),
        before: None,
        after: Some(json!({
            "flows": rows.len(),
            "breaks": breaks,
            "heals": heals,
            "outcome": outcome,
        })),
        period: None,
    };
    rows.insert(0, counts);
    Ok(Some(DiffSection {
        name: SECTION.into(),
        rows,
    }))
}

/// The living flows whose stored evaluation differs from one computed anew from now on
/// (the oracle of design 011 « Stratégie de test »): none, when the re-evaluation of
/// each changeset touched every flow it changed.
///
/// # Errors
///
/// A failure of the database.
pub async fn drift(conn: &mut PgConnection) -> Result<Vec<Uuid>, StoreError> {
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    let living: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT id FROM flow.flow
          WHERE upper(valid_period) IS NULL OR upper(valid_period) > now() ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut out = Vec::new();
    for id in living {
        let Some(stored) = pg::load(conn, id).await? else {
            continue;
        };
        let start: Option<OffsetDateTime> =
            sqlx::query_scalar("SELECT min(lower(valid_period)) FROM flow.flow WHERE id = $1")
                .bind(id)
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| store(&e))?;
        let from = start.map_or(now, |s| s.max(now));
        let to = stored.flow.period.end().map(pg::time);
        let wanted = merged(evaluate_over(conn, &stored.flow, from, to).await?);
        let held = merged(pg::read_states(conn, id, now).await?);
        if !same(&held, &wanted) {
            out.push(id);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds).unwrap()
    }

    fn summary(name: &str) -> State {
        State {
            summary: Some(Summary {
                verdict: name.into(),
                outcome: "automatic".into(),
                allowed: i32::from(name != "denied"),
                denied: i32::from(name == "denied"),
            }),
            ..State::default()
        }
    }

    fn segment(from: i64, to: Option<i64>, state: State) -> Segment {
        Segment {
            from: at(from),
            to: to.map(at),
            state,
            revision: 0,
        }
    }

    #[test]
    fn contiguous_equal_segments_merge_and_empty_ones_drop() {
        let out = merged(vec![
            segment(0, Some(10), summary("allowed")),
            segment(10, Some(20), summary("allowed")),
            segment(20, Some(30), State::default()),
            segment(30, None, summary("allowed")),
        ]);
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].from, out[0].to), (at(0), Some(at(20))));
        assert_eq!((out[1].from, out[1].to), (at(30), None));
        assert!(same(
            &out,
            &merged(vec![
                segment(0, Some(20), summary("allowed")),
                segment(30, None, summary("allowed")),
            ])
        ));
        assert!(!same(
            &out,
            &merged(vec![segment(0, None, summary("allowed"))])
        ));
    }

    #[test]
    fn a_flow_without_summary_counts_as_denied() {
        assert!(State::default().denied());
        assert!(summary("denied").denied());
        assert!(!summary("partial").denied());
    }
}
