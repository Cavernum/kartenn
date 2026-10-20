//! Changesets (spec 006, REQ-006-11, 21, 22; task 006-14): `POST /v1/changesets`
//! submits an atomic batch of operations (004) with its motive and period, or simulates it
//! with `?simulate=true`. Applied → 201, pending → 202, simulated → 200; refused by the
//! rules → 422 with every violation, each with the index of its operation and a JSON
//! Pointer into the request. The response links to the journaled changeset.
//!
//! `Idempotency-Key` (REQ-006-15, task 006-15): a key in progress → 409; a replay is
//! answered from the journal of the first changeset, with the same status and body.

use std::collections::BTreeSet;

use axum::body::Bytes;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use kartenn_core::rules::{CHANGESET_CONFLICT, PROP_EXPECTED_MISMATCH, Severity, Violation};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use kartenn_service::operations::PeriodSpec;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::{DiffCount, DiffItem};
use kartenn_service::{ChangesetRequest, ObjectKind, Outcome, submit_in};
use kartenn_store_pg::idempotency;
use kartenn_store_pg::list::{self, Kind};
use kartenn_store_pg::property_read;
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::rbac_read;
use kartenn_store_pg::read;
use kartenn_store_pg::write::PgStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::cycle;
use crate::dated;
use crate::objects::{self, Api, bad, instant, store};
use crate::operation::{OperationDto, TargetKindDto};
use crate::problem::{Problem, ProblemKind};
use crate::properties::masked_gap;
use crate::rights::SecretShown;

/// The relation type of the link to a changeset: an extension type, hence an URI
/// (RFC 8288 §2.1.2).
pub const REL_CHANGESET: &str = "urn:kartenn:rel:changeset";

/// The routes of the changesets.
pub fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new()
        .routes(routes!(submit, cycle::list))
        .routes(routes!(cycle::show))
        .routes(routes!(cycle::diff))
        .routes(routes!(cycle::validation))
        .routes(routes!(cycle::rejection))
        .routes(routes!(cycle::withdrawal))
        .routes(routes!(cycle::undo))
        .routes(routes!(cycle::proposals))
}

/// The path of a changeset.
#[must_use]
pub fn href(id: Uuid) -> String {
    format!("/v1/changesets/{id}")
}

/// The body of a submission (design « Écritures », arbitrage of 2026-10-19).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = Submission)]
struct Body {
    /// Why; required by the rules.
    #[serde(default)]
    motive: String,
    /// The start of the period of the changeset, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the changeset, RFC 3339; unbounded by default.
    valid_to: Option<String>,
    /// The operations, applied in order (at most 10 000).
    operations: Vec<OperationDto>,
}

/// Whether to simulate: `simulate=true|false`, at most once; anything else → 400.
pub(crate) fn simulate(query: Option<&str>) -> Result<bool, Problem> {
    let mut simulate = None;
    for (name, value) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        if name != "simulate" {
            return Err(bad(format!("unknown parameter {name}")));
        }
        let flag = match value.as_ref() {
            "true" => true,
            "false" => false,
            _ => return Err(bad("simulate must be true or false")),
        };
        if simulate.replace(flag).is_some() {
            return Err(bad("simulate is given more than once"));
        }
    }
    Ok(simulate.unwrap_or(false))
}

/// Name of the header of an idempotent submission (draft-ietf-httpapi-idempotency-key).
pub const IDEMPOTENCY_KEY: &str = "idempotency-key";

/// The idempotency key of the request: at most one, 1 to 255 printable characters
/// (design « Écritures »), otherwise 400.
pub(crate) fn idempotency_key(headers: &HeaderMap) -> Result<Option<String>, Problem> {
    let mut values = headers.get_all(IDEMPOTENCY_KEY).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(bad("Idempotency-Key is given more than once"));
    }
    match value.to_str() {
        Ok(key) if (1..=255).contains(&key.len()) => Ok(Some(key.to_owned())),
        _ => Err(bad(
            "Idempotency-Key must be 1 to 255 printable ASCII characters",
        )),
    }
}

