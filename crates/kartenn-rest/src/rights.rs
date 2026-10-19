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
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use kartenn_auth::secrets;
use kartenn_core::audit::rfc3339;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::operations::{BindingScope, BindingSubject, Ref};
use kartenn_service::reads::{objects_read, rbac_readable};
use kartenn_service::{ChangesetRequest, Operation, RequestError};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::rbac_read::{
    self, AccountRead, AttachmentRead, BindingRead, IdentityRead, RoleRead,
};
use kartenn_store_pg::read;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::changesets::{Answer, Location, Pointers, idempotency_key, period, submit_answered};
use crate::compression::Secret;
use crate::cursor::{Page, next_href, respond_page};
use crate::dated::{Dates, Moment};
use crate::objects::{Api, audited, bad, instant, store};
use crate::problem::{Problem, ProblemKind};
use crate::resources::{MERGE_PATCH, Reserved, WHOLE, invalid, members, reserved, typed};

/// The routes of the rights.
pub fn routes(api: Api) -> Router {
    Router::new()
        .route("/roles", get(roles))
        .route("/roles/{key}", get(role).put(set_role).delete(close_role))
        .route("/bindings", get(bindings).post(bind))
        .route("/bindings/{id}", get(binding).delete(unbind))
        .route(
            "/issuer-attachments",
            get(attachments).post(attach).delete(detach),
        )
        .route("/service-accounts", get(accounts).post(create_account))
        .route("/service-accounts/{id}", get(account))
        .route("/service-accounts/{id}/secrets", post(add_secret))
        .route(
            "/service-accounts/{id}/secrets/{secret}",
            axum::routing::delete(revoke_secret),
        )
        .route("/identities", get(identities))
        .route("/identities/{id}", get(identity).patch(identity_state))
        .with_state(api)
}

fn time(at: OffsetDateTime) -> String {
    rfc3339(instant(at))
}

/// 403 and its audit without `core.rbac.read`.
async fn readable(api: &Api, context: &RequestContext) -> Result<(), Problem> {
    rbac_readable(&PgAuthorizer::new(api.pool.clone()), &api.journal, context).await?;
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
async fn served<T>(
    api: &Api,
    context: &RequestContext,
    headers: &HeaderMap,
    (page, moment): (&Page, &Moment),
    name: &str,
    mut found: Vec<T>,
    key: impl Fn(&T) -> Value,
    (id, item): (impl Fn(&T) -> Option<Uuid>, impl Fn(&T) -> Value),
) -> Result<Response, Problem> {
    let size = usize::from(page.size);
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next_current(&api.ring, name, moment, key(&found[size - 1]));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let ids: Vec<Uuid> = found.iter().filter_map(&id).collect();
    audit_read(api, context, &ids, json!({ name: {} })).await?;
    let items: Vec<Value> = found.iter().map(item).collect();
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
        precondition: false,
        shown,
    }
}

/// The body of a write as a JSON object (an empty body is an empty object when
/// `optional`), its reserved members taken out.
fn body(
    headers: &HeaderMap,
    raw: &[u8],
    media_type: &str,
    optional: bool,
) -> Result<(Map<String, Value>, Reserved), Problem> {
    let mut members = if optional && raw.is_empty() {
        Map::new()
    } else {
        members(headers, raw, media_type)?
    };
    let reserved = reserved(&mut members)?;
    Ok((members, reserved))
}

/// The member `name` of `members` as `T`, or 400 when it is required and absent.
fn member<T: DeserializeOwned>(
    members: &mut Map<String, Value>,
    name: &str,
) -> Result<Option<T>, Problem> {
    members
        .remove(name)
        .filter(|v| !v.is_null())
        .map(|v| typed(name, v))
        .transpose()
}

fn required<T: DeserializeOwned>(
    members: &mut Map<String, Value>,
    name: &str,
) -> Result<T, Problem> {
    member(members, name)?
        .ok_or_else(|| invalid(&format!("/{name}"), format!("{name} is required")))
}

