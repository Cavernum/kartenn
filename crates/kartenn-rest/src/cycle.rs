//! The cycle of a changeset (spec 006, REQ-006-16; task 006-17; design « Cycle d'un
//! changeset »): its read, the list filtered by state and author, its differential by
//! pages, the proposals the caller may validate, and the actions validate, reject,
//! withdraw and undo.
//!
//! The reads need `core.changeset.read`, global; the values of a sensitive property are
//! masked without `prop.<key>.read` on their object (REQ-005-21). An action needs
//! `If-Match` with the state of the changeset, its strong `ETag`. Undoing a changeset
//! submits its inverse (REQ-004-34), unless a later changeset wrote over it (409 with the
//! conflicts, REQ-004-35).

use std::collections::{BTreeMap, BTreeSet};

use axum::Extension;
use axum::body::{Body, Bytes};
use axum::extract::{Path, RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use kartenn_core::audit::rfc3339;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::port::{DiffItem, RbacRow};
use kartenn_service::reads::{changesets_readable, objects_read, sensitive_disclosed};
use kartenn_service::review::{proposals_for, reject, validate_pending, withdraw};
use kartenn_service::undo::inverse;
use kartenn_service::{Operation, Ref, RequestError};
use kartenn_store_pg::audit::diff_page;
use kartenn_store_pg::changesets::{
    self, ChangesetFilter, ChangesetRead, ChangesetSummary, STATES,
};
use kartenn_store_pg::idempotency;
use kartenn_store_pg::property_read;
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read;
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::anomalies::motive;
use crate::changesets::{
    Answer, Pointers, answered, cascaded, href, linked, simulate, submit_answered, summary,
    violations, write_store,
};
use crate::cursor::{Page, next_href, respond_page};
use crate::dated::{Dates, Moment};
use crate::objects::{Api, audited, bad, instant, store};
use crate::problem::{Problem, ProblemKind};
use crate::resources::{entity_tag, stale};

/// How a validation is answered: 200 when applied, no `Location`.
const VALIDATION: Answer<'static> = Answer {
    pointers: Pointers::Operations,
    applied: axum::http::StatusCode::OK,
    location: None,
    precondition: false,
    shown: None,
};

fn time(at: OffsetDateTime) -> String {
    rfc3339(instant(at))
}

fn internal(e: impl std::fmt::Display) -> Problem {
    tracing::error!(name: "http.changeset.serialise", error = %e, "a changeset does not serialise");
    Problem::new(ProblemKind::Internal)
}

/// 403 and its audit without `core.changeset.read`.
async fn readable(api: &Api, context: &RequestContext) -> Result<(), Problem> {
    changesets_readable(&PgAuthorizer::new(api.pool.clone()), &api.journal, context).await?;
    Ok(())
}

