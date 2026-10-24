//! The administration of the rights (spec 006, REQ-006-4, 32; task 006-18; design
//! « Droits »): roles, bindings, attachments of issuers, service accounts and their
//! secrets, state of the identities (`GET /v1/me` is in [`crate::auth`]).
//!
//! The reads need `core.rbac.read`, global, and are audited. Each write wraps a changeset
//! of one administration operation (005), submitted as the routes by resource do (006-16):
//! the checks without escalation are those of the pipeline. The server generates the
//! secret of a service account and shows it once, in a response never compressed nor
//! stored (`Secret`).

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use kartenn_auth::secrets;
use kartenn_core::audit::rfc3339;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::operations::{BindingScope, Ref};
use kartenn_service::reads::{objects_read, rbac_readable};
use kartenn_service::{ChangesetRequest, Operation, RequestError};
use kartenn_store_pg::rbac_read::{
    self, AccountRead, AttachmentRead, BindingRead, IdentityRead, RoleRead,
};
use kartenn_store_pg::read;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::changesets::{
    Answer, ChangesetApplied, ChangesetPending, Location, Pointers, idempotency_key, period,
    submit_answered,
};
use crate::compression::Secret;
use crate::cursor::{Listing, Page, PageParams, next_href, respond_page};
use crate::dated::{Dates, Moment};
use crate::objects::{Api, audited, bad, instant, store};
use crate::operation::SubjectDto;
use crate::problem::{Problem, ProblemKind};
use crate::resources::{
    Closure, MERGE_PATCH, Reserved, WHOLE, invalid, members, reserved, typed, written,
};

/// The routes of the rights.
pub fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new()
        .routes(routes!(roles))
        .routes(routes!(role, set_role, close_role))
        .routes(routes!(bindings, bind))
        .routes(routes!(binding, unbind))
        .routes(routes!(attachments, attach, detach))
        .routes(routes!(accounts, create_account))
        .routes(routes!(account))
        .routes(routes!(add_secret))
        .routes(routes!(revoke_secret))
        .routes(routes!(identities))
        .routes(routes!(identity, identity_state))
}

fn time(at: OffsetDateTime) -> String {
    rfc3339(instant(at))
}

/// 403 and its audit without `core.rbac.read`.
async fn readable(api: &Api, context: &RequestContext) -> Result<(), Problem> {
    rbac_readable(&api.authorizer(), &api.journal, context).await?;
    Ok(())
}

/// Records the read of `ids` (`data.objects.read`, target `rbac`).
async fn audit_read(
    api: &Api,
    context: &RequestContext,
    ids: &[Uuid],
    criteria: Value,
) -> Result<(), Problem> {
    let event = objects_read(context, "rbac", ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(())
}

/// A page of the list `name`, read at the current state: no filter, and `valid_at` or
/// `as_of` on its first page → 400.
async fn page_of(api: &Api, query: Option<&str>, name: &str) -> Result<(Page, Moment), Problem> {
    let page = Page::from_query(query.unwrap_or_default(), &api.ring, name)?;
    page.unfiltered()?;
    if page.after().is_none() && page.dates != Dates::default() {
        return Err(bad(format!("the {name} take neither valid_at nor as_of")));
    }
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    Ok((page, Dates::default().at(now, api.bound)?))
}

/// The key a page starts after, as written in its cursor.
fn after<T: DeserializeOwned>(page: &Page) -> Result<Option<T>, Problem> {
    page.after()
        .map(|key| {
            serde_json::from_value(key.clone())
                .map_err(|_| Problem::new(ProblemKind::InvalidCursor))
        })
        .transpose()
}

/// Serves a page of `found` (one more than the page asks: the next exists), keyed by
/// `key`, after auditing the read of their `ids`.
#[allow(clippy::too_many_arguments)]
async fn served<T, I: Serialize>(
    api: &Api,
    context: &RequestContext,
    headers: &HeaderMap,
    (page, moment): (&Page, &Moment),
    name: &str,
    mut found: Vec<T>,
    key: impl Fn(&T) -> Value,
    (id, item): (impl Fn(&T) -> Option<Uuid>, impl Fn(&T) -> I),
) -> Result<Response, Problem> {
    let size = usize::from(page.size);
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next_current(&api.ring, name, moment, key(&found[size - 1]));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let ids: Vec<Uuid> = found.iter().filter_map(&id).collect();
    audit_read(api, context, &ids, json!({ name: {} })).await?;
    let items: Vec<I> = found.iter().map(item).collect();
    Ok(respond_page(headers, moment, &items, next.as_deref()))
}

fn limit(page: &Page) -> i64 {
    i64::from(page.size) + 1
}

/// The response of a write that shows a secret: never compressed, never stored.
fn secret_shown(response: Response) -> Response {
    (Secret, response).into_response()
}

/// Submits `operations` from the reserved members of the body, answered as `answer`.
async fn submit(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    (reserved, idempotency_key): (Reserved, Option<String>),
    operations: Vec<Operation>,
    answer: &Answer<'_>,
) -> Result<Response, Problem> {
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(
            api,
            reserved.valid_from.as_deref(),
            reserved.valid_to.as_deref(),
        )
        .await?,
        operations,
        simulate: false,
        idempotency_key,
        apply_own: context.apply_own,
    };
    submit_answered(api, context, &request, answer).await
}

