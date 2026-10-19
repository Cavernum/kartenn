//! The objects of the core (spec 006, REQ-006-1, 2, 4, 23; task 006-12a): entities,
//! sites, domains, prefixes, ranges and addresses as flat collections under `/v1`, each
//! object at `/v1/<collection>/{uuid}`, its natural key a filter of the collection; each
//! object whole, with the summary `{id, slug, href}` of the object it links to; its
//! history. An object the caller may not read answers as an absent one: the read
//! predicate does not tell them apart (ADR 0021).

use std::time::Duration;

use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Json, Router};
use kartenn_auth::keys::KeyRing;
use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::audit::{AUTHZ_DENIED, rfc3339};
use kartenn_core::family::ALL;
use kartenn_core::objects::{AddressUsage, Nature, RangeUsage, Slug};
use kartenn_core::period::Instant;
use kartenn_service::audit::{AuditJournal, Outcome, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::authz::Authorizer;
use kartenn_service::reads::{ReadAudit, objects_read, read_object};
use kartenn_service::{RequestError, StoreError};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::filter::Condition;
use kartenn_store_pg::list::{self, Criteria, Key, Kind, Version};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read;
use kartenn_store_pg::write::PgStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::cursor::{Page, next_href, respond_page};
use crate::dated::{Dates, ETag, Moment, respond};
use crate::filter::{self, Field, ListQuery};
use crate::problem::{Problem, ProblemKind};

/// What the routes of the objects read with.
#[derive(Debug, Clone)]
pub struct Api {
    /// The pool of the requests.
    pub pool: PgPool,
    /// The keys of the cursors.
    pub ring: KeyRing,
    /// The longest write transaction (`transport::Settings::write_transaction_timeout`).
    pub bound: Duration,
    /// The audit journal of the reads.
    pub journal: PgAuditJournal,
    /// How much a read event records.
    pub audit: ReadAudit,
}

/// The collection of each kind.
#[must_use]
pub const fn collection(kind: Kind) -> &'static str {
    match kind {
        Kind::Entity => "entities",
        Kind::Site => "sites",
        Kind::Domain => "domains",
        Kind::Prefix => "prefixes",
        Kind::Range => "ranges",
        Kind::Address => "addresses",
    }
}

/// The URL of the object `id` of `kind`.
#[must_use]
pub fn href(kind: Kind, id: Uuid) -> String {
    format!("/v1/{}/{id}", collection(kind))
}

/// The kind an object of `kind` links to, by its `domain` or `parent` member.
const fn linked(kind: Kind) -> Option<Kind> {
    match kind {
        Kind::Entity => Some(Kind::Entity),
        Kind::Site => Some(Kind::Site),
        Kind::Domain => None,
        Kind::Prefix | Kind::Range | Kind::Address => Some(Kind::Domain),
    }
}

/// The filters of the natural key of a collection.
const fn key_fields(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Entity | Kind::Site => &["slug", "parent"],
        Kind::Domain => &["slug"],
        Kind::Prefix | Kind::Address => &["domain", "value"],
        Kind::Range => &["domain", "first"],
    }
}

/// The routes of the six collections.
pub fn routes(api: Api) -> Router {
    let mut router = Router::new();
    for kind in Kind::ALL {
        let name = collection(kind);
        router = router
            .route(
                &format!("/{name}"),
                filter::collection(
                    move |state: State<Api>,
                          principal: Extension<Principal>,
                          context: Extension<RequestContext>,
                          query: ListQuery,
                          headers: HeaderMap| {
                        list(kind, state, principal, context, query, headers)
                    },
                ),
            )
            .route(
                &format!("/{name}/{{id}}"),
                get(
                    move |state: State<Api>,
                          context: Extension<RequestContext>,
                          id: Path<Uuid>,
                          dates: Dates,
                          headers: HeaderMap| {
                        object(kind, state, context, id, dates, headers)
                    },
                ),
            )
            .route(
                &format!("/{name}/{{id}}/history"),
                get(
                    move |state: State<Api>,
                          context: Extension<RequestContext>,
                          id: Path<Uuid>,
                          query: RawQuery,
                          headers: HeaderMap| {
                        history(kind, state, context, id, query, headers)
                    },
                ),
            );
    }
    router.with_state(api)
}

// ---------------------------------------------------------------- representations

/// The summary of a linked object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    /// Identifier.
    pub id: Uuid,
    /// Slug; `null` if the linked object did not exist at the dates of the read.
    pub slug: Option<String>,
    /// Its URL.
    pub href: String,
}

