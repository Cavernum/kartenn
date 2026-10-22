//! Reads of the registry (task 011-7; REQ-011-14, 15, 16, 25; design 011 « Lecture et
//! filtres », « Non-conformités »): the flows at a date filtered by their fields, the
//! flows of an object, the evaluations of a flow at (`valid_at`, `as_of`) without
//! recomputing, and the non-conformities.
//!
//! The rights are the routes' (task 011-9): these reads filter nothing by the reader,
//! except the non-conformities, which [`NonconformityFilter::requester`] narrows to the
//! requests of one requester (REQ-011-25).

use kartenn_service::port::StoreError;
use serde_json::{Value, json};
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgConnection, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::model::EndpointKind;
use crate::pg::{self, store};
use crate::state::{Creation, FlowState, StateFacts, Summary, state_at};

/// The dates of a read: `valid_at`, and `as_of` (now when `None`).
#[derive(Debug, Clone, Copy)]
pub struct Dates {
    /// The valid time.
    pub valid_at: OffsetDateTime,
    /// The system time; `None` for now.
    pub as_of: Option<OffsetDateTime>,
}

/// A value of an axis an endpoint carries (REQ-011-14): the key of the property, and the
/// key of its value (closed list) or its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisValue {
    /// The key of the property (`policy.environment`, `core.owner`…).
    pub property: String,
    /// The key of the value, or its text.
    pub value: String,
}

/// The filters of the flows at a date (REQ-011-14); each one given narrows the list.
#[derive(Debug, Clone, Default)]
pub struct FlowFilter {
    /// An endpoint, exactly.
    pub endpoint: Option<(EndpointKind, Uuid)>,
    /// A prefix: flows with an endpoint address or prefix in its subtree.
    pub within_prefix: Option<Uuid>,
    /// A machine: flows whose endpoint is it, one of its interfaces or an address attached
    /// to one of them.
    pub machine: Option<Uuid>,
    /// The service.
    pub service: Option<Uuid>,
    /// The flow type of the service.
    pub flow_type: Option<Uuid>,
    /// `requested` or `observed`.
    pub kind: Option<String>,
    /// The derived state at the date.
    pub state: Option<FlowState>,
    /// The summary at the date.
    pub summary: Option<String>,
    /// The outcome at the date.
    pub outcome: Option<String>,
    /// The decisive rule of one of its evaluations at the date.
    pub decisive: Option<Uuid>,
    /// Whether a waiver covers the date.
    pub waived: Option<bool>,
    /// Flows, or waivers covering the date, whose end falls before this instant.
    pub ending_before: Option<OffsetDateTime>,
    /// The entity.
    pub entity: Option<Uuid>,
    /// The requester.
    pub requester: Option<Uuid>,
    /// An external reference: system, identifier.
    pub external_ref: Option<(Uuid, String)>,
    /// A value of an axis an endpoint address carries at the date.
    pub axis: Option<AxisValue>,
}

/// A flow as listed: its fields, its derived state and its summary at the date.
#[derive(Debug, Clone)]
pub struct FlowView {
    /// Its identifier.
    pub id: Uuid,
    /// Its last version as asserted, as stored (without its period).
    pub flow: Value,
    /// The start of its period.
    pub start: OffsetDateTime,
    /// The end of its period, if it has one.
    pub end: Option<OffsetDateTime>,
    /// Its derived state at the date.
    pub state: &'static str,
    /// Its summary at the date, if evaluated there.
    pub summary: Option<String>,
    /// Its outcome at the date, if evaluated there.
    pub outcome: Option<String>,
    /// Whether a waiver covers the date.
    pub waived: bool,
}

/// The name a state is shown under.
#[must_use]
pub fn state_name(state: FlowState) -> &'static str {
    match state {
        FlowState::Requested => "requested",
        FlowState::Rejected => "rejected",
        FlowState::Scheduled => "scheduled",
        FlowState::Active => "active",
        FlowState::Noncompliant => "noncompliant",
        FlowState::Waived => "waived",
        FlowState::Expired => "expired",
        FlowState::Withdrawn => "withdrawn",
    }
}