const WHOLE_BODY: [&str; 1] = [WHOLE];

fn pointers() -> Vec<String> {
    WHOLE_BODY.iter().map(|p| (*p).to_owned()).collect()
}

/// The answer of a write by resource: `applied` once applied, `location` then.
fn answer<'a>(
    pointers: &'a [String],
    applied: StatusCode,
    location: Option<Location<'a>>,
    shown: Option<&'a Value>,
) -> Answer<'a> {
    Answer {
        pointers: Pointers::Members(pointers),
        applied,
        location,
        precondition: None,
        shown,
    }
}

/// The body of a write (an empty body is an empty object when `optional`): its reserved
/// members, and the others as `T`.
fn body<T: DeserializeOwned>(
    headers: &HeaderMap,
    raw: &[u8],
    media_type: &str,
    optional: bool,
) -> Result<(T, Reserved), Problem> {
    let mut members = if optional && raw.is_empty() {
        Map::new()
    } else {
        members(headers, raw, media_type)?
    };
    let reserved = reserved(&mut members)?;
    Ok((typed("the body", Value::Object(members))?, reserved))
}

/// A member the write requires, or 422 pointing at it.
fn required<T>(value: Option<T>, name: &str) -> Result<T, Problem> {
    value.ok_or_else(|| invalid(&format!("/{name}"), format!("{name} is required")))
}

/// An instant of RFC 3339 as microseconds since the epoch.
fn micros(name: &str, value: &str) -> Result<i64, Problem> {
    let at = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        invalid(
            &format!("/{name}"),
            format!("{name} is not an RFC 3339 instant"),
        )
    })?;
    Ok(instant(at).0)
}

/// A write with no member of its own.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct NoMembers {}

written!(RoleWrite, RoleMembers, "The body that defines a role.");
written!(BindWrite, BindMembers, "The body of a binding.");
written!(AttachWrite, AttachMembers, "The body of an attachment.");
written!(
    AccountWrite,
    AccountMembers,
    "The body of a service account."
);
written!(SecretWrite, SecretMembers, "The body of a new secret.");
written!(
    IdentityWrite,
    IdentityPatch,
    "The body that changes the state of an identity."
);

/// The optional body of a closure: `{motive, valid_from}`, no `valid_to`.
fn closure(headers: &HeaderMap, raw: &[u8]) -> Result<Reserved, Problem> {
    let (NoMembers {}, reserved) = body(headers, raw, "application/json", true)?;
    if reserved.valid_to.is_some() {
        return Err(bad("a closure has no valid_to"));
    }
    Ok(reserved)
}

// ---- Roles ----

fn role_href(key: &str) -> String {
    format!("/v1/roles/{key}")
}

/// A role.
#[derive(Debug, Serialize, ToSchema)]
struct RoleItem {
    /// Stable key.
    key: String,
    /// Its URL.
    href: String,
    /// Labels by language, `{<tag>: <text>}` (spec 018, R20): the dictionary
    /// `GET /v1/labels` serves them resolved.
    #[schema(value_type = std::collections::BTreeMap<String, String>)]
    labels: Value,
    /// Built in kartenn.
    builtin: bool,
    /// Its entries: permissions, `prop.<key>.<action>`, `prop.*.<action>`, `*`.
    entries: Vec<String>,
}

