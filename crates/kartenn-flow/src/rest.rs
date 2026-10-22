//! The routes of the module under `/v1` (feature `rest`; spec 011, REQ-011-14 to 18, 25;
//! design « API »), mounted by kartenn-server: the flows as a collection, dated, paged and
//! filtered; a request by `POST /v1/flows`, triaged (REQ-011-21), answered with the flow
//! and its evaluations; the evaluations of a flow at (`valid_at`, `as_of`); the
//! non-conformities; the flows of an address, a prefix, an interface or a machine.
//!
//! Rights (REQ-011-17): a flow is read under `flow.flow.read` on its entity, by the
//! predicate of the core (ADR 0021); its requester reads it too, with the explanation
//! reduced (REQ-010-25). A flow the caller may not read answers as an absent one.

use std::collections::{BTreeMap, HashMap, HashSet};

use axum::body::{Body, Bytes};
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Extension, Json};
use kartenn_rest::ModuleRouter;
use kartenn_rest::kit::{
    self, Api, Dates as DateParams, DatesParams, ETag, ListQuery, MERGE_PATCH, Moment, Page,
    PageParams, Problem, ProblemKind, documented, listing_of, next_href, op, queryable, respond,
    respond_page, schemas_of,
};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use kartenn_store_pg::rbac::{ReadScope, readable};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::types::time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa::openapi::path::HttpMethod;
use utoipa::openapi::{RefOr, Schema};
use uuid::Uuid;

use crate::handler::REVIEW;
use crate::model::EndpointKind;
use crate::module::{FLOW, flow_type};
use crate::read::{
    self, AxisValue, Cursor, Dates, EvaluationView, FlowFilter, FlowView, Nonconformity,
    NonconformityFilter, Review,
};
use crate::state::FlowState;

const COLLECTION: &str = "flows";
const NONCONFORMITIES: &str = "flow-nonconformities";

/// The members a request may carry (REQ-011-1, 20, 21).
const REQUEST_MEMBERS: &[&str] = &[
    "kind",
    "source",
    "destination",
    "service",
    "external_refs",
    "justification",
    "on_behalf_of",
    "responsibles",
    "data_nature",
    "personal_data",
    "entity",
    "end",
];

/// The members a modification may carry (REQ-011-4: the identity does not change).
const UPDATE_MEMBERS: &[&str] = &[
    "external_refs",
    "justification",
    "responsibles",
    "data_nature",
    "personal_data",
    "end",
];

/// The members of a waiver (REQ-011-9).
const WAIVER_MEMBERS: &[&str] = &["reason", "accepted_risk", "end"];

/// The filters of the list (REQ-011-14).
const FILTERS: &[(&str, &str)] = &[
    (
        "endpoint",
        "An endpoint, exact: `<kind>:<id>` (address, prefix, interface, machine, range).",
    ),
    (
        "prefix",
        "A prefix: the flows with an endpoint address in its subtree.",
    ),
    (
        "machine",
        "A machine: the flows with an endpoint address of the machine.",
    ),
    ("service", "The service."),
    ("flow_type", "The flow type of the service."),
    ("kind", "`requested` or `observed`."),
    (
        "state",
        "One or more of `requested`, `rejected`, `scheduled`, `active`, `noncompliant`, \
         `waived`, `expired` and `withdrawn`, separated by commas. Citing `requested` adds \
         the requests pending a review, without identifier, after the applied flows.",
    ),
    (
        "review",
        "`pending`: only the reviews pending, requests and extensions, with their triage.",
    ),
    ("changeset", "A proposal: only the rows pending its review."),
    ("summary", "`allowed`, `partial` or `denied`."),
    ("outcome", "`automatic`, `review` or `noncompliant`."),
    ("decisive", "The decisive rule or pattern of an evaluation."),
    ("waived", "`true` or `false`."),
    ("ending_before", "Ending before this instant."),
    ("entity", "The entity (the application)."),
    ("requester", "The requester."),
    (
        "external_ref",
        "An external reference: `<system>:<identifier>`.",
    ),
    (
        "axis",
        "A value of an axis an endpoint carries: `<property>=<value>`.",
    ),
];

/// The filters of the non-conformities (REQ-011-25).
const NONCONFORMITY_FILTERS: &[&str] =
    &["entity", "requester", "rule", "from", "to", "state", "kind"];

/// A flow as listed or read (REQ-011-14, 24): its fields as stored, its period, and its
/// derived state, summary and outcome at the date. A request pending a review (F19) has
/// no identifier yet, and carries its `review`.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Flow)]
pub struct FlowDto {
    /// Its identifier; `null` for a request pending a review (the flow exists once
    /// applied).
    #[schema(required = true)]
    pub id: Option<Uuid>,
    /// Its fields as stored: kind, endpoints, service, external references, justification,
    /// requester, entity…
    #[schema(value_type = Object)]
    pub fields: Value,
    /// The start of its period.
    pub valid_from: String,
    /// The end of its period, if any.
    pub valid_to: Option<String>,
    /// Its derived state at the date.
    pub state: String,
    /// Its summary at the date, if evaluated there.
    pub summary: Option<String>,
    /// Its outcome at the date, if evaluated there.
    pub outcome: Option<String>,
    /// Whether a waiver covers the date.
    pub waived: bool,
    /// The review the row awaits, for a request or an extension pending one (F19).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDto>,
}