/// The body as a submission: JSON only (another media type → 415), a changeset or 400.
fn body(headers: &HeaderMap, body: &[u8]) -> Result<Body, Problem> {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Err(Problem::new(ProblemKind::UnsupportedMediaType));
    }
    serde_json::from_slice(body).map_err(|e| bad(format!("the body is not a changeset: {e}")))
}

/// The period of the changeset: `valid_to` absent or `null` is unbounded, `valid_from`
/// absent is the time of the database (REQ-004-3); both absent, the service takes the
/// time of its transaction.
pub(crate) async fn period(
    api: &Api,
    valid_from: Option<&str>,
    valid_to: Option<&str>,
) -> Result<Option<PeriodSpec>, Problem> {
    let date = |name: &str, text: Option<&str>| {
        text.map(|t| dated::instant(name, t).map(|at| instant(at).0))
            .transpose()
    };
    let (from, to) = (date("valid_from", valid_from)?, date("valid_to", valid_to)?);
    let start = match (from, to) {
        (None, None) => return Ok(None),
        (Some(start), _) => start,
        (None, Some(_)) => instant(read::now(&api.pool).await.map_err(|e| store(&e))?).0,
    };
    Ok(Some(PeriodSpec { start, end: to }))
}

/// Where a violation points in the request (RFC 6901).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Pointers<'a> {
    /// `POST /v1/changesets`: the operation, by its index.
    Operations,
    /// A route by resource: the member each operation comes from, the index omitted
    /// (design « Erreurs »).
    Members(&'a [String]),
}

/// How the outcome of a changeset is answered.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Answer<'a> {
    /// Where the violations point.
    pub pointers: Pointers<'a>,
    /// The status of an applied changeset: 201, or 200 for a modification or a closure.
    pub applied: StatusCode,
    /// The created object `Location` names instead of the changeset; `None` without
    /// creation.
    pub location: Option<Location<'a>>,
    /// A conflict on `If-Match` answers 412 instead of 422 (REQ-006-14).
    pub precondition: bool,
    /// A secret generated for the changeset, shown once as the member `secret` of the
    /// response applied or pending, never on a replay (design « Droits »).
    pub shown: Option<&'a Value>,
}

/// What `Location` names.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Location<'a> {
    /// The changeset.
    Changeset,
    /// The object of this kind the changeset created.
    Created(Kind),
    /// This path, known before the changeset.
    Fixed(&'a str),
    /// The binding the changeset created.
    Binding,
    /// The service account of this secret, which the changeset created.
    Account(Uuid),
}

impl Answer<'_> {
    /// The answer of `POST /v1/changesets`.
    pub(crate) const SUBMISSION: Answer<'static> = Answer {
        pointers: Pointers::Operations,
        applied: StatusCode::CREATED,
        location: Some(Location::Changeset),
        precondition: false,
        shown: None,
    };
}

/// A violation of a rule (REQ-006-21), as served.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct ViolationDto {
    /// The rule.
    rule: &'static str,
    /// `error` or `warning`.
    severity: &'static str,
    /// The message of the rule.
    detail: String,
    /// The objects concerned.
    subjects: Vec<Uuid>,
    /// `POST /v1/changesets`: the index of the operation; `null` for a rule of the final
    /// state. Absent from the routes by resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<usize>)]
    #[allow(clippy::option_option)] // Absent, `null` or an index: three states.
    operation: Option<Option<usize>>,
    /// The JSON Pointer (RFC 6901) to what the violation comes from in the request;
    /// `null` for a rule of the final state.
    pointer: Option<String>,
}

/// A violation as served: its operation and a pointer to it in the request
/// (RFC 6901), `null` for a rule of the final state (REQ-004-7).
fn violation(v: &Violation, pointers: Pointers<'_>) -> ViolationDto {
    let (operation, pointer) = match pointers {
        Pointers::Operations => (
            Some(v.operation),
            v.operation.map(|i| format!("/operations/{i}")),
        ),
        Pointers::Members(members) => (None, v.operation.and_then(|i| members.get(i).cloned())),
    };
    ViolationDto {
        rule: v.rule.id,
        severity: match v.rule.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        },
        detail: v.message.clone(),
        subjects: v.subjects.clone(),
        operation,
        pointer,
    }
}

