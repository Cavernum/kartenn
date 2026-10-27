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

use std::collections::{BTreeMap, BTreeSet};

use kartenn_service::ObjectKind;
use kartenn_service::module::{DiffContext, DiffRow, DiffSection};
use kartenn_service::port::StoreError;
use serde_json::{Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::evaluation::{LOT, Policies, evaluate_many};
use crate::model::Flow;
use crate::pg::{self, store};

/// The section of the module in the differential (REQ-011-13).
pub const SECTION: &str = "flow.flows";
/// The kind of a row of a flow whose summary, a verdict or the outcome flips.
pub(crate) const FLOW_ROW: &str = "flow.flow";
/// The kind of the counted row, first in the section.
pub(crate) const SUMMARY_ROW: &str = "flow.summary";

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

/// What the changeset writes of the policy a verdict reads.
#[derive(Default)]
struct PolicyWrites {
    /// An object every verdict may read (an axis, a value, a pattern, a condition, a
    /// signal, an exempt role): every flow is concerned.
    everything: bool,
    rules: Vec<Uuid>,
    services: Vec<Uuid>,
    flow_types: Vec<Uuid>,
}

impl PolicyWrites {
    fn of(touched: &[(ObjectKind, Uuid)]) -> Self {
        let mut writes = Self::default();
        for (kind, id) in touched {
            let ObjectKind::Module(k) = kind else {
                continue;
            };
            if k.module() != "policy" || k.as_str().contains("inference") {
                continue;
            }
            match k.as_str() {
                kartenn_policy::module::RULE => writes.rules.push(*id),
                kartenn_policy::module::SERVICE => writes.services.push(*id),
                kartenn_policy::module::FLOW_TYPE => writes.flow_types.push(*id),
                _ => writes.everything = true,
            }
        }
        writes
    }

    /// The services whose flows a write of rules, services or flow types concerns: a
    /// verdict reads the rules of the flow type of its service, or citing its service
    /// (010 « Évaluation »), in their versions before and after the changeset.
    async fn services(&self, conn: &mut PgConnection) -> Result<Vec<Uuid>, StoreError> {
        if self.rules.is_empty() && self.services.is_empty() && self.flow_types.is_empty() {
            return Ok(Vec::new());
        }
        sqlx::query_scalar(
            "WITH r AS (SELECT flow_type_ids, service_ids FROM policy.policy_rule
                         WHERE id = ANY($1)
                        UNION ALL
                        SELECT flow_type_ids, service_ids FROM policy.policy_rule_history
                         WHERE id = ANY($1) AND sys_end >= now()),
                  types AS (SELECT unnest($3::uuid[]) AS id
                            UNION SELECT unnest(flow_type_ids) FROM r)
             SELECT id FROM policy.policy_service
              WHERE flow_type_id IN (SELECT id FROM types)
             UNION SELECT unnest(service_ids) FROM r
             UNION SELECT unnest($2::uuid[])",
        )
        .bind(&self.rules)
        .bind(&self.services)
        .bind(&self.flow_types)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
    }
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
    // A flow is no endpoint nor service: its own differential (its anchor) names it only.
    let objects: Vec<Uuid> = objects.difference(&written).copied().collect();
    let written: Vec<Uuid> = written.into_iter().collect();
    // A write of the policy concerns the flows of the services it may change the verdicts
    // of, or every flow (task 011-10).
    let policy = PolicyWrites::of(context.touched);
    let services = policy.services(conn).await?;
    if !policy.everything && objects.is_empty() && services.is_empty() {
        // The flows written only, through their index.
        return sqlx::query_scalar(
            "SELECT DISTINCT f.id FROM flow.flow f
              WHERE f.id = ANY($1)
                AND (upper(f.valid_period) IS NULL OR upper(f.valid_period) > now())
              ORDER BY f.id",
        )
        .persistent(false)
        .bind(&written)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e));
    }
    // Planned for its arrays (`persistent(false)`): the generic plan, chosen after five
    // executions on a connection, scanned the endpoints of every flow through their GiST
    // key for each address, 50 s for a container of 7 000 addresses instead of 0.1 s.
    sqlx::query_scalar(
        "SELECT DISTINCT f.id FROM flow.flow f
          WHERE (upper(f.valid_period) IS NULL OR upper(f.valid_period) > now())
            AND ($3 OR f.id = ANY($1) OR f.service_id = ANY($4)
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
    .persistent(false)
    .bind(&written)
    .bind(&objects)
    .bind(policy.everything)
    .bind(&services)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))
}

