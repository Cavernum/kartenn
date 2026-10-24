//! Drafts kept by the server (spec 024, REQ-024-1 to 7a; tasks 024-5, 6; design « A »):
//! `/v1/drafts` and its members, the private operations of a person, which apply nothing
//! until submitted.
//!
//! Only its author reads or writes a draft: another identity gets 404, its existence
//! untold, unless it holds `core.draft.admin` (global), which lists, reads, deletes and
//! takes over the drafts of someone else, each action audited with the author. A service
//! account has no draft (403). Every modification takes the version of the draft in
//! `If-Match` (412 once changed, REQ-024-3). The submission creates a changeset by the
//! ordinary pipeline (`apply=own` and `Idempotency-Key` as `POST /v1/changesets`): the
//! draft is deleted with its creation, kept on a refusal (422).

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use kartenn_core::audit::{
    DRAFT_ADMIN_READ, DRAFT_CREATED, DRAFT_DELETED, DRAFT_TAKEN_OVER, rfc3339,
};
use kartenn_service::ChangesetRequest;
use kartenn_service::audit::{AuditJournal, Outcome, RequestContext};
use kartenn_service::authn::{IdentityKind, Principal};
use kartenn_service::authz::Authorizer;
use kartenn_service::port::DraftOrigin;
use kartenn_service::reads::{DRAFT_ADMIN_PERMISSION, DRAFTS, drafts_administrable};
use kartenn_store_pg::drafts::{self, Change, Draft, DraftError, DraftSummary, NewDraft};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::changesets::{
    Answer, ChangesetApplied, ChangesetPending, ChangesetSimulated, idempotency_key, period,
    replayed, simulate, submit_answered,
};
use crate::objects::{Api, audited, instant};
use crate::operation::OperationDto;
use crate::problem::{Problem, ProblemKind};
use crate::resources::{entity_tag, invalid, stale};

/// The routes of the drafts.
pub(crate) fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(show, change, remove))
        .routes(routes!(add))
        .routes(routes!(simulation))
        .routes(routes!(submission))
        .routes(routes!(takeover))
        .routes(routes!(of_identity))
}

/// The path of a draft.
fn href(id: Uuid) -> String {
    format!("/v1/drafts/{id}")
}

/// The longest title, in characters.
const TITLE_MAX: usize = 200;