fn role_item(role: &RoleRead) -> RoleItem {
    RoleItem {
        key: role.key.clone(),
        href: role_href(&role.key),
        labels: role.labels.clone(),
        builtin: role.builtin,
        entries: role.entries.clone(),
    }
}

/// `GET /v1/roles`.
#[utoipa::path(
    get,
    path = "/roles",
    operation_id = "list_roles",
    tag = "rights",
    params(PageParams),
    responses((status = 200, description = "The roles.", body = Listing<RoleItem>))
)]
async fn roles(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let (page, moment) = page_of(&api, query.as_deref(), "roles").await?;
    let after: Option<String> = after(&page)?;
    let found = rbac_read::roles(&api.pool, after.as_deref(), limit(&page)).await?;
    served(
        &api,
        &context,
        &headers,
        (&page, &moment),
        "roles",
        found,
        |r| json!(r.key),
        (|r: &RoleRead| Some(r.id), role_item),
    )
    .await
}

/// `GET /v1/roles/{key}`.
#[utoipa::path(
    get,
    path = "/roles/{key}",
    operation_id = "show_role",
    tag = "rights",
    params(("key" = String, Path, description = "The role.")),
    responses((status = 200, description = "The role.", body = RoleItem))
)]
async fn role(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(key): Path<String>,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let found = rbac_read::role(&api.pool, &key)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    audit_read(&api, &context, &[found.id], json!({ "role": key })).await?;
    Ok(Json(role_item(&found)).into_response())
}

/// The members of a role.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct RoleMembers {
    /// Labels by language, `{<tag>: <text>}`: the default language of the instance at
    /// least (spec 018, REQ-018-9).
    #[schema(required = true, value_type = std::collections::BTreeMap<String, String>)]
    labels: Option<kartenn_service::operations::RawLabels>,
    /// Entries.
    #[schema(required = true, value_type = Vec<String>)]
    entries: Option<Vec<String>>,
}

/// `PUT /v1/roles/{key}` `{labels, entries}`: defines or replaces the role; 201 when it is
/// created.
#[utoipa::path(
    put,
    path = "/roles/{key}",
    operation_id = "set_role",
    tag = "rights",
    params(("key" = String, Path, description = "The role.")),
    request_body = RoleWrite,
    responses(
        (status = 200, description = "Replaced.", body = ChangesetApplied),
        (status = 201, description = "Created.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
    )
)]
async fn set_role(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(key): Path<String>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (members, reserved): (RoleMembers, _) = body(&headers, &raw, "application/json", false)?;
    let labels = required(members.labels, "labels")?;
    let entries = required(members.entries, "entries")?;
    let exists = rbac_read::role(&api.pool, &key).await?.is_some();
    let href = role_href(&key);
    let pointers = pointers();
    let answer = if exists {
        answer(&pointers, StatusCode::OK, None, None)
    } else {
        answer(
            &pointers,
            StatusCode::CREATED,
            Some(Location::Fixed(&href)),
            None,
        )
    };
    let operation = Operation::SetRole {
        key,
        labels,
        entries,
    };
    submit(
        &api,
        &principal,
        &context,
        (reserved, None),
        vec![operation],
        &answer,
    )
    .await
}

/// `DELETE /v1/roles/{key}`: closes the role and its bindings.
#[utoipa::path(
    delete,
    path = "/roles/{key}",
    operation_id = "close_role",
    tag = "rights",
    params(("key" = String, Path, description = "The role.")),
    request_body(content = Option<Closure>, description = "The motive and the start of the closure."),
    responses(
        (status = 200, description = "Closed.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
    )
)]
async fn close_role(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(key): Path<String>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let reserved = closure(&headers, &raw)?;
    let pointers = pointers();
    let answer = answer(&pointers, StatusCode::OK, None, None);
    let operation = Operation::CloseRole { key };
    submit(
        &api,
        &principal,
        &context,
        (reserved, None),
        vec![operation],
        &answer,
    )
    .await
}

// ---- Bindings ----

/// The scope of a binding as served and received: `{kind, id}`, no `id` when global.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Scope {
    /// `global`, `entity`, `site`, `domain` or `prefix`.
    kind: String,
    /// The object; absent when global.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<Uuid>,
}

impl TryFrom<Scope> for BindingScope {
    type Error = Problem;

