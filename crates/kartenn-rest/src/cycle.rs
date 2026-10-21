//! The cycle of a changeset (spec 006, REQ-006-16; task 006-17; design « Cycle d'un
//! changeset »): its read, the list filtered by state, author, object, period and
//! retroactivity (REQ-004-32), its differential by
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
use serde::Serialize;
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::anomalies::{Motive, motive};
use crate::changesets::{
    Answer, Cascaded, ChangesetApplied, ChangesetPending, ChangesetRef, ChangesetSimulated,
    DiffCountDto, Pointers, SectionRowDto, ViolationDto, answered, cascaded, href, linked,
    simulate, submit_answered, summary, violations, write_store,
};
use crate::cursor::{Listing, ListingLinks, Page, PageParams, next_href, respond_page};
use crate::dated::{Dates, Moment};
use crate::objects::{Api, audited, bad, instant, store};
use crate::operation::OperationDto;
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
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ChangesetItem {
    /// Identifier.
    id: Uuid,
    /// Its URL.
    href: String,
    /// A state of the journal.
    state: String,
    /// `direct`, `approval` or `proposal`.
    kind: String,
    /// Its author.
    author: Uuid,
    /// Why.
    motive: String,
    /// Its submission, RFC 3339.
    submitted_at: String,
}

/// A changeset as listed.
fn listed(found: &ChangesetSummary) -> ChangesetItem {
    ChangesetItem {
        id: found.id,
        href: href(found.id),
        state: found.state.clone(),
        kind: found.kind.clone(),
        author: found.author,
        motive: found.motive.clone(),
        submitted_at: time(found.submitted_at),
    }
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

/// The filter of the list, each criterion at most once (the cursor sorts them by name).
#[derive(Debug, Default)]
struct Criteria {
    state: Option<String>,
    author: Option<Uuid>,
    object: Option<Uuid>,
    from: Option<OffsetDateTime>,
    to: Option<OffsetDateTime>,
    retroactive: Option<bool>,
}

/// The filter of the list, as the contract names it.
#[derive(Debug, IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)]
struct CriteriaParams {
    /// A state of the journal.
    state: Option<String>,
    /// The author.
    author: Option<Uuid>,
    /// An object the changeset wrote.
    object: Option<Uuid>,
    /// The first instant of submission, RFC 3339 with an offset.
    from: Option<String>,
    /// The last instant of submission, RFC 3339 with an offset.
    to: Option<String>,
    /// Only the changesets that start before their submission, or only the others.
    retroactive: Option<bool>,
}

/// The filter of the list: `state` (a state of the journal), `author` and `object` (UUIDs),
/// `from` and `to` (RFC 3339 with an offset) and `retroactive` (`true` or `false`).
fn filter(page: &Page) -> Result<Criteria, Problem> {
    fn once<T>(slot: &mut Option<T>, name: &str, value: Result<T, Problem>) -> Result<(), Problem> {
        if slot.is_some() {
            return Err(bad(format!("{name} is given more than once")));
        }
        *slot = Some(value?);
        Ok(())
    }
    let uuid = |name: &str, value: &str| {
        value
            .parse::<Uuid>()
            .map_err(|_| bad(format!("{name} must be a UUID")))
    };
    let date = |name: &str, value: &str| {
        OffsetDateTime::parse(value, &Rfc3339)
            .map_err(|_| bad(format!("{name} must be an RFC 3339 date with an offset")))
    };
    let mut c = Criteria::default();
    for (name, value) in &page.filters {
        match name.as_str() {
            "state" => {
                let state = if STATES.contains(&value.as_str()) {
                    Ok(value.clone())
                } else {
                    Err(bad(format!("state must be one of {}", STATES.join(", "))))
                };
                once(&mut c.state, name, state)?;
            }
            "author" => once(&mut c.author, name, uuid(name, value))?,
            "object" => once(&mut c.object, name, uuid(name, value))?,
            "from" => once(&mut c.from, name, date(name, value))?,
            "to" => once(&mut c.to, name, date(name, value))?,
            "retroactive" => {
                let retroactive = value
                    .parse::<bool>()
                    .map_err(|_| bad("retroactive must be true or false"));
                once(&mut c.retroactive, name, retroactive)?;
            }
            other => return Err(bad(format!("unknown parameter {other}"))),
        }
    }
    Ok(c)
}

/// `GET /v1/changesets?state=…&author=…&object=…&from=…&to=…&retroactive=…`: the
/// changesets, newest first (REQ-004-32).
#[utoipa::path(
    get,
    path = "/changesets",
    operation_id = "list_changesets",
    tag = "changesets",
    params(CriteriaParams, PageParams),
    responses((status = 200, description = "The changesets, newest first.", body = Listing<ChangesetItem>))
)]
pub(crate) async fn list(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    readable(&api, &context).await?;
    let name = "changesets";
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, name)?;
    let criteria = filter(&page)?;
    let after = after_key(&page)?;
    let moment = current(&api, &page).await?;
    let size = usize::from(page.size);
    let mut found = changesets::page(
        &api.pool,
        ChangesetFilter {
            state: criteria.state.as_deref(),
            author: criteria.author,
            object: criteria.object,
            from: criteria.from,
            to: criteria.to,
            retroactive: criteria.retroactive,
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
    let criteria = json!({ "changesets": {
        "state": criteria.state,
        "author": criteria.author,
        "object": criteria.object,
        "from": criteria.from.map(|at| rfc3339(instant(at))),
        "to": criteria.to.map(|at| rfc3339(instant(at))),
        "retroactive": criteria.retroactive,
    } });
    audit_read(&api, &context, &ids, criteria).await?;
    let items: Vec<ChangesetItem> = found.iter().map(listed).collect();
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
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ChangesetDto {
    /// Identifier.
    id: Uuid,
    /// A state of the journal.
    state: String,
    /// `direct`, `approval` or `proposal`.
    kind: String,
    /// Its author.
    author: Uuid,
    /// Who validated it.
    validator: Option<Uuid>,
    /// Why.
    motive: String,
    /// The start of its period, RFC 3339.
    valid_from: String,
    /// The end of its period, RFC 3339; `null`: unbounded.
    valid_to: Option<String>,
    /// Its submission, RFC 3339.
    submitted_at: String,
    /// Its application, RFC 3339.
    applied_at: Option<String>,
    /// The end of the wait for a decision, RFC 3339.
    expires_at: Option<String>,
    /// The approval policy that held it.
    policy: Option<Uuid>,
    /// The motive of its rejection.
    rejection: Option<String>,
    /// Its operations, as submitted: the values of a sensitive property are `null` with
    /// `masked: true` without the right to read them, the digest of a secret never shown.
    #[schema(value_type = Vec<OperationDto>)]
    operations: Vec<Value>,
    /// The violations of its result.
    violations: Vec<ViolationDto>,
    /// The counted differential.
    #[schema(value_type = Vec<DiffCountDto>)]
    diff: Value,
    /// The objects closed by cascade.
    cascaded: Vec<Cascaded>,
    /// Links.
    links: ChangesetLinks,
}

/// The links of a changeset.
#[derive(Debug, Serialize, ToSchema)]
struct ChangesetLinks {
    /// Its differential, by pages.
    diff: String,
}

/// A changeset as read.
async fn represent(
    api: &Api,
    context: &RequestContext,
    found: &ChangesetRead,
) -> Result<ChangesetDto, Problem> {
    let s = &found.summary;
    Ok(ChangesetDto {
        id: s.id,
        state: s.state.clone(),
        kind: s.kind.clone(),
        author: s.author,
        validator: found.validator,
        motive: s.motive.clone(),
        valid_from: time(found.valid_from),
        valid_to: found.valid_to.map(time),
        submitted_at: time(s.submitted_at),
        applied_at: found.applied_at.map(time),
        expires_at: found.expires_at.map(time),
        policy: found.policy,
        rejection: found.rejection.clone(),
        operations: operations(api, context, &found.operations).await?,
        violations: violations(api, &found.violations, Pointers::Operations).await?,
        diff: counts(api, context, s.id, found.diff.clone()).await?,
        cascaded: cascaded(&found.cascaded),
        links: ChangesetLinks {
            diff: format!("{}/diff", href(s.id)),
        },
    })
}

/// The strong `ETag` of a changeset: its state.
fn etag(state: &str) -> Result<HeaderValue, Problem> {
    HeaderValue::try_from(format!("\"{state}\"")).map_err(internal)
}

/// `GET /v1/changesets/{id}`: state, operations, result and counted differential.
#[utoipa::path(
    get,
    path = "/changesets/{id}",
    operation_id = "show_changeset",
    tag = "changesets",
    params(("id" = Uuid, Path, description = "The changeset.")),
    responses((status = 200, description = "The changeset; its state is its strong ETag.", body = ChangesetDto))
)]
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