/// The review a row awaits (F19): the proposal to review, the kind of the change, and the
/// triage that held it, as the operation keeps it, never recomputed.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = FlowReview)]
pub struct ReviewDto {
    /// The proposal to review.
    pub changeset: Uuid,
    /// `request` (a new flow) or `extension` (a later end of a flow).
    pub kind: String,
    /// The triage; `null` when the request named an endpoint not yet created.
    pub triage: Option<TriageDto>,
}

/// The triage of a request (REQ-011-21), as its operation keeps it. Reduced for a reader
/// without the read of the flow (REQ-010-25): the decisive element and the addresses are
/// not named.
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
#[schema(as = FlowTriage)]
pub struct TriageDto {
    /// `allowed`, `partial` or `denied`; `null` when an endpoint holds no address.
    #[serde(default)]
    pub summary: Option<String>,
    /// `automatic`, `review` or `noncompliant`.
    #[serde(default)]
    pub outcome: Option<String>,
    /// The decisive element of the worst pair; `null` when reduced.
    #[serde(default)]
    pub decisive: Option<String>,
    /// The key of the decisive rule, if any.
    #[serde(default)]
    pub rule: Option<String>,
    /// The conditions not met by the decisive pair.
    #[serde(default)]
    pub unmet_conditions: Vec<Uuid>,
    /// The informative score of the decisive pair.
    #[serde(default)]
    pub score: Option<i32>,
    /// The warnings (risk signals) of the decisive pair.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// The revision of the policy it was computed against.
    #[serde(default, rename(deserialize = "revision"))]
    pub policy_revision: Option<i64>,
    /// The evaluations by pair of addresses.
    #[serde(default)]
    pub evaluations: Vec<EvaluationDto>,
}

/// An evaluation of a flow by pair of addresses (REQ-011-16, 23): stored, never
/// recomputed. Reduced for a requester without the read of the flow (REQ-010-25): the
/// addresses and the decisive element are not named.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(as = FlowEvaluation)]
pub struct EvaluationDto {
    /// The source address; `null` when reduced.
    pub source: Option<Uuid>,
    /// The destination address; `null` when reduced.
    pub destination: Option<Uuid>,
    /// `allowed`, `conditional` or `denied`.
    pub verdict: String,
    /// `automatic`, `review` or `noncompliant`.
    pub outcome: String,
    /// The reason.
    pub reason: String,
    /// The decisive rule or pattern; `null` when reduced.
    pub decisive: Option<Uuid>,
    /// The values of the axes of the pair.
    #[schema(value_type = Object)]
    pub combination: Value,
    /// The conditions not met.
    pub unmet_conditions: Vec<Uuid>,
    /// The informative score.
    pub score: Option<i32>,
    /// The revision of the policy it was computed against.
    pub policy_revision: i64,
}

/// A non-conformity (REQ-011-25).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = FlowNonconformity)]
pub struct NonconformityDto {
    /// `rejected`, `noncompliant` or `waived`.
    pub state: String,
    /// The flow; `null` for a rejected request.
    pub flow: Option<Uuid>,
    /// The changeset of the rejected request.
    pub changeset: Option<Uuid>,
    /// The requester.
    pub requester: Uuid,
    /// The entity.
    pub entity: Uuid,
    /// The motive.
    pub motive: Option<String>,
    /// The rule in cause.
    pub rule: Option<Uuid>,
    /// Its key.
    pub rule_key: Option<String>,
    /// Its label.
    pub rule_label: Option<String>,
    /// Its educational texts by language.
    #[schema(value_type = Option<Object>)]
    pub rule_texts: Option<Value>,
    /// When it arose.
    pub at: String,
    /// `requested` or `observed`.
    pub kind: String,
}

/// The body of `POST /v1/flows` (REQ-011-1, 20, 21), besides `motive`, `valid_from`.
#[allow(dead_code)] // documents the body only: the members are read by the operation
#[derive(Debug, ToSchema)]
#[schema(as = FlowRequest)]
pub struct FlowRequestDto {
    /// `requested` (default) or `observed`.
    kind: Option<String>,
    /// The source: `{ "kind": "address" | "prefix" | "interface" | "machine" | "range",
    /// "id": { "id": … } }`.
    #[schema(value_type = Object)]
    source: Value,
    /// The destination, as the source.
    #[schema(value_type = Object)]
    destination: Value,
    /// The service: `{ "id": … }`.
    #[schema(value_type = Object)]
    service: Value,
    /// The external references, at least one: `{ "system": …, "id": … }`.
    #[schema(value_type = Vec<Object>)]
    external_refs: Vec<Value>,
    /// The justification.
    justification: Option<String>,
    /// The requester the request is made for.
    #[schema(value_type = Option<Object>)]
    on_behalf_of: Option<Value>,
    /// The persons responsible.
    responsibles: Vec<Uuid>,
    /// The natures of the data carried.
    data_nature: Vec<Uuid>,
    /// Whether personal data are carried.
    personal_data: Option<bool>,
    /// The entity (the application): `{ "id": … }`.
    #[schema(value_type = Object)]
    entity: Value,
    /// The end of the flow, an RFC 3339 instant; none for a flow without end.
    end: Option<String>,
    /// The motive of the changeset.
    motive: Option<String>,
}

