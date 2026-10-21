//! The handler of the module in the write pipeline (ADR 0026, clause 6; design 011
//! « Opérations et règles »): the operations `flow.flow.request | register | update |
//! waive | approve_review`, under the lock `flow`; the rules of a flow on the final state
//! of its changeset; the references of its endpoints (REQ-011-3).
//!
//! Triage (REQ-011-21): `request` is evaluated against the policy when it is prepared, on
//! the state of the transaction; a request whose summary is not `allowed` with the outcome
//! `automatic` asks for a review (`flow.flow.review`), so the changeset becomes a proposal
//! whatever the rights of its author. Its validator joins `approve_review` (end of the
//! flow) and, for a denied flow, `waive`. `register` is the direct write of maintenance
//! (`flow.flow.write`, reading both endpoints): it is never triaged, and refused unless
//! compliant, reviewed or waived (REQ-011-8).

use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::Violation;
use kartenn_policy::evaluate::{Outcome as PolicyOutcome, SynthesisVerdict};
use kartenn_service::module::{
    BoxFuture, CheckContext, DiffContext, DiffSection, EndContext, Ending, Findings, ModuleHandler,
    ModuleObjectRow, ModulePlan, ModuleTx, Reference,
};
use kartenn_service::operations::{ModuleKind, ModuleOperation, RawInput, Ref};
use kartenn_service::port::{StoreError, WriteError};
use kartenn_service::reads::read_permission;
use kartenn_service::{ObjectKind, Operation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::evaluation::evaluate_flow;
use crate::model::{
    self, Endpoint, EndpointKind, Facts, Flow, FlowKind, REVIEW_MAX_MONTHS, WAIVER_MAX_MONTHS,
    Waiver,
};
use crate::module::FLOW;
use crate::pg;
use crate::rules::{
    ENDPOINT_INVALID, FIELD_INVALID, REVIEW_END_REQUIRED, REVIEW_REQUIRED, UPDATE_IDENTITY,
    VERDICT_PARTIAL, WAIVER_INVALID, WAIVER_REQUIRED,
};

/// The permission whose holder reviews a request (REQ-011-22).
pub const REVIEW: &str = "flow.flow.review";

/// The handler of the module.
#[derive(Debug, Clone, Copy, Default)]
pub struct Handler;

fn flow_kind() -> ModuleKind {
    static KIND: OnceLock<ModuleKind> = OnceLock::new();
    *KIND.get_or_init(|| ModuleKind::parse(FLOW).expect("a valid kind"))
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut()
        .ok_or_else(|| StoreError("not a PostgreSQL transaction".into()))
}

/// What the module completes on an operation before it is planned: overwritten at each
/// preparation, never taken from the client.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Completed {
    /// `core.owner`, to anchor a new flow on its entity.
    owner_property: Option<Uuid>,
    /// The end given, in microseconds.
    end: Option<i64>,
    /// Whether the end given is not an instant.
    end_invalid: bool,
    /// The evaluation of a request against the policy (REQ-011-21).
    triage: Option<Value>,
    /// The identity of a flow updated: its endpoints, service and kind.
    current: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointIn {
    kind: String,
    id: Ref,
}

/// The body of `request` and `register` (REQ-011-1).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct FlowIn {
    // The local name and the end are read by the core and by the completion.
    #[serde(default, rename = "local")]
    _local: Option<u32>,
    #[serde(default)]
    kind: Option<String>,
    source: EndpointIn,
    destination: EndpointIn,
    service: Ref,
    #[serde(default)]
    external_refs: Vec<Value>,
    #[serde(default)]
    justification: Option<String>,
    #[serde(default)]
    on_behalf_of: Option<Value>,
    #[serde(default)]
    responsibles: Vec<Uuid>,
    #[serde(default)]
    data_nature: Vec<Uuid>,
    #[serde(default)]
    personal_data: bool,
    entity: Ref,
    #[serde(default, rename = "end")]
    _end: Option<String>,
    #[serde(default)]
    completed: Option<Completed>,
}