    fn try_from(scope: Scope) -> Result<Self, Problem> {
        let wrong = || invalid("/scope", "a global scope has no id, any other one has one");
        match (scope.kind.as_str(), scope.id) {
            ("global", None) => Ok(Self::Global),
            ("entity", Some(id)) => Ok(Self::Entity { id: Ref::Id(id) }),
            ("site", Some(id)) => Ok(Self::Site { id: Ref::Id(id) }),
            ("domain", Some(id)) => Ok(Self::Domain { id: Ref::Id(id) }),
            ("prefix", Some(id)) => Ok(Self::Prefix { id: Ref::Id(id) }),
            ("global" | "entity" | "site" | "domain" | "prefix", _) => Err(wrong()),
            _ => Err(invalid(
                "/scope/kind",
                "the kind of a scope is global, entity, site, domain or prefix",
            )),
        }
    }
}

fn binding_href(id: Uuid) -> String {
    format!("/v1/bindings/{id}")
}

/// Who a binding serves, as served.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SubjectItem {
    /// One identity.
    Identity {
        /// Its identifier.
        id: Option<Uuid>,
    },
    /// The members of a group of an issuer.
    Group {
        /// Issuer.
        issuer: Option<String>,
        /// Group.
        group: Option<String>,
    },
    /// Every identity of an issuer.
    Issuer {
        /// Issuer.
        issuer: Option<String>,
    },
}

/// A binding.
#[derive(Debug, Serialize, ToSchema)]
struct BindingItem {
    /// Identifier.
    id: Uuid,
    /// Its URL.
    href: String,
    /// Who it serves.
    subject: SubjectItem,
    /// Role key.
    role: String,
    /// Where it acts.
    scope: Scope,
    /// Its start, RFC 3339.
    valid_from: String,
    /// Its end, RFC 3339; `null` while open.
    valid_to: Option<String>,
}

fn binding_item(binding: &BindingRead) -> BindingItem {
    BindingItem {
        id: binding.id,
        href: binding_href(binding.id),
        subject: subject(binding),
        role: binding.role.clone(),
        scope: Scope {
            kind: binding.scope_kind.clone(),
            id: binding.scope_id,
        },
        valid_from: time(binding.valid_from),
        valid_to: binding.valid_to.map(time),
    }
}

fn subject(binding: &BindingRead) -> SubjectItem {
    match binding.subject_kind.as_str() {
        "identity" => SubjectItem::Identity {
            id: binding.subject_id,
        },
        "group" => SubjectItem::Group {
            issuer: binding.issuer.clone(),
            group: binding.group_name.clone(),
        },
        _ => SubjectItem::Issuer {
            issuer: binding.issuer.clone(),
        },
    }
}

/// `GET /v1/bindings`.
#[utoipa::path(
    get,
    path = "/bindings",
    operation_id = "list_bindings",
    tag = "rights",
    params(PageParams),
    responses((status = 200, description = "The bindings.", body = Listing<BindingItem>))
)]
async fn bindings(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let (page, moment) = page_of(&api, query.as_deref(), "bindings").await?;
    let after: Option<Uuid> = after(&page)?;
    let found = rbac_read::bindings(&api.pool, after, limit(&page)).await?;
    served(
        &api,
        &context,
        &headers,
        (&page, &moment),
        "bindings",
        found,
        |b| json!(b.id),
        (|b: &BindingRead| Some(b.id), binding_item),
    )
    .await
}

/// `GET /v1/bindings/{id}`.
#[utoipa::path(
    get,
    path = "/bindings/{id}",
    operation_id = "show_binding",
    tag = "rights",
    params(("id" = Uuid, Path, description = "The binding.")),
    responses((status = 200, description = "The binding.", body = BindingItem))
)]
async fn binding(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let found = rbac_read::binding(&api.pool, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    audit_read(&api, &context, &[id], json!({ "binding": id })).await?;
    Ok(Json(binding_item(&found)).into_response())
}

/// The members of a binding.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct BindMembers {
    /// Who it serves.
    #[schema(required = true, value_type = SubjectDto)]
    subject: Option<SubjectDto>,
    /// Role key.
    #[schema(required = true, value_type = String)]
    role: Option<String>,
    /// Where it acts.
    #[schema(required = true, value_type = Scope)]
    scope: Option<Scope>,
}