/// 400 for the first member left unread.
fn no_other(members: &Map<String, Value>) -> Result<(), Problem> {
    match members.keys().next() {
        Some(unknown) => Err(bad(format!("unknown member {unknown}"))),
        None => Ok(()),
    }
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

// ---- Roles ----

fn role_href(key: &str) -> String {
    format!("/v1/roles/{key}")
}

fn role_item(role: &RoleRead) -> Value {
    json!({
        "key": role.key,
        "href": role_href(&role.key),
        "label": role.label,
        "builtin": role.builtin,
        "entries": role.entries,
    })
}

/// `GET /v1/roles`.
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

/// `PUT /v1/roles/{key}` `{label, entries}`: defines or replaces the role; 201 when it is
/// created.
async fn set_role(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(key): Path<String>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (mut members, reserved) = body(&headers, &raw, "application/json", false)?;
    let label: String = required(&mut members, "label")?;
    let entries: Vec<String> = required(&mut members, "entries")?;
    no_other(&members)?;
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
        label,
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
async fn close_role(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(key): Path<String>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (members, reserved) = closure(&headers, &raw)?;
    no_other(&members)?;
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

/// The optional body of a closure: `{motive, valid_from}`, no `valid_to`.
fn closure(headers: &HeaderMap, raw: &[u8]) -> Result<(Map<String, Value>, Reserved), Problem> {
    let (members, reserved) = body(headers, raw, "application/json", true)?;
    if reserved.valid_to.is_some() {
        return Err(bad("a closure has no valid_to"));
    }
    Ok((members, reserved))
}

// ---- Bindings ----

/// The scope of a binding as served and received: `{kind, id}`, no `id` when global.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scope {
    kind: String,
    #[serde(default)]
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

fn binding_item(binding: &BindingRead) -> Value {
    json!({
        "id": binding.id,
        "href": binding_href(binding.id),
        "subject": subject(binding),
        "role": binding.role,
        "scope": scope(binding),
        "valid_from": time(binding.valid_from),
        "valid_to": binding.valid_to.map(time),
    })
}

fn subject(binding: &BindingRead) -> Value {
    match binding.subject_kind.as_str() {
        "identity" => json!({ "kind": "identity", "id": binding.subject_id }),
        "group" => {
            json!({ "kind": "group", "issuer": binding.issuer, "group": binding.group_name })
        }
        kind => json!({ "kind": kind, "issuer": binding.issuer }),
    }
}

fn scope(binding: &BindingRead) -> Value {
    match binding.scope_id {
        Some(id) => json!({ "kind": binding.scope_kind, "id": id }),
        None => json!({ "kind": binding.scope_kind }),
    }
}

/// `GET /v1/bindings`.
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

/// `POST /v1/bindings` `{subject, role, scope}`: 201 and `Location`.
async fn bind(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let (mut members, reserved) = body(&headers, &raw, "application/json", false)?;
    let subject: BindingSubject = required(&mut members, "subject")?;
    let role: String = required(&mut members, "role")?;
    let scope: Scope = required(&mut members, "scope")?;
    no_other(&members)?;
    let operation = Operation::Bind {
        local: None,
        subject,
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
async fn unbind(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (members, reserved) = closure(&headers, &raw)?;
    no_other(&members)?;
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

fn attachment_item(attachment: &AttachmentRead) -> Value {
    json!({ "issuer": attachment.issuer, "entity": attachment.entity })
}

/// `GET /v1/issuer-attachments`.
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

/// `POST /v1/issuer-attachments` `{issuer, entity}`: 201.
async fn attach(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let (mut members, reserved) = body(&headers, &raw, "application/json", false)?;
    let issuer: String = required(&mut members, "issuer")?;
    let entity: Option<Uuid> = member(&mut members, "entity")?;
    no_other(&members)?;
    let operation = Operation::AttachIssuer {
        issuer,
        entity: entity.map(Ref::Id),
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
            other => return Err(bad(format!("unknown parameter {other}"))),
        }
    }
    Ok((issuer.ok_or_else(|| bad("issuer is required"))?, entity))
}

/// `DELETE /v1/issuer-attachments?issuer=…[&entity=…]`: detaches the issuer from the
/// entity, or its global attachment without `entity`.
async fn detach(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (issuer, entity) = attachment_named(query.as_deref())?;
    let (members, reserved) = closure(&headers, &raw)?;
    no_other(&members)?;
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

fn account_item(account: &AccountRead) -> Value {
    let secrets: Vec<Value> = account
        .secrets
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "key_id": s.key_id,
                "created_at": time(s.created_at),
                "expires_at": s.expires_at.map(time),
                "revoked": s.revoked,
            })
        })
        .collect();
    json!({
        "id": account.id,
        "href": account_href(account.id),
        "name": account.name,
        "disabled": account.disabled,
        "secrets": secrets,
    })
}

/// `GET /v1/service-accounts`.
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

/// A new secret valid until the member `expires_at` of `members`, if given, and what
/// the response shows of it.
fn new_secret(
    api: &Api,
    members: &mut Map<String, Value>,
) -> Result<(secrets::NewSecret, Value), Problem> {
    let expires: Option<String> = member(members, "expires_at")?;
    let expires_at = expires
        .as_deref()
        .map(|v| micros("expires_at", v))
        .transpose()?;
    let secret = secrets::generate(&api.ring, expires_at);
    let shown = json!({
        "id": secret.record.id,
        "value": secret.plaintext,
        "expires_at": expires,
    });
    Ok((secret, shown))
}

/// `POST /v1/service-accounts` `{name, expires_at}`: 201, `Location`, and the first secret
/// shown once.
async fn create_account(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let (mut members, reserved) = body(&headers, &raw, "application/json", false)?;
    let name: String = required(&mut members, "name")?;
    let (secret, shown) = new_secret(&api, &mut members)?;
    no_other(&members)?;
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

/// `POST /v1/service-accounts/{id}/secrets` `{expires_at}`: 201 and the secret shown once.
async fn add_secret(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let (mut members, reserved) = body(&headers, &raw, "application/json", true)?;
    let (secret, shown) = new_secret(&api, &mut members)?;
    no_other(&members)?;
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
async fn revoke_secret(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path((id, secret)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (members, reserved) = closure(&headers, &raw)?;
    no_other(&members)?;
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

pub(crate) fn identity_item(identity: &IdentityRead) -> Value {
    json!({
        "id": identity.id,
        "href": identity_href(identity.id),
        "kind": identity.kind,
        "name": identity.name,
        "issuer": identity.issuer,
        "disabled": identity.disabled,
        "revoked_after": identity.revoked_after.map(time),
    })
}

/// `GET /v1/identities`.
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

/// `PATCH /v1/identities/{id}` (JSON Merge Patch) `{disabled, revoke_sessions}`.
async fn identity_state(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let (mut members, reserved) = body(&headers, &raw, MERGE_PATCH, false)?;
    let disabled: Option<bool> = member(&mut members, "disabled")?;
    let revoke_sessions: Option<bool> = member(&mut members, "revoke_sessions")?;
    no_other(&members)?;
    let pointers = pointers();
    let answer = answer(&pointers, StatusCode::OK, None, None);
    let operation = Operation::SetIdentityState {
        identity: id,
        disabled,
        revoke_sessions: revoke_sessions.unwrap_or(false),
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