/// The body of `update` (REQ-011-4): the fields that may change; the identity, if given,
/// must be unchanged.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateIn {
    id: Ref,
    #[serde(default)]
    external_refs: Option<Vec<Value>>,
    #[serde(default)]
    justification: Option<String>,
    #[serde(default)]
    responsibles: Option<Vec<Uuid>>,
    #[serde(default)]
    data_nature: Option<Vec<Uuid>>,
    #[serde(default)]
    personal_data: Option<bool>,
    #[serde(default, rename = "end")]
    _end: Option<String>,
    #[serde(default)]
    source: Option<EndpointIn>,
    #[serde(default)]
    destination: Option<EndpointIn>,
    #[serde(default)]
    service: Option<Ref>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    completed: Option<Completed>,
}

/// The body of `waive` (REQ-011-9).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WaiveIn {
    flow: Ref,
    reason: String,
    accepted_risk: String,
    #[serde(default, rename = "end")]
    _end: Option<String>,
    #[serde(default)]
    completed: Option<Completed>,
}

/// The body of `approve_review` (REQ-011-22).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewIn {
    flow: Ref,
    #[serde(default, rename = "end")]
    _end: Option<String>,
    #[serde(default)]
    completed: Option<Completed>,
}

fn invalid(operation: &ModuleOperation, why: impl std::fmt::Display) -> Violation {
    Violation::new(&FIELD_INVALID, format!("{}: {why}", operation.tag()))
}

fn decode<T: serde::de::DeserializeOwned>(operation: &ModuleOperation) -> Result<T, Violation> {
    serde_json::from_value(Value::Object(operation.body.clone())).map_err(|e| invalid(operation, e))
}

fn resolved(
    operation: &ModuleOperation,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
    r: Ref,
) -> Result<Uuid, Violation> {
    resolve(r).ok_or_else(|| invalid(operation, format!("{r:?} names no object")))
}

fn completed(operation: &ModuleOperation, c: Option<Completed>) -> Result<Completed, Violation> {
    let c = c.ok_or_else(|| invalid(operation, "not prepared"))?;
    if c.end_invalid {
        return Err(invalid(operation, "the end is not an instant"));
    }
    Ok(c)
}

/// The identifier given directly, if any.
fn id_of(body: &serde_json::Map<String, Value>, key: &str) -> Option<Uuid> {
    match serde_json::from_value(body.get(key)?.clone()).ok()? {
        Ref::Id(id) => Some(id),
        Ref::Local(_) => None,
    }
}

fn endpoint_id(body: &serde_json::Map<String, Value>, side: &str) -> Option<(EndpointKind, Uuid)> {
    let e = body.get(side)?;
    let kind = pg::kind_of(e.get("kind")?.as_str()?)?;
    match serde_json::from_value(e.get("id")?.clone()).ok()? {
        Ref::Id(id) => Some((kind, id)),
        Ref::Local(_) => None,
    }
}

fn instant_at(micros: i64) -> OffsetDateTime {
    pg::time(Instant(micros))
}

async fn now(conn: &mut PgConnection) -> Result<OffsetDateTime, StoreError> {
    sqlx::query_scalar("SELECT now()")
        .fetch_one(conn)
        .await
        .map_err(|e| pg::store(&e))
}

impl Handler {
    /// Completes an operation: the end read as an instant, the anchor property, and for a
    /// request its triage against the policy at the start of its period.
    async fn complete(
        tx: &mut dyn ModuleTx,
        operation: &mut ModuleOperation,
        period: Option<Period>,
    ) -> Result<(), StoreError> {
        let conn = connection(tx)?;
        let (from, _) = pg::bounds(period);
        let at = match from {
            Some(at) => at,
            None => now(conn).await?,
        };
        let mut completed = Completed::default();
        if let Some(text) = operation.body.get("end").and_then(Value::as_str) {
            match pg::parse_instant(conn, text).await? {
                Some(end) => completed.end = Some(end.0),
                None => completed.end_invalid = true,
            }
        }
        match operation.name.as_str() {
            "request" | "register" => {
                completed.owner_property = pg::property(conn, "core.owner", Some(at)).await?;
                if operation.name == "request" {
                    completed.triage =
                        Self::triage(conn, &operation.body, at, completed.end).await?;
                }
            }
            "update" => {
                if let Some(id) = id_of(&operation.body, "id")
                    && let Some(stored) = pg::load(conn, id).await?
                {
                    let f = &stored.flow;
                    completed.current = Some(json!({
                        "source": [pg::kind_name(f.source.kind), f.source.id],
                        "destination": [pg::kind_name(f.destination.kind), f.destination.id],
                        "service": f.service,
                        "kind": if f.kind == FlowKind::Observed { "observed" } else { "requested" },
                    }));
                }
            }
            _ => {}
        }
        operation.body.insert(
            "completed".into(),
            serde_json::to_value(&completed).expect("a JSON object"),
        );
        Ok(())
    }