/// The rows of `flow.<table>` asserted at `$2` (now when null), with `columns` of the
/// alias `t`: the current table, and its history for a past assertion (ADR 0017).
fn asserted(table: &str, columns: &str) -> String {
    format!(
        "(SELECT {columns} FROM flow.{table} t
           WHERE $2::timestamptz IS NULL OR t.sys_start <= $2::timestamptz
          UNION ALL
          SELECT {columns} FROM flow.{table}_history t
           WHERE $2::timestamptz IS NOT NULL
             AND t.sys_start <= $2::timestamptz AND t.sys_end > $2::timestamptz)"
    )
}

const FLOW_COLUMNS: &str = "t.id, t.kind, t.source_kind, t.source_id, t.destination_kind, \
    t.destination_id, t.service_id, t.external_refs, t.justification, t.requester_id, \
    t.on_behalf_of, t.entity_id, t.reviewed_by, t.valid_period, t.changeset_id";

/// A query whose first two binds are `valid_at` (`$1`) and `as_of` (`$2`): the text pushed
/// after cites them by number, every other value is pushed as a bind.
fn dated_query(dates: Dates, head: &str) -> QueryBuilder<Postgres> {
    let mut q = QueryBuilder::new("WITH dates AS (SELECT ");
    q.push_bind(dates.valid_at);
    q.push("::timestamptz AS valid_at, ");
    q.push_bind(dates.as_of);
    q.push("::timestamptz AS as_of) ");
    q.push(head);
    q
}