/// The links of an object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Links {
    /// The object.
    #[serde(rename = "self")]
    pub this: String,
    /// Its versions.
    pub history: String,
    /// The resolved values of its properties.
    pub properties: String,
    /// A prefix: its direct children.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<String>,
    /// A prefix: its ancestors, nearest first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ancestors: Option<String>,
    /// A prefix: its occupancy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occupancy: Option<String>,
}

impl Links {
    fn of(kind: Kind, id: Uuid) -> Self {
        let this = href(kind, id);
        let tree = |route: &str| (kind == Kind::Prefix).then(|| format!("{this}/{route}"));
        Self {
            history: format!("{this}/history"),
            properties: format!("{this}/properties"),
            children: tree("children"),
            ancestors: tree("ancestors"),
            occupancy: tree("occupancy"),
            this,
        }
    }
}

/// An addressing domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomainDto {
    /// Identifier.
    pub id: Uuid,
    /// Slug.
    pub slug: String,
    /// Links.
    pub links: Links,
}

/// An entity or a site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeDto {
    /// Identifier.
    pub id: Uuid,
    /// Slug.
    pub slug: String,
    /// The parent; `null` at the root.
    pub parent: Option<Summary>,
    /// Links.
    pub links: Links,
}

/// A prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrefixDto {
    /// Identifier.
    pub id: Uuid,
    /// Domain.
    pub domain: Summary,
    /// Canonical CIDR (REQ-002-6).
    pub value: String,
    /// Nature.
    pub nature: Nature,
    /// Point-to-point link.
    pub point_to_point: bool,
    /// Anycast.
    pub anycast: bool,
    /// Public addressing.
    pub public: bool,
    /// Links.
    pub links: Links,
}

/// A range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RangeDto {
    /// Identifier.
    pub id: Uuid,
    /// Domain.
    pub domain: Summary,
    /// First address, canonical.
    pub first: String,
    /// Last address (included), canonical.
    pub last: String,
    /// Usage.
    pub usage: RangeUsage,
    /// Public addressing.
    pub public: bool,
    /// Links.
    pub links: Links,
}

/// An address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddressDto {
    /// Identifier.
    pub id: Uuid,
    /// Domain.
    pub domain: Summary,
    /// Canonical address (REQ-002-6).
    pub value: String,
    /// Usage.
    pub usage: AddressUsage,
    /// Public addressing.
    pub public: bool,
    /// Links.
    pub links: Links,
}

/// The columns of an object, as the store reads them.
#[derive(Debug, Deserialize)]
struct Columns {
    id: Uuid,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    parent_id: Option<Uuid>,
    #[serde(default)]
    domain_id: Option<Uuid>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    range: Option<String>,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    nature: Option<Nature>,
    #[serde(default)]
    usage: Option<String>,
    #[serde(default)]
    point_to_point: bool,
    #[serde(default)]
    anycast: bool,
    #[serde(default)]
    public: bool,
}

/// A row the store gave that does not decode: an error of kartenn, logged.
fn malformed(kind: Kind, what: &str) -> Problem {
    tracing::error!(name: "http.object.malformed", kind = kind.as_str(), what, "an object read does not decode");
    Problem::new(ProblemKind::Internal)
}

fn address_text(text: &str) -> Option<String> {
    // `inet` prints a host mask only when it is not that of a host.
    let bare = text.split_once('/').map_or(text, |(addr, _)| addr);
    Addr::parse_any(&ALL, bare).ok().map(|a| a.to_string())
}

