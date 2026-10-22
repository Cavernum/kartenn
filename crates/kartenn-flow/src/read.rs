//! Reads of the registry (task 011-7; REQ-011-14, 15, 16, 25; design 011 « Lecture et
//! filtres », « Non-conformités »): the flows at a date filtered by their fields, the
//! flows of an object, the evaluations of a flow at (`valid_at`, `as_of`) without
//! recomputing, and the non-conformities. On request, the list adds the requests pending a
//! review at `as_of`, read in their proposals with their triage (task 011-12, F19).
//!
//! The rights are the routes' (task 011-9): these reads filter nothing by the reader,
//! except the non-conformities, which [`NonconformityFilter::requester`] narrows to the
//! requests of one requester (REQ-011-25).

use kartenn_service::port::StoreError;
use serde_json::{Value, json};
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgConnection, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::handler;
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
    /// The derived states at the date, any of them; none for any state. Citing
    /// `requested` adds the requests pending at `as_of` (F19).
    pub states: Vec<FlowState>,
    /// Only the reviews pending at `as_of`, requests and extensions (`review=pending`, F19).
    pub review_pending: bool,
    /// Only the pending rows of this proposal (`changeset=<id>`, F19).
    pub changeset: Option<Uuid>,
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

impl FlowFilter {
    /// Whether the rows pending a review are read (F19): on request only, `requested`
    /// cited, `review=pending` or `changeset=<id>`, and no state cited that excludes them.
    #[must_use]
    pub fn wants_pending(&self) -> bool {
        let requested = self.states.contains(&FlowState::Requested);
        (self.states.is_empty() || requested)
            && (requested || self.review_pending || self.changeset.is_some())
    }

    /// Whether the applied flows are read: not for the pending reviews nor a proposal, nor
    /// when `requested` is the only state cited (an applied flow never is).
    #[must_use]
    pub fn wants_applied(&self) -> bool {
        !self.review_pending
            && self.changeset.is_none()
            && (self.states.is_empty() || self.states.iter().any(|s| *s != FlowState::Requested))
    }

    /// Whether a row pending a review passes the filters (F19), read in the request and
    /// its triage; a filter on the expanded addresses, a machine or an axis excludes it (no
    /// address is expanded before the application). `services`: those of the flow type
    /// filtered.
    fn passes(&self, row: &FlowView, services: Option<&[Uuid]>) -> bool {
        let Some(review) = &row.review else {
            return false;
        };
        if self.within_prefix.is_some() || self.machine.is_some() || self.axis.is_some() {
            return false;
        }
        let f = &row.flow;
        let id_at = |key: &str| f.get(key).and_then(|v| v.as_str()?.parse::<Uuid>().ok());
        let text_at = |key: &str| f.get(key).and_then(Value::as_str);
        if let Some((kind, id)) = self.endpoint {
            let name = pg::kind_name(kind);
            let side = |s: &str| {
                text_at(&format!("{s}_kind")) == Some(name) && id_at(&format!("{s}_id")) == Some(id)
            };
            if !side("source") && !side("destination") {
                return false;
            }
        }
        let service = id_at("service_id");
        if self.service.is_some() && service != self.service {
            return false;
        }
        if services.is_some_and(|s| !service.is_some_and(|id| s.contains(&id))) {
            return false;
        }
        if self
            .kind
            .as_deref()
            .is_some_and(|k| text_at("kind") != Some(k))
        {
            return false;
        }
        if self.summary.is_some() && row.summary != self.summary {
            return false;
        }
        if self.outcome.is_some() && row.outcome != self.outcome {
            return false;
        }
        if let Some(decisive) = self.decisive {
            let decisive = decisive.to_string();
            let named = review
                .triage
                .as_ref()
                .and_then(|t| t.get("evaluations"))
                .and_then(Value::as_array)
                .is_some_and(|pairs| {
                    pairs
                        .iter()
                        .any(|p| p.get("decisive").and_then(Value::as_str) == Some(&decisive))
                });
            if !named {
                return false;
            }
        }
        if self.waived == Some(true) {
            return false;
        }
        if self
            .ending_before
            .is_some_and(|before| row.end.is_none_or(|end| end >= before))
        {
            return false;
        }
        if self.entity.is_some() && review.entity != self.entity {
            return false;
        }
        if self.requester.is_some_and(|r| r != review.requester) {
            return false;
        }
        if let Some((system, id)) = &self.external_ref {
            let system = system.to_string();
            let cited = f
                .get("external_refs")
                .and_then(Value::as_array)
                .is_some_and(|refs| {
                    refs.iter().any(|r| {
                        r.get("system").and_then(Value::as_str) == Some(&system)
                            && r.get("id").and_then(Value::as_str) == Some(id.as_str())
                    })
                });
            if !cited {
                return false;
            }
        }
        true
    }
}