/// The violations as served, the message of a deviation of a sensitive property replaced:
/// it cites the resolved value (REQ-005-21, design « Erreurs »).
pub(crate) async fn violations(
    api: &Api,
    found: &[Violation],
    pointers: Pointers<'_>,
) -> Result<Vec<ViolationDto>, Problem> {
    let sensitive = if found.iter().any(|v| v.rule.id == PROP_EXPECTED_MISMATCH.id) {
        property_read::sensitive_keys(&api.pool)
            .await
            .map_err(|e| store(&e))?
    } else {
        BTreeSet::new()
    };
    Ok(found
        .iter()
        .map(|v| {
            let mut served = violation(v, pointers);
            if v.rule.id == PROP_EXPECTED_MISMATCH.id
                && let Some((key, _)) = v.message.split_once(':')
                && sensitive.contains(key)
            {
                served.detail = masked_gap(key);
            }
            served
        })
        .collect())
}

/// The violations as the member of a problem.
pub(crate) fn violations_value(served: &[ViolationDto]) -> Value {
    serde_json::to_value(served).expect("violations serialise")
}

/// A changeset, named.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct ChangesetRef {
    /// Identifier.
    pub(crate) id: Uuid,
    /// Its URL.
    pub(crate) href: String,
}

pub(crate) fn summary(id: Uuid) -> ChangesetRef {
    ChangesetRef { id, href: href(id) }
}

/// `Link` to changeset `id`, and `Location` to `location` when given.
pub(crate) fn linked(mut response: Response, id: Uuid, location: Option<String>) -> Response {
    if let Ok(link) = HeaderValue::try_from(format!("<{}>; rel=\"{REL_CHANGESET}\"", href(id))) {
        response.headers_mut().insert(header::LINK, link);
    }
    if let Some(location) = location
        && let Ok(location) = HeaderValue::try_from(location)
    {
        response.headers_mut().insert(header::LOCATION, location);
    }
    response
}

/// An object closed by cascade (REQ-004-26).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct Cascaded {
    /// Its kind.
    #[schema(value_type = TargetKindDto)]
    kind: ObjectKind,
    /// Identifier.
    id: Uuid,
}

pub(crate) fn cascaded(closures: &[(ObjectKind, Uuid)]) -> Vec<Cascaded> {
    closures
        .iter()
        .map(|&(kind, id)| Cascaded { kind, id })
        .collect()
}

/// A line of the counted differential (REQ-003-19): how many objects see a property go
/// from one value to another.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct DiffCountDto {
    /// Property key.
    property: String,
    /// The value before, as JSON text; `null`: no value.
    before: Option<String>,
    /// The value after, as JSON text; `null`: no value.
    after: Option<String>,
    /// Objects.
    count: u64,
    /// In a changeset read: the values are masked, the caller may not read them on every
    /// object counted (REQ-005-21).
    #[serde(skip_serializing_if = "Option::is_none")]
    masked: Option<bool>,
}

impl From<&DiffCount> for DiffCountDto {
    fn from(d: &DiffCount) -> Self {
        Self {
            property: d.property.clone(),
            before: d.before.clone(),
            after: d.after.clone(),
            count: d.count,
            masked: None,
        }
    }
}

/// One object of the differential (REQ-003-19).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct DiffItemDto {
    /// Property key.
    property: String,
    /// The object.
    object: Uuid,
    /// The value before, as JSON text; `null`: no value.
    before: Option<String>,
    /// The value after, as JSON text; `null`: no value.
    after: Option<String>,
}

impl From<&DiffItem> for DiffItemDto {
    fn from(d: &DiffItem) -> Self {
        Self {
            property: d.property.clone(),
            object: d.object,
            before: d.before.clone(),
            after: d.after.clone(),
        }
    }
}

/// A changeset applied.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ChangesetApplied {
    /// The changeset.
    changeset: ChangesetRef,
    /// `applied`.
    state: &'static str,
    /// The warnings of the rules.
    warnings: Vec<ViolationDto>,
    /// The counted differential.
    #[schema(value_type = Vec<DiffCountDto>)]
    diff: Value,
    /// The objects closed by cascade.
    cascaded: Vec<Cascaded>,
    /// The secret the changeset generated, shown once.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<SecretShown>)]
    secret: Option<Value>,
}