/// What a pass over the flows reads once, every write of its changeset done: the policy
/// at each instant, and the instants where a version of the policy starts or ends, the
/// same for every flow (task 011-10).
#[derive(Default)]
struct Pass {
    policies: Policies,
    policy_bounds: Option<Vec<OffsetDateTime>>,
}

impl Pass {
    /// The instants where a version of a policy object starts or ends, in order.
    async fn policy_bounds(
        &mut self,
        conn: &mut PgConnection,
    ) -> Result<&[OffsetDateTime], StoreError> {
        if self.policy_bounds.is_none() {
            let periods: Vec<String> = POLICY_TABLES
                .iter()
                .map(|t| format!("SELECT valid_period FROM {t}"))
                .collect();
            let bounds: Vec<OffsetDateTime> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT DISTINCT b FROM ({}) p,
                        LATERAL (VALUES (lower(valid_period)), (upper(valid_period))) v(b)
                  WHERE b IS NOT NULL ORDER BY b",
                periods.join(" UNION ALL ")
            )))
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
            self.policy_bounds = Some(bounds);
        }
        Ok(self.policy_bounds.as_deref().unwrap_or_default())
    }
}

/// A flow to evaluate anew over [`from`, `to`).
struct Window<'a> {
    id: Uuid,
    flow: &'a Flow,
    from: OffsetDateTime,
    to: Option<OffsetDateTime>,
}

/// For each of `windows`, in their order, the instants in (`from`, `to`) where what the
/// evaluation of its flow reads may change: a version of the policy, an attachment of an
/// endpoint, an origin or a value of an endpoint, of an address it develops into or of a
/// machine holding one, a version of a prefix endpoint (design 011 « Réévaluation »,
/// step 2). One query for the batch; each window is not empty.
async fn boundaries(
    conn: &mut PgConnection,
    pass: &mut Pass,
    windows: &[&Window<'_>],
) -> Result<Vec<Vec<OffsetDateTime>>, StoreError> {
    let policy: Vec<OffsetDateTime> = pass.policy_bounds(conn).await?.to_vec();
    let (mut end_flows, mut end_ids) = (Vec::new(), Vec::new());
    let (mut ids, mut froms, mut tos) = (Vec::new(), Vec::new(), Vec::new());
    for w in windows {
        for end in [w.flow.source.id, w.flow.destination.id] {
            end_flows.push(w.id);
            end_ids.push(end);
        }
        ids.push(w.id);
        froms.push(w.from);
        tos.push(w.to);
    }
    let own: Vec<(Uuid, OffsetDateTime)> = sqlx::query_as(
        "WITH w AS (SELECT f AS flow_id, tstzrange(l, u) AS r
                      FROM unnest($3::uuid[], $4::timestamptz[], $5::timestamptz[]) AS x(f, l, u)),
              ends AS (SELECT e.flow_id, e.id, w.r
                         FROM unnest($1::uuid[], $2::uuid[]) AS e(flow_id, id)
                         JOIN w USING (flow_id)),
              att AS (
                  SELECT e.flow_id, e.r, a.valid_period, a.address_id, i.machine_id
                    FROM ends e
                    JOIN machines.attachment a ON a.interface_id = e.id
                    JOIN machines.interface i ON i.id = a.interface_id
                   WHERE a.valid_period && e.r
                  UNION
                  SELECT e.flow_id, e.r, a.valid_period, a.address_id, i.machine_id
                    FROM ends e
                    JOIN machines.interface i ON i.machine_id = e.id
                    JOIN machines.attachment a ON a.interface_id = i.id
                   WHERE a.valid_period && e.r
                  UNION
                  SELECT e.flow_id, e.r, a.valid_period, a.address_id, i.machine_id
                    FROM ends e
                    JOIN machines.attachment a ON a.address_id = e.id
                    JOIN machines.interface i ON i.id = a.interface_id
                   WHERE a.valid_period && e.r),
              objs AS (SELECT flow_id, r, id FROM ends
                       UNION SELECT flow_id, r, address_id FROM att
                       UNION SELECT flow_id, r, machine_id FROM att),
              origins AS (
                  SELECT s.flow_id, s.r, o.valid_period, o.origin_object_id, o.property_id
                    FROM objs s
                    JOIN public.property_origin o ON o.object_id = s.id
                   WHERE o.valid_period && s.r),
              periods AS (
                  SELECT flow_id, r, valid_period FROM att
                  UNION ALL SELECT flow_id, r, valid_period FROM origins
                  UNION ALL SELECT o.flow_id, o.r, a.valid_period
                              FROM public.property_assignment a
                              JOIN origins o ON a.object_id = o.origin_object_id
                                            AND a.property_id = o.property_id
                             WHERE a.valid_period && o.r
                  UNION ALL SELECT e.flow_id, e.r, p.valid_period
                              FROM ends e JOIN public.prefix p ON p.id = e.id
                             WHERE p.valid_period && e.r)
         SELECT DISTINCT flow_id, b FROM periods,
                LATERAL (VALUES (lower(valid_period)), (upper(valid_period))) v(b)
          WHERE b > lower(r) AND (upper(r) IS NULL OR b < upper(r))",
    )
    .persistent(false)
    .bind(&end_flows)
    .bind(&end_ids)
    .bind(&ids)
    .bind(&froms)
    .bind(&tos)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    let mut of: BTreeMap<Uuid, Vec<OffsetDateTime>> = BTreeMap::new();
    for (flow, b) in own {
        of.entry(flow).or_default().push(b);
    }
    Ok(windows
        .iter()
        .map(|w| {
            let inside = |b: &OffsetDateTime| *b > w.from && w.to.is_none_or(|t| *b < t);
            let mut bounds: Vec<OffsetDateTime> = policy.iter().copied().filter(inside).collect();
            bounds.extend(of.remove(&w.id).unwrap_or_default());
            bounds.sort_unstable();
            bounds.dedup();
            bounds
        })
        .collect())
}

