//! The routes of the module under `/v1` (feature `rest`; spec 011, REQ-011-14 to 18, 25;
//! design « API »), mounted by kartenn-server: the flows as a collection, dated, paged and
//! filtered; a request by `POST /v1/flows`, triaged (REQ-011-21), answered with the flow
//! and its evaluations; the evaluations of a flow at (`valid_at`, `as_of`); the
//! non-conformities; the flows of an address, a prefix, an interface or a machine; the
//! test of a connection by `GET` and `QUERY /v1/flow-checks` (REQ-011-26): the query of
//! compliance of the 010 and, for each of its evaluations, the flows that open it.
//!
//! Rights (REQ-011-17): a flow is read under `flow.read` on its entity, by the
//! predicate of the core (ADR 0021); its requester reads it too; so does a holder of
//! `flow.read` on any perimeter who reads one of its endpoints (spec 027, REQ-027-1). A
//! flow is served as is to every reader, whatever the way it is read (REQ-027-2): its
//! endpoints by kind, identifier and value, its evaluations in full, and no datum of an
//! object of the registry (name, property), which its own routes serve under its own read.
//! A flow the caller may not read answers as an absent one.

use std::collections::{BTreeMap, HashMap, HashSet};

use axum::Extension;
use axum::body::{Body, Bytes};
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use kartenn_rest::ModuleRouter;
use kartenn_rest::kit::{
    self, Api, Dates as DateParams, DatesParams, ListQuery, MERGE_PATCH, Moment, Page, PageParams,
    Problem, ProblemKind, documented, listing_of, next_href, op, queryable, respond_page,
    schemas_of,
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
        "An endpoint, exact: `<kind>:<id>` (address, prefix, interface, machine, range); \
         when the caller does not read the object, the flows it reads through one of their \
         endpoints only are left out (REQ-027-3). Or a value, \
         an address or `<address>/<length>`: the flows with an endpoint address or prefix \
         within it, whatever the caller reads of the objects (a value is a datum of the \
         flow).",
    ),
    (
        "prefix",
        "A prefix: the flows with an endpoint address in its subtree; as `endpoint` for a \
         prefix the caller does not read.",
    ),
    (
        "machine",
        "A machine: the flows with an endpoint address of the machine; as `endpoint` for a \
         machine the caller does not read.",
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
    /// The source with its value at the date (REQ-027-2), the same for every reader.
    /// Absent only for a request pending a review that names an endpoint not yet created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<EndpointDto>,
    /// The destination, as `source`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<EndpointDto>,
}