/// 201 (or 200): the changeset applied, its warnings, differential and closures by cascade.
async fn applied(
    api: &Api,
    changeset: Uuid,
    warnings: &[Violation],
    diff: Value,
    closures: &[(ObjectKind, Uuid)],
    answer: &Answer<'_>,
) -> Result<Response, Problem> {
    let body = ChangesetApplied {
        changeset: summary(changeset),
        state: "applied",
        warnings: violations(api, warnings, answer.pointers).await?,
        diff,
        cascaded: cascaded(closures),
        secret: answer.shown.cloned(),
    };
    Ok(linked(
        (answer.applied, Json(body)).into_response(),
        changeset,
        location(api, changeset, answer).await?,
    ))
}

/// A changeset held for a decision.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ChangesetPending {
    /// The changeset.
    changeset: ChangesetRef,
    /// `pending`.
    state: &'static str,
    /// `approval` or `proposal`.
    kind: String,
    /// The approval policy that holds it.
    policy: Option<Uuid>,
    /// The counted differential.
    #[schema(value_type = Vec<DiffCountDto>)]
    diff: Value,
    /// The secret the changeset generated, shown once.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<SecretShown>)]
    secret: Option<Value>,
}

/// 202: the changeset held for a decision.
fn pending(
    changeset: Uuid,
    kind: &str,
    policy: Option<Uuid>,
    diff: &Value,
    shown: Option<&Value>,
) -> Response {
    let body = ChangesetPending {
        changeset: summary(changeset),
        state: "pending",
        kind: kind.to_owned(),
        policy,
        diff: diff.clone(),
        secret: shown.cloned(),
    };
    linked(
        (StatusCode::ACCEPTED, Json(body)).into_response(),
        changeset,
        None,
    )
}

/// A changeset simulated: nothing is written (REQ-004-9).
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ChangesetSimulated {
    /// No violation is an error.
    valid: bool,
    /// Every violation, errors and warnings.
    violations: Vec<ViolationDto>,
    /// The counted differential applying would produce.
    diff: Vec<DiffCountDto>,
    /// Its first entries.
    sample: Vec<DiffItemDto>,
    /// The closures a cascade would add.
    cascaded: Vec<Cascaded>,
}

/// What `Location` names for the changeset `changeset` applied.
async fn location(
    api: &Api,
    changeset: Uuid,
    answer: &Answer<'_>,
) -> Result<Option<String>, Problem> {
    Ok(match answer.location {
        None => None,
        Some(Location::Changeset) => Some(href(changeset)),
        Some(Location::Created(kind)) => list::created(&api.pool, kind, changeset)
            .await
            .map_err(|e| store(&e))?
            .map(|id| objects::href(kind, id)),
        Some(Location::Fixed(path)) => Some((*path).to_owned()),
        Some(Location::Binding) => rbac_read::created_binding(&api.pool, changeset)
            .await?
            .map(|id| format!("/v1/bindings/{id}")),
        Some(Location::Account(secret)) => rbac_read::account_of_secret(&api.pool, secret)
            .await?
            .map(|id| format!("/v1/service-accounts/{id}")),
    })
}

/// 422: every violation of the refused changeset, errors then warnings as journaled; 412
/// when a precondition is answered and a version conflicts (REQ-006-14).
async fn refused(
    api: &Api,
    changeset: Uuid,
    mut found: Vec<Violation>,
    answer: &Answer<'_>,
) -> Result<Response, Problem> {
    found.sort_by_key(|v| !v.is_error());
    let conflict = answer.precondition && found.iter().any(|v| v.rule.id == CHANGESET_CONFLICT.id);
    let problem = if conflict {
        Problem::new(ProblemKind::PreconditionFailed)
            .detail("the object was modified since the version of If-Match")
    } else {
        Problem::new(ProblemKind::ChangesetRefused).detail(format!(
            "the changeset is refused by {} violation(s)",
            found.iter().filter(|v| v.is_error()).count()
        ))
    };
    let served = violations(api, &found, answer.pointers).await?;
    Ok(linked(
        problem
            .extension("violations", violations_value(&served))
            .extension(
                "changeset",
                serde_json::to_value(summary(changeset)).expect("a summary serialises"),
            )
            .into_response(),
        changeset,
        None,
    ))
}