    /// The triage of a request (REQ-011-21): its evaluation at `at`, when its endpoints and
    /// service are named by identifier; otherwise none, and the request is reviewed.
    async fn triage(
        conn: &mut PgConnection,
        body: &serde_json::Map<String, Value>,
        at: OffsetDateTime,
        end: Option<i64>,
    ) -> Result<Option<Value>, StoreError> {
        let (Some(source), Some(destination), Some(service), Some(entity)) = (
            endpoint_id(body, "source"),
            endpoint_id(body, "destination"),
            id_of(body, "service"),
            id_of(body, "entity"),
        ) else {
            return Ok(None);
        };
        let Ok(period) = Period::new(pg::instant(at), end.map(Instant)) else {
            return Ok(None);
        };
        let flow = Flow {
            kind: FlowKind::Requested,
            source: Endpoint {
                kind: source.0,
                id: source.1,
            },
            destination: Endpoint {
                kind: destination.0,
                id: destination.1,
            },
            service,
            external_refs: Vec::new(),
            justification: None,
            requester: Uuid::nil(),
            on_behalf_of: None,
            responsibles: body
                .get("responsibles")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str()?.parse().ok())
                .collect(),
            data_nature: body
                .get("data_nature")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str()?.parse().ok())
                .collect(),
            personal_data: false,
            entity,
            period,
        };
        let evaluated = evaluate_flow(conn, &flow, at).await?;
        Ok(Some(pg::triage_json(&evaluated)))
    }

    fn plan_flow(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let input: FlowIn = decode(operation)?;
        let c = completed(operation, input.completed.clone())?;
        if input.external_refs.is_empty() {
            return Err(Violation::new(
                &crate::rules::REFERENCE_MISSING,
                "a flow carries at least one external reference",
            ));
        }
        let side = |e: &EndpointIn, name: &str| -> Result<(EndpointKind, Uuid), Violation> {
            let kind = pg::kind_of(&e.kind).ok_or_else(|| {
                Violation::new(
                    &ENDPOINT_INVALID,
                    format!("the {name} is a {}, not an endpoint", e.kind),
                )
            })?;
            Ok((kind, resolved(operation, resolve, e.id)?))
        };
        let source = side(&input.source, "source")?;
        let destination = side(&input.destination, "destination")?;
        let service = resolved(operation, resolve, input.service)?;
        let entity = resolved(operation, resolve, input.entity)?;
        let owner = c
            .owner_property
            .ok_or_else(|| invalid(operation, "core.owner is not declared"))?;
        let kind = match input.kind.as_deref() {
            None | Some("requested") => "requested",
            Some("observed") => "observed",
            Some(other) => return Err(invalid(operation, format!("unknown kind {other}"))),
        };
        let record = json!({
            "kind": kind,
            "source_kind": pg::kind_name(source.0),
            "source_id": source.1,
            "destination_kind": pg::kind_name(destination.0),
            "destination_id": destination.1,
            "service": service,
            "external_refs": input.external_refs,
            "justification": input.justification,
            "on_behalf_of": input.on_behalf_of,
            "responsibles": input.responsibles,
            "data_nature": input.data_nature,
            "personal_data": input.personal_data,
            "entity": entity,
        });
        Ok(ModulePlan {
            kind: flow_kind(),
            object,
            creates: true,
            domains: Vec::new(),
            keys: vec!["flow".into()],
            core: vec![Operation::AssignProperty {
                object: Ref::Id(object),
                kind: ObjectKind::Module(flow_kind()),
                property: Ref::Id(owner),
                values: vec![RawInput::Text(entity.to_string())],
            }],
            data: json!({
                "op": operation.name,
                "record": record,
                "end": c.end,
                "triage": c.triage,
            }),
        })
    }

    fn plan_update(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let input: UpdateIn = decode(operation)?;
        let c = completed(operation, input.completed.clone())?;
        let id = resolved(operation, resolve, input.id)?;
        if let Some(current) = &c.current {
            let same_side = |e: &Option<EndpointIn>, key: &str| match e {
                None => true,
                Some(e) => {
                    let given = (e.kind.as_str(), resolve(e.id));
                    current[key][0].as_str() == Some(given.0)
                        && current[key][1]
                            .as_str()
                            .and_then(|s| s.parse::<Uuid>().ok())
                            == given.1
                }
            };
            let same_service = input.service.is_none_or(|s| {
                current["service"]
                    .as_str()
                    .and_then(|s| s.parse::<Uuid>().ok())
                    == resolve(s)
            });
            let same_kind = input
                .kind
                .as_deref()
                .is_none_or(|k| current["kind"].as_str() == Some(k));
            if !(same_side(&input.source, "source")
                && same_side(&input.destination, "destination")
                && same_service
                && same_kind)
            {
                return Err(Violation::new(
                    &UPDATE_IDENTITY,
                    "changing an endpoint, the service or the kind of a flow creates a new flow",
                )
                .with_subjects([id]));
            }
        }
        let mut patch = serde_json::Map::new();
        if let Some(refs) = input.external_refs {
            patch.insert("external_refs".into(), Value::Array(refs));
        }
        if let Some(j) = input.justification {
            patch.insert("justification".into(), Value::String(j));
        }
        if let Some(r) = input.responsibles {
            patch.insert("responsibles".into(), json!(r));
        }
        if let Some(d) = input.data_nature {
            patch.insert("data_nature".into(), json!(d));
        }
        if let Some(p) = input.personal_data {
            patch.insert("personal_data".into(), Value::Bool(p));
        }
        Ok(ModulePlan {
            kind: flow_kind(),
            object: id,
            creates: matches!(input.id, Ref::Local(_)),
            domains: Vec::new(),
            keys: vec!["flow".into()],
            core: Vec::new(),
            data: json!({ "op": "update", "patch": patch, "end": c.end }),
        })
    }

    fn plan_on_flow(
        operation: &ModuleOperation,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let (flow, data) = if operation.name == "waive" {
            let input: WaiveIn = decode(operation)?;
            let c = completed(operation, input.completed.clone())?;
            let end = c
                .end
                .ok_or_else(|| Violation::new(&WAIVER_INVALID, "a waiver has an end"))?;
            (
                input.flow,
                json!({ "op": "waive", "reason": input.reason,
                        "accepted_risk": input.accepted_risk, "end": end }),
            )
        } else {
            let input: ReviewIn = decode(operation)?;
            let c = completed(operation, input.completed.clone())?;
            let end = c.end.ok_or_else(|| {
                Violation::new(&REVIEW_END_REQUIRED, "an instructed flow has an end")
            })?;
            (input.flow, json!({ "op": "approve_review", "end": end }))
        };
        Ok(ModulePlan {
            kind: flow_kind(),
            object: resolved(operation, resolve, flow)?,
            // A flow requested in the same changeset is checked once written.
            creates: matches!(flow, Ref::Local(_)),
            domains: Vec::new(),
            keys: vec!["flow".into()],
            core: Vec::new(),
            data,
        })
    }

    async fn apply_plan(
        tx: &mut dyn ModuleTx,
        plan: &ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let actors = tx.actors();
        let op = plan.data["op"].as_str().unwrap_or_default().to_owned();
        let end = plan.data["end"].as_i64();
        match op.as_str() {
            "request" | "register" => {
                let mut record = plan.data["record"].clone();
                // The requester is the identity the changeset is for, never the client's
                // word (REQ-011-1, N31).
                let requester = actors.on_behalf_of.or(actors.author).unwrap_or_default();
                record["requester"] = json!(requester);
                let conn = connection(tx)?;
                let (from, to) = pg::bounds(period);
                let start = match from {
                    Some(at) => at,
                    None => now(conn).await?,
                };
                let to = end.map(instant_at).or(to);
                pg::insert_flow(conn, plan.object, &record, (Some(start), to), changeset).await?;
                pg::mark(conn, plan.object, &op).await?;
                let span = Period::new(pg::instant(start), to.map(pg::instant))
                    .map_err(|e| WriteError::Store(StoreError(e.to_string())))?;
                tx.register(
                    ModuleObjectRow {
                        id: plan.object,
                        kind: plan.kind,
                        parent: None,
                        domain: None,
                    },
                    Some(span),
                    changeset,
                )
                .await?;
            }
            "update" => {
                let conn = connection(tx)?;
                let patch = plan.data["patch"].clone();
                if patch.as_object().is_some_and(|p| !p.is_empty()) {
                    pg::patch_flow(conn, plan.object, &patch, pg::bounds(period), changeset)
                        .await?;
                }
                pg::mark(conn, plan.object, &op).await?;
                if let Some(end) = end {
                    tx.close_versioned(
                        pg::FLOW_TABLE,
                        plan.object,
                        Some(Period::from(Instant(end))),
                        changeset,
                    )
                    .await?;
                }
            }
            "waive" => {
                let mut data = plan.data.clone();
                data["granted_by"] = json!(actors.validator.or(actors.author));
                let conn = connection(tx)?;
                let (from, _) = pg::bounds(period);
                pg::insert_waiver(conn, plan.object, &data, from, changeset).await?;
                pg::mark(conn, plan.object, &op).await?;
            }
            "approve_review" => {
                let conn = connection(tx)?;
                // Only the validator of a proposal reviews it (REQ-016-11); joined outside
                // a validation, the reviewer stays empty and the rules refuse it.
                pg::patch_flow(
                    conn,
                    plan.object,
                    &json!({ "reviewed_by": actors.validator }),
                    (None, None),
                    changeset,
                )
                .await?;
                pg::mark(conn, plan.object, &op).await?;
                if let Some(end) = end {
                    tx.close_versioned(
                        pg::FLOW_TABLE,
                        plan.object,
                        Some(Period::from(Instant(end))),
                        changeset,
                    )
                    .await?;
                }
            }
            other => {
                return Err(WriteError::Store(StoreError(format!(
                    "flow.{other}: not an operation of the module"
                ))));
            }
        }
        Ok(())
    }

    /// The rules of one flow written by the changeset, on its final state.
    #[allow(clippy::too_many_lines)] // the rules of design 011, one after the other
    async fn check_flow(
        tx: &mut dyn ModuleTx,
        id: Uuid,
        changeset: Uuid,
        out: &mut Vec<Violation>,
    ) -> Result<(), StoreError> {
        let validating = tx.actors().validator.is_some();
        let conn = connection(tx)?;
        let marks = pg::marks(conn, id).await?;
        if marks.is_empty() {
            return Ok(());
        }
        let Some(stored) = pg::load(conn, id).await? else {
            return Ok(());
        };
        let flow = &stored.flow;
        let now = now(conn).await?;
        let now_instant = pg::instant(now);
        let mut facts = Facts {
            machines_present: true,
            ..Facts::default()
        };
        if pg::exists_over(conn, flow.source.kind, flow.source.id, flow.period).await? {
            facts.source_versions = vec![flow.period];
        }
        if pg::exists_over(
            conn,
            flow.destination.kind,
            flow.destination.id,
            flow.period,
        )
        .await?
        {
            facts.destination_versions = vec![flow.period];
        }
        if pg::service_over(conn, flow.service, flow.period).await? {
            facts.service_versions = vec![flow.period];
        }
        let subjects = |v: Violation| v.with_subjects([id]);
        out.extend(model::check(flow, &facts).into_iter().map(subjects));
        let created = marks.iter().any(|m| m == "request" || m == "register");
        let requested = marks.iter().any(|m| m == "request");
        if created {
            let at = flow.period.start().max(now_instant);
            let at_time = pg::time(at);
            // Its evaluation is written by the section of the differential (`diff`).
            let evaluated = evaluate_flow(conn, flow, at_time).await?;
            let summary = evaluated.summary;
            let compliant = summary.is_some_and(|s| {
                s.verdict == SynthesisVerdict::Allowed && s.outcome == PolicyOutcome::Automatic
            });
            let denied = summary.is_none_or(|s| s.verdict == SynthesisVerdict::Denied);
            if summary.is_some_and(|s| s.verdict == SynthesisVerdict::Partial) {
                out.push(subjects(Violation::new(
                    &VERDICT_PARTIAL,
                    "only part of the pairs of addresses of the flow is allowed",
                )));
            }
            // A request still to review becomes a proposal (`review`); everything else,
            // and a request at its validation, is applied only reviewed or compliant.
            let applied_now = !requested || validating;
            if !compliant && applied_now {
                let triage = pg::triage_json(&evaluated);
                let why = triage["decisive"]
                    .as_str()
                    .unwrap_or("no address")
                    .to_owned();
                if stored.reviewed_by.is_none() {
                    out.push(subjects(Violation::new(
                        &REVIEW_REQUIRED,
                        format!("the flow is not compliant without review ({why})"),
                    )));
                }
                if denied && pg::waiver_at(conn, id, at_time).await?.is_none() {
                    out.push(subjects(Violation::new(
                        &WAIVER_REQUIRED,
                        format!("the policy denies the flow ({why}) and no waiver covers it"),
                    )));
                }
            }
            if compliant || applied_now {
                pg::register_evaluation(
                    conn,
                    id,
                    &evaluated,
                    stored.reviewed_by,
                    at_time,
                    changeset,
                )
                .await?;
            }
        }
        if marks.iter().any(|m| m == "approve_review") {
            match stored.reviewed_by {
                None => out.push(subjects(Violation::new(
                    &REVIEW_REQUIRED,
                    "an approval of review is joined by the validator of a proposal",
                ))),
                Some(reviewer) => out.extend(
                    model::check_review(
                        reviewer,
                        flow.requester,
                        now_instant,
                        flow.period.end(),
                        REVIEW_MAX_MONTHS,
                    )
                    .into_iter()
                    .map(subjects),
                ),
            }
        }
        if marks.iter().any(|m| m == "waive")
            && let Some((reason, accepted_risk, granted_by, _, end)) =
                pg::waiver_written(conn, id, changeset).await?
        {
            let waiver = Waiver {
                reason,
                accepted_risk,
                granted_by,
                granted_at: now_instant,
                end: end.map(pg::instant),
            };
            out.extend(
                model::check_waiver(
                    &waiver,
                    flow.requester,
                    flow.period.end(),
                    WAIVER_MAX_MONTHS,
                )
                .into_iter()
                .map(subjects),
            );
        }
        Ok(())
    }
}