/// An endpoint of a flow, a datum of the flow served to every reader (spec 027,
/// REQ-027-2): its kind, its identifier and its value at the date; never a name, a
/// property nor a value of an axis of the object, which its own route serves under its
/// own read permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = FlowEndpoint)]
pub struct EndpointDto {
    /// `address`, `prefix`, `interface`, `machine` or `range`.
    pub kind: String,
    /// The object.
    pub id: Uuid,
    /// For an address, a prefix or a range: its value at the date; absent when the object
    /// does not live at the date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// For an interface or a machine: the addresses it holds at the date, sorted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
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

/// The triage of a request (REQ-011-21), as its operation keeps it, the same for every
/// reader (REQ-027-2).
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
#[schema(as = FlowTriage)]
pub struct TriageDto {
    /// `allowed`, `partial` or `denied`; `null` when an endpoint holds no address.
    #[serde(default)]
    pub summary: Option<String>,
    /// `automatic`, `review` or `noncompliant`.
    #[serde(default)]
    pub outcome: Option<String>,
    /// The decisive element of the worst pair, if any.
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
/// recomputed, the same for every reader of the flow (REQ-027-2).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(as = FlowEvaluation)]
pub struct EvaluationDto {
    /// The source address.
    pub source: Uuid,
    /// The destination address.
    pub destination: Uuid,
    /// `allowed`, `conditional` or `denied`.
    pub verdict: String,
    /// `automatic`, `review` or `noncompliant`.
    pub outcome: String,
    /// The reason.
    pub reason: String,
    /// The decisive rule or pattern, if any.
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
    /// Its key; its labels are served by `GET /v1/labels` (space `policy.rule`).
    pub rule_key: Option<String>,
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
    /// The source: `{ "kind": "address" | "prefix" | "interface", "id": { "id": … } }`; a
    /// range is refused (`flow.endpoint.invalid`), a machine too (`flow.endpoint.machine`).
    #[schema(value_type = Object)]
    source: Value,
    /// The destination, as the source.
    #[schema(value_type = Object)]
    destination: Value,
    /// The service: `{ "id": … }`.
    #[schema(value_type = Object)]
    service: Value,
    /// The external references, at least one: `{ "system": …, "id": …, "label": … }`, the
    /// system by the key of its value of `core.external_system` (or by its identifier, as a
    /// flow is read with); a system that names none is refused (`flow.reference.system`).
    #[schema(value_type = Vec<Object>)]
    external_refs: Vec<Value>,
    /// The justification.
    justification: Option<String>,
    /// The requester the request is made for: `{ "text": … }` or `{ "external": … }`, an
    /// external reference as in `external_refs`.
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
    /// The external references, as for a request.
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
    /// An earlier end, applied; or a later one, or `null` on a dated flow: an extension,
    /// triaged on the portion it adds and held for a review unless automatic (F20).
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

/// A row as served, the same for every reader (REQ-027-2): its endpoints with their
/// values, taken from `values`.
fn flow_dto(view: FlowView, values: &HashMap<Endpoint, EndpointDto>) -> FlowDto {
    let [source, destination] = endpoints_of(&view.flow).map(|side| values.get(&side?).cloned());
    let review = view.review.map(|r| ReviewDto {
        changeset: r.changeset,
        kind: r.kind.to_owned(),
        triage: r
            .triage
            .map(|t| serde_json::from_value(t).unwrap_or_default()),
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
        source,
        destination,
    }
}

/// The rows `views` as served at `at`: the values of their endpoints read in one query
/// for the set.
async fn served(
    api: &Api,
    at: OffsetDateTime,
    views: Vec<FlowView>,
) -> Result<Vec<FlowDto>, Problem> {
    let endpoints: Vec<Endpoint> = views
        .iter()
        .flat_map(|v| endpoints_of(&v.flow).into_iter().flatten())
        .collect();
    let values = endpoint_values(api, at, &endpoints).await?;
    Ok(views.into_iter().map(|v| flow_dto(v, &values)).collect())
}

/// The two sides of a flow, in the order of [`SIDES`].
type Sides<T> = [Option<T>; 2];

const SIDES: [&str; 2] = ["source", "destination"];

/// An endpoint as a flow stores it: the name of its kind and its identifier.
type Endpoint = (String, Uuid);

/// The endpoints of a flow as listed (`fields`), source then destination.
fn endpoints_of(flow: &Value) -> Sides<Endpoint> {
    SIDES.map(|side| {
        let kind = flow.get(format!("{side}_kind"))?.as_str()?;
        let id = flow.get(format!("{side}_id"))?.as_str()?.parse().ok()?;
        Some((kind.to_owned(), id))
    })
}

fn evaluation_dto(view: EvaluationView) -> EvaluationDto {
    EvaluationDto {
        source: view.source,
        destination: view.destination,
        verdict: view.verdict,
        outcome: view.outcome,
        reason: view.reason,
        decisive: view.decisive,
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

/// An address or `<address>/<length>` as `<address>/<length>`; `None` when it is not one.
fn network(text: &str) -> Option<String> {
    let (address, length) = match text.split_once('/') {
        Some((address, length)) => (address, Some(length)),
        None => (text, None),
    };
    let address: std::net::IpAddr = address.parse().ok()?;
    let width = if address.is_ipv4() { 32 } else { 128 };
    let length = match length {
        Some(length) => length.parse::<u8>().ok().filter(|l| *l <= width)?,
        None => width,
    };
    Some(format!("{address}/{length}"))
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
                // `<kind>:<id>`, else a value (an IPv6 address holds `:` too).
                match value
                    .split_once(':')
                    .and_then(|(kind, id)| Some((endpoint_kind(kind)?, id)))
                {
                    Some((kind, id)) => filter.endpoint = Some((kind, uuid(name, id)?)),
                    None => {
                        filter.within_value = Some(network(value).ok_or_else(|| {
                            kit::bad(
                                "endpoint is `<kind>:<id>`, an address or `<address>/<length>`",
                            )
                        })?);
                    }
                }
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

/// The flows of `ids` the caller reads at `at`: under `flow.read` by the predicate
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

/// The condition that the endpoint `x` (`x.kind`, `x.id`) is readable at `$1` under the
/// scope `s`, by the read permission of its kind (REQ-027-1): the predicate of the core
/// for an address, a prefix or a range, the type of the module Machines for an interface
/// or a machine.
fn endpoint_readable() -> String {
    let core = |table: &str, kind: &str, value: &str| {
        format!(
            "coalesce((SELECT {} FROM {table} o
                        WHERE o.id = x.id AND o.valid_period @> $1::timestamptz LIMIT 1), false)",
            readable("o.id", &format!("'{kind}'"), "o.domain_id", value, "$1")
        )
    };
    let module = |kind: &str| {
        readable(
            "x.id",
            &format!("'{kind}'"),
            "NULL::uuid",
            "NULL::inet",
            "$1",
        )
    };
    format!(
        "CASE x.kind WHEN 'address' THEN {} WHEN 'prefix' THEN {} WHEN 'range' THEN {}
                     WHEN 'interface' THEN {} WHEN 'machine' THEN {} ELSE false END",
        core("public.address", "address", "o.address"),
        core("public.prefix", "prefix", "o.prefix"),
        core("public.ip_range", "range", "lower(o.range)"),
        module(kartenn_machines::module::INTERFACE),
        module(kartenn_machines::module::MACHINE),
    )
}

/// The value of the endpoint `x` at `$1` (REQ-027-2): its address, prefix or range; the
/// addresses an interface or a machine holds.
const ENDPOINT_VALUES: &str = "CASE x.kind
    WHEN 'address' THEN ARRAY(SELECT host(o.address) FROM public.address o
                               WHERE o.id = x.id AND o.valid_period @> $1::timestamptz LIMIT 1)
    WHEN 'prefix' THEN ARRAY(SELECT o.prefix::text FROM public.prefix o
                              WHERE o.id = x.id AND o.valid_period @> $1::timestamptz LIMIT 1)
    WHEN 'range' THEN ARRAY(SELECT o.range::text FROM public.ip_range o
                             WHERE o.id = x.id AND o.valid_period @> $1::timestamptz LIMIT 1)
    WHEN 'interface' THEN ARRAY(
        SELECT host(a.address) FROM machines.attachment t
          JOIN public.address a ON a.id = t.address_id AND a.valid_period @> $1::timestamptz
         WHERE t.interface_id = x.id AND t.valid_period @> $1::timestamptz
         ORDER BY a.address)
    WHEN 'machine' THEN ARRAY(
        SELECT host(d.address) FROM (
            SELECT DISTINCT a.address FROM machines.interface i
              JOIN machines.attachment t ON t.interface_id = i.id
               AND t.valid_period @> $1::timestamptz
              JOIN public.address a ON a.id = t.address_id AND a.valid_period @> $1::timestamptz
             WHERE i.machine_id = x.id AND i.valid_period @> $1::timestamptz) d
         ORDER BY d.address)
    END";

/// The distinct endpoints of `endpoints`, as the arrays of their kinds and identifiers.
fn unnested(endpoints: &[Endpoint]) -> (Vec<&str>, Vec<Uuid>) {
    let mut unique: Vec<&Endpoint> = endpoints.iter().collect();
    unique.sort_unstable();
    unique.dedup();
    unique.into_iter().map(|e| (e.0.as_str(), e.1)).unzip()
}

/// The endpoints among `endpoints` the caller reads at `at` by the read permission of
/// their kind (REQ-027-1); this decides only whether a flow is readable, never what it
/// shows (REQ-027-2). One query for the whole set, never one per row.
async fn endpoints_read(
    api: &Api,
    scope: &ReadScope,
    at: OffsetDateTime,
    endpoints: &[Endpoint],
) -> Result<HashSet<Endpoint>, Problem> {
    let Some(json) = scope.to_json() else {
        return Ok(endpoints.iter().cloned().collect());
    };
    if endpoints.is_empty() {
        return Ok(HashSet::new());
    }
    let (kinds, ids) = unnested(endpoints);
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($2))
         SELECT x.kind, x.id FROM unnest($3::text[], $4::uuid[]) AS x(kind, id)
          WHERE {}",
        endpoint_readable()
    );
    let rows: Vec<Endpoint> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(at)
        .bind(json)
        .bind(&kinds)
        .bind(&ids)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    Ok(rows.into_iter().collect())
}

/// The values of `endpoints` at `at` (REQ-027-2), data of the flows served to every
/// reader whatever it reads of the objects. One query for the whole set.
async fn endpoint_values(
    api: &Api,
    at: OffsetDateTime,
    endpoints: &[Endpoint],
) -> Result<HashMap<Endpoint, EndpointDto>, Problem> {
    if endpoints.is_empty() {
        return Ok(HashMap::new());
    }
    let (kinds, ids) = unnested(endpoints);
    let sql = format!(
        "SELECT x.kind, x.id, {ENDPOINT_VALUES}
           FROM unnest($2::text[], $3::uuid[]) AS x(kind, id)"
    );
    let rows: Vec<(String, Uuid, Option<Vec<String>>)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(at)
        .bind(&kinds)
        .bind(&ids)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(kind, id, values)| {
            let values = values.unwrap_or_default();
            let several = matches!(kind.as_str(), "interface" | "machine");
            let dto = EndpointDto {
                kind: kind.clone(),
                id,
                value: if several {
                    None
                } else {
                    values.first().cloned()
                },
                values: several.then_some(values),
            };
            ((kind, id), dto)
        })
        .collect())
}

/// The flows among `flows` (identifier, endpoints) the caller reads at `at`: under
/// `flow.read` on their entity or as their requester (REQ-011-17), or, holding
/// `flow.read` elsewhere, through one of their endpoints (REQ-027-1). When the caller does
/// not read every object of `objects`, those a filter names, a flow read through its
/// endpoints only is left out (REQ-027-3).
async fn flows_seen(
    api: &Api,
    scope: &ReadScope,
    principal: &Principal,
    at: OffsetDateTime,
    flows: &[(Uuid, Sides<Endpoint>)],
    objects: &[Endpoint],
) -> Result<HashSet<Uuid>, Problem> {
    let ids: Vec<Uuid> = flows.iter().map(|f| f.0).collect();
    let mut seen = visible(api, scope, principal, at, &ids).await?;
    let rest: Vec<&(Uuid, Sides<Endpoint>)> =
        flows.iter().filter(|f| !seen.contains(&f.0)).collect();
    if rest.is_empty() || !scope.reads(FLOW) {
        return Ok(seen);
    }
    let asked: Vec<Endpoint> = rest
        .iter()
        .flat_map(|f| f.1.iter().flatten().cloned())
        .chain(objects.iter().cloned())
        .collect();
    let read = endpoints_read(api, scope, at, &asked).await?;
    if !objects.iter().all(|o| read.contains(o)) {
        return Ok(seen);
    }
    seen.extend(
        rest.into_iter()
            .filter(|f| f.1.iter().flatten().any(|e| read.contains(e)))
            .map(|f| f.0),
    );
    Ok(seen)
}

/// The flows among `ids` the caller reads at `at` through one of their endpoints, holding
/// `flow.read` elsewhere (REQ-027-1): one query for the set.
async fn through_endpoints(
    api: &Api,
    scope: &ReadScope,
    at: OffsetDateTime,
    ids: &[Uuid],
) -> Result<HashSet<Uuid>, Problem> {
    if ids.is_empty() || !scope.reads(FLOW) {
        return Ok(HashSet::new());
    }
    let Some(json) = scope.to_json() else {
        return Ok(ids.iter().copied().collect());
    };
    let sql = format!(
        "WITH s AS MATERIALIZED (SELECT * FROM kartenn_scopes($2))
         SELECT DISTINCT f.id FROM flow.flow f
          CROSS JOIN LATERAL (VALUES (f.source_kind, f.source_id),
                                     (f.destination_kind, f.destination_id)) AS x(kind, id)
          WHERE f.id = ANY($3) AND {}",
        endpoint_readable()
    );
    let rows: Vec<(Uuid,)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(at)
        .bind(json)
        .bind(ids)
        .fetch_all(&api.pool)
        .await
        .map_err(|e| kit::store(&e))?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// The moment and the read scope of a request of `principal` (the scope of
/// `flow.read`).
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
    Fut: std::future::Future<Output = Result<(Vec<FlowView>, Page, Vec<Endpoint>), Problem>>,
{
    let page = Page::from_query(query, &api.ring, collection)?;
    let after = page.after().map(cursor_of).transpose()?;
    let (moment, scope) = reading(api, principal, &page).await?;
    let size = usize::from(page.size);
    let (mut rows, page, objects) =
        fetch(dates(&moment), after, i64::from(page.size) + 1, page).await?;
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
    let flows: Vec<(Uuid, Sides<Endpoint>)> = rows
        .iter()
        .filter(|r| r.review.is_none())
        .filter_map(|r| Some((r.id?, endpoints_of(&r.flow))))
        .collect();
    let seen = flows_seen(api, &scope, principal, moment.valid_at, &flows, &objects).await?;
    let reviews = reviews_seen(api, &scope, principal, moment.valid_at, &rows).await?;
    // A flow under review shows the state proposed, not its own version: only the flows
    // in force carry `version` (006/H19).
    let mut kept: Vec<(FlowView, bool)> = Vec::with_capacity(rows.len());
    let (mut ids, mut changesets) = (Vec::new(), Vec::new());
    for row in rows {
        match (row.id, &row.review) {
            (id, Some(review)) => {
                if reviews.contains(&(review.changeset, review.ordinal)) {
                    changesets.push(review.changeset);
                    ids.extend(id);
                    kept.push((row, false));
                }
            }
            (Some(id), None) if seen.contains(&id) => {
                ids.push(id);
                kept.push((row, true));
            }
            _ => {}
        }
    }
    let (views, in_force): (Vec<FlowView>, Vec<bool>) = kept.into_iter().unzip();
    let items: Vec<(FlowDto, bool)> = served(api, moment.valid_at, views)
        .await?
        .into_iter()
        .zip(in_force)
        .collect();
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
    let in_force: Vec<&FlowDto> = items.iter().filter(|i| i.1).map(|i| &i.0).collect();
    let mut versioned = kit::versioned(api, &moment, &in_force).await?.into_iter();
    let items: Vec<Value> = items
        .iter()
        .map(|(flow, in_force)| {
            if *in_force {
                versioned.next().expect("one item per flow in force")
            } else {
                serde_json::to_value(flow).expect("JSON values serialise")
            }
        })
        .collect();
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

/// The requests pending a review among `rows` the caller reads, by proposal and rank (F19,
/// REQ-011-17): under `flow.read` or `flow.review` on the entity of the request, or as its
/// requester; each served whole (REQ-027-2).
async fn reviews_seen(
    api: &Api,
    scope: &ReadScope,
    principal: &Principal,
    at: OffsetDateTime,
    rows: &[FlowView],
) -> Result<HashSet<(Uuid, i32)>, Problem> {
    let reviews: Vec<&Review> = rows.iter().filter_map(|r| r.review.as_ref()).collect();
    if reviews.is_empty() {
        return Ok(HashSet::new());
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
            (full || r.requester == principal.identity).then_some((r.changeset, r.ordinal))
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
            let objects: Vec<Endpoint> = filter
                .endpoint
                .map(|(kind, id)| (crate::pg::kind_name(kind).to_owned(), id))
                .into_iter()
                .chain(filter.within_prefix.map(|id| ("prefix".to_owned(), id)))
                .chain(filter.machine.map(|id| ("machine".to_owned(), id)))
                .collect();
            let mut conn = pool.acquire().await.map_err(|e| kit::store(&e))?;
            let rows = read::flows(&mut conn, dates, &filter, after, limit).await?;
            Ok((rows, page, objects))
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
            Ok((
                rows,
                page,
                vec![(crate::pg::kind_name(kind).to_owned(), id)],
            ))
        },
    )
    .await
}

/// Whether the caller reads the flow `id` at `at` (REQ-011-17, REQ-027-1): under
/// `flow.read` on its entity, as its requester, or through one of its endpoints. The way
/// it is read changes nothing of what is served (REQ-027-2).
async fn access(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    id: Uuid,
    at: OffsetDateTime,
) -> Result<bool, Problem> {
    type Row = (bool, String, Uuid, String, Uuid);
    match kit::gate(api, context, flow_type(), id).await {
        Ok(()) => return Ok(true),
        Err(problem) if problem.kind() == ProblemKind::NotFound => {}
        Err(problem) => return Err(problem),
    }
    let row: Option<Row> = sqlx::query_as(
        "SELECT requester_id = $2, source_kind, source_id, destination_kind, destination_id
           FROM flow.flow WHERE id = $1 ORDER BY requester_id = $2 DESC LIMIT 1",
    )
    .bind(id)
    .bind(principal.identity)
    .fetch_optional(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    let Some((own, source_kind, source, destination_kind, destination)) = row else {
        return Ok(false);
    };
    if own {
        return Ok(true);
    }
    let scope = kit::scope(api, principal, kit::now(api).await?).await?;
    if !scope.reads(FLOW) {
        return Ok(false);
    }
    let sides = [(source_kind, source), (destination_kind, destination)];
    let read = endpoints_read(api, &scope, at, &sides).await?;
    Ok(sides.iter().any(|e| read.contains(e)))
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
    let now = kit::now(&api).await?;
    let moment = dates_given.at(now, api.bound)?;
    if !access(&api, &principal, &context, id, moment.valid_at).await? {
        return Err(Problem::new(ProblemKind::NotFound));
    }
    let view = flow_at(&api, id, &moment).await?;
    let version = kit::version(&api, id, &moment)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let flow = served(&api, moment.valid_at, vec![view])
        .await?
        .pop()
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    kit::record_read(&api, &context, FLOW, &[id], json!({"id": id})).await?;
    Ok(kit::respond_object(&headers, &moment, version, &flow))
}

/// `GET /v1/flows/{id}/evaluations`: the evaluations valid at the date, as asserted then,
/// never recomputed, the same for every reader of the flow (REQ-011-16, 23; REQ-027-2).
async fn evaluations(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates_given: DateParams,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let now = kit::now(&api).await?;
    let moment = dates_given.at(now, api.bound)?;
    if !access(&api, &principal, &context, id, moment.valid_at).await? {
        return Err(Problem::new(ProblemKind::NotFound));
    }
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
    let items: Vec<EvaluationDto> = rows.into_iter().map(evaluation_dto).collect();
    Ok(respond_page(&headers, &moment, &items, None))
}

/// `GET /v1/flow-nonconformities` (REQ-011-25): newest first, filtered; a caller reads the
/// rows of the entities where it holds `flow.read`, those of its own requests, and those of
/// the flows it reads through one of their endpoints (REQ-027-1).
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
    filter.after = page
        .after()
        .map(|key| {
            let invalid = || Problem::new(ProblemKind::InvalidCursor);
            let (at, id) =
                serde_json::from_value::<(String, Uuid)>(key.clone()).map_err(|_| invalid())?;
            Ok::<_, Problem>((instant("cursor", &at).map_err(|_| invalid())?, id))
        })
        .transpose()?;
    let (moment, scope) = reading(&api, &principal, &page).await?;
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let size = usize::from(page.size);
    let mut rows =
        read::nonconformities(&mut conn, dates(&moment), &filter, i64::from(page.size) + 1).await?;
    // The key of the last row read, before the filter of the rights (design 011).
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let (at, id) = rows[size - 1].key();
        let cursor = page.next(
            &api.ring,
            NONCONFORMITIES,
            &moment,
            json!([rfc3339(at), id]),
        );
        next_href(&format!("/v1/{NONCONFORMITIES}"), &cursor)
    });
    let entities: Vec<Uuid> = rows.iter().map(|r| r.entity).collect();
    let readable = readable_entities(&api, &scope, moment.valid_at, &entities).await?;
    let mine =
        |r: &Nonconformity| r.requester == principal.identity || readable.contains(&r.entity);
    let others: Vec<Uuid> = rows
        .iter()
        .filter(|r| !mine(r))
        .filter_map(|r| r.flow)
        .collect();
    let through = through_endpoints(&api, &scope, moment.valid_at, &others).await?;
    let items: Vec<NonconformityDto> = rows
        .into_iter()
        .filter(|r| mine(r) || r.flow.is_some_and(|f| through.contains(&f)))
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
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// The entities of `ids` where the scope of `flow.read` reaches: globally, or the
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
    if let Ok(view) = flow_at(&api, id, &moment).await
        && let Some(flow) = served(&api, moment.valid_at, vec![view]).await?.pop()
    {
        answer.insert("flow".into(), json!(flow));
    }
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let evaluations: Vec<EvaluationDto> = read::evaluations(&mut conn, dates(&moment), id)
        .await?
        .into_iter()
        .map(evaluation_dto)
        .collect();
    answer.insert("evaluations".into(), json!(evaluations));
    parts.headers.remove(header::CONTENT_LENGTH);
    let body = serde_json::to_vec(&answer)
        .map_err(|e| Problem::new(ProblemKind::Internal).detail(e.to_string()))?;
    Ok(Response::from_parts(parts, Body::from(body)))
}