/// `POST /v1/bindings` `{subject, role, scope}`: 201 and `Location`.
#[utoipa::path(
    post,
    path = "/bindings",
    operation_id = "bind",
    tag = "rights",
    params(("Idempotency-Key" = Option<String>, Header, description = "Makes the write idempotent for 24 h.")),
    request_body = BindWrite,
    responses(
        (status = 201, description = "Bound.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
    )
)]
async fn bind(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let (members, reserved): (BindMembers, _) = body(&headers, &raw, "application/json", false)?;
    let subject = required(members.subject, "subject")?;
    let role = required(members.role, "role")?;
    let scope = required(members.scope, "scope")?;
    let operation = Operation::Bind {
        local: None,
        subject: subject.into(),
        role,
        scope: scope.try_into()?,
    };
    let pointers = pointers();
    let answer = answer(
        &pointers,
        StatusCode::CREATED,
        Some(Location::Binding),
        None,
    );
    submit(
        &api,
        &principal,
        &context,
        (reserved, idempotency_key),
        vec![operation],
        &answer,
    )
    .await
}

/// `DELETE /v1/bindings/{id}`: ends the binding.
#[utoipa::path(
    delete,
    path = "/bindings/{id}",
    operation_id = "unbind",
    tag = "rights",
    params(("id" = Uuid, Path, description = "The binding.")),
    request_body(content = Option<Closure>, description = "The motive and the start of the closure."),
    responses(
        (status = 200, description = "Ended.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
    )
)]
async fn unbind(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let reserved = closure(&headers, &raw)?;
    let pointers = pointers();
    let answer = answer(&pointers, StatusCode::OK, None, None);
    let operation = Operation::Unbind { id };
    submit(
        &api,
        &principal,
        &context,
        (reserved, None),
        vec![operation],
        &answer,
    )
    .await
}

// ---- Attachments of issuers ----

/// The attachment of an issuer.
#[derive(Debug, Serialize, ToSchema)]
struct AttachmentItem {
    /// Issuer identifier (`iss`).
    issuer: String,
    /// The entity; `null`: everywhere.
    entity: Option<Uuid>,
}

fn attachment_item(attachment: &AttachmentRead) -> AttachmentItem {
    AttachmentItem {
        issuer: attachment.issuer.clone(),
        entity: attachment.entity,
    }
}

/// `GET /v1/issuer-attachments`.
#[utoipa::path(
    get,
    path = "/issuer-attachments",
    operation_id = "list_issuer_attachments",
    tag = "rights",
    params(PageParams),
    responses((status = 200, description = "The attachments.", body = Listing<AttachmentItem>))
)]
async fn attachments(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let name = "issuer-attachments";
    let (page, moment) = page_of(&api, query.as_deref(), name).await?;
    let after: Option<(String, Option<Uuid>)> = after(&page)?;
    let found = rbac_read::attachments(
        &api.pool,
        after
            .as_ref()
            .map(|(issuer, entity)| (issuer.as_str(), *entity)),
        limit(&page),
    )
    .await?;
    served(
        &api,
        &context,
        &headers,
        (&page, &moment),
        name,
        found,
        |a| json!([a.issuer, a.entity]),
        (|_: &AttachmentRead| None, attachment_item),
    )
    .await
}

/// The members of an attachment.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct AttachMembers {
    /// Issuer identifier (`iss`).
    #[schema(required = true, value_type = String)]
    issuer: Option<String>,
    /// The entity; absent: everywhere.
    entity: Option<Uuid>,
}

/// `POST /v1/issuer-attachments` `{issuer, entity}`: 201.
#[utoipa::path(
    post,
    path = "/issuer-attachments",
    operation_id = "attach_issuer",
    tag = "rights",
    params(("Idempotency-Key" = Option<String>, Header, description = "Makes the write idempotent for 24 h.")),
    request_body = AttachWrite,
    responses(
        (status = 201, description = "Attached.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
    )
)]
async fn attach(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let (members, reserved): (AttachMembers, _) = body(&headers, &raw, "application/json", false)?;
    let issuer = required(members.issuer, "issuer")?;
    let operation = Operation::AttachIssuer {
        issuer,
        entity: members.entity.map(Ref::Id),
    };
    let pointers = pointers();
    let answer = answer(&pointers, StatusCode::CREATED, None, None);
    submit(
        &api,
        &principal,
        &context,
        (reserved, idempotency_key),
        vec![operation],
        &answer,
    )
    .await
}