/// Where a page of flows starts again: after an applied flow, or after a row pending a
/// review (F19), the rows pending following the applied flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    /// After the applied flow of this identifier.
    Flow(Uuid),
    /// After the operation `ordinal` of the proposal `changeset`.
    Pending {
        /// The proposal.
        changeset: Uuid,
        /// The rank of the operation in the proposal.
        ordinal: i32,
    },
}

/// The review a row awaits (F19): its proposal, the kind of the change, and the triage its
/// completed operation keeps, never recomputed.
#[derive(Debug, Clone)]
pub struct Review {
    /// The proposal to review.
    pub changeset: Uuid,
    /// The rank of the operation in the proposal.
    pub ordinal: i32,
    /// `request` (a new flow) or `extension` (a later end of a flow, F20).
    pub kind: &'static str,
    /// The triage as the completed operation keeps it, if the request was triaged.
    pub triage: Option<Value>,
    /// The entity of the request, when named by identifier.
    pub entity: Option<Uuid>,
    /// The requester: the identity the proposal is for.
    pub requester: Uuid,
}

/// A flow as listed: its fields, its derived state and its summary at the date; or a
/// request pending a review, without identifier, with its review (F19).
#[derive(Debug, Clone)]
pub struct FlowView {
    /// Its identifier; `None` for a request pending a review.
    pub id: Option<Uuid>,
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
    /// The review it awaits, for a row pending one (F19).
    pub review: Option<Review>,
}

impl FlowView {
    /// The cursor after this row.
    #[must_use]
    pub fn cursor(&self) -> Cursor {
        match (&self.review, self.id) {
            (Some(r), None) => Cursor::Pending {
                changeset: r.changeset,
                ordinal: r.ordinal,
            },
            (_, id) => Cursor::Flow(id.unwrap_or_default()),
        }
    }
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
/// `after` (REQ-011-14); then, when the filter asks for them, the requests pending a review
/// at `as_of`, by proposal and rank of operation (REQ-011-5, F19).
///
/// # Errors
///
/// A failure of the database.
pub async fn flows(
    conn: &mut PgConnection,
    dates: Dates,
    filter: &FlowFilter,
    after: Option<Cursor>,
    limit: i64,
) -> Result<Vec<FlowView>, StoreError> {
    let mut out = Vec::new();
    let in_pending = matches!(after, Some(Cursor::Pending { .. }));
    if filter.wants_applied() && !in_pending {
        let after = match after {
            Some(Cursor::Flow(id)) => Some(id),
            _ => None,
        };
        out = applied(conn, dates, filter, after, limit).await?;
    }
    let left = limit - i64::try_from(out.len()).unwrap_or(i64::MAX);
    if filter.wants_pending() && left > 0 {
        let after = match after {
            Some(Cursor::Pending { changeset, ordinal }) => Some((changeset, ordinal)),
            _ => None,
        };
        out.extend(pending(conn, dates, filter, after, left).await?);
    }
    Ok(out)
}

/// The proposals pending at `as_of` (now when null, `$1`), of the kind `proposal`:
/// submitted before, not ended then (004). A proposal pending at a date expires after it: the bound
/// keeps the history read short; the end of a proposal not applied is the instant of its
/// event in the journal (012), or its submission when the journal no longer holds it.
const PENDING_NOW: &str = "c.kind = 'proposal' AND c.state = 'pending'";
const PENDING_AT: &str = "c.kind = 'proposal' AND c.submitted_at <= $1::timestamptz
    AND (c.expires_at IS NULL OR c.expires_at > $1::timestamptz)
    AND (c.state = 'pending'
      OR (c.state = 'applied' AND c.applied_at > $1::timestamptz)
      OR (c.state NOT IN ('pending', 'applied') AND coalesce(
            (SELECT min(e.occurred_at) FROM audit.event e
              WHERE e.target_ids @> ARRAY[c.id]
                AND e.name IN ('changeset.state.rejected', 'changeset.state.withdrawn',
                               'changeset.state.expired', 'changeset.state.stale')),
            c.submitted_at) > $1::timestamptz))";

/// The requests pending a review at `dates.as_of` that pass `filter`, by proposal and rank
/// of operation, at most `limit` after `after` (F19): each one as its completed operation
/// keeps it, with its triage, never recomputed; `valid_at` does not set them aside.
///
/// # Errors
///
/// A failure of the database.
async fn pending(
    conn: &mut PgConnection,
    dates: Dates,
    filter: &FlowFilter,
    after: Option<(Uuid, i32)>,
    limit: i64,
) -> Result<Vec<FlowView>, StoreError> {
    type PendingRow = (Uuid, i32, Value, Uuid, OffsetDateTime);
    let rows: Vec<PendingRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT c.id, (o.ordinal - 1)::int, b.body, coalesce(c.on_behalf_of, c.author_id),
                c.submitted_at
           FROM changeset c
           CROSS JOIN LATERAL jsonb_array_elements(c.request -> 'operations')
                WITH ORDINALITY AS o(op, ordinal)
           CROSS JOIN LATERAL (SELECT jsonb_path_query_first(
                o.op, 'lax $.** ? (@.op == \"flow.request\")') AS body) b
          WHERE {} AND b.body IS NOT NULL
            AND ($2::uuid IS NULL OR c.id = $2)
            AND ($3::uuid IS NULL OR (c.id, o.ordinal - 1) > ($3, $4::int))
          ORDER BY c.id, o.ordinal",
        if dates.as_of.is_some() {
            PENDING_AT
        } else {
            PENDING_NOW
        }
    )))
    .bind(dates.as_of)
    .bind(filter.changeset)
    .bind(after.map(|(c, _)| c))
    .bind(after.map_or(0, |(_, o)| o))
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let services: Option<Vec<Uuid>> = match filter.flow_type {
        Some(flow_type) => Some(
            sqlx::query_scalar(
                "SELECT id FROM policy.policy_service
                  WHERE flow_type_id = $1 AND valid_period @> $2",
            )
            .bind(flow_type)
            .bind(dates.valid_at)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| store(&e))?,
        ),
        None => None,
    };
    let state = state_name(state_at(
        &StateFacts {
            creation: Creation::Pending,
            period: kartenn_core::period::Period::new(pg::instant(dates.valid_at), None)
                .map_err(|e| StoreError(e.to_string()))?,
            closed_early: false,
            summary: None,
            waived: false,
        },
        pg::instant(dates.valid_at),
    ));
    let mut out = Vec::new();
    for (changeset, ordinal, body, requester, submitted) in rows {
        let Value::Object(body) = body else {
            continue;
        };
        let row = pending_view(changeset, ordinal, &body, requester, submitted, state);
        if !filter.passes(&row, services.as_deref()) {
            continue;
        }
        out.push(row);
        if i64::try_from(out.len()).unwrap_or(i64::MAX) >= limit {
            break;
        }
    }
    Ok(out)
}