// ---------------------------------------------------------------- test of a connection

/// A flow that opens a pair (REQ-011-26).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = FlowOpening)]
pub struct OpeningDto {
    /// The flow.
    id: Uuid,
    /// `active` or `waived` (a waiver covers the date: the policy denies it).
    state: String,
    /// The end of the flow, if dated.
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<String>,
}

/// One evaluation of the test: is it open, is it conformant (REQ-011-26).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = FlowCheckPair)]
pub struct CheckPairDto {
    /// Index of the evaluation in `verdict.evaluations`.
    evaluation: usize,
    /// Whether a flow of the registry the caller reads opens it (a flow it does not read
    /// never counts, REQ-027-1); absent when the verdict is reduced (REQ-010-25): it names
    /// no address, so no flow is matched against the pair.
    #[serde(skip_serializing_if = "Option::is_none")]
    open: Option<bool>,
    /// The flows that open it, `active` or `waived`, those the caller reads.
    flows: Vec<OpeningDto>,
    /// Whether the policy allows it (verdict other than `denied`).
    conformant: bool,
    /// `open_conformant`, `open_nonconformant`, `not_open_conformant` or
    /// `not_open_nonconformant`; absent when `open` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    case: Option<String>,
}

/// The test of a connection (REQ-011-26, F21): the query of compliance, and for each of
/// its evaluations whether a flow of the registry opens it; nothing is written.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = FlowCheck)]
pub struct CheckDto {
    /// The answer of the query of compliance (`QUERY /v1/verdicts`).
    verdict: kartenn_policy::rest::VerdictDto,
    /// One per evaluation, in its order.
    pairs: Vec<CheckPairDto>,
    /// The test summed up: open when every evaluation is, conformant when every one is;
    /// absent without evaluation (`verdict.empty`) or when the verdict is reduced.
    #[serde(skip_serializing_if = "Option::is_none")]
    case: Option<String>,
}