/// `?issuer=…[&entity=…]`: the attachment a `DELETE` names.
fn attachment_named(query: Option<&str>) -> Result<(String, Option<Uuid>), Problem> {
    let (mut issuer, mut entity) = (None, None);
    for (name, value) in form_urlencoded::parse(query.unwrap_or_default().as_bytes()) {
        match name.as_ref() {
            "issuer" if issuer.is_none() => issuer = Some(value.into_owned()),
            "entity" if entity.is_none() => {
                entity = Some(value.parse().map_err(|_| bad("entity must be a UUID"))?);
            }
            "issuer" | "entity" => return Err(bad(format!("{name} is given more than once"))),
            // Read by the authentication (spec 024, REQ-024-13, 17).
            crate::changesets::APPLY | crate::changesets::SIMULATE => {}
            other => return Err(bad(format!("unknown parameter {other}"))),
        }
    }
    Ok((issuer.ok_or_else(|| bad("issuer is required"))?, entity))
}

/// `DELETE /v1/issuer-attachments?issuer=…[&entity=…]`: detaches the issuer from the
/// entity, or its global attachment without `entity`.
#[utoipa::path(
    delete,
    path = "/issuer-attachments",
    operation_id = "detach_issuer",
    tag = "rights",
    params(
        ("issuer" = String, Query, description = "The issuer."),
        ("entity" = Option<Uuid>, Query, description = "The entity; absent: the global attachment."),
    ),
    request_body(content = Option<Closure>, description = "The motive and the start of the closure."),
    responses(
        (status = 200, description = "Detached.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
    )
)]
async fn detach(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (issuer, entity) = attachment_named(query.as_deref())?;
    let reserved = closure(&headers, &raw)?;
    let pointers = pointers();
    let answer = answer(&pointers, StatusCode::OK, None, None);
    let operation = Operation::DetachIssuer {
        issuer,
        entity: entity.map(Ref::Id),
    };
    submit(
        &api,
        &principal,
        &context,
        (reserved, None),
        vec![operation],
        &answer,
    )
    .await
}

// ---- Service accounts ----

fn account_href(id: Uuid) -> String {
    format!("/v1/service-accounts/{id}")
}

/// A secret of a service account, never its value.
#[derive(Debug, Serialize, ToSchema)]
struct SecretItem {
    /// Identifier.
    id: Uuid,
    /// The key of the ring that made its digest.
    key_id: String,
    /// Its creation, RFC 3339.
    created_at: String,
    /// Its expiry, RFC 3339; `null`: none.
    expires_at: Option<String>,
    /// Revoked.
    revoked: bool,
}

/// A service account.
#[derive(Debug, Serialize, ToSchema)]
struct AccountItem {
    /// Identifier.
    id: Uuid,
    /// Its URL.
    href: String,
    /// Name, the Basic user.
    name: String,
    /// Disabled.
    disabled: bool,
    /// Its secrets.
    secrets: Vec<SecretItem>,
}

fn account_item(account: &AccountRead) -> AccountItem {
    AccountItem {
        id: account.id,
        href: account_href(account.id),
        name: account.name.clone(),
        disabled: account.disabled,
        secrets: account
            .secrets
            .iter()
            .map(|s| SecretItem {
                id: s.id,
                key_id: s.key_id.clone(),
                created_at: time(s.created_at),
                expires_at: s.expires_at.map(time),
                revoked: s.revoked,
            })
            .collect(),
    }
}

/// `GET /v1/service-accounts`.
#[utoipa::path(
    get,
    path = "/service-accounts",
    operation_id = "list_service_accounts",
    tag = "rights",
    params(PageParams),
    responses((status = 200, description = "The service accounts.", body = Listing<AccountItem>))
)]
async fn accounts(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let name = "service-accounts";
    let (page, moment) = page_of(&api, query.as_deref(), name).await?;
    let after: Option<Uuid> = after(&page)?;
    let found = rbac_read::accounts(&api.pool, after, limit(&page)).await?;
    served(
        &api,
        &context,
        &headers,
        (&page, &moment),
        name,
        found,
        |a| json!(a.id),
        (|a: &AccountRead| Some(a.id), account_item),
    )
    .await
}