impl ModuleHandler for Handler {
    fn prepare<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        operation: &'a mut ModuleOperation,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(Self::complete(tx, operation, period))
    }

    fn plan(
        &self,
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        match operation.name.as_str() {
            "request" | "register" => Self::plan_flow(operation, object, resolve),
            "update" => Self::plan_update(operation, resolve),
            "waive" | "approve_review" => Self::plan_on_flow(operation, resolve),
            _ => Err(invalid(operation, "unknown operation")),
        }
    }

    fn reads(&self, plan: &ModulePlan) -> Vec<(String, Uuid)> {
        // The direct write reads both endpoints (REQ-011-17): an endpoint it may not read
        // is refused as an absent object; a request does not.
        if plan.data["op"] != "register" {
            return Vec::new();
        }
        ["source", "destination"]
            .iter()
            .filter_map(|side| {
                let record = &plan.data["record"];
                let kind = pg::kind_of(record[format!("{side}_kind")].as_str()?)?;
                let id = record[format!("{side}_id")].as_str()?.parse().ok()?;
                Some((read_permission(pg::object_kind(kind)), id))
            })
            .collect()
    }

    fn review(&self, plan: &ModulePlan) -> Option<&'static str> {
        if plan.data["op"] != "request" {
            return None;
        }
        let triage = &plan.data["triage"];
        let compliant = triage["summary"] == "allowed" && triage["outcome"] == "automatic";
        (!compliant).then_some(REVIEW)
    }

    fn apply<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        plan: &'a ModulePlan,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        Box::pin(Self::apply_plan(tx, plan, period, changeset))
    }

    fn close<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        let _ = kind;
        Box::pin(async move {
            tx.close_versioned(pg::FLOW_TABLE, id, period, changeset)
                .await?;
            let (at, _) = pg::bounds(period);
            pg::close_dependents(connection(tx)?, id, at, changeset).await?;
            // REQ-011-7: with the changeset, never written into the flow.
            tx.emit(
                "flow.closed",
                Some(id.to_string()),
                json!({ "flow": id, "changeset": changeset }),
            )
            .map_err(WriteError::Store)?;
            Ok(())
        })
    }

    fn restore<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        kind: ModuleKind,
        id: Uuid,
        before: Uuid,
        period: Option<Period>,
        changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        let _ = kind;
        tx.restore_versioned(pg::FLOW_TABLE, id, before, period, changeset)
    }

    fn referrers<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        reference: &'a Reference,
        target: Uuid,
        period: Option<Period>,
    ) -> BoxFuture<'a, Result<Vec<Uuid>, StoreError>> {
        Box::pin(async move {
            let (at, _) = pg::bounds(period);
            pg::referrers(connection(tx)?, reference.to, target, at).await
        })
    }

    fn diff<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a DiffContext<'a>,
    ) -> BoxFuture<'a, Result<Option<DiffSection>, StoreError>> {
        Box::pin(async move { crate::reevaluation::section(connection(tx)?, context).await })
    }

    fn check<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move {
            let mut out = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for (kind, id) in context.touched {
                if *kind == flow_kind() && seen.insert(*id) {
                    Self::check_flow(tx, *id, context.changeset, &mut out).await?;
                }
            }
            crate::expiry::outcome_changed(tx, context)?;
            let conn = connection(tx)?;
            let now = now(conn).await?;
            let mut findings = crate::approval::anomalies(conn, context, now).await?;
            findings.violations.extend(out);
            Ok(findings)
        })
    }

    fn run_job_findings<'a>(
        &'a self,
        name: &'a str,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        Box::pin(async move {
            if name == crate::expiry::EXPIRY {
                crate::expiry::run(tx).await
            } else {
                Ok(Findings::default())
            }
        })
    }

    fn measure<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Vec<(&'static str, u64)>, StoreError>> {
        Box::pin(async move { crate::approval::measures(connection(tx)?, context).await })
    }

    fn ended<'a>(
        &'a self,
        tx: &'a mut dyn ModuleTx,
        context: &'a EndContext<'a>,
    ) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            // A request rejected at review is a non-conformity of its requester
            // (REQ-011-22, 25; N38); withdrawn or expired, it leaves nothing.
            if context.how != Ending::Rejected {
                return Ok(());
            }
            let actors = tx.actors();
            let requester = actors.on_behalf_of.or(actors.author).unwrap_or_default();
            let Some(by) = context.by else {
                return Ok(());
            };
            let conn = connection(tx)?;
            for operation in &context.operations {
                if operation.name != "request" {
                    continue;
                }
                let Some(entity) = id_of(&operation.body, "entity") else {
                    continue;
                };
                let evaluation = operation
                    .body
                    .get("completed")
                    .and_then(|c| c.get("triage"))
                    .cloned()
                    .filter(Value::is_object)
                    .unwrap_or_else(|| json!({}));
                let mut request = operation.body.clone();
                request.remove("completed");
                pg::reject(
                    conn,
                    context.changeset,
                    &Value::Object(request),
                    &evaluation,
                    context.motive.unwrap_or("rejected"),
                    by,
                    requester,
                    entity,
                )
                .await?;
            }
            Ok(())
        })
    }
}