/// The name of one of the four cases (REQ-011-26).
fn case_of(open: bool, conformant: bool) -> String {
    match (open, conformant) {
        (true, true) => "open_conformant",
        (true, false) => "open_nonconformant",
        (false, true) => "not_open_conformant",
        (false, false) => "not_open_nonconformant",
    }
    .into()
}

/// The identifiers of the services of `keys` at `at`.
async fn services_of(
    api: &Api,
    keys: &[String],
    at: OffsetDateTime,
) -> Result<HashMap<String, Uuid>, Problem> {
    let rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT key, id FROM policy.policy_service WHERE key = ANY($1) AND valid_period @> $2",
    )
    .bind(keys)
    .bind(at)
    .fetch_all(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    Ok(rows.into_iter().collect())
}

/// Tests the connection of `dto` for the caller (REQ-011-26): the query of compliance,
/// then the flows that open each evaluation, those the caller reads.
#[allow(clippy::too_many_lines)] // the steps of the test, in their order
async fn check(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    dto: kartenn_policy::rest::VerdictQueryDto,
) -> Result<Response, Problem> {
    use kartenn_policy::evaluate::Verdict;
    use kartenn_policy::verdict::Access;

    let answered = kartenn_policy::rest::evaluate(api, principal, dto).await?;
    let (moment, answer) = (answered.moment, answered.answer);
    let full = answer.access == Access::Full;
    let keys: Vec<String> = answer
        .evaluations
        .iter()
        .filter_map(|e| e.evaluation.service.as_ref().map(ToString::to_string))
        .collect();
    let services = services_of(api, &keys, moment.valid_at).await?;
    // Each evaluation whose points and service are named, by index.
    let mut asked = Vec::new();
    let mut tested = Vec::new();
    for (i, e) in answer.evaluations.iter().enumerate() {
        let source = answer.sources.get(e.source).and_then(|p| p.id);
        let destination = answer.destinations.get(e.destination).and_then(|p| p.id);
        let service = e
            .evaluation
            .service
            .as_ref()
            .and_then(|k| services.get(&k.to_string()).copied());
        if let (Some(source), Some(destination), Some(service)) = (source, destination, service) {
            asked.push(i);
            tested.push(read::Tested {
                source,
                destination,
                service,
            });
        }
    }
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let opened = read::open(&mut conn, dates(&moment), &tested).await?;
    drop(conn);
    let mut flows: Vec<(Uuid, Sides<Endpoint>)> = opened
        .iter()
        .flatten()
        .filter_map(|v| Some((v.id?, endpoints_of(&v.flow))))
        .collect();
    flows.sort_unstable_by_key(|f| f.0);
    flows.dedup_by_key(|f| f.0);
    let scope = kit::scope(api, principal, kit::now(api).await?).await?;
    let seen = flows_seen(api, &scope, principal, moment.valid_at, &flows, &[]).await?;
    let mut flows_of: HashMap<usize, Vec<OpeningDto>> = HashMap::new();
    for (i, views) in asked.into_iter().zip(opened) {
        flows_of.insert(
            i,
            views
                .into_iter()
                .filter(|v| v.id.is_some_and(|id| seen.contains(&id)))
                .map(|v| OpeningDto {
                    id: v.id.unwrap_or_default(),
                    state: v.state.into(),
                    end: v.end.map(rfc3339),
                })
                .collect(),
        );
    }
    let pairs: Vec<CheckPairDto> = answer
        .evaluations
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let flows = flows_of.remove(&i).unwrap_or_default();
            let conformant = e.evaluation.verdict != Verdict::Denied;
            let open = full.then_some(!flows.is_empty());
            CheckPairDto {
                evaluation: i,
                open,
                flows,
                conformant,
                case: open.map(|o| case_of(o, conformant)),
            }
        })
        .collect();
    let case = (full && !pairs.is_empty()).then(|| {
        case_of(
            pairs.iter().all(|p| p.open == Some(true)),
            pairs.iter().all(|p| p.conformant),
        )
    });
    let cited: Vec<Uuid> = pairs
        .iter()
        .flat_map(|p| p.flows.iter().map(|f| f.id))
        .collect();
    kit::record_read(
        api,
        context,
        FLOW,
        &cited,
        json!({
            "check": [answered.query.source.id, answered.query.destination.id],
            "valid_at": rfc3339(moment.valid_at),
            "as_of": rfc3339(moment.as_of()),
        }),
    )
    .await?;
    let mut response = axum::Json(CheckDto {
        verdict: kartenn_policy::rest::verdict_dto(answer),
        pairs,
        case,
    })
    .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