/// A request pending a review as listed (F19): its fields in the form of a stored flow,
/// without identifier, its period as completed, and its review.
fn pending_view(
    changeset: Uuid,
    ordinal: i32,
    body: &serde_json::Map<String, Value>,
    requester: Uuid,
    submitted: OffsetDateTime,
    state: &'static str,
) -> FlowView {
    let completed = body.get("completed");
    let micros = |key: &str| {
        completed
            .and_then(|c| c.get(key))
            .and_then(Value::as_i64)
            .map(|m| pg::time(kartenn_core::period::Instant(m)))
    };
    let triage = completed
        .and_then(|c| c.get("triage"))
        .filter(|t| t.is_object())
        .cloned();
    let side = |name: &str| {
        let kind = body
            .get(name)
            .and_then(|e| e.get("kind"))
            .and_then(Value::as_str);
        (kind, handler::endpoint_id(body, name).map(|(_, id)| id))
    };
    let (source_kind, source_id) = side("source");
    let (destination_kind, destination_id) = side("destination");
    let entity = handler::id_of(body, "entity");
    let text = |key: &str| {
        triage
            .as_ref()
            .and_then(|t| t.get(key))
            .and_then(Value::as_str)
    };
    let flow = json!({
        "id": null,
        "kind": body.get("kind").and_then(Value::as_str).unwrap_or("requested"),
        "source_kind": source_kind,
        "source_id": source_id,
        "destination_kind": destination_kind,
        "destination_id": destination_id,
        "service_id": handler::id_of(body, "service"),
        "external_refs": body.get("external_refs").cloned().unwrap_or_else(|| json!([])),
        "justification": body.get("justification"),
        "requester_id": requester,
        "on_behalf_of": body.get("on_behalf_of"),
        "entity_id": entity,
        "reviewed_by": null,
        "changeset_id": changeset,
    });
    FlowView {
        id: None,
        flow,
        start: micros("start").unwrap_or(submitted),
        end: micros("end"),
        state,
        summary: text("summary").map(str::to_owned),
        outcome: Some(text("outcome").unwrap_or("review").to_owned()),
        waived: false,
        review: Some(Review {
            changeset,
            ordinal,
            kind: "request",
            triage,
            entity,
            requester,
        }),
    }
}

/// The applied flows at `dates` that pass `filter`, ordered by identifier, at most `limit`
/// after `after` (REQ-011-14).
#[allow(clippy::too_many_lines)] // one clause per filter of REQ-011-14
async fn applied(
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
    if filter.states.is_empty() {
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
        if !filter.states.is_empty() && !filter.states.contains(&state) {
            continue;
        }
        out.push(FlowView {
            id: Some(row.try_get("id").map_err(|e| store(&e))?),
            flow: row.try_get("flow").map_err(|e| store(&e))?,
            start,
            end,
            state: state_name(state),
            summary,
            outcome: row.try_get("outcome").map_err(|e| store(&e))?,
            waived: covered,
            review: None,
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
    applied(conn, dates, &filter, after, limit).await
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