/// The body of `PATCH /v1/flows/{id}`: the fields that may change (REQ-011-4).
#[allow(dead_code)] // documents the body only: the members are read by the operation
#[derive(Debug, ToSchema)]
#[schema(as = FlowPatch)]
pub struct FlowPatchDto {
    /// The external references.
    #[schema(value_type = Option<Vec<Object>>)]
    external_refs: Option<Vec<Value>>,
    /// The justification.
    justification: Option<String>,
    /// The persons responsible.
    responsibles: Option<Vec<Uuid>>,
    /// The natures of the data carried.
    data_nature: Option<Vec<Uuid>>,
    /// Whether personal data are carried.
    personal_data: Option<bool>,
    /// An earlier end.
    end: Option<String>,
    /// The motive of the changeset.
    motive: Option<String>,
}

/// The body of `POST /v1/flows/{id}/waiver` (REQ-011-9).
#[allow(dead_code)] // documents the body only: the members are read by the operation
#[derive(Debug, ToSchema)]
#[schema(as = FlowWaiver)]
pub struct FlowWaiverDto {
    /// The reason.
    reason: String,
    /// The risk accepted.
    accepted_risk: String,
    /// The end of the waiver, mandatory.
    end: String,
    /// The motive of the changeset.
    motive: Option<String>,
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&time::format_description::well_known::Rfc3339)
        .expect("an instant formats")
}

/// A row as served; the triage of a review reduced when `reduced` (REQ-010-25).
fn flow_dto(view: FlowView, reduced: bool) -> FlowDto {
    let review = view.review.map(|r| ReviewDto {
        changeset: r.changeset,
        kind: r.kind.to_owned(),
        triage: r.triage.map(|t| {
            let mut triage: TriageDto = serde_json::from_value(t).unwrap_or_default();
            if reduced {
                triage.decisive = None;
                for e in &mut triage.evaluations {
                    e.source = None;
                    e.destination = None;
                    e.decisive = None;
                }
            }
            triage
        }),
    });
    FlowDto {
        id: view.id,
        fields: view.flow,
        valid_from: rfc3339(view.start),
        valid_to: view.end.map(rfc3339),
        state: view.state.to_owned(),
        summary: view.summary,
        outcome: view.outcome,
        waived: view.waived,
        review,
    }
}

fn evaluation_dto(view: EvaluationView, reduced: bool) -> EvaluationDto {
    EvaluationDto {
        source: (!reduced).then_some(view.source),
        destination: (!reduced).then_some(view.destination),
        verdict: view.verdict,
        outcome: view.outcome,
        reason: view.reason,
        decisive: if reduced { None } else { view.decisive },
        combination: view.combination,
        unmet_conditions: view.unmet_conditions,
        score: view.score,
        policy_revision: view.policy_revision,
    }
}

fn nonconformity_dto(n: Nonconformity) -> NonconformityDto {
    NonconformityDto {
        state: n.state,
        flow: n.flow,
        changeset: n.changeset,
        requester: n.requester,
        entity: n.entity,
        motive: n.motive,
        rule: n.rule,
        rule_key: n.rule_key,
        rule_label: n.rule_label,
        rule_texts: n.rule_texts,
        at: rfc3339(n.at),
        kind: n.kind,
    }
}

fn dates(moment: &Moment) -> Dates {
    Dates {
        valid_at: moment.valid_at,
        as_of: moment.as_of,
    }
}

fn uuid(name: &str, text: &str) -> Result<Uuid, Problem> {
    text.parse()
        .map_err(|_| kit::bad(format!("{name} is not an identifier")))
}

fn instant(name: &str, text: &str) -> Result<OffsetDateTime, Problem> {
    OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        .map_err(|_| kit::bad(format!("{name} is not an RFC 3339 instant")))
}

fn endpoint_kind(name: &str) -> Option<EndpointKind> {
    Some(match name {
        "address" => EndpointKind::Address,
        "prefix" => EndpointKind::Prefix,
        "interface" => EndpointKind::Interface,
        "machine" => EndpointKind::Machine,
        "range" => EndpointKind::Range,
        _ => return None,
    })
}

fn state(name: &str) -> Option<FlowState> {
    Some(match name {
        "requested" => FlowState::Requested,
        "rejected" => FlowState::Rejected,
        "scheduled" => FlowState::Scheduled,
        "active" => FlowState::Active,
        "noncompliant" => FlowState::Noncompliant,
        "waived" => FlowState::Waived,
        "expired" => FlowState::Expired,
        "withdrawn" => FlowState::Withdrawn,
        _ => return None,
    })
}

/// The filters of a page, each known and given once.
fn wanted<'a>(page: &'a Page, names: &[&str]) -> Result<BTreeMap<&'a str, &'a str>, Problem> {
    let mut found = BTreeMap::new();
    for (name, value) in &page.filters {
        if !names.contains(&name.as_str()) {
            return Err(kit::bad(format!("unknown parameter {name}")));
        }
        if found.insert(name.as_str(), value.as_str()).is_some() {
            return Err(kit::bad(format!("{name} is given more than once")));
        }
    }
    Ok(found)
}