/// `GET /v1/flow-checks?source=<kind>:<id>&destination=…&service=…`: the test of a
/// connection by the parameters of `GET /v1/verdicts` (REQ-011-26).
async fn check_get(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(raw): RawQuery,
) -> Result<Response, Problem> {
    let dto = kartenn_policy::rest::query_of_params(raw.as_deref())?;
    check(&api, &principal, &context, dto).await
}

/// `QUERY /v1/flow-checks` with the body of `QUERY /v1/verdicts`; any other method but
/// `GET` and `HEAD` → 405.
async fn check_query(State(api): State<Api>, request: axum::extract::Request) -> Response {
    let (principal, context, dto) = match kartenn_policy::rest::query_body(request).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    match check(&api, &principal, &context, dto).await {
        Ok(response) => response,
        Err(problem) => problem.into_response(),
    }
}

/// Adds to the path item of `/flow-checks` the `QUERY` of its body, in the extension
/// `x-query` (design 006 « Versions et contrat »).
fn check_query_extension(mut router: ModuleRouter) -> ModuleRouter {
    let operation = op(
        "query_flow_check",
        "flow",
        "Tests a connection (REQ-011-26): the query of compliance, and whether a flow of \
         the registry opens each of its evaluations; nothing is written.",
    )
    .body("application/json", "PolicyVerdictQuery", true)
    .answers("200", "The test.", "FlowCheck")
    .build();
    let document = router.get_openapi_mut();
    let item = document
        .paths
        .paths
        .get_mut("/flow-checks")
        .expect("the path of a QUERY is routed");
    item.extensions.get_or_insert_default().merge(
        utoipa::openapi::extensions::ExtensionsBuilder::new()
            .add(
                kartenn_rest::openapi::QUERY_EXTENSION,
                serde_json::to_value(&operation).expect("an operation serialises"),
            )
            .build(),
    );
    router
}