/// What a draft submits as its motive (M9): a summary, and a description.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = DraftReason)]
struct ReasonDto {
    /// The first line of the motive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    /// The rest of the motive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

impl ReasonDto {
    /// The motive of the changeset: the summary, then the description after a blank
    /// line; empty without summary.
    fn motive(&self) -> String {
        let summary = self.summary.as_deref().unwrap_or_default().trim();
        match self.description.as_deref().map(str::trim) {
            Some(d) if !d.is_empty() && !summary.is_empty() => format!("{summary}\n\n{d}"),
            _ => summary.to_owned(),
        }
    }
}

/// A draft as listed: without its operations.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = DraftSummary)]
struct SummaryDto {
    /// Identifier.
    id: Uuid,
    /// Its URL.
    href: String,
    /// Its author.
    author: Uuid,
    /// Title.
    title: String,
    /// The number of its operations.
    operation_count: i64,
    /// Version, its entity tag.
    version: i64,
    /// Creation, RFC 3339.
    created_at: String,
    /// Last modification, RFC 3339.
    updated_at: String,
    /// When the retention deletes it, RFC 3339: announced from 7 days before, after 83
    /// days without modification (REQ-024-7); absent until then.
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
}

/// A draft, whole.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Draft)]
struct DraftDto {
    /// What the list shows.
    #[serde(flatten)]
    summary: SummaryDto,
    /// The motive it submits.
    reason: ReasonDto,
    /// Its operations, `Operation`s of the contract, in order.
    #[schema(value_type = Vec<OperationDto>)]
    operations: Value,
}

/// The drafts of an identity: at most 50, no page.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = DraftList)]
struct ListDto {
    /// The most recently modified first.
    items: Vec<SummaryDto>,
}

fn time(at: OffsetDateTime) -> String {
    rfc3339(instant(at))
}

fn summarised(s: &DraftSummary) -> SummaryDto {
    SummaryDto {
        id: s.id,
        href: href(s.id),
        author: s.author,
        title: s.title.clone(),
        operation_count: s.operations,
        version: s.version,
        created_at: time(s.created_at),
        updated_at: time(s.updated_at),
        expires_at: s.expires_at.map(time),
    }
}

fn reason_of(stored: &Value) -> ReasonDto {
    serde_json::from_value(stored.clone()).unwrap_or_default()
}

/// The strong entity tag of a version.
fn etag(version: i64) -> HeaderValue {
    HeaderValue::try_from(format!("\"{version}\"")).expect("a number is a header value")
}

/// 200 (or `status`) with the draft and its `ETag`.
fn answered(draft: &Draft, status: StatusCode) -> Response {
    let body = DraftDto {
        summary: summarised(&draft.summary),
        reason: reason_of(&draft.reason),
        operations: draft.operations.clone(),
    };
    let mut response = (status, Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::ETAG, etag(draft.summary.version));
    if status == StatusCode::CREATED
        && let Ok(location) = HeaderValue::try_from(href(draft.summary.id))
    {
        response.headers_mut().insert(header::LOCATION, location);
    }
    response
}

/// The version `If-Match` names (REQ-024-3): absent → 428, another form → 412.
fn if_match(headers: &HeaderMap) -> Result<i64, Problem> {
    entity_tag(
        headers,
        "read the draft, then send its ETag in If-Match to modify it",
    )?
    .parse()
    .map_err(|_| stale())
}

fn not_found() -> Problem {
    Problem::new(ProblemKind::NotFound).detail("no such draft")
}

fn refused(e: DraftError) -> Problem {
    match e {
        DraftError::NotFound => not_found(),
        DraftError::Stale => Problem::new(ProblemKind::PreconditionFailed)
            .detail("the draft was modified since the entity tag of If-Match"),
        DraftError::Limit(detail) => Problem::new(ProblemKind::DraftLimit).detail(detail),
        DraftError::Store(e) => e.into(),
    }
}

/// A service account has no draft (REQ-024-2, M2): it submits directly.
fn person(principal: &Principal) -> Result<(), Problem> {
    if principal.kind == IdentityKind::ServiceAccount {
        return Err(Problem::new(ProblemKind::Forbidden)
            .detail("a service account has no draft: it submits its changesets directly"));
    }
    Ok(())
}

/// Whether the caller holds `core.draft.admin` (REQ-024-7a), without auditing a lack:
/// without it, the draft of someone else is absent (404).
async fn administers(api: &Api, principal: &Principal) -> Result<bool, Problem> {
    Ok(api
        .authorizer()
        .authorize_many(&principal.subject(), DRAFT_ADMIN_PERMISSION, &[DRAFTS])
        .await?
        .is_empty())
}

/// Draft `id` as its author, or as an administrator (`admin`) for whom it is someone
/// else's; 404 otherwise.
async fn found(api: &Api, principal: &Principal, id: Uuid, admin: bool) -> Result<Draft, Problem> {
    let draft = drafts::find(&api.pool, id).await?.ok_or_else(not_found)?;
    if draft.summary.author == principal.identity || (admin && administers(api, principal).await?) {
        Ok(draft)
    } else {
        Err(not_found())
    }
}

/// The body as JSON of `T`: another media type → 415, another form → 400.
fn json<T: for<'de> Deserialize<'de>>(headers: &HeaderMap, raw: &[u8]) -> Result<T, Problem> {
    let json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Err(Problem::new(ProblemKind::UnsupportedMediaType));
    }
    serde_json::from_slice(raw)
        .map_err(|e| crate::objects::bad(format!("the body is malformed: {e}")))
}