/// The flows at `dates` that pass `filter`, ordered by identifier, at most `limit` after
/// `after` (REQ-011-14).
///
/// # Errors
///
/// A failure of the database.
#[allow(clippy::too_many_lines)] // one clause per filter of REQ-011-14
pub async fn flows(
    conn: &mut PgConnection,
    dates: Dates,
    filter: &FlowFilter,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<FlowView>, StoreError> {
    let flow = asserted("flow", FLOW_COLUMNS);
    let summary = asserted(
        "flow_summary",
        "t.flow_id, t.summary, t.outcome, t.valid_period",
    );
    let waiver = asserted("flow_waiver", "t.flow_id, t.valid_period");
    let endpoint = asserted(
        "flow_endpoint_address",
        "t.flow_id, t.address_id, t.valid_period",
    );
    let evaluation = asserted(
        "flow_evaluation",
        "t.flow_id, t.decisive_id, t.valid_period",
    );
    let mut q = dated_query(
        dates,
        &format!(", f AS (SELECT DISTINCT ON (t.id) t.* FROM {flow} t WHERE TRUE"),
    );
    // The flows of which a version has the entity, the service or the requester filtered,
    // through their indexes; the filters below then hold on their last version (task
    // 011-10).
    for (column, value) in [
        ("entity_id", filter.entity),
        ("service_id", filter.service),
        ("requester_id", filter.requester),
    ] {
        if let Some(value) = value {
            q.push(format!(
                " AND t.id IN (SELECT n.id FROM {flow} n WHERE n.{column} = "
            ));
            q.push_bind(value);
            q.push(")");
        }
    }
    q.push(format!(
        " ORDER BY t.id, upper(t.valid_period) DESC NULLS FIRST)
             SELECT to_jsonb(f) - 'valid_period' AS flow, f.id,
                    lower(f.valid_period) AS start, upper(f.valid_period) AS end_,
                    s.summary, s.outcome,
                    EXISTS (SELECT 1 FROM {waiver} w WHERE w.flow_id = f.id
                             AND w.valid_period @> $1::timestamptz) AS waived,
                    (upper(f.valid_period) IS NOT NULL AND EXISTS (
                        SELECT 1 FROM flow.flow_history h WHERE h.id = f.id
                           AND (upper(h.valid_period) IS NULL
                                OR upper(h.valid_period) > upper(f.valid_period))))
                        AS closed_early
               FROM f
               LEFT JOIN {summary} s ON s.flow_id = f.id AND s.valid_period @> $1::timestamptz
              WHERE TRUE"
    ));
    if let Some((kind, id)) = filter.endpoint {
        let name = pg::kind_name(kind);
        q.push(" AND ((f.source_kind = ");
        q.push_bind(name);
        q.push(" AND f.source_id = ");
        q.push_bind(id);
        q.push(") OR (f.destination_kind = ");
        q.push_bind(name);
        q.push(" AND f.destination_id = ");
        q.push_bind(id);
        q.push("))");
    }
    if let Some(prefix) = filter.within_prefix {
        q.push(" AND EXISTS (SELECT 1 FROM public.prefix p WHERE p.id = ");
        q.push_bind(prefix);
        q.push(format!(
            " AND p.valid_period @> $1::timestamptz
                AND (EXISTS (SELECT 1 FROM {endpoint} e
                               JOIN public.address a ON a.id = e.address_id
                                AND a.valid_period @> $1::timestamptz
                              WHERE e.flow_id = f.id AND e.valid_period @> $1::timestamptz
                                AND a.address <<= p.prefix)
                  OR EXISTS (SELECT 1 FROM public.prefix c
                              WHERE c.prefix <<= p.prefix AND c.domain_id = p.domain_id
                                AND c.valid_period @> $1::timestamptz
                                AND ((f.source_kind = 'prefix' AND f.source_id = c.id)
                                  OR (f.destination_kind = 'prefix' AND f.destination_id = c.id)))))"
        ));
    }
    if let Some(machine) = filter.machine {
        q.push(" AND EXISTS (SELECT 1 FROM (SELECT ");
        q.push_bind(machine);
        q.push(format!(
            "::uuid AS m) x WHERE
                (f.source_kind = 'machine' AND f.source_id = x.m)
             OR (f.destination_kind = 'machine' AND f.destination_id = x.m)
             OR EXISTS (SELECT 1 FROM machines.interface i
                         WHERE i.machine_id = x.m AND i.valid_period @> $1::timestamptz
                           AND ((f.source_kind = 'interface' AND f.source_id = i.id)
                             OR (f.destination_kind = 'interface' AND f.destination_id = i.id)))
             OR EXISTS (SELECT 1 FROM {endpoint} e
                          JOIN machines.attachment t ON t.address_id = e.address_id
                           AND t.valid_period @> $1::timestamptz
                          JOIN machines.interface i ON i.id = t.interface_id
                           AND i.valid_period @> $1::timestamptz
                         WHERE e.flow_id = f.id AND e.valid_period @> $1::timestamptz
                           AND i.machine_id = x.m))"
        ));
    }
    if let Some(service) = filter.service {
        q.push(" AND f.service_id = ");
        q.push_bind(service);
    }
    if let Some(flow_type) = filter.flow_type {
        q.push(
            " AND EXISTS (SELECT 1 FROM policy.policy_service ps
                          WHERE ps.id = f.service_id AND ps.valid_period @> $1::timestamptz
                            AND ps.flow_type_id = ",
        );
        q.push_bind(flow_type);
        q.push(")");
    }
    if let Some(kind) = &filter.kind {
        q.push(" AND f.kind = ");
        q.push_bind(kind.clone());
    }
    if let Some(summary) = &filter.summary {
        q.push(" AND s.summary = ");
        q.push_bind(summary.clone());
    }
    if let Some(outcome) = &filter.outcome {
        q.push(" AND s.outcome = ");
        q.push_bind(outcome.clone());
    }
    if let Some(decisive) = filter.decisive {
        q.push(format!(
            " AND EXISTS (SELECT 1 FROM {evaluation} v WHERE v.flow_id = f.id
                           AND v.valid_period @> $1::timestamptz AND v.decisive_id = "
        ));
        q.push_bind(decisive);
        q.push(")");
    }
    if let Some(waived) = filter.waived {
        q.push(format!(
            " AND EXISTS (SELECT 1 FROM {waiver} w WHERE w.flow_id = f.id
                           AND w.valid_period @> $1::timestamptz) = "
        ));
        q.push_bind(waived);
    }
    if let Some(before) = filter.ending_before {
        q.push(" AND (upper(f.valid_period) < ");
        q.push_bind(before);
        q.push(format!(
            " OR EXISTS (SELECT 1 FROM {waiver} w WHERE w.flow_id = f.id
                          AND w.valid_period @> $1::timestamptz AND upper(w.valid_period) < "
        ));
        q.push_bind(before);
        q.push("))");
    }
    if let Some(entity) = filter.entity {
        q.push(" AND f.entity_id = ");
        q.push_bind(entity);
    }
    if let Some(requester) = filter.requester {
        q.push(" AND f.requester_id = ");
        q.push_bind(requester);
    }
    if let Some((system, id)) = &filter.external_ref {
        q.push(" AND f.external_refs @> ");
        q.push_bind(json!([{ "system": system, "id": id }]));
    }
    if let Some(axis) = &filter.axis {
        q.push(" AND EXISTS (SELECT 1 FROM (SELECT ");
        q.push_bind(axis.property.clone());
        q.push("::text AS key, ");
        q.push_bind(axis.value.clone());
        q.push(format!(
            "::text AS value) x
               JOIN public.property_declaration d ON d.key = x.key
               JOIN {endpoint} e ON e.flow_id = f.id AND e.valid_period @> $1::timestamptz
               JOIN public.property_origin o ON o.object_id = e.address_id
                AND o.property_id = d.id AND o.valid_period @> $1::timestamptz
               JOIN public.property_assignment pa ON pa.object_id = o.origin_object_id
                AND pa.property_id = d.id AND pa.valid_period @> $1::timestamptz
                AND pa.kind = 'value'
              WHERE pa.v_text = x.value OR x.value = ANY (pa.v_texts)
                 OR pa.v_uuid IN (SELECT ev.id FROM public.property_enum_value ev
                                   WHERE ev.property_id = d.id AND ev.key = x.value)
                 OR pa.v_uuids && ARRAY(SELECT ev.id FROM public.property_enum_value ev
                                          WHERE ev.property_id = d.id AND ev.key = x.value))"
        ));
    }
    if let Some(after) = after {
        q.push(" AND f.id > ");
        q.push_bind(after);
    }
    q.push(" ORDER BY f.id");
    // The state is derived in memory: the page is cut after it.
    if filter.state.is_none() {
        q.push(" LIMIT ");
        q.push_bind(limit);
    }
    let rows = q
        .build()
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
    let at = pg::instant(dates.valid_at);
    let mut out = Vec::new();
    for row in rows {
        let start: OffsetDateTime = row.try_get("start").map_err(|e| store(&e))?;
        let end: Option<OffsetDateTime> = row.try_get("end_").map_err(|e| store(&e))?;
        let summary: Option<String> = row.try_get("summary").map_err(|e| store(&e))?;
        let covered: bool = row.try_get("waived").map_err(|e| store(&e))?;
        let closed_early: bool = row.try_get("closed_early").map_err(|e| store(&e))?;
        let period = kartenn_core::period::Period::new(pg::instant(start), end.map(pg::instant))
            .map_err(|e| StoreError(e.to_string()))?;
        let state = state_at(
            &StateFacts {
                creation: Creation::Applied,
                period,
                closed_early,
                summary: summary.as_deref().map(summary_of),
                waived: covered,
            },
            at,
        );
        if filter.state.is_some_and(|s| s != state) {
            continue;
        }
        out.push(FlowView {
            id: row.try_get("id").map_err(|e| store(&e))?,
            flow: row.try_get("flow").map_err(|e| store(&e))?,
            start,
            end,
            state: state_name(state),
            summary,
            outcome: row.try_get("outcome").map_err(|e| store(&e))?,
            waived: covered,
        });
        if i64::try_from(out.len()).unwrap_or(i64::MAX) >= limit {
            break;
        }
    }
    Ok(out)
}