/// The routes of the test of a connection (REQ-011-26).
fn check_routes(router: ModuleRouter) -> ModuleRouter {
    let check_op = op(
        "show_flow_check",
        "flow",
        "Tests a connection, by parameters (REQ-011-26): the query of compliance, and \
         whether a flow of the registry opens each of its evaluations; nothing is written.",
    )
    .query(
        "source",
        "The source, `<kind>:<id>` (address, prefix or interface).",
    )
    .query(
        "destination",
        "The destination, `<kind>:<id>` (address, prefix or interface).",
    )
    .query("service", "A service, by its key.")
    .query("transport", "A transport, with `port`.")
    .query("port", "A port, with `transport`.")
    .query("flow_type", "A flow type, by its key.")
    .query("entity", "The entity of the request.")
    .params::<DatesParams>()
    .answers("200", "The test.", "FlowCheck")
    .build();
    let router = router.routes(documented(
        "/flow-checks",
        vec![(HttpMethod::Get, check_op)],
        Vec::new(),
        get(check_get).fallback(check_query),
    ));
    check_query_extension(router)
}

fn schemas() -> Vec<(String, RefOr<Schema>)> {
    let mut schemas = Vec::new();
    schemas_of::<OpeningDto>(&mut schemas);
    schemas_of::<CheckPairDto>(&mut schemas);
    schemas_of::<CheckDto>(&mut schemas);
    schemas_of::<FlowDto>(&mut schemas);
    schemas_of::<EndpointDto>(&mut schemas);
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
        .answers("200", kit::OBJECT_READ, "Flow")
        .build();
    let modify_op = op(
        "modify_flow",
        "flow",
        "Modifies the flow (JSON Merge Patch); its identity does not change (REQ-011-4). \
         An extension (a later end, or `end: null` on a dated flow) is triaged: applied when \
         automatic, otherwise held for a review (F20).",
    )
    .path("id", the, true)
    .header("If-Match", true, kit::IF_MATCH_OBJECT)
    .body(MERGE_PATCH, "FlowPatch", true)
    .answers("200", "Modified.", "ChangesetApplied")
    .answers(
        "202",
        "Held for a review or a decision.",
        "ChangesetPending",
    )
    .build();
    let close_op = op("close_flow", "flow", "Closes the flow.")
        .path("id", the, true)
        .header("If-Match", true, kit::IF_MATCH_OBJECT)
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
                    kit::modify_nullable(
                        &api,
                        &principal,
                        &context,
                        id,
                        &headers,
                        &raw,
                        flow_type(),
                        ("flow.update", UPDATE_MEMBERS, &["end"]),
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
         (REQ-011-16), the same for every reader of the flow (REQ-027-2).",
    )
    .path("id", the, true)
    .params::<DatesParams>()
    .answers("200", "The evaluations.", "Listing_FlowEvaluation")
    .build();
    let waiver_op = op(
        "waive_flow",
        "flow",
        "Grants a dated waiver to the flow (REQ-011-9), under `flow.waiver.grant`.",
    )
    .path("id", the, true)
    .header("If-Match", true, kit::IF_MATCH_OBJECT)
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
    // A flow has no natural key: it is named by its UUID (REQ-011-4).
    router = kit::presented(
        router,
        "/flows",
        &[],
        &[
            "summary",
            "state",
            "review",
            "outcome",
            "waived",
            "valid_from",
        ],
    );
    router = kit::presented(
        router,
        "/flow-nonconformities",
        &[],
        &["at", "kind", "state", "rule_key", "requester", "entity"],
    );
    router = object_routes(router, "addresses", EndpointKind::Address, "address");
    router = object_routes(router, "prefixes", EndpointKind::Prefix, "prefix");
    router = object_routes(router, "interfaces", EndpointKind::Interface, "interface");
    router = object_routes(router, "machines", EndpointKind::Machine, "machine");
    check_routes(router)
}