/// `GET /v1/service-accounts/{id}`.
#[utoipa::path(
    get,
    path = "/service-accounts/{id}",
    operation_id = "show_service_account",
    tag = "rights",
    params(("id" = Uuid, Path, description = "The service account.")),
    responses((status = 200, description = "The service account.", body = AccountItem))
)]
async fn account(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let found = rbac_read::account(&api.pool, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    audit_read(&api, &context, &[id], json!({ "service_account": id })).await?;
    Ok(Json(account_item(&found)).into_response())
}

/// A secret generated by the server, shown once (design « Droits »).
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct SecretShown {
    /// Identifier.
    id: Uuid,
    /// The secret: the Basic password.
    value: String,
    /// Its expiry, RFC 3339, as asked; `null`: none.
    expires_at: Option<String>,
}

/// A new secret valid until `expires`, if given, and what the response shows of it.
fn new_secret(api: &Api, expires: Option<String>) -> Result<(secrets::NewSecret, Value), Problem> {
    let expires_at = expires
        .as_deref()
        .map(|v| micros("expires_at", v))
        .transpose()?;
    let secret = secrets::generate(&api.ring, expires_at);
    let shown = SecretShown {
        id: secret.record.id,
        value: secret.plaintext.clone(),
        expires_at: expires,
    };
    let shown = serde_json::to_value(shown).expect("a secret serialises");
    Ok((secret, shown))
}

/// The members of a service account.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct AccountMembers {
    /// Name, the Basic user (no `:`).
    #[schema(required = true, value_type = String)]
    name: Option<String>,
    /// The expiry of its first secret, RFC 3339; none by default.
    expires_at: Option<String>,
}

/// `POST /v1/service-accounts` `{name, expires_at}`: 201, `Location`, and the first secret
/// shown once.
#[utoipa::path(
    post,
    path = "/service-accounts",
    operation_id = "create_service_account",
    tag = "rights",
    params(("Idempotency-Key" = Option<String>, Header, description = "Makes the write idempotent for 24 h.")),
    request_body = AccountWrite,
    responses(
        (status = 201, description = "Created, its secret in `secret`.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision, its secret in `secret`.", body = ChangesetPending),
    )
)]
async fn create_account(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let (members, reserved): (AccountMembers, _) = body(&headers, &raw, "application/json", false)?;
    let name = required(members.name, "name")?;
    let (secret, shown) = new_secret(&api, members.expires_at)?;
    let pointers = pointers();
    let answer = answer(
        &pointers,
        StatusCode::CREATED,
        Some(Location::Account(secret.record.id)),
        Some(&shown),
    );
    let operation = Operation::CreateServiceAccount {
        local: None,
        name,
        secret: secret.record,
    };
    let response = submit(
        &api,
        &principal,
        &context,
        (reserved, idempotency_key),
        vec![operation],
        &answer,
    )
    .await?;
    Ok(secret_shown(response))
}

/// The members of a new secret.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct SecretMembers {
    /// Its expiry, RFC 3339; none by default.
    expires_at: Option<String>,
}

/// `POST /v1/service-accounts/{id}/secrets` `{expires_at}`: 201 and the secret shown once.
#[utoipa::path(
    post,
    path = "/service-accounts/{id}/secrets",
    operation_id = "add_service_secret",
    tag = "rights",
    params(
        ("id" = Uuid, Path, description = "The service account."),
        ("Idempotency-Key" = Option<String>, Header, description = "Makes the write idempotent for 24 h."),
    ),
    request_body(content = Option<SecretWrite>),
    responses(
        (status = 201, description = "Added, the secret in `secret`.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision, the secret in `secret`.", body = ChangesetPending),
    )
)]
async fn add_secret(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let (members, reserved): (SecretMembers, _) = body(&headers, &raw, "application/json", true)?;
    let (secret, shown) = new_secret(&api, members.expires_at)?;
    let href = account_href(id);
    let pointers = pointers();
    let answer = answer(
        &pointers,
        StatusCode::CREATED,
        Some(Location::Fixed(&href)),
        Some(&shown),
    );
    let operation = Operation::AddServiceSecret {
        account: id,
        secret: secret.record,
    };
    let response = submit(
        &api,
        &principal,
        &context,
        (reserved, idempotency_key),
        vec![operation],
        &answer,
    )
    .await?;
    Ok(secret_shown(response))
}