fn summary_of(name: &str) -> Summary {
    match name {
        "allowed" => Summary::Allowed,
        "partial" => Summary::Partial,
        _ => Summary::Denied,
    }
}

/// The flows of an object at `dates` (REQ-011-15): those it is an endpoint of, or whose
/// endpoint it contains — an address in a prefix's subtree, an interface or an address of
/// a machine.
///
/// # Errors
///
/// A failure of the database.
pub async fn of_object(
    conn: &mut PgConnection,
    dates: Dates,
    kind: EndpointKind,
    id: Uuid,
    after: Option<Uuid>,
    limit: i64,
) -> Result<Vec<FlowView>, StoreError> {
    let filter = match kind {
        EndpointKind::Prefix => FlowFilter {
            within_prefix: Some(id),
            ..FlowFilter::default()
        },
        EndpointKind::Machine => FlowFilter {
            machine: Some(id),
            ..FlowFilter::default()
        },
        other => FlowFilter {
            endpoint: Some((other, id)),
            ..FlowFilter::default()
        },
    };
    flows(conn, dates, &filter, after, limit).await
}

/// One evaluation of a flow by pair of addresses, as stored (REQ-011-16).
#[derive(Debug, Clone)]
pub struct EvaluationView {
    /// The source address.
    pub source: Uuid,
    /// The destination address.
    pub destination: Uuid,
    /// The verdict.
    pub verdict: String,
    /// The outcome.
    pub outcome: String,
    /// The reason.
    pub reason: String,
    /// The decisive element, if any.
    pub decisive: Option<Uuid>,
    /// The combination of the values of the axes.
    pub combination: Value,
    /// The conditions not met.
    pub unmet_conditions: Vec<Uuid>,
    /// The informative score.
    pub score: Option<i32>,
    /// The revision of the policy it was computed against.
    pub policy_revision: i64,
}