/// The operations, each an `Operation` of the contract (400 otherwise), at most 10 000
/// (422 `draft-limit`).
fn operations(values: Vec<Value>) -> Result<Vec<Value>, Problem> {
    if values.len() > drafts::MAX_OPERATIONS {
        return Err(Problem::new(ProblemKind::DraftLimit).detail(format!(
            "a draft holds at most {} operations",
            drafts::MAX_OPERATIONS
        )));
    }
    for (i, value) in values.iter().enumerate() {
        serde_json::from_value::<OperationDto>(value.clone()).map_err(|e| {
            crate::objects::bad(format!("operation {i} is not an operation: {e}"))
                .extension("pointer", Value::String(format!("/operations/{i}")))
        })?;
    }
    Ok(values)
}

fn title(text: &str) -> Result<String, Problem> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > TITLE_MAX {
        return Err(invalid(
            "/title",
            format!("a title has 1 to {TITLE_MAX} characters"),
        ));
    }
    Ok(text.to_owned())
}

fn reason(dto: ReasonDto) -> Value {
    serde_json::to_value(dto).unwrap_or_else(|_| Value::Object(serde_json::Map::new()))
}

/// `GET /v1/drafts`: the drafts of the caller, without their operations.
#[utoipa::path(
    get,
    path = "/drafts",
    operation_id = "list_drafts",
    tag = "drafts",
    responses(
        (status = 200, description = "The drafts of the caller, the most recently modified first.", body = ListDto),
    )
)]
async fn list(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
) -> Result<Response, Problem> {
    person(&principal)?;
    let found = drafts::list(&api.pool, principal.identity).await?;
    Ok(Json(ListDto {
        items: found.iter().map(summarised).collect(),
    })
    .into_response())
}

/// The body of a creation.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = DraftCreation)]
struct CreationBody {
    /// 1 to 200 characters.
    title: String,
    /// The motive it will submit.
    #[serde(default)]
    reason: Option<ReasonDto>,
    /// Its first operations.
    #[serde(default)]
    #[schema(value_type = Option<Vec<OperationDto>>)]
    operations: Option<Vec<Value>>,
}