/// The evaluation of each of `windows` over its window, in their order, one segment per
/// stretch where what it reads holds still: the flows evaluated at a same instant are
/// evaluated together (design 011 « Réévaluation », step 2).
async fn evaluate_over(
    conn: &mut PgConnection,
    pass: &mut Pass,
    windows: &[Window<'_>],
) -> Result<Vec<Vec<Segment>>, StoreError> {
    let live: Vec<usize> = (0..windows.len())
        .filter(|&i| windows[i].to.is_none_or(|t| t > windows[i].from))
        .collect();
    let of_live: Vec<&Window<'_>> = live.iter().map(|&i| &windows[i]).collect();
    let bounds = boundaries(conn, pass, &of_live).await?;
    let mut cuts: Vec<Vec<OffsetDateTime>> = vec![Vec::new(); windows.len()];
    let mut at: BTreeMap<OffsetDateTime, Vec<(usize, usize)>> = BTreeMap::new();
    for (&i, bounds) in live.iter().zip(bounds) {
        cuts[i] = std::iter::once(windows[i].from).chain(bounds).collect();
        for (j, t) in cuts[i].iter().enumerate() {
            at.entry(*t).or_default().push((i, j));
        }
    }
    let mut found: Vec<Vec<Option<(State, i64)>>> =
        cuts.iter().map(|c| vec![None; c.len()]).collect();
    for (t, items) in at {
        let flows: Vec<&Flow> = items.iter().map(|&(i, _)| windows[i].flow).collect();
        let evaluated = evaluate_many(conn, &mut pass.policies, &flows, t).await?;
        let states = pg::states_of(conn, &evaluated, t).await?;
        for ((&(i, j), state), e) in items.iter().zip(states).zip(&evaluated) {
            found[i][j] = Some((state, i64::try_from(e.revision).unwrap_or(i64::MAX)));
        }
    }
    Ok(windows
        .iter()
        .zip(cuts)
        .zip(found)
        .map(|((w, cuts), found)| {
            cuts.iter()
                .zip(found)
                .enumerate()
                .filter_map(|(j, (from, found))| {
                    let (state, revision) = found?;
                    Some(Segment {
                        from: *from,
                        to: cuts.get(j + 1).copied().or(w.to),
                        state,
                        revision,
                    })
                })
                .collect()
        })
        .collect())
}

/// A flow whose evaluation the changeset rewrote: its state at the start of the window
/// before and after.
struct Rewritten {
    flow: Uuid,
    before: State,
    after: State,
}

/// The evaluation of each of the flows `ids` computed anew from `now` on, and the one
/// stored, both merged, with the start of its window; in the order of `ids`, a flow that
/// no longer exists left out. Each step reads the batch at once (design 011
/// « Réévaluation », step 2).
async fn fresh(
    conn: &mut PgConnection,
    pass: &mut Pass,
    ids: &[Uuid],
    now: OffsetDateTime,
) -> Result<Vec<(Uuid, OffsetDateTime, Vec<Segment>, Vec<Segment>)>, StoreError> {
    let loaded = pg::load_many(conn, ids).await?;
    let windows: Vec<Window<'_>> = ids
        .iter()
        .filter_map(|id| {
            let (stored, start) = loaded.get(id)?;
            Some(Window {
                id: *id,
                flow: &stored.flow,
                from: (*start).max(now),
                to: stored.flow.period.end().map(pg::time),
            })
        })
        .collect();
    let wanted = evaluate_over(conn, pass, &windows).await?;
    let present: Vec<Uuid> = windows.iter().map(|w| w.id).collect();
    let mut held = pg::read_states(conn, &present, now).await?;
    Ok(windows
        .iter()
        .zip(wanted)
        .map(|(w, wanted)| {
            let held = merged(held.remove(&w.id).unwrap_or_default());
            (w.id, w.from, held, merged(wanted))
        })
        .collect())
}

/// Evaluates the flows `ids` anew from `now` on and rewrites the evaluation of those that
/// differ from the stored one (design 011 « Réévaluation », steps 2 and 3), in the order
/// of `ids`.
async fn reevaluate(
    conn: &mut PgConnection,
    pass: &mut Pass,
    ids: &[Uuid],
    now: OffsetDateTime,
    changeset: Uuid,
) -> Result<Vec<Rewritten>, StoreError> {
    let mut out = Vec::new();
    let mut writes = Vec::new();
    for (flow, from, held, wanted) in fresh(conn, pass, ids, now).await? {
        if same(&held, &wanted) {
            continue;
        }
        out.push(Rewritten {
            flow,
            before: state_at(&held, from),
            after: state_at(&wanted, from),
        });
        writes.push((flow, held, wanted));
    }
    pg::write_states(conn, &writes, now, changeset).await?;
    Ok(out)
}

/// Re-evaluates every flow the changeset of `context` touches and gives the section
/// `flow.flows` (REQ-011-11, 13): first its counts (`nonconforming`: flows that become
/// non-conforming to the flow matrix, i.e. denied; `conforming`: denied flows that conform
/// again; `outcome`: flows whose outcome changes), then
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
    let (mut nonconforming, mut conforming, mut outcome) = (0_u64, 0_u64, 0_u64);
    let mut rows = Vec::new();
    // Every write of the changeset is done: the policy holds still over the pass.
    let mut pass = Pass::default();
    let touched = touched(conn, context).await?;
    let mut rewritten = Vec::new();
    for lot in touched.chunks(LOT) {
        rewritten.extend(reevaluate(conn, &mut pass, lot, now, context.changeset).await?);
    }
    for r in rewritten {
        if r.before.is_empty() || r.before.verdicts() == r.after.verdicts() {
            continue;
        }
        nonconforming += u64::from(!r.before.denied() && r.after.denied());
        conforming += u64::from(r.before.denied() && !r.after.denied());
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
            "nonconforming": nonconforming,
            "conforming": conforming,
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
        // The oracle shares nothing between flows: a policy read for each, one flow a
        // batch.
        let mut pass = Pass::default();
        for (flow, _, held, wanted) in fresh(conn, &mut pass, &[id], now).await? {
            if !same(&held, &wanted) {
                out.push(flow);
            }
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