/// An object of the differential.
#[derive(Debug, Serialize, ToSchema)]
struct DiffObject {
    /// Identifier.
    id: Uuid,
}

/// One object of the differential, as served.
#[derive(Debug, Serialize, ToSchema)]
struct DiffEntry {
    /// Property key.
    property: String,
    /// The object.
    object: DiffObject,
    /// The value before; `null`: no value, or masked.
    #[schema(value_type = Object)]
    before: Value,
    /// The value after; `null`: no value, or masked.
    #[schema(value_type = Object)]
    after: Value,
    /// The values are masked: the caller may not read them.
    masked: bool,
}

/// A differential entry as served, its values masked unless `shown`.
fn diff_entry(item: &DiffItem, shown: bool) -> Result<DiffEntry, Problem> {
    let value = |text: &Option<String>| -> Result<Value, Problem> {
        match text {
            Some(text) if shown => serde_json::from_str(text).map_err(internal),
            _ => Ok(Value::Null),
        }
    };
    Ok(DiffEntry {
        property: item.property.clone(),
        object: DiffObject { id: item.object },
        before: value(&item.before)?,
        after: value(&item.after)?,
        masked: !shown,
    })
}

/// A section of the differential contributed by a module, as counted with the
/// differential.
#[derive(Debug, Serialize, ToSchema)]
struct SectionCount {
    /// Its name, `<module>.<name>`.
    name: String,
    /// Its rows.
    count: i64,
}