/// `DELETE /v1/service-accounts/{id}/secrets/{secret}`: revokes the secret.
#[utoipa::path(
    delete,
    path = "/service-accounts/{id}/secrets/{secret}",
    operation_id = "revoke_service_secret",
    tag = "rights",
    params(
        ("id" = Uuid, Path, description = "The service account."),
        ("secret" = Uuid, Path, description = "The secret."),
    ),
    request_body(content = Option<Closure>, description = "The motive and the start of the closure."),
    responses(
        (status = 200, description = "Revoked.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
    )
)]
async fn revoke_secret(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path((id, secret)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let reserved = closure(&headers, &raw)?;
    let pointers = pointers();
    let answer = answer(&pointers, StatusCode::OK, None, None);
    let operation = Operation::RevokeServiceSecret {
        account: id,
        secret,
    };
    submit(
        &api,
        &principal,
        &context,
        (reserved, None),
        vec![operation],
        &answer,
    )
    .await
}

// ---- Identities ----

fn identity_href(id: Uuid) -> String {
    format!("/v1/identities/{id}")
}

/// An identity and its state.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct IdentityItem {
    /// Identifier.
    id: Uuid,
    /// Its URL.
    href: String,
    /// `person`, `service_account` or `breakglass`.
    kind: String,
    /// Display name.
    name: String,
    /// Its issuer.
    issuer: Option<String>,
    /// Disabled.
    disabled: bool,
    /// The sessions opened before this instant are revoked, RFC 3339.
    revoked_after: Option<String>,
}

pub(crate) fn identity_item(identity: &IdentityRead) -> IdentityItem {
    IdentityItem {
        id: identity.id,
        href: identity_href(identity.id),
        kind: identity.kind.clone(),
        name: identity.name.clone(),
        issuer: identity.issuer.clone(),
        disabled: identity.disabled,
        revoked_after: identity.revoked_after.map(time),
    }
}

/// `GET /v1/identities`.
#[utoipa::path(
    get,
    path = "/identities",
    operation_id = "list_identities",
    tag = "rights",
    params(PageParams),
    responses((status = 200, description = "The identities.", body = Listing<IdentityItem>))
)]
async fn identities(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let (page, moment) = page_of(&api, query.as_deref(), "identities").await?;
    let after: Option<Uuid> = after(&page)?;
    let found = rbac_read::identities(&api.pool, after, limit(&page)).await?;
    served(
        &api,
        &context,
        &headers,
        (&page, &moment),
        "identities",
        found,
        |i| json!(i.id),
        (|i: &IdentityRead| Some(i.id), identity_item),
    )
    .await
}

/// `GET /v1/identities/{id}`.
#[utoipa::path(
    get,
    path = "/identities/{id}",
    operation_id = "show_identity",
    tag = "rights",
    params(("id" = Uuid, Path, description = "The identity.")),
    responses((status = 200, description = "The identity.", body = IdentityItem))
)]
async fn identity(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let found = rbac_read::identity(&api.pool, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    audit_read(&api, &context, &[id], json!({ "identity": id })).await?;
    Ok(Json(identity_item(&found)).into_response())
}

/// The state of an identity, as a JSON Merge Patch (RFC 7396).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct IdentityPatch {
    /// `true` disables, `false` enables.
    disabled: Option<bool>,
    /// Revokes the sessions opened until now.
    revoke_sessions: Option<bool>,
}

/// `PATCH /v1/identities/{id}` (JSON Merge Patch) `{disabled, revoke_sessions}`.
#[utoipa::path(
    patch,
    path = "/identities/{id}",
    operation_id = "set_identity_state",
    tag = "rights",
    params(("id" = Uuid, Path, description = "The identity.")),
    request_body(content = IdentityWrite, content_type = "application/merge-patch+json"),
    responses(
        (status = 200, description = "Changed.", body = ChangesetApplied),
        (status = 202, description = "Held for a decision.", body = ChangesetPending),
    )
)]
async fn identity_state(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (members, reserved): (IdentityPatch, _) = body(&headers, &raw, MERGE_PATCH, false)?;
    let pointers = pointers();
    let answer = answer(&pointers, StatusCode::OK, None, None);
    let operation = Operation::SetIdentityState {
        identity: id,
        disabled: members.disabled,
        revoke_sessions: members.revoke_sessions.unwrap_or(false),
    };
    submit(
        &api,
        &principal,
        &context,
        (reserved, None),
        vec![operation],
        &answer,
    )
    .await
}