/// The first response to the key, rebuilt from the journal of its changeset
/// (design « Idempotence »): 202 for a changeset held for a decision, even decided since.
async fn replayed(api: &Api, changeset: Uuid, answer: &Answer<'_>) -> Result<Response, Problem> {
    let answer = &Answer {
        shown: None,
        ..*answer
    };
    let journal = idempotency::journaled(&api.pool, changeset)
        .await?
        .ok_or_else(|| {
            tracing::error!(name: "http.changeset.replay_lost", %changeset, "the changeset of an idempotency key is not journaled");
            Problem::new(ProblemKind::Internal)
        })?;
    match (journal.kind.as_str(), journal.state.as_str()) {
        ("approval" | "proposal", _) => Ok(pending(
            changeset,
            &journal.kind,
            journal.policy,
            &journal.diff,
            None,
        )),
        (_, "applied") => {
            applied(
                api,
                changeset,
                &journal.violations,
                journal.diff,
                &journal.cascaded,
                answer,
            )
            .await
        }
        (_, "refused") => refused(api, changeset, journal.violations, answer).await,
        (kind, state) => {
            tracing::error!(name: "http.changeset.replay_unexpected", %changeset, kind, state, "a replayed changeset has an unexpected state");
            Err(Problem::new(ProblemKind::Internal))
        }
    }
}

/// The counted differential as served, the form the journal keeps.
fn counted(diff: &[DiffCount]) -> Result<Value, Problem> {
    let served: Vec<DiffCountDto> = diff.iter().map(DiffCountDto::from).collect();
    serde_json::to_value(served).map_err(|e| {
        tracing::error!(name: "http.changeset.diff", error = %e, "a differential does not serialise");
        Problem::new(ProblemKind::Internal)
    })
}

/// The store of the writes of `api`.
pub(crate) fn write_store(api: &Api) -> PgStore {
    PgStore::new(api.pool.clone())
        .with_instance(api.journal.instance().clone())
        .with_modules(api.modules.clone())
}

/// Submits `request` in `context` and answers its outcome as `answer` says.
pub(crate) async fn submit_answered(
    api: &Api,
    context: &RequestContext,
    request: &ChangesetRequest,
    answer: &Answer<'_>,
) -> Result<Response, Problem> {
    let outcome = submit_in(
        &write_store(api),
        &PgAuthorizer::new(api.pool.clone()),
        &api.sources,
        context,
        request,
        MAX_OPERATIONS,
    )
    .await?;
    answered(api, outcome, answer).await
}

/// The response to `outcome`, as `answer` says; a failed revalidation is refused (422).
pub(crate) async fn answered(
    api: &Api,
    outcome: Outcome,
    answer: &Answer<'_>,
) -> Result<Response, Problem> {
    let response = match outcome {
        Outcome::Applied {
            changeset,
            warnings,
            diff,
            cascaded: closures,
        } => {
            applied(
                api,
                changeset,
                &warnings,
                counted(&diff)?,
                &closures,
                answer,
            )
            .await?
        }
        Outcome::Pending {
            changeset,
            kind,
            policy,
            diff,
        } => pending(
            changeset,
            kind.as_str(),
            policy,
            &counted(&diff)?,
            answer.shown,
        ),
        Outcome::Refused {
            changeset,
            violations: found,
        }
        | Outcome::FailedRevalidation {
            changeset,
            violations: found,
        } => refused(api, changeset, found, answer).await?,
        Outcome::Replayed { changeset, .. } => replayed(api, changeset, answer).await?,
        // Nothing journaled: no changeset to link to (REQ-004-9).
        Outcome::Simulated {
            violations: found,
            diff,
            sample,
            cascaded: closures,
        } => Json(ChangesetSimulated {
            valid: !found.iter().any(Violation::is_error),
            violations: violations(api, &found, answer.pointers).await?,
            diff: diff.iter().map(DiffCountDto::from).collect(),
            sample: sample.iter().map(DiffItemDto::from).collect(),
            cascaded: cascaded(&closures),
        })
        .into_response(),
        other => {
            tracing::error!(name: "http.changeset.unexpected", outcome = ?other, "a changeset had an unexpected outcome");
            Problem::new(ProblemKind::Internal).into_response()
        }
    };
    Ok(response)
}