#[allow(clippy::too_many_lines)] // one clause per filter of REQ-011-14
fn filter_of(wanted: &BTreeMap<&str, &str>) -> Result<FlowFilter, Problem> {
    let mut filter = FlowFilter::default();
    for (&name, &value) in wanted {
        match name {
            "endpoint" => {
                let (kind, id) = value
                    .split_once(':')
                    .ok_or_else(|| kit::bad("endpoint is `<kind>:<id>`"))?;
                let kind = endpoint_kind(kind)
                    .ok_or_else(|| kit::bad(format!("{kind} is not an endpoint kind")))?;
                filter.endpoint = Some((kind, uuid(name, id)?));
            }
            "prefix" => filter.within_prefix = Some(uuid(name, value)?),
            "machine" => filter.machine = Some(uuid(name, value)?),
            "service" => filter.service = Some(uuid(name, value)?),
            "flow_type" => filter.flow_type = Some(uuid(name, value)?),
            "kind" => match value {
                "requested" | "observed" => filter.kind = Some(value.to_owned()),
                other => return Err(kit::bad(format!("{other} is not a kind of flow"))),
            },
            "state" => {
                for name in value.split(',') {
                    let state =
                        state(name).ok_or_else(|| kit::bad(format!("{name} is not a state")))?;
                    if !filter.states.contains(&state) {
                        filter.states.push(state);
                    }
                }
            }
            "review" => match value {
                "pending" => filter.review_pending = true,
                other => return Err(kit::bad(format!("review is `pending`, not {other}"))),
            },
            "changeset" => filter.changeset = Some(uuid(name, value)?),
            "summary" => match value {
                "allowed" | "partial" | "denied" => filter.summary = Some(value.to_owned()),
                other => return Err(kit::bad(format!("{other} is not a summary"))),
            },
            "outcome" => match value {
                "automatic" | "review" | "noncompliant" => {
                    filter.outcome = Some(value.to_owned());
                }
                other => return Err(kit::bad(format!("{other} is not an outcome"))),
            },
            "decisive" => filter.decisive = Some(uuid(name, value)?),
            "waived" => {
                filter.waived = Some(
                    value
                        .parse()
                        .map_err(|_| kit::bad("waived is `true` or `false`"))?,
                );
            }
            "ending_before" => filter.ending_before = Some(instant(name, value)?),
            "entity" => filter.entity = Some(uuid(name, value)?),
            "requester" => filter.requester = Some(uuid(name, value)?),
            "external_ref" => {
                let (system, id) = value
                    .split_once(':')
                    .ok_or_else(|| kit::bad("external_ref is `<system>:<identifier>`"))?;
                filter.external_ref = Some((uuid(name, system)?, id.to_owned()));
            }
            "axis" => {
                let (property, value) = value
                    .split_once('=')
                    .ok_or_else(|| kit::bad("axis is `<property>=<value>`"))?;
                filter.axis = Some(AxisValue {
                    property: property.to_owned(),
                    value: value.to_owned(),
                });
            }
            _ => unreachable!("the names are checked by `wanted`"),
        }
    }
    Ok(filter)
}

/// The flows of `ids` the caller reads at `at`: under `flow.flow.read` by the predicate
/// of the core on their entity (ADR 0021), or as their requester (REQ-011-17).
async fn visible(
    api: &Api,
    scope: &ReadScope,
    principal: &Principal,
    at: OffsetDateTime,
    ids: &[Uuid],
) -> Result<HashSet<Uuid>, Problem> {
    if ids.is_empty() {
        return Ok(HashSet::new());
    }
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($2))
         SELECT x.id FROM unnest($3::uuid[]) AS x(id)
          WHERE $2::jsonb IS NULL OR {}
             OR EXISTS (SELECT 1 FROM flow.flow f
                         WHERE f.id = x.id AND f.requester_id = $4)",
        readable(
            "x.id",
            &format!("'{FLOW}'"),
            "(SELECT m.domain_id FROM module_object m
               WHERE m.id = x.id AND m.valid_period @> $1::timestamptz LIMIT 1)",
            "NULL::inet",
            "$1",
        )
    );
    let rows: Vec<(Uuid,)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(at)
        .bind(scope.to_json())
        .bind(ids)
        .bind(principal.identity)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// The moment and the read scope of a request of `principal` (the scope of
/// `flow.flow.read`).
async fn reading(
    api: &Api,
    principal: &Principal,
    page: &Page,
) -> Result<(Moment, ReadScope), Problem> {
    let now = kit::now(api).await?;
    let moment = page.dates.at(now, api.bound)?;
    let scope = kit::scope(api, principal, now).await?;
    Ok((moment, scope))
}