/// The evaluations of `flow` valid at `dates`, as asserted then, never recomputed
/// (REQ-011-16, 23).
///
/// # Errors
///
/// A failure of the database.
pub async fn evaluations(
    conn: &mut PgConnection,
    dates: Dates,
    flow: Uuid,
) -> Result<Vec<EvaluationView>, StoreError> {
    let evaluation = asserted(
        "flow_evaluation",
        "t.flow_id, t.source_address_id, t.destination_address_id, t.verdict, t.outcome, \
         t.reason, t.decisive_id, t.combination, t.unmet_conditions, t.score, \
         t.policy_revision, t.valid_period",
    );
    let mut q = dated_query(
        dates,
        &format!(
            "SELECT v.source_address_id, v.destination_address_id, v.verdict, v.outcome,
                    v.reason, v.decisive_id, v.combination, v.unmet_conditions, v.score,
                    v.policy_revision
               FROM {evaluation} v
              WHERE v.valid_period @> $1::timestamptz AND v.flow_id = "
        ),
    );
    q.push_bind(flow);
    q.push(" ORDER BY v.source_address_id, v.destination_address_id");
    let rows = q.build().fetch_all(conn).await.map_err(|e| store(&e))?;
    rows.iter()
        .map(|row| {
            Ok(EvaluationView {
                source: row.try_get(0)?,
                destination: row.try_get(1)?,
                verdict: row.try_get(2)?,
                outcome: row.try_get(3)?,
                reason: row.try_get(4)?,
                decisive: row.try_get(5)?,
                combination: row.try_get(6)?,
                unmet_conditions: row.try_get(7)?,
                score: row.try_get(8)?,
                policy_revision: row.try_get(9)?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(|e| store(&e))
}

/// The filters of the non-conformities (REQ-011-25).
#[derive(Debug, Clone, Default)]
pub struct NonconformityFilter {
    /// Only the rows of this requester: a reader without the read of the entity sees the
    /// rows of their own requests only.
    pub requester: Option<Uuid>,
    /// The entity (the application).
    pub entity: Option<Uuid>,
    /// The rule in cause.
    pub rule: Option<Uuid>,
    /// Rows from this instant.
    pub from: Option<OffsetDateTime>,
    /// Rows before this instant.
    pub to: Option<OffsetDateTime>,
    /// `rejected`, `noncompliant` or `waived`.
    pub state: Option<String>,
    /// `requested` or `observed`.
    pub kind: Option<String>,
}

/// A non-conformity (REQ-011-25): a request rejected at review, a flow `noncompliant` or
/// `waived` at the date, with its motive, the rule in cause and its educational texts.
#[derive(Debug, Clone)]
pub struct Nonconformity {
    /// `rejected`, `noncompliant` or `waived`.
    pub state: String,
    /// The flow; none for a rejected request (it was never applied).
    pub flow: Option<Uuid>,
    /// The changeset of the rejected request.
    pub changeset: Option<Uuid>,
    /// The requester.
    pub requester: Uuid,
    /// The entity.
    pub entity: Uuid,
    /// The motive: of the rejection, of the waiver, or the reason of the evaluation.
    pub motive: Option<String>,
    /// The rule in cause.
    pub rule: Option<Uuid>,
    /// Its key, label and educational texts at the date.
    pub rule_key: Option<String>,
    /// Its label.
    pub rule_label: Option<String>,
    /// Its texts by language.
    pub rule_texts: Option<Value>,
    /// When it arose: the rejection, or the start of the evaluation.
    pub at: OffsetDateTime,
    /// `requested` or `observed`.
    pub kind: String,
}

/// The non-conformities at `dates` that pass `filter`, newest first, at most `limit`
/// (REQ-011-25).
///
/// # Errors
///
/// A failure of the database.
pub async fn nonconformities(
    conn: &mut PgConnection,
    dates: Dates,
    filter: &NonconformityFilter,
    limit: i64,
) -> Result<Vec<Nonconformity>, StoreError> {
    let flow = asserted("flow", FLOW_COLUMNS);
    let summary = asserted("flow_summary", "t.flow_id, t.summary, t.valid_period");
    let waiver = asserted("flow_waiver", "t.flow_id, t.reason, t.valid_period");
    let evaluation = asserted(
        "flow_evaluation",
        "t.flow_id, t.verdict, t.reason, t.decisive_id, t.valid_period",
    );
    let mut q = dated_query(
        dates,
        &format!(
            ", rows AS (
                 SELECT 'rejected'::text AS state, NULL::uuid AS flow, r.changeset_id AS changeset,
                        r.requester_id AS requester, r.entity_id AS entity, r.reason AS motive,
                        r.rule_id AS rule, r.rejected_at AS at,
                        coalesce(r.request->>'kind', 'requested') AS kind
                   FROM flow.flow_rejection r
                  WHERE $2::timestamptz IS NULL OR r.rejected_at <= $2::timestamptz
                 UNION ALL
                 SELECT CASE WHEN w.flow_id IS NULL THEN 'noncompliant' ELSE 'waived' END,
                        f.id, NULL, f.requester_id, f.entity_id,
                        coalesce(w.reason, v.reason), v.decisive_id, lower(s.valid_period),
                        f.kind
                   FROM {summary} s
                   -- From the denied summaries, few, to their flow (task 011-10).
                   CROSS JOIN LATERAL (
                     SELECT t.* FROM {flow} t
                      WHERE t.id = s.flow_id AND t.valid_period @> $1::timestamptz
                      ORDER BY upper(t.valid_period) DESC NULLS FIRST LIMIT 1) f
                   LEFT JOIN {waiver} w ON w.flow_id = f.id AND w.valid_period @> $1::timestamptz
                   LEFT JOIN LATERAL (
                     SELECT e.reason, e.decisive_id FROM {evaluation} e
                      WHERE e.flow_id = f.id AND e.valid_period @> $1::timestamptz
                        AND e.verdict = 'denied'
                      ORDER BY e.decisive_id NULLS LAST LIMIT 1) v ON TRUE
                  WHERE s.valid_period @> $1::timestamptz AND s.summary = 'denied')
             SELECT x.state, x.flow, x.changeset, x.requester, x.entity, x.motive, x.rule,
                    pr.key, pr.label, pr.texts, x.at, x.kind
               FROM rows x
               LEFT JOIN policy.policy_rule pr ON pr.id = x.rule
                AND pr.valid_period @> $1::timestamptz
              WHERE TRUE"
        ),
    );
    if let Some(requester) = filter.requester {
        q.push(" AND x.requester = ");
        q.push_bind(requester);
    }
    if let Some(entity) = filter.entity {
        q.push(" AND x.entity = ");
        q.push_bind(entity);
    }
    if let Some(rule) = filter.rule {
        q.push(" AND x.rule = ");
        q.push_bind(rule);
    }
    if let Some(from) = filter.from {
        q.push(" AND x.at >= ");
        q.push_bind(from);
    }
    if let Some(to) = filter.to {
        q.push(" AND x.at < ");
        q.push_bind(to);
    }
    if let Some(state) = &filter.state {
        q.push(" AND x.state = ");
        q.push_bind(state.clone());
    }
    if let Some(kind) = &filter.kind {
        q.push(" AND x.kind = ");
        q.push_bind(kind.clone());
    }
    q.push(" ORDER BY x.at DESC, x.flow, x.changeset LIMIT ");
    q.push_bind(limit);
    let rows = q.build().fetch_all(conn).await.map_err(|e| store(&e))?;
    rows.iter()
        .map(|row| {
            Ok(Nonconformity {
                state: row.try_get(0)?,
                flow: row.try_get(1)?,
                changeset: row.try_get(2)?,
                requester: row.try_get(3)?,
                entity: row.try_get(4)?,
                motive: row.try_get(5)?,
                rule: row.try_get(6)?,
                rule_key: row.try_get(7)?,
                rule_label: row.try_get(8)?,
                rule_texts: row.try_get(9)?,
                at: row.try_get(10)?,
                kind: row.try_get(11)?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(|e| store(&e))
}