/// `POST /v1/changesets[?simulate=true]` with `{motive, valid_from, valid_to,
/// operations}`. The problems come in the order of HTTP (REQ-006-22): the authentication
/// (layer 6) before the syntax, the syntax (400, 415) before the rules (422).
#[utoipa::path(
    post,
    path = "/changesets",
    operation_id = "submit_changeset",
    tag = "changesets",
    params(
        ("simulate" = Option<bool>, Query, description = "Validate and report without writing."),
        ("Idempotency-Key" = Option<String>, Header, description = "Makes the submission idempotent for 24 h."),
    ),
    request_body = Body,
    responses(
        (status = 201, description = "Applied.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
        (status = 200, description = "Simulated.", body = ChangesetSimulated),
    )
)]
async fn submit(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let simulate = simulate(query.as_deref())?;
    let idempotency_key = idempotency_key(&headers)?;
    let body = body(&headers, &raw)?;
    let request = ChangesetRequest {
        author: principal.identity,
        motive: body.motive.clone(),
        period: period(&api, body.valid_from.as_deref(), body.valid_to.as_deref()).await?,
        operations: body.operations.into_iter().map(Into::into).collect(),
        simulate,
        idempotency_key,
    };
    submit_answered(&api, &context, &request, &Answer::SUBMISSION).await
}

#[cfg(test)]
mod tests {
    use kartenn_core::rules;

    use super::*;

    #[test]
    fn simulate_is_a_flag_given_once() {
        assert!(!simulate(None).unwrap());
        assert!(simulate(Some("simulate=true")).unwrap());
        for wrong in ["simulate=yes", "simulate=true&simulate=false", "dry=true"] {
            assert_eq!(
                simulate(Some(wrong)).unwrap_err().kind(),
                ProblemKind::BadRequest,
                "{wrong}"
            );
        }
    }

    #[test]
    fn the_body_is_a_json_changeset() {
        let mut json = HeaderMap::new();
        json.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        let mut patch = HeaderMap::new();
        patch.insert(
            header::CONTENT_TYPE,
            "application/merge-patch+json".parse().unwrap(),
        );
        let ok = br#"{"motive": "m", "operations": [{"op": "create_domain", "local": 1, "slug": "lab"}]}"#;
        assert_eq!(body(&json, ok).unwrap().operations.len(), 1);
        assert_eq!(
            body(&patch, ok).unwrap_err().kind(),
            ProblemKind::UnsupportedMediaType
        );
        for wrong in [
            &b"{"[..],
            br#"{"motive": "m"}"#,
            br#"{"operations": [], "x": 1}"#,
        ] {
            assert_eq!(
                body(&json, wrong).unwrap_err().kind(),
                ProblemKind::BadRequest
            );
        }
    }

    #[test]
    fn a_violation_points_to_its_operation() {
        let v = Violation::new(&rules::PREFIX_DUPLICATE, "dup").at(2);
        let served = serde_json::to_value(violation(&v, Pointers::Operations)).unwrap();
        assert_eq!(served["operation"], 2);
        assert_eq!(served["pointer"], "/operations/2");
        assert_eq!(served["severity"], "error");
        let final_state = serde_json::to_value(violation(
            &Violation::new(&rules::PREFIX_DUPLICATE, "dup"),
            Pointers::Operations,
        ))
        .unwrap();
        assert_eq!(final_state["pointer"], Value::Null);
        assert_eq!(final_state["operation"], Value::Null);
        let by_member = serde_json::to_value(violation(&v, Pointers::Members(&[]))).unwrap();
        assert!(by_member.get("operation").is_none());
    }
}