/// A page of flows read by `fetch` after the cursor, kept to those the caller reads.
#[allow(clippy::too_many_arguments)] // the request, then what the route reads
async fn page_of<F, Fut>(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    headers: &HeaderMap,
    query: &str,
    collection: &str,
    path: &str,
    fetch: F,
) -> Result<Response, Problem>
where
    F: FnOnce(Dates, Option<Cursor>, i64, Page) -> Fut,
    Fut: std::future::Future<Output = Result<(Vec<FlowView>, Page), Problem>>,
{
    let page = Page::from_query(query, &api.ring, collection)?;
    let after = page.after().map(cursor_of).transpose()?;
    let (moment, scope) = reading(api, principal, &page).await?;
    let size = usize::from(page.size);
    let (mut rows, page) = fetch(dates(&moment), after, i64::from(page.size) + 1, page).await?;
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let cursor = page.next(
            &api.ring,
            collection,
            &moment,
            cursor_json(rows[size - 1].cursor()),
        );
        next_href(path, &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().filter_map(|r| r.id).collect();
    let seen = visible(api, &scope, principal, moment.valid_at, &ids).await?;
    let reviews = reviews_seen(api, &scope, principal, moment.valid_at, &rows).await?;
    let mut items = Vec::with_capacity(rows.len());
    let (mut ids, mut changesets) = (Vec::new(), Vec::new());
    for row in rows {
        match (row.id, &row.review) {
            (Some(id), _) if seen.contains(&id) => {
                ids.push(id);
                items.push(flow_dto(row, false));
            }
            (None, Some(review)) => {
                if let Some(&reduced) = reviews.get(&(review.changeset, review.ordinal)) {
                    changesets.push(review.changeset);
                    items.push(flow_dto(row, reduced));
                }
            }
            _ => {}
        }
    }
    kit::record_read(
        api,
        context,
        FLOW,
        &ids,
        json!({
            "collection": collection,
            "filters": page.filters,
            "valid_at": rfc3339(moment.valid_at),
            "as_of": rfc3339(moment.as_of()),
            "changesets": changesets,
        }),
    )
    .await?;
    Ok(respond_page(headers, &moment, &items, next.as_deref()))
}

/// The cursor of a page: the identifier of the last flow, or `{changeset, ordinal}` of
/// the last request pending a review (F19).
fn cursor_json(cursor: Cursor) -> Value {
    match cursor {
        Cursor::Flow(id) => json!(id),
        Cursor::Pending { changeset, ordinal } => {
            json!({ "changeset": changeset, "ordinal": ordinal })
        }
    }
}

fn cursor_of(key: &Value) -> Result<Cursor, Problem> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Pending {
        changeset: Uuid,
        ordinal: i32,
    }
    if let Ok(id) = serde_json::from_value::<Uuid>(key.clone()) {
        return Ok(Cursor::Flow(id));
    }
    serde_json::from_value::<Pending>(key.clone())
        .map(|p| Cursor::Pending {
            changeset: p.changeset,
            ordinal: p.ordinal,
        })
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))
}

/// The requests pending a review among `rows` the caller reads, by proposal and rank,
/// each with whether its triage is reduced (F19, REQ-011-17): under `flow.flow.read` or
/// `flow.flow.review` on the entity of the request, in full; as its requester, reduced
/// (REQ-010-25).
async fn reviews_seen(
    api: &Api,
    scope: &ReadScope,
    principal: &Principal,
    at: OffsetDateTime,
    rows: &[FlowView],
) -> Result<HashMap<(Uuid, i32), bool>, Problem> {
    let reviews: Vec<&Review> = rows.iter().filter_map(|r| r.review.as_ref()).collect();
    if reviews.is_empty() {
        return Ok(HashMap::new());
    }
    let entities: Vec<Uuid> = reviews.iter().filter_map(|r| r.entity).collect();
    let readable = readable_entities(api, scope, at, &entities).await?;
    let reviewer = api
        .authorizer()
        .visibility(&principal.subject(), REVIEW, at)
        .await?;
    Ok(reviews
        .into_iter()
        .filter_map(|r| {
            let full = r.entity.is_some_and(|e| {
                readable.contains(&e) || reviewer.global || reviewer.entities.contains(&e)
            });
            (full || r.requester == principal.identity).then_some(((r.changeset, r.ordinal), !full))
        })
        .collect())
}

/// `GET`/`QUERY /v1/flows`: by identifier, filtered (REQ-011-14), those the caller reads.
async fn list(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let names: Vec<&str> = FILTERS.iter().map(|(n, _)| *n).collect();
    let pool = api.pool.clone();
    page_of(
        &api,
        &principal,
        &context,
        &headers,
        &query,
        COLLECTION,
        "/v1/flows",
        |dates, after, limit, page| async move {
            let filter = filter_of(&wanted(&page, &names)?)?;
            let mut conn = pool.acquire().await.map_err(|e| kit::store(&e))?;
            let rows = read::flows(&mut conn, dates, &filter, after, limit).await?;
            Ok((rows, page))
        },
    )
    .await
}

/// The flows of the object `id` of `kind` (REQ-011-15), those the caller reads.
#[allow(clippy::too_many_arguments)] // the request, then the object read
async fn of_object(
    api: Api,
    principal: Principal,
    context: RequestContext,
    query: Option<String>,
    headers: HeaderMap,
    kind: EndpointKind,
    collection: &'static str,
    id: Uuid,
) -> Result<Response, Problem> {
    let pool = api.pool.clone();
    let path = format!("/v1/{collection}/{id}/flows");
    let name = format!("{collection}-flows");
    page_of(
        &api,
        &principal,
        &context,
        &headers,
        query.as_deref().unwrap_or_default(),
        &name,
        &path,
        |dates, after, limit, page| async move {
            wanted(&page, &[])?;
            let after = match after {
                None => None,
                Some(Cursor::Flow(id)) => Some(id),
                Some(Cursor::Pending { .. }) => {
                    return Err(Problem::new(ProblemKind::InvalidCursor));
                }
            };
            let mut conn = pool.acquire().await.map_err(|e| kit::store(&e))?;
            let rows = read::of_object(&mut conn, dates, kind, id, after, limit).await?;
            Ok((rows, page))
        },
    )
    .await
}