/// Records the read of `ids` (`data.objects.read`, target `changeset`).
async fn audit_read(
    api: &Api,
    context: &RequestContext,
    ids: &[Uuid],
    criteria: Value,
) -> Result<(), Problem> {
    let event = objects_read(context, "changeset", ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(())
}

/// A changeset as listed.
fn listed(found: &ChangesetSummary) -> Value {
    json!({
        "id": found.id,
        "href": href(found.id),
        "state": found.state,
        "kind": found.kind,
        "author": found.author,
        "motive": found.motive,
        "submitted_at": time(found.submitted_at),
    })
}

/// The moment of a list read at the current state; `valid_at` or `as_of` on its first
/// page → 400, a changeset being no dated object.
async fn current(api: &Api, page: &Page) -> Result<Moment, Problem> {
    if page.after().is_none() && page.dates != Dates::default() {
        return Err(bad("a list of changesets takes neither valid_at nor as_of"));
    }
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    Dates::default().at(now, api.bound)
}

/// The key `(submitted_at, id)` a page starts after, as written by [`key`].
fn after_key(page: &Page) -> Result<Option<(OffsetDateTime, Uuid)>, Problem> {
    page.after()
        .map(|key| {
            let (at, id): (String, Uuid) = serde_json::from_value(key.clone()).ok()?;
            Some((OffsetDateTime::parse(&at, &Rfc3339).ok()?, id))
        })
        .map(|key| key.ok_or_else(|| Problem::new(ProblemKind::InvalidCursor)))
        .transpose()
}

fn key(found: &ChangesetSummary) -> Value {
    json!([
        found.submitted_at.format(&Rfc3339).unwrap_or_default(),
        found.id
    ])
}

/// The filter of the list: `state` (a state of the journal) and `author` (a UUID), each
/// at most once (the cursor sorts them by name).
fn filter(page: &Page) -> Result<(Option<String>, Option<Uuid>), Problem> {
    let (mut state, mut author) = (None, None);
    for (name, value) in &page.filters {
        match name.as_str() {
            "state" if state.is_none() => {
                if !STATES.contains(&value.as_str()) {
                    return Err(bad(format!("state must be one of {}", STATES.join(", "))));
                }
                state = Some(value.clone());
            }
            "author" if author.is_none() => {
                author = Some(value.parse().map_err(|_| bad("author must be a UUID"))?);
            }
            "state" | "author" => return Err(bad(format!("{name} is given more than once"))),
            other => return Err(bad(format!("unknown parameter {other}"))),
        }
    }
    Ok((state, author))
}

/// `GET /v1/changesets?state=…&author=…`: the changesets, newest first.
pub(crate) async fn list(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let name = "changesets";
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, name)?;
    let (state, author) = filter(&page)?;
    let after = after_key(&page)?;
    let moment = current(&api, &page).await?;
    let size = usize::from(page.size);
    let mut found = changesets::page(
        &api.pool,
        ChangesetFilter {
            state: state.as_deref(),
            author,
        },
        after,
        i64::from(page.size) + 1,
    )
    .await?;
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next_current(&api.ring, name, &moment, key(&found[size - 1]));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let ids: Vec<Uuid> = found.iter().map(|c| c.id).collect();
    let criteria = json!({ "changesets": { "state": state, "author": author } });
    audit_read(&api, &context, &ids, criteria).await?;
    let items: Vec<Value> = found.iter().map(listed).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// Among `wanted` (objects by sensitive key), the `(key, object)` pairs whose value the
/// caller may see (and audited as such).
async fn disclosed(
    api: &Api,
    context: &RequestContext,
    wanted: BTreeMap<String, Vec<Uuid>>,
) -> Result<BTreeSet<(String, Uuid)>, Problem> {
    let authorizer = PgAuthorizer::new(api.pool.clone());
    let mut out = BTreeSet::new();
    for (key, objects) in wanted {
        for object in
            sensitive_disclosed(&authorizer, &api.journal, context, &key, &objects).await?
        {
            out.insert((key.clone(), object));
        }
    }
    Ok(out)
}

/// The counted differential of changeset `id` as served: an entry of a sensitive property
/// keeps its values only if the caller may see them on every object it counts.
async fn counts(
    api: &Api,
    context: &RequestContext,
    id: Uuid,
    mut counted: Value,
) -> Result<Value, Problem> {
    let sensitive = property_read::sensitive_keys(&api.pool)
        .await
        .map_err(|e| store(&e))?;
    let Some(entries) = counted.as_array_mut() else {
        return Ok(counted);
    };
    let key = |entry: &Value| entry["property"].as_str().map(str::to_owned);
    let keys: Vec<String> = entries
        .iter()
        .filter_map(key)
        .filter(|k| sensitive.contains(k))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if keys.is_empty() {
        return Ok(counted);
    }
    let objects = changesets::diff_objects(&api.pool, id, &keys).await?;
    let shown = disclosed(api, context, objects.clone()).await?;
    let open = |k: &str| {
        objects
            .get(k)
            .is_some_and(|all| all.iter().all(|o| shown.contains(&(k.to_owned(), *o))))
    };
    for entry in entries.iter_mut() {
        if let Some(k) = key(entry).filter(|k| sensitive.contains(k))
            && !open(&k)
        {
            entry["before"] = Value::Null;
            entry["after"] = Value::Null;
            entry["masked"] = Value::Bool(true);
        }
    }
    Ok(counted)
}

/// The operations as served: the values of a sensitive property masked without
/// `prop.<key>.read` on the object (always for a local property or object), the digest of
/// a secret never served.
async fn operations(
    api: &Api,
    context: &RequestContext,
    found: &[Operation],
) -> Result<Vec<Value>, Problem> {
    let sensitive = property_read::sensitive_ids(&api.pool)
        .await
        .map_err(|e| store(&e))?;
    // The sensitive key an assignment carries, and its object when it is known.
    let carried = |op: &Operation| match op {
        Operation::AssignProperty {
            object, property, ..
        } => match property {
            Ref::Local(_) => Some((None, None)),
            Ref::Id(id) => sensitive.get(id).map(|key| {
                let object = match object {
                    Ref::Id(object) => Some(*object),
                    Ref::Local(_) => None,
                };
                (Some(key.clone()), object)
            }),
        },
        _ => None,
    };
    let mut wanted: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    for op in found {
        if let Some((Some(key), Some(object))) = carried(op) {
            wanted.entry(key).or_default().push(object);
        }
    }
    let shown = disclosed(api, context, wanted).await?;
    found
        .iter()
        .map(|op| {
            let mut served = serde_json::to_value(op).map_err(internal)?;
            if let Some(secret) = served.get_mut("secret").and_then(Value::as_object_mut) {
                secret.remove("digest");
            }
            let masked = match carried(op) {
                None => false,
                Some((Some(key), Some(object))) => !shown.contains(&(key, object)),
                Some(_) => true,
            };
            if masked {
                served["values"] = Value::Null;
                served["masked"] = Value::Bool(true);
            }
            Ok(served)
        })
        .collect()
}

/// A changeset as read.
async fn represent(
    api: &Api,
    context: &RequestContext,
    found: &ChangesetRead,
) -> Result<Value, Problem> {
    let s = &found.summary;
    Ok(json!({
        "id": s.id,
        "state": s.state,
        "kind": s.kind,
        "author": s.author,
        "validator": found.validator,
        "motive": s.motive,
        "valid_from": time(found.valid_from),
        "valid_to": found.valid_to.map(time),
        "submitted_at": time(s.submitted_at),
        "applied_at": found.applied_at.map(time),
        "expires_at": found.expires_at.map(time),
        "policy": found.policy,
        "rejection": found.rejection,
        "operations": operations(api, context, &found.operations).await?,
        "violations": violations(api, &found.violations, Pointers::Operations).await?,
        "diff": counts(api, context, s.id, found.diff.clone()).await?,
        "cascaded": cascaded(&found.cascaded),
        "links": { "diff": format!("{}/diff", href(s.id)) },
    }))
}

/// The strong `ETag` of a changeset: its state.
fn etag(state: &str) -> Result<HeaderValue, Problem> {
    HeaderValue::try_from(format!("\"{state}\"")).map_err(internal)
}

/// `GET /v1/changesets/{id}`: state, operations, result and counted differential.
pub(crate) async fn show(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let found = changesets::find(&api.pool, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let body = represent(&api, &context, &found).await?;
    audit_read(&api, &context, &[id], json!({ "changeset": id })).await?;
    let mut response = axum::Json(body).into_response();
    response
        .headers_mut()
        .insert(header::ETAG, etag(&found.summary.state)?);
    Ok(response)
}

/// A differential entry as served, its values masked unless `shown`.
fn diff_entry(item: &DiffItem, shown: bool) -> Result<Value, Problem> {
    let value = |text: &Option<String>| -> Result<Value, Problem> {
        match text {
            Some(text) if shown => serde_json::from_str(text).map_err(internal),
            _ => Ok(Value::Null),
        }
    };
    Ok(json!({
        "property": item.property,
        "object": { "id": item.object },
        "before": value(&item.before)?,
        "after": value(&item.after)?,
        "masked": !shown,
    }))
}

/// `GET /v1/changesets/{id}/diff?property=…`: the counted differential and its detail by
/// pages, in the order of the journal.
pub(crate) async fn diff(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let name = format!("changesets/{id}/diff");
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &name)?;
    let mut property = None;
    for (key, value) in &page.filters {
        match key.as_str() {
            "property" if property.is_none() => property = Some(value.clone()),
            "property" => return Err(bad("property is given more than once")),
            other => return Err(bad(format!("unknown parameter {other}"))),
        }
    }
    let after = page
        .after()
        .map(|seq| {
            seq.as_i64()
                .ok_or_else(|| Problem::new(ProblemKind::InvalidCursor))
        })
        .transpose()?;
    let moment = current(&api, &page).await?;
    let journal = idempotency::journaled(&api.pool, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let size = usize::from(page.size);
    let mut found = diff_page(
        &api.pool,
        id,
        property.as_deref(),
        after,
        i64::from(page.size) + 1,
    )
    .await
    .map_err(|e| store(&e))?;
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next_current(&api.ring, &name, &moment, json!(found[size - 1].0));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let sensitive = property_read::sensitive_keys(&api.pool)
        .await
        .map_err(|e| store(&e))?;
    let mut wanted: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    for (_, item) in &found {
        if sensitive.contains(&item.property) {
            wanted
                .entry(item.property.clone())
                .or_default()
                .push(item.object);
        }
    }
    let shown = disclosed(&api, &context, wanted).await?;
    let items = found
        .iter()
        .map(|(_, item)| {
            diff_entry(
                item,
                !sensitive.contains(&item.property)
                    || shown.contains(&(item.property.clone(), item.object)),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    audit_read(
        &api,
        &context,
        &[id],
        json!({ "changeset": id, "diff": { "property": property } }),
    )
    .await?;
    let mut response = axum::Json(json!({
        "counts": counts(&api, &context, id, journal.diff).await?,
        "items": items,
        "links": { "next": next },
    }))
    .into_response();
    if let Some(next) = &next {
        response.headers_mut().insert(
            header::LINK,
            HeaderValue::try_from(format!("<{next}>; rel=\"next\"")).map_err(internal)?,
        );
    }
    Ok(response)
}

/// `GET /v1/proposals`: the pending proposals the caller may validate (REQ-004-43), oldest
/// first.
pub(crate) async fn proposals(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = "proposals";
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, name)?;
    page.unfiltered()?;
    let after = after_key(&page)?;
    let moment = current(&api, &page).await?;
    let pending = proposals_for(
        &write_store(&api),
        &PgAuthorizer::new(api.pool.clone()),
        &principal.subject(),
    )
    .await?;
    let ids: Vec<Uuid> = pending.iter().map(|p| p.id).collect();
    let mut found = changesets::summaries(&api.pool, &ids).await?;
    found.sort_by_key(|c| (c.submitted_at, c.id));
    found.retain(|c| after.is_none_or(|after| (c.submitted_at, c.id) > after));
    let size = usize::from(page.size);
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next_current(&api.ring, name, &moment, key(&found[size - 1]));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let items: Vec<Value> = found
        .iter()
        .map(|c| {
            let mut item = listed(c);
            let required = pending
                .iter()
                .find(|p| p.id == c.id)
                .map(|p| &p.pending.required);
            item["required"] = required
                .into_iter()
                .flatten()
                .map(
                    |(permission, objects)| json!({ "permission": permission, "objects": objects }),
                )
                .collect();
            item
        })
        .collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// The state of changeset `id` (404 when absent), which `If-Match` must name: absent or
/// `*` → 428, another state → 412.
async fn expected(api: &Api, id: Uuid, headers: &HeaderMap) -> Result<String, Problem> {
    let state = changesets::summaries(&api.pool, &[id])
        .await?
        .pop()
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?
        .state;
    let tag = entity_tag(
        headers,
        "read the changeset, then send its ETag in If-Match to act on it",
    )?;
    if tag != state {
        return Err(stale());
    }
    Ok(state)
}

/// 200 `{changeset, state}` and a link to the changeset.
fn decided(id: Uuid, state: &str) -> Response {
    linked(
        axum::Json(json!({ "changeset": summary(id), "state": state })).into_response(),
        id,
        None,
    )
}

/// `POST /v1/changesets/{id}/validation`: applies a pending changeset again on the current
/// state (REQ-004-21, 22, 42); 200, or 422 when it no longer holds.
pub(crate) async fn validation(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    expected(&api, id, &headers).await?;
    let outcome = validate_pending(
        &write_store(&api),
        &PgAuthorizer::new(api.pool.clone()),
        &api.sources,
        id,
        &principal.subject(),
        MAX_OPERATIONS,
    )
    .await?;
    answered(&api, outcome, &VALIDATION).await
}

/// `POST /v1/changesets/{id}/rejection` with `{"motive": "…"}` (REQ-004-23, 43).
pub(crate) async fn rejection(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Problem> {
    expected(&api, id, &headers).await?;
    let motive = motive(&headers, &body, "a rejection needs a motive")?;
    reject(
        &write_store(&api),
        &PgAuthorizer::new(api.pool.clone()),
        id,
        &principal.subject(),
        &motive,
    )
    .await?;
    Ok(decided(id, "rejected"))
}

/// `POST /v1/changesets/{id}/withdrawal`: by its author only (REQ-004-23).
pub(crate) async fn withdrawal(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    expected(&api, id, &headers).await?;
    withdraw(&write_store(&api), id, principal.identity).await?;
    Ok(decided(id, "withdrawn"))
}

fn rbac_row(row: &RbacRow) -> Value {
    match row {
        RbacRow::Role { key } => json!({ "role": key }),
        RbacRow::Binding { id } => json!({ "binding": id }),
        RbacRow::Attachment { issuer, entity } => {
            json!({ "attachment": { "issuer": issuer, "entity": entity } })
        }
    }
}

/// `response` with `members` added to its JSON body.
async fn with_members(
    response: Response,
    members: Map<String, Value>,
) -> Result<Response, Problem> {
    let (mut parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(internal)?;
    let mut value: Value = serde_json::from_slice(&bytes).map_err(internal)?;
    if let Some(object) = value.as_object_mut() {
        object.extend(members);
    }
    parts.headers.remove(header::CONTENT_LENGTH);
    let bytes = serde_json::to_vec(&value).map_err(internal)?;
    Ok(Response::from_parts(parts, Body::from(bytes)))
}

/// `POST /v1/changesets/{id}/undo[?simulate=true]`: submits the inverse of an applied
/// changeset under the rights of the caller (REQ-004-34), answered as a submission with
/// `undoes` and `unsupported`; 409 `undo-conflict` with the objects and rights written
/// since (REQ-004-35).
pub(crate) async fn undo(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let simulate = simulate(query.as_deref())?;
    readable(&api, &context).await?;
    expected(&api, id, &headers).await?;
    let mut found = inverse(&write_store(&api), id, principal.identity)
        .await?
        .ok_or_else(|| {
            Problem::new(ProblemKind::Conflict).detail(format!("changeset {id} is not applied"))
        })?;
    if !found.conflicts.is_empty() || !found.rbac_conflicts.is_empty() {
        let conflicts: Vec<Value> = found
            .conflicts
            .iter()
            .map(|c| json!({ "kind": c.kind, "id": c.id, "property": c.property, "by": c.by }))
            .chain(
                found
                    .rbac_conflicts
                    .iter()
                    .map(|c| json!({ "row": rbac_row(&c.row), "by": c.by })),
            )
            .collect();
        return Err(Problem::new(ProblemKind::UndoConflict)
            .detail(format!(
                "{} later write(s) would be overwritten by undoing changeset {id}",
                conflicts.len()
            ))
            .extension("conflicts", Value::Array(conflicts))
            .extension("changeset", summary(id)));
    }
    found.request.simulate = simulate;
    let response = submit_answered(&api, &context, &found.request, &Answer::SUBMISSION).await?;
    let mut members = Map::new();
    members.insert("undoes".into(), summary(id));
    members.insert(
        "unsupported".into(),
        found
            .unsupported
            .iter()
            .map(|(kind, id)| json!({ "kind": kind, "id": id }))
            .collect(),
    );
    if simulate {
        members.insert(
            "operations".into(),
            serde_json::to_value(&found.request.operations).map_err(internal)?,
        );
    }
    with_members(response, members).await
}