/// The differential of a changeset: its counts, and its objects by pages.
#[derive(Debug, Serialize, ToSchema)]
struct DiffPage<'a> {
    /// The counted differential.
    #[schema(value_type = Vec<DiffCountDto>)]
    counts: Value,
    /// The sections the modules contributed, with their number of rows (REQ-008-26).
    sections: Vec<SectionCount>,
    /// The objects of the page.
    items: Vec<DiffEntry>,
    /// The links of the page.
    links: ListingLinks<'a>,
}

/// A section of the differential of a changeset, by pages (REQ-008-26).
#[derive(Debug, Serialize, ToSchema)]
struct SectionPage<'a> {
    /// Its name.
    section: String,
    /// The rows of the page, in the order the module gave.
    rows: Vec<SectionRowDto>,
    /// The links of the page.
    links: ListingLinks<'a>,
}

/// The differential of the properties, or one section of the modules.
#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
enum DiffAnswer<'a> {
    /// The differential of the property values.
    Properties(DiffPage<'a>),
    /// One section, with `section=`.
    Section(SectionPage<'a>),
}

/// `GET /v1/changesets/{id}/diff?property=…`: the counted differential and its detail by
/// pages, in the order of the journal; `?section=<module>.<name>`: the rows of a section
/// contributed by a module, by pages (REQ-008-26).
#[utoipa::path(
    get,
    path = "/changesets/{id}/diff",
    operation_id = "changeset_diff",
    tag = "changesets",
    params(
        ("id" = Uuid, Path, description = "The changeset."),
        ("property" = Option<String>, Query, description = "Only this property."),
        ("section" = Option<String>, Query, description = "This section of a module instead of the properties."),
        PageParams,
    ),
    responses((status = 200, description = "The differential, or a section.", body = DiffAnswer))
)]
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
    let mut section = None;
    for (key, value) in &page.filters {
        match key.as_str() {
            "property" if property.is_none() => property = Some(value.clone()),
            "section" if section.is_none() => section = Some(value.clone()),
            "property" | "section" => return Err(bad(format!("{key} is given more than once"))),
            other => return Err(bad(format!("unknown parameter {other}"))),
        }
    }
    if let Some(section) = section {
        if property.is_some() {
            return Err(bad("property and section exclude each other"));
        }
        return section_diff(&api, &context, id, &name, &page, section).await;
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
    let sections = kartenn_store_pg::sections::section_counts(&api.pool, id)
        .await
        .map_err(Problem::from)?
        .into_iter()
        .map(|(name, count)| SectionCount { name, count })
        .collect();
    let mut response = axum::Json(DiffAnswer::Properties(DiffPage {
        counts: counts(&api, &context, id, journal.diff).await?,
        sections,
        items,
        links: ListingLinks {
            next: next.as_deref(),
        },
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

/// The rows of the section `section` of the changeset `id`, by pages (REQ-008-26).
async fn section_diff(
    api: &Api,
    context: &RequestContext,
    id: Uuid,
    name: &str,
    page: &Page,
    section: String,
) -> Result<Response, Problem> {
    let after = page
        .after()
        .map(|seq| {
            seq.as_i64()
                .ok_or_else(|| Problem::new(ProblemKind::InvalidCursor))
        })
        .transpose()?;
    let moment = current(api, page).await?;
    idempotency::journaled(&api.pool, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let size = usize::from(page.size);
    let mut found = kartenn_store_pg::sections::section_page(
        &api.pool,
        id,
        &section,
        after,
        i64::from(page.size) + 1,
    )
    .await
    .map_err(Problem::from)?;
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next_current(&api.ring, name, &moment, json!(found[size - 1].0));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    audit_read(
        api,
        context,
        &[id],
        json!({ "changeset": id, "diff": { "section": section } }),
    )
    .await?;
    let mut response = axum::Json(DiffAnswer::Section(SectionPage {
        rows: found
            .iter()
            .map(|(_, row)| SectionRowDto::from(row))
            .collect(),
        section,
        links: ListingLinks {
            next: next.as_deref(),
        },
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

/// A write permission the author of a proposal lacks, with the objects concerned.
#[derive(Debug, Serialize, ToSchema)]
struct Required {
    /// The permission.
    permission: String,
    /// The objects.
    objects: Vec<Uuid>,
}

/// A proposal the caller may validate.
#[derive(Debug, Serialize, ToSchema)]
struct ProposalItem {
    /// The changeset.
    #[serde(flatten)]
    changeset: ChangesetItem,
    /// The write permissions its author lacks.
    required: Vec<Required>,
}

/// `GET /v1/proposals`: the pending proposals the caller may validate (REQ-004-43), oldest
/// first.
#[utoipa::path(
    get,
    path = "/proposals",
    operation_id = "list_proposals",
    tag = "changesets",
    params(PageParams),
    responses((status = 200, description = "The proposals, oldest first.", body = Listing<ProposalItem>))
)]
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
    let items: Vec<ProposalItem> = found
        .iter()
        .map(|c| ProposalItem {
            changeset: listed(c),
            required: pending
                .iter()
                .find(|p| p.id == c.id)
                .map(|p| &p.pending.required)
                .into_iter()
                .flatten()
                .map(|(permission, objects)| Required {
                    permission: permission.clone(),
                    objects: objects.clone(),
                })
                .collect(),
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

/// A changeset decided.
#[derive(Debug, Serialize, ToSchema)]
struct Decided {
    /// The changeset.
    changeset: ChangesetRef,
    /// Its state now.
    state: &'static str,
}

/// 200 `{changeset, state}` and a link to the changeset.
fn decided(id: Uuid, state: &'static str) -> Response {
    linked(
        axum::Json(Decided {
            changeset: summary(id),
            state,
        })
        .into_response(),
        id,
        None,
    )
}

/// `POST /v1/changesets/{id}/validation`: applies a pending changeset again on the current
/// state (REQ-004-21, 22, 42); 200, or 422 when it no longer holds.
#[utoipa::path(
    post,
    path = "/changesets/{id}/validation",
    operation_id = "validate_changeset",
    tag = "changesets",
    params(
        ("id" = Uuid, Path, description = "The changeset."),
        ("If-Match" = String, Header, description = "Its state, as its ETag."),
    ),
    responses((status = 200, description = "Applied.", body = ChangesetApplied))
)]
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
#[utoipa::path(
    post,
    path = "/changesets/{id}/rejection",
    operation_id = "reject_changeset",
    tag = "changesets",
    params(
        ("id" = Uuid, Path, description = "The changeset."),
        ("If-Match" = String, Header, description = "Its state, as its ETag."),
    ),
    request_body(content = Motive, content_type = "application/json"),
    responses((status = 200, description = "Rejected.", body = Decided))
)]
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
#[utoipa::path(
    post,
    path = "/changesets/{id}/withdrawal",
    operation_id = "withdraw_changeset",
    tag = "changesets",
    params(
        ("id" = Uuid, Path, description = "The changeset."),
        ("If-Match" = String, Header, description = "Its state, as its ETag."),
    ),
    responses((status = 200, description = "Withdrawn.", body = Decided))
)]
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

/// An object the inverse of a changeset does not restore.
#[derive(Debug, Serialize, ToSchema)]
struct Unsupported {
    /// Its kind.
    kind: String,
    /// Identifier.
    id: Uuid,
}

/// What an undo adds to the answer of its inverse.
#[derive(Debug, Serialize, ToSchema)]
struct UndoMembers {
    /// The changeset undone.
    undoes: ChangesetRef,
    /// What the inverse does not restore.
    unsupported: Vec<Unsupported>,
    /// Simulated: the operations of the inverse.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Vec<OperationDto>>)]
    operations: Option<Value>,
}

/// The inverse of a changeset applied, and what the undo adds.
#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
struct UndoApplied {
    #[serde(flatten)]
    answer: ChangesetApplied,
    #[serde(flatten)]
    undo: UndoMembers,
}

/// The inverse of a changeset held for a decision, and what the undo adds.
#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
struct UndoPending {
    #[serde(flatten)]
    answer: ChangesetPending,
    #[serde(flatten)]
    undo: UndoMembers,
}

/// The inverse of a changeset simulated, and what the undo adds.
#[derive(Debug, Serialize, ToSchema)]
#[allow(dead_code)]
struct UndoSimulated {
    #[serde(flatten)]
    answer: ChangesetSimulated,
    #[serde(flatten)]
    undo: UndoMembers,
}

/// `POST /v1/changesets/{id}/undo[?simulate=true]`: submits the inverse of an applied
/// changeset under the rights of the caller (REQ-004-34), answered as a submission with
/// `undoes` and `unsupported`; 409 `undo-conflict` with the objects and rights written
/// since (REQ-004-35).
#[utoipa::path(
    post,
    path = "/changesets/{id}/undo",
    operation_id = "undo_changeset",
    tag = "changesets",
    params(
        ("id" = Uuid, Path, description = "The changeset."),
        ("If-Match" = String, Header, description = "Its state, as its ETag."),
        ("simulate" = Option<bool>, Query, description = "Validate the inverse without writing."),
    ),
    responses(
        (status = 201, description = "The inverse applied.", body = UndoApplied),
        (status = 202, description = "The inverse held for a decision.", body = UndoPending),
        (status = 200, description = "The inverse simulated.", body = UndoSimulated),
    )
)]
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
            .extension(
                "changeset",
                serde_json::to_value(summary(id)).map_err(internal)?,
            ));
    }
    found.request.simulate = simulate;
    let response = submit_answered(&api, &context, &found.request, &Answer::SUBMISSION).await?;
    let added = UndoMembers {
        undoes: summary(id),
        unsupported: found
            .unsupported
            .iter()
            .map(|(kind, id)| Unsupported {
                kind: kind.clone(),
                id: *id,
            })
            .collect(),
        operations: simulate
            .then(|| serde_json::to_value(&found.request.operations))
            .transpose()
            .map_err(internal)?,
    };
    let Value::Object(members) = serde_json::to_value(added).map_err(internal)? else {
        return Err(internal("the members of an undo are no object"));
    };
    with_members(response, members).await
}