/// Whether the caller reads the flow `id`: `Some(false)` under `flow.flow.read`,
/// `Some(true)` as its requester only (explanation reduced), `None` otherwise.
async fn access(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    id: Uuid,
) -> Result<Option<bool>, Problem> {
    match kit::gate(api, context, flow_type(), id).await {
        Ok(()) => Ok(Some(false)),
        Err(problem) if problem.kind() == ProblemKind::NotFound => {
            let own: Option<bool> = sqlx::query_scalar(
                "SELECT true FROM flow.flow WHERE id = $1 AND requester_id = $2 LIMIT 1",
            )
            .bind(id)
            .bind(principal.identity)
            .fetch_optional(&api.pool)
            .await
            .map_err(|e| kit::store(&e))?;
            Ok(own.map(|_| true))
        }
        Err(problem) => Err(problem),
    }
}

/// The flow `id` at `moment`, as listed.
async fn flow_at(api: &Api, id: Uuid, moment: &Moment) -> Result<FlowView, Problem> {
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    // The list ordered by identifier from the one before `id`: its first row is `id` when
    // it lives at the date.
    let before = predecessor(id);
    let rows = read::flows(
        &mut conn,
        dates(moment),
        &FlowFilter::default(),
        before.map(Cursor::Flow),
        1,
    )
    .await?;
    rows.into_iter()
        .find(|r| r.id == Some(id))
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))
}

/// The identifier just before `id` in the order of the list, `None` for the smallest.
fn predecessor(id: Uuid) -> Option<Uuid> {
    let value = id.as_u128();
    value.checked_sub(1).map(Uuid::from_u128)
}