/// `POST /v1/drafts`: a new draft of the caller, audited (REQ-024-6); 50 at most.
#[utoipa::path(
    post,
    path = "/drafts",
    operation_id = "create_draft",
    tag = "drafts",
    request_body = CreationBody,
    responses(
        (status = 201, description = "Created; `ETag` its version, `Location` its URL.", body = DraftDto),
        (status = 422, description = "Beyond 50 drafts or 10 000 operations (`draft-limit`), or a title out of bounds.", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn create(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    person(&principal)?;
    let body: CreationBody = json(&headers, &raw)?;
    let draft = NewDraft {
        id: Uuid::now_v7(),
        author: principal.identity,
        title: title(&body.title)?,
        reason: reason(body.reason.unwrap_or_default()),
        operations: operations(body.operations.unwrap_or_default())?,
    };
    let audit = context
        .event(&DRAFT_CREATED, Outcome::Success)
        .map_err(audited)?
        .target("draft", [draft.id])
        .detail("operations", draft.operations.len())
        .map_err(audited)?;
    let created = drafts::create(&api.pool, draft, &audit, api.journal.instance())
        .await
        .map_err(refused)?;
    Ok(answered(&created, StatusCode::CREATED))
}

/// `GET /v1/drafts/{id}`: the draft whole, to its author; to an administrator, audited
/// with its author (REQ-024-7a).
#[utoipa::path(
    get,
    path = "/drafts/{id}",
    operation_id = "get_draft",
    tag = "drafts",
    params(("id" = Uuid, Path, description = "The draft.")),
    responses(
        (status = 200, description = "The draft; `ETag` its version.", body = DraftDto),
        (status = 404, description = "Absent, or someone else's.", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn show(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    person(&principal)?;
    let draft = found(&api, &principal, id, true).await?;
    if draft.summary.author != principal.identity {
        let read = context
            .event(&DRAFT_ADMIN_READ, Outcome::Success)
            .map_err(audited)?
            .target("draft", [id])
            .detail("author", draft.summary.author.to_string())
            .map_err(audited)?
            .detail("count", 1)
            .map_err(audited)?;
        api.journal.record(&read).await.map_err(audited)?;
    }
    Ok(answered(&draft, StatusCode::OK))
}

/// The body of a modification: each member given replaces its own.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = DraftChange)]
struct ChangeBody {
    /// 1 to 200 characters.
    title: Option<String>,
    /// The motive it will submit.
    reason: Option<ReasonDto>,
    /// Operations that replace its own.
    #[schema(value_type = Option<Vec<OperationDto>>)]
    operations: Option<Vec<Value>>,
}

/// `PATCH /v1/drafts/{id}`: its title, motive or operations (the list replaced), under
/// `If-Match` (REQ-024-3); not audited (REQ-024-6).
#[utoipa::path(
    patch,
    path = "/drafts/{id}",
    operation_id = "update_draft",
    tag = "drafts",
    params(
        ("id" = Uuid, Path, description = "The draft."),
        ("If-Match" = String, Header, description = "The ETag of the draft read."),
    ),
    request_body = ChangeBody,
    responses(
        (status = 200, description = "Modified; `ETag` its new version.", body = DraftDto),
        (status = 404, description = "Absent, or someone else's.", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Modified since (another tab, another device).", body = Problem, content_type = "application/problem+json"),
        (status = 428, description = "`If-Match` is missing.", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn change(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    person(&principal)?;
    let version = if_match(&headers)?;
    let body: ChangeBody = json(&headers, &raw)?;
    let change = Change {
        title: body.title.as_deref().map(title).transpose()?,
        reason: body.reason.map(reason),
        operations: body.operations.map(operations).transpose()?,
        append: Vec::new(),
    };
    let draft = drafts::update(&api.pool, id, principal.identity, version, change)
        .await
        .map_err(refused)?;
    Ok(answered(&draft, StatusCode::OK))
}

/// The body of an addition.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = DraftOperations)]
struct OperationsBody {
    /// Operations added after its own, in order.
    #[schema(value_type = Vec<OperationDto>)]
    operations: Vec<Value>,
}

/// `POST /v1/drafts/{id}/operations`: operations added after its own, under `If-Match`.
#[utoipa::path(
    post,
    path = "/drafts/{id}/operations",
    operation_id = "add_draft_operations",
    tag = "drafts",
    params(
        ("id" = Uuid, Path, description = "The draft."),
        ("If-Match" = String, Header, description = "The ETag of the draft read."),
    ),
    request_body = OperationsBody,
    responses(
        (status = 200, description = "Added; `ETag` its new version.", body = DraftDto),
        (status = 404, description = "Absent, or someone else's.", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Modified since.", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Beyond 10 000 operations (`draft-limit`).", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn add(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    person(&principal)?;
    let version = if_match(&headers)?;
    let body: OperationsBody = json(&headers, &raw)?;
    let change = Change {
        append: operations(body.operations)?,
        ..Change::default()
    };
    let draft = drafts::update(&api.pool, id, principal.identity, version, change)
        .await
        .map_err(refused)?;
    Ok(answered(&draft, StatusCode::OK))
}

/// `DELETE /v1/drafts/{id}`: by its author, or by an administrator; under `If-Match`,
/// audited with its author (REQ-024-6, 7a).
#[utoipa::path(
    delete,
    path = "/drafts/{id}",
    operation_id = "delete_draft",
    tag = "drafts",
    params(
        ("id" = Uuid, Path, description = "The draft."),
        ("If-Match" = String, Header, description = "The ETag of the draft read."),
    ),
    responses(
        (status = 204, description = "Deleted."),
        (status = 404, description = "Absent, or someone else's.", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Modified since.", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn remove(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    person(&principal)?;
    let version = if_match(&headers)?;
    let draft = found(&api, &principal, id, true).await?;
    let author = draft.summary.author;
    let cause = if author == principal.identity {
        "author"
    } else {
        "admin"
    };
    let audit = context
        .event(&DRAFT_DELETED, Outcome::Success)
        .map_err(audited)?
        .target("draft", [id])
        .detail("cause", cause)
        .map_err(audited)?
        .detail("author", author.to_string())
        .map_err(audited)?;
    drafts::delete(
        &api.pool,
        id,
        author,
        version,
        &audit,
        api.journal.instance(),
    )
    .await
    .map_err(refused)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// The body of a simulation or a submission: the period of the changeset.
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = DraftSubmission)]
struct SubmissionBody {
    /// The start of the period of the changeset, RFC 3339; now by default.
    valid_from: Option<String>,
    /// The end of the period of the changeset, RFC 3339; unbounded by default.
    valid_to: Option<String>,
}

/// The changeset of draft `draft` of `principal`, its motive that of the draft; for a
/// simulation, the title when the draft has none yet.
async fn request(
    api: &Api,
    principal: &Principal,
    draft: &Draft,
    (headers, raw): (&HeaderMap, &Bytes),
    simulate: bool,
) -> Result<ChangesetRequest, Problem> {
    let body: SubmissionBody = if raw.is_empty() {
        SubmissionBody::default()
    } else {
        json(headers, raw)?
    };
    let operations: Vec<OperationDto> = serde_json::from_value(draft.operations.clone())
        .map_err(|e| crate::objects::bad(format!("the draft holds an invalid operation: {e}")))?;
    let mut motive = reason_of(&draft.reason).motive();
    if simulate && motive.is_empty() {
        motive.clone_from(&draft.summary.title);
    }
    Ok(ChangesetRequest {
        author: principal.identity,
        motive,
        period: period(api, body.valid_from.as_deref(), body.valid_to.as_deref()).await?,
        operations: operations.into_iter().map(Into::into).collect(),
        simulate,
        idempotency_key: if simulate {
            None
        } else {
            idempotency_key(headers)?
        },
        apply_own: false,
    })
}

/// `POST /v1/drafts/{id}/simulation`: as `POST /v1/changesets?simulate=true`, under the
/// rights of its author now (REQ-024-4): nothing is written.
#[utoipa::path(
    post,
    path = "/drafts/{id}/simulation",
    operation_id = "simulate_draft",
    tag = "drafts",
    params(("id" = Uuid, Path, description = "The draft.")),
    request_body(content = Option<SubmissionBody>, description = "The period of the changeset; none by default."),
    responses(
        (status = 200, description = "Simulated: violations, differential, sections.", body = ChangesetSimulated),
        (status = 404, description = "Absent, or someone else's.", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn simulation(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    person(&principal)?;
    let draft = found(&api, &principal, id, false).await?;
    let request = request(&api, &principal, &draft, (&headers, &raw), true).await?;
    submit_answered(&api, &context, &request, &Answer::SUBMISSION).await
}

/// `POST /v1/drafts/{id}/submission[?apply=own]`: the changeset of the draft, by the
/// ordinary pipeline under the rights of its author now (REQ-024-5, 13). Created, pending
/// (202) or applied by its author (201), it names the draft, which is deleted with it;
/// refused (422), the draft stays. `Idempotency-Key` as `POST /v1/changesets`: retried
/// once the draft is gone, the first answer comes again.
#[utoipa::path(
    post,
    path = "/drafts/{id}/submission",
    operation_id = "submit_draft",
    tag = "drafts",
    params(
        ("id" = Uuid, Path, description = "The draft."),
        ("If-Match" = Option<String>, Header, description = "The ETag of the draft read: when given, a draft modified since is not submitted (412)."),
        ("Idempotency-Key" = Option<String>, Header, description = "Makes the submission idempotent for 24 h."),
    ),
    request_body(content = Option<SubmissionBody>, description = "The period of the changeset; none by default."),
    responses(
        (status = 201, description = "Applied by its author (`apply=own`); the draft is deleted.", body = ChangesetApplied),
        (status = 202, description = "Submitted, pending its application; the draft is deleted.", body = ChangesetPending),
        (status = 404, description = "Absent, or someone else's.", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Modified since the ETag of `If-Match`.", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Refused (violations, motive missing): the draft stays.", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn submission(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    person(&principal)?;
    let simulated = simulate(query.as_deref())?;
    let draft = match drafts::find(&api.pool, id).await? {
        Some(draft) if draft.summary.author == principal.identity => draft,
        // Gone with the changeset of a first attempt: the same key answers it again.
        None if !simulated => {
            let key = idempotency_key(&headers)?.ok_or_else(not_found)?;
            let changeset = drafts::submission_of(&api.pool, principal.identity, &key, id)
                .await?
                .ok_or_else(not_found)?;
            return replayed(&api, changeset, &Answer::SUBMISSION).await;
        }
        _ => return Err(not_found()),
    };
    if headers.contains_key(header::IF_MATCH) && if_match(&headers)? != draft.summary.version {
        return Err(refused(DraftError::Stale));
    }
    let mut request = request(&api, &principal, &draft, (&headers, &raw), simulated).await?;
    request.apply_own = context.apply_own;
    let mut context = context.clone();
    context.draft = (!simulated).then_some(DraftOrigin {
        id,
        version: draft.summary.version,
    });
    submit_answered(&api, &context, &request, &Answer::SUBMISSION).await
}

/// `POST /v1/drafts/{id}/takeover`: the draft of someone else becomes the caller's, under
/// `core.draft.admin` and `If-Match`; it leaves the drafts of its former author. Audited
/// with its former author (REQ-024-7a).
#[utoipa::path(
    post,
    path = "/drafts/{id}/takeover",
    operation_id = "take_over_draft",
    tag = "drafts",
    params(
        ("id" = Uuid, Path, description = "The draft."),
        ("If-Match" = String, Header, description = "The ETag of the draft read."),
    ),
    responses(
        (status = 200, description = "The draft, now the caller's; `ETag` its new version.", body = DraftDto),
        (status = 404, description = "Absent, or someone else's without `core.draft.admin`.", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "The draft is the caller's already.", body = Problem, content_type = "application/problem+json"),
        (status = 412, description = "Modified since.", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "The caller keeps 50 drafts already (`draft-limit`).", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn takeover(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    person(&principal)?;
    let version = if_match(&headers)?;
    let draft = found(&api, &principal, id, true).await?;
    let author = draft.summary.author;
    if author == principal.identity {
        return Err(Problem::new(ProblemKind::Conflict).detail("the draft is yours already"));
    }
    let audit = context
        .event(&DRAFT_TAKEN_OVER, Outcome::Success)
        .map_err(audited)?
        .target("draft", [id])
        .detail("author", author.to_string())
        .map_err(audited)?;
    let taken = drafts::take_over(
        &api.pool,
        id,
        (author, version),
        principal.identity,
        &audit,
        api.journal.instance(),
    )
    .await
    .map_err(refused)?;
    Ok(answered(&taken, StatusCode::OK))
}

/// `GET /v1/identities/{id}/drafts`: the drafts of an identity, under `core.draft.admin`
/// for someone else's, the listing audited with that identity (REQ-024-7a).
#[utoipa::path(
    get,
    path = "/identities/{id}/drafts",
    operation_id = "list_identity_drafts",
    tag = "drafts",
    params(("id" = Uuid, Path, description = "The identity.")),
    responses(
        (status = 200, description = "Its drafts, the most recently modified first.", body = ListDto),
        (status = 403, description = "Someone else's without `core.draft.admin`.", body = Problem, content_type = "application/problem+json"),
    )
)]
async fn of_identity(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(identity): Path<Uuid>,
) -> Result<Response, Problem> {
    person(&principal)?;
    if identity != principal.identity {
        drafts_administrable(&api.authorizer(), &api.journal, &context).await?;
    }
    let found = drafts::list(&api.pool, identity).await?;
    if identity != principal.identity {
        let read = context
            .event(&DRAFT_ADMIN_READ, Outcome::Success)
            .map_err(audited)?
            .target("draft", found.iter().map(|d| d.id))
            .detail("author", identity.to_string())
            .map_err(audited)?
            .detail("count", found.len())
            .map_err(audited)?;
        api.journal.record(&read).await.map_err(audited)?;
    }
    Ok(Json(ListDto {
        items: found.iter().map(summarised).collect(),
    })
    .into_response())
}