/// The id of the object `fields` links to.
fn link_id(kind: Kind, fields: &Value) -> Option<Uuid> {
    let member = match linked(kind)? {
        Kind::Domain => "domain_id",
        _ => "parent_id",
    };
    fields
        .get(member)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// The representation of an object of `kind` from its columns `fields` and the slug
/// `link` of the object it links to.
fn represent(kind: Kind, fields: &Value, link: Option<&str>) -> Result<Value, Problem> {
    let c: Columns =
        serde_json::from_value(fields.clone()).map_err(|_| malformed(kind, "columns"))?;
    let links = Links::of(kind, c.id);
    let summary = |to: Kind, id: Option<Uuid>, slug: Option<String>| {
        id.map(|id| Summary {
            id,
            slug,
            href: href(to, id),
        })
    };
    let slug = || c.slug.clone().ok_or_else(|| malformed(kind, "slug"));
    let domain = || {
        summary(Kind::Domain, c.domain_id, link.map(str::to_owned))
            .ok_or_else(|| malformed(kind, "domain"))
    };
    let usage = || c.usage.clone().ok_or_else(|| malformed(kind, "usage"));
    let value = match kind {
        Kind::Domain => serde_json::to_value(DomainDto {
            id: c.id,
            slug: slug()?,
            links,
        }),
        Kind::Entity | Kind::Site => serde_json::to_value(NodeDto {
            id: c.id,
            slug: slug()?,
            parent: summary(kind, c.parent_id, link.map(str::to_owned)),
            links,
        }),
        Kind::Prefix => {
            let text = c
                .prefix
                .as_deref()
                .ok_or_else(|| malformed(kind, "prefix"))?;
            let value = Prefix::parse_any(&ALL, text)
                .map_err(|_| malformed(kind, "prefix"))?
                .to_string();
            serde_json::to_value(PrefixDto {
                id: c.id,
                domain: domain()?,
                value,
                nature: c.nature.ok_or_else(|| malformed(kind, "nature"))?,
                point_to_point: c.point_to_point,
                anycast: c.anycast,
                public: c.public,
                links,
            })
        }
        Kind::Range => {
            let (first, last) = c
                .range
                .as_deref()
                .and_then(|r| r.strip_prefix('[')?.strip_suffix(']')?.split_once(','))
                .and_then(|(a, b)| Some((address_text(a.trim())?, address_text(b.trim())?)))
                .ok_or_else(|| malformed(kind, "range"))?;
            serde_json::to_value(RangeDto {
                id: c.id,
                domain: domain()?,
                first,
                last,
                usage: serde_json::from_value(Value::String(usage()?))
                    .map_err(|_| malformed(kind, "usage"))?,
                public: c.public,
                links,
            })
        }
        Kind::Address => {
            let value = c
                .address
                .as_deref()
                .and_then(address_text)
                .ok_or_else(|| malformed(kind, "address"))?;
            serde_json::to_value(AddressDto {
                id: c.id,
                domain: domain()?,
                value,
                usage: serde_json::from_value(Value::String(usage()?))
                    .map_err(|_| malformed(kind, "usage"))?,
                public: c.public,
                links,
            })
        }
    };
    value.map_err(|_| malformed(kind, "representation"))
}

// ---------------------------------------------------------------- handlers

pub(crate) fn store(error: &sqlx::Error) -> Problem {
    Problem::from(StoreError(error.to_string()))
}

pub(crate) fn bad(detail: impl Into<String>) -> Problem {
    Problem::new(ProblemKind::BadRequest).detail(detail)
}

pub(crate) fn instant(at: OffsetDateTime) -> Instant {
    Instant(i64::try_from(at.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MAX))
}

pub(crate) fn audited(e: impl std::fmt::Display) -> Problem {
    Problem::from(RequestError::Audit(StoreError(e.to_string())))
}

/// The values of an equality filter `name`: one, or one of several.
fn equals(name: &str, condition: Condition) -> Result<Vec<String>, Problem> {
    match condition {
        Condition::Equals(value) => Ok(vec![value]),
        Condition::OneOf(values) => Ok(values),
        _ => Err(bad(format!(
            "{name}: a natural key is filtered by equality"
        ))),
    }
}

/// `values` checked and made canonical by `canonical`, a refusal naming `name`.
fn canonical(
    name: &str,
    values: Vec<String>,
    canonical: impl Fn(&str) -> Option<String>,
) -> Result<Vec<String>, Problem> {
    values
        .into_iter()
        .map(|v| canonical(&v).ok_or_else(|| bad(format!("{name}: {v:?} is not valid"))))
        .collect()
}

fn slug(text: &str) -> Option<String> {
    Slug::parse(text).ok().map(|s| s.to_string())
}

/// The criteria of the filters of a list of `kind`.
#[derive(Debug, Default)]
struct Wanted {
    slugs: Vec<String>,
    links: Vec<String>,
    values: Vec<String>,
}

fn wanted(kind: Kind, params: &[(String, String)]) -> Result<Wanted, Problem> {
    let mut wanted = Wanted::default();
    for found in filter::parse(params, key_fields(kind))? {
        let name = match found.field {
            Field::Core(name) => name,
            Field::Property(key) => {
                return Err(bad(format!(
                    "prop.{key}: properties do not filter this collection yet"
                )));
            }
        };
        let values = equals(name, found.condition)?;
        match name {
            "slug" => wanted.slugs.extend(canonical(name, values, slug)?),
            "parent" | "domain" => wanted.links.extend(canonical(name, values, slug)?),
            "value" if kind == Kind::Prefix => {
                wanted.values.extend(canonical(name, values, |v| {
                    Prefix::parse_any(&ALL, v).ok().map(|p| p.to_string())
                })?);
            }
            _ => wanted.values.extend(canonical(name, values, |v| {
                Addr::parse_any(&ALL, v).ok().map(|a| a.to_string())
            })?),
        }
    }
    Ok(wanted)
}

/// `GET`/`QUERY /v1/<collection>`: a page of the objects of `kind` the caller may read,
/// filtered by natural key, sorted by it.
async fn list(
    kind: Kind,
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = collection(kind);
    let page = Page::from_query(&query, &api.ring, name)?;
    let wanted = wanted(kind, &page.filters)?;
    let after = page
        .after()
        .map(|key| serde_json::from_value::<(Option<Uuid>, String, Uuid)>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?
        .map(|(domain, value, id)| Key { domain, value, id });
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = page.dates.at(now, api.bound)?;
    let scope = PgAuthorizer::new(api.pool.clone())
        .read_scope(&principal.subject(), now)
        .await?;
    let size = usize::from(page.size);
    let mut rows = list::objects(
        &api.pool,
        kind,
        &Criteria {
            slugs: &wanted.slugs,
            links: &wanted.links,
            values: &wanted.values,
        },
        moment.valid_at,
        Some(moment.as_of()),
        after.as_ref(),
        i64::from(page.size) + 1,
        &scope,
    )
    .await
    .map_err(|e| store(&e))?;
    let next = (rows.len() > size).then(|| {
        rows.truncate(size);
        let last = &rows[size - 1].key;
        let cursor = page.next(
            &api.ring,
            name,
            &moment,
            json!([last.domain, last.value, last.id]),
        );
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let ids: Vec<Uuid> = rows.iter().map(|row| row.key.id).collect();
    let items = rows
        .into_iter()
        .map(|row| represent(kind, &row.fields, row.link.as_deref()))
        .collect::<Result<Vec<_>, _>>()?;
    let criteria = json!({
        "collection": name,
        "filters": page.filters,
        "valid_at": rfc3339(instant(moment.valid_at)),
        "as_of": rfc3339(instant(moment.as_of())),
    });
    let event =
        objects_read(&context, kind.as_str(), &ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// `GET /v1/<collection>/{id}`: the object, 404 if it is absent, invisible or of another
/// kind.
async fn object(
    kind: Kind,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment: Moment = dates.at(now, api.bound)?;
    let found = read_object(
        &PgStore::new(api.pool.clone()),
        &PgAuthorizer::new(api.pool.clone()),
        &api.journal,
        &context,
        id,
        instant(moment.valid_at),
        moment.as_of.map(instant),
        api.audit,
    )
    .await?
    .filter(|found| found.kind == kind.as_str())
    .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let version = read::version(&api.pool, id, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let link = match link_id(kind, &found.fields) {
        Some(to) => list::link(&api.pool, kind, to, moment.valid_at, moment.as_of)
            .await
            .map_err(|e| store(&e))?,
        None => None,
    };
    let body = represent(kind, &found.fields, link.as_deref())?;
    Ok(respond(
        &headers,
        &moment,
        &ETag::version(version),
        Json(body),
    ))
}

/// A version as served.
fn version_of(kind: Kind, version: &Version) -> Result<Value, Problem> {
    let at = |t: OffsetDateTime| rfc3339(instant(t));
    Ok(json!({
        "valid_from": at(version.valid_from),
        "valid_to": version.valid_to.map(at),
        "asserted_from": at(version.asserted_from),
        "asserted_to": version.asserted_to.map(at),
        "changeset": {
            "id": version.changeset,
            "href": format!("/v1/changesets/{}", version.changeset),
        },
        "object": represent(kind, &version.fields, version.link.as_deref())?,
    }))
}

/// 404, with `authz.access.denied` recorded, unless the subject of `context` may read the
/// object `id` of `kind` now: a sub-resource answers as its object (REQ-006-23).
pub(crate) async fn gate(
    api: &Api,
    context: &RequestContext,
    kind: Kind,
    id: Uuid,
) -> Result<(), Problem> {
    if let Some(subject) = context.subject() {
        let permission = format!("core.{}.read", kind.as_str());
        let denied = PgAuthorizer::new(api.pool.clone())
            .authorize_many(&subject, &permission, &[id])
            .await?;
        if !denied.is_empty() {
            let event = context
                .event(&AUTHZ_DENIED, Outcome::Denied)
                .map_err(audited)?
                .target("object", [id])
                .reason(&permission)
                .detail("permission", permission.as_str())
                .map_err(audited)?;
            api.journal
                .record(&event)
                .await
                .map_err(RequestError::Audit)?;
            return Err(Problem::new(ProblemKind::NotFound));
        }
    }
    Ok(())
}

/// The key of a version as written in a cursor by [`history`], with the `valid_at` the
/// first page asked for: the cursor freezes a valid time even when none was given.
type VersionKey = ((OffsetDateTime, OffsetDateTime), Option<OffsetDateTime>);

fn version_key(key: &Value) -> Result<VersionKey, Problem> {
    let invalid = || Problem::new(ProblemKind::InvalidCursor);
    let parse = |t: &str| OffsetDateTime::parse(t, &Rfc3339).map_err(|_| invalid());
    let (valid, asserted, valid_at) =
        serde_json::from_value::<(String, String, Option<String>)>(key.clone())
            .map_err(|_| invalid())?;
    let valid_at = valid_at.as_deref().map(parse).transpose()?;
    Ok(((parse(&valid)?, parse(&asserted)?), valid_at))
}

/// `GET /v1/<collection>/{id}/history`: the versions of an object the caller may read
/// now, on both axes (REQ-004-18), as known at `as_of` (a version retracted later shown
/// as current), only those valid at `valid_at` when given (REQ-006-5), by pages
/// (REQ-006-7, 8) keyed by the start of validity then of assertion; 404 otherwise.
async fn history(
    kind: Kind,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = format!("{}/{id}/history", collection(kind));
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &name)?;
    page.unfiltered()?;
    let (after, valid_at) = match page.after().map(version_key).transpose()? {
        Some((after, valid_at)) => (Some(after), valid_at),
        None => (None, page.dates.valid_at),
    };
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = page.dates.at(now, api.bound)?;
    gate(&api, &context, kind, id).await?;
    let size = usize::from(page.size);
    let mut versions = list::history_page(
        &api.pool,
        kind,
        id,
        valid_at,
        moment.as_of,
        after,
        Some(i64::from(page.size) + 1),
    )
    .await
    .map_err(|e| store(&e))?;
    if versions.is_empty() && after.is_none() {
        return Err(Problem::new(ProblemKind::NotFound));
    }
    let next = (versions.len() > size).then(|| {
        versions.truncate(size);
        let last = &versions[size - 1];
        let at = |t: OffsetDateTime| rfc3339(instant(t));
        let cursor = page.next(
            &api.ring,
            &name,
            &moment,
            json!([
                at(last.valid_from),
                at(last.asserted_from),
                valid_at.map(at)
            ]),
        );
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let items = versions
        .into_iter()
        .map(|v| version_of(kind, &v))
        .collect::<Result<Vec<_>, _>>()?;
    let criteria = json!({ "object": id, "history": true });
    let event =
        objects_read(&context, kind.as_str(), &[id], criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_reads_as_its_bounds() {
        let id = Uuid::now_v7();
        let domain = Uuid::now_v7();
        let fields = json!({
            "id": id, "domain_id": domain, "range": "[10.0.0.10,10.0.0.20]",
            "usage": "dhcp", "public": false,
        });
        let body = represent(Kind::Range, &fields, Some("lab")).unwrap();
        assert_eq!(body["first"], "10.0.0.10");
        assert_eq!(body["last"], "10.0.0.20");
        assert_eq!(body["domain"]["slug"], "lab");
        assert_eq!(body["domain"]["href"], format!("/v1/domains/{domain}"));
        assert_eq!(body["links"]["self"], format!("/v1/ranges/{id}"));
    }

    #[test]
    fn values_are_canonical() {
        let fields = json!({
            "id": Uuid::now_v7(), "domain_id": Uuid::now_v7(), "address": "2001:db8:0:0::1",
            "usage": "host", "public": false,
        });
        let body = represent(Kind::Address, &fields, None).unwrap();
        assert_eq!(body["value"], "2001:db8::1");
        let params = |q: &str| vec![("value".to_owned(), q.to_owned())];
        let found = wanted(Kind::Prefix, &params("2001:DB8::/32")).unwrap();
        assert_eq!(found.values, ["2001:db8::/32"]);
        assert!(wanted(Kind::Prefix, &params("10.0.0.1/24")).is_err());
        let unknown = wanted(Kind::Domain, &[("value".into(), "x".into())]).unwrap_err();
        assert!(format!("{unknown:?}").contains("value"), "{unknown:?}");
    }
}