/// `GET /v1/flows/{id}`.
async fn show(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates_given: DateParams,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    access(&api, &principal, &context, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let now = kit::now(&api).await?;
    let moment = dates_given.at(now, api.bound)?;
    let view = flow_at(&api, id, &moment).await?;
    let version = kit::version(&api, id, &moment)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    kit::record_read(&api, &context, FLOW, &[id], json!({"id": id})).await?;
    Ok(respond(
        &headers,
        &moment,
        &ETag::version(version),
        Json(flow_dto(view, false)),
    ))
}

/// `GET /v1/flows/{id}/evaluations`: the evaluations valid at the date, as asserted then,
/// never recomputed (REQ-011-16, 23); reduced for the requester (REQ-010-25).
async fn evaluations(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates_given: DateParams,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let reduced = access(&api, &principal, &context, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let now = kit::now(&api).await?;
    let moment = dates_given.at(now, api.bound)?;
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let rows = read::evaluations(&mut conn, dates(&moment), id).await?;
    kit::record_read(
        &api,
        &context,
        FLOW,
        &[id],
        json!({"id": id, "evaluations": true}),
    )
    .await?;
    let items: Vec<EvaluationDto> = rows
        .into_iter()
        .map(|r| evaluation_dto(r, reduced))
        .collect();
    Ok(respond_page(&headers, &moment, &items, None))
}

/// `GET /v1/flow-nonconformities` (REQ-011-25): newest first, filtered; a caller reads the
/// rows of the entities where it holds `flow.flow.read`, and those of its own requests.
async fn nonconformities(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let page = Page::from_query(
        query.as_deref().unwrap_or_default(),
        &api.ring,
        NONCONFORMITIES,
    )?;
    let wanted = wanted(&page, NONCONFORMITY_FILTERS)?;
    let mut filter = NonconformityFilter::default();
    for (&name, &value) in &wanted {
        match name {
            "entity" => filter.entity = Some(uuid(name, value)?),
            "requester" => filter.requester = Some(uuid(name, value)?),
            "rule" => filter.rule = Some(uuid(name, value)?),
            "from" => filter.from = Some(instant(name, value)?),
            "to" => filter.to = Some(instant(name, value)?),
            "state" => match value {
                "rejected" | "noncompliant" | "waived" => filter.state = Some(value.to_owned()),
                other => {
                    return Err(kit::bad(format!(
                        "{other} is not a state of non-conformity"
                    )));
                }
            },
            "kind" => match value {
                "requested" | "observed" => filter.kind = Some(value.to_owned()),
                other => return Err(kit::bad(format!("{other} is not a kind of flow"))),
            },
            _ => unreachable!("the names are checked by `wanted`"),
        }
    }
    let (moment, scope) = reading(&api, &principal, &page).await?;
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let rows =
        read::nonconformities(&mut conn, dates(&moment), &filter, i64::from(page.size)).await?;
    let entities: Vec<Uuid> = rows.iter().map(|r| r.entity).collect();
    let readable = readable_entities(&api, &scope, moment.valid_at, &entities).await?;
    let items: Vec<NonconformityDto> = rows
        .into_iter()
        .filter(|r| r.requester == principal.identity || readable.contains(&r.entity))
        .map(nonconformity_dto)
        .collect();
    let flows: Vec<Uuid> = items.iter().filter_map(|n| n.flow).collect();
    kit::record_read(
        &api,
        &context,
        FLOW,
        &flows,
        json!({
            "collection": NONCONFORMITIES,
            "filters": page.filters,
            "valid_at": rfc3339(moment.valid_at),
            "as_of": rfc3339(moment.as_of()),
        }),
    )
    .await?;
    Ok(respond_page(&headers, &moment, &items, None))
}

/// The entities of `ids` where the scope of `flow.flow.read` reaches: globally, or the
/// entity granted (a flow is anchored on its entity, ADR 0026).
async fn readable_entities(
    api: &Api,
    scope: &ReadScope,
    _at: OffsetDateTime,
    ids: &[Uuid],
) -> Result<HashSet<Uuid>, Problem> {
    let Some(json) = scope.to_json() else {
        return Ok(ids.iter().copied().collect());
    };
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($1))
         SELECT x.id FROM unnest($2::uuid[]) AS x(id)
          WHERE EXISTS (SELECT 1 FROM s
                         WHERE s.kind = $3 AND (s.global OR x.id = ANY (s.entities)))",
    )
    .bind(json)
    .bind(ids)
    .bind(FLOW)
    .fetch_all(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// `POST /v1/flows`: a request, triaged (REQ-011-21). Applied (201), the answer carries
/// the flow and its evaluations; held for a review (202), the changeset.
async fn request(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let response = kit::create(
        &api,
        &principal,
        &context,
        &headers,
        &raw,
        flow_type(),
        "flow.request",
        REQUEST_MEMBERS,
    )
    .await?;
    if response.status() != StatusCode::CREATED {
        return Ok(response);
    }
    let Some(id) = response
        .headers()
        .get(header::LOCATION)
        .and_then(|l| l.to_str().ok())
        .and_then(|l| l.rsplit('/').next())
        .and_then(|l| l.parse::<Uuid>().ok())
    else {
        return Ok(response);
    };
    let (mut parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| Problem::new(ProblemKind::Internal).detail(e.to_string()))?;
    let mut answer: Map<String, Value> = serde_json::from_slice(&bytes).unwrap_or_default();
    let now = kit::now(&api).await?;
    let moment = DateParams::default().at(now, api.bound)?;
    if let Ok(view) = flow_at(&api, id, &moment).await {
        answer.insert("flow".into(), json!(flow_dto(view, false)));
    }
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let evaluations: Vec<EvaluationDto> = read::evaluations(&mut conn, dates(&moment), id)
        .await?
        .into_iter()
        .map(|r| evaluation_dto(r, false))
        .collect();
    answer.insert("evaluations".into(), json!(evaluations));
    parts.headers.remove(header::CONTENT_LENGTH);
    let body = serde_json::to_vec(&answer)
        .map_err(|e| Problem::new(ProblemKind::Internal).detail(e.to_string()))?;
    Ok(Response::from_parts(parts, Body::from(body)))
}

fn schemas() -> Vec<(String, RefOr<Schema>)> {
    let mut schemas = Vec::new();
    schemas_of::<FlowDto>(&mut schemas);
    schemas_of::<ReviewDto>(&mut schemas);
    schemas_of::<TriageDto>(&mut schemas);
    schemas_of::<EvaluationDto>(&mut schemas);
    schemas_of::<NonconformityDto>(&mut schemas);
    schemas_of::<FlowRequestDto>(&mut schemas);
    schemas_of::<FlowPatchDto>(&mut schemas);
    schemas_of::<FlowWaiverDto>(&mut schemas);
    schemas.push(listing_of::<FlowDto>());
    schemas.push(listing_of::<EvaluationDto>());
    schemas.push(listing_of::<NonconformityDto>());
    schemas
}

/// The routes of the flows of an object of `collection`.
fn object_routes(
    router: ModuleRouter,
    collection: &'static str,
    kind: EndpointKind,
    one: &str,
) -> ModuleRouter {
    let list_op = op(
        &format!("{one}_flows"),
        "flow",
        &format!("The flows of the {one}, those the caller may read (REQ-011-15)."),
    )
    .path("id", &format!("The {one}."), true)
    .params::<DatesParams>()
    .params::<PageParams>()
    .answers("200", "A page of flows.", "Listing_Flow")
    .build();
    router.routes(documented(
        &format!("/{collection}/{{id}}/flows"),
        vec![(HttpMethod::Get, list_op)],
        Vec::new(),
        get(
            move |State(api): State<Api>,
                  Extension(principal): Extension<Principal>,
                  Extension(context): Extension<RequestContext>,
                  Path(id): Path<Uuid>,
                  RawQuery(query): RawQuery,
                  headers: HeaderMap| async move {
                of_object(
                    api, principal, context, query, headers, kind, collection, id,
                )
                .await
            },
        ),
    ))
}

/// The routes of the module, mounted under `/v1` (ADR 0026, clause 9).
#[must_use]
#[allow(clippy::too_many_lines)] // the contract of each route beside its handler
pub fn routes() -> ModuleRouter {
    let mut list_op = op(
        "list_flows",
        "flow",
        "The flows the caller may read (REQ-011-14).",
    );
    for (filter, description) in FILTERS {
        list_op = list_op.query(filter, description);
    }
    let list_op = list_op
        .params::<DatesParams>()
        .params::<PageParams>()
        .answers("200", "A page of flows.", "Listing_Flow")
        .build();
    let request_op = op(
        "request_flow",
        "flow",
        "Requests a flow, triaged against the policy (REQ-011-21); once applied, the answer \
         carries the flow and its evaluations (members `flow`, `evaluations`).",
    )
    .header(
        "Idempotency-Key",
        false,
        "Makes the write idempotent for 24 h.",
    )
    .body("application/json", "FlowRequest", true)
    .answers("201", "Applied.", "ChangesetApplied")
    .answers("202", "Held for a review.", "ChangesetPending")
    .build();
    let mut router = ModuleRouter::new().routes(documented(
        "/flows",
        vec![(HttpMethod::Get, list_op), (HttpMethod::Post, request_op)],
        schemas(),
        kit::collection(list).post(request),
    ));
    router = queryable(router, "/flows", "query_flows");
    let the = "The flow.";
    let show_op = op("show_flow", "flow", the)
        .path("id", the, true)
        .params::<DatesParams>()
        .answers("200", "The flow; its version is its ETag.", "Flow")
        .build();
    let modify_op = op(
        "modify_flow",
        "flow",
        "Modifies the flow (JSON Merge Patch); its identity does not change (REQ-011-4).",
    )
    .path("id", the, true)
    .header("If-Match", true, "The version read, as its ETag.")
    .body(MERGE_PATCH, "FlowPatch", true)
    .answers("200", "Modified.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let close_op = op("close_flow", "flow", "Closes the flow.")
        .path("id", the, true)
        .header("If-Match", true, "The version read, as its ETag.")
        .answers("200", "Closed.", "ChangesetApplied")
        .answers("202", "Held for a decision.", "ChangesetPending")
        .build();
    router = router.routes(documented(
        "/flows/{id}",
        vec![
            (HttpMethod::Get, show_op),
            (HttpMethod::Patch, modify_op),
            (HttpMethod::Delete, close_op),
        ],
        Vec::new(),
        get(show)
            .patch(
                |State(api): State<Api>,
                 Extension(principal): Extension<Principal>,
                 Extension(context): Extension<RequestContext>,
                 Path(id): Path<Uuid>,
                 headers: HeaderMap,
                 raw: Bytes| async move {
                    kit::modify(
                        &api,
                        &principal,
                        &context,
                        id,
                        &headers,
                        &raw,
                        flow_type(),
                        "flow.update",
                        UPDATE_MEMBERS,
                    )
                    .await
                },
            )
            .delete(
                |State(api): State<Api>,
                 Extension(principal): Extension<Principal>,
                 Extension(context): Extension<RequestContext>,
                 Path(id): Path<Uuid>,
                 headers: HeaderMap,
                 raw: Bytes| async move {
                    kit::close(&api, &principal, &context, id, &headers, &raw, flow_type()).await
                },
            ),
    ));
    let evaluations_op = op(
        "flow_evaluations",
        "flow",
        "The evaluations of the flow by pair of addresses at the date, as stored \
         (REQ-011-16); reduced for its requester (REQ-010-25).",
    )
    .path("id", the, true)
    .params::<DatesParams>()
    .answers("200", "The evaluations.", "Listing_FlowEvaluation")
    .build();
    let waiver_op = op(
        "waive_flow",
        "flow",
        "Grants a dated waiver to the flow (REQ-011-9), under `flow.flow.waive`.",
    )
    .path("id", the, true)
    .header("If-Match", true, "The version read, as its ETag.")
    .body("application/json", "FlowWaiver", true)
    .answers("200", "Granted.", "ChangesetApplied")
    .answers("202", "Held for a decision.", "ChangesetPending")
    .build();
    let nonconformities_op = op(
        "list_flow_nonconformities",
        "flow",
        "The non-conformities the caller may read, newest first (REQ-011-25).",
    )
    .query("entity", "The entity.")
    .query("requester", "The requester.")
    .query("rule", "The rule in cause.")
    .query("from", "From this instant.")
    .query("to", "Before this instant.")
    .query("state", "`rejected`, `noncompliant` or `waived`.")
    .query("kind", "`requested` or `observed`.")
    .params::<DatesParams>()
    .params::<PageParams>()
    .answers("200", "The non-conformities.", "Listing_FlowNonconformity")
    .build();
    router = router
        .routes(documented(
            "/flows/{id}/evaluations",
            vec![(HttpMethod::Get, evaluations_op)],
            Vec::new(),
            get(evaluations),
        ))
        .routes(documented(
            "/flows/{id}/waiver",
            vec![(HttpMethod::Post, waiver_op)],
            Vec::new(),
            post(
                |State(api): State<Api>,
                 Extension(principal): Extension<Principal>,
                 Extension(context): Extension<RequestContext>,
                 Path(id): Path<Uuid>,
                 headers: HeaderMap,
                 raw: Bytes| async move {
                    let mut fixed = Map::new();
                    fixed.insert("flow".into(), json!({ "id": id }));
                    kit::act(
                        &api,
                        &principal,
                        &context,
                        id,
                        &headers,
                        &raw,
                        flow_type(),
                        ("flow.waive", WAIVER_MEMBERS, fixed),
                        StatusCode::OK,
                    )
                    .await
                },
            ),
        ))
        .routes(documented(
            "/flow-nonconformities",
            vec![(HttpMethod::Get, nonconformities_op)],
            Vec::new(),
            get(nonconformities),
        ));
    router = object_routes(router, "addresses", EndpointKind::Address, "address");
    router = object_routes(router, "prefixes", EndpointKind::Prefix, "prefix");
    router = object_routes(router, "interfaces", EndpointKind::Interface, "interface");
    object_routes(router, "machines", EndpointKind::Machine, "machine")
}
