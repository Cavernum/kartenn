//! The objects of the core (spec 006, REQ-006-1, 2, 4, 23; task 006-12a): entities,
//! sites, domains, prefixes, ranges and addresses as flat collections under `/v1`, each
//! object at `/v1/<collection>/{uuid}`, its natural key a filter of the collection; each
//! object whole, with the summary `{id, slug, href}` of the object it links to; its
//! history. An object the caller may not read answers as an absent one: the read
//! predicate does not tell them apart (ADR 0021).

use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Json};
use kartenn_auth::keys::KeyRing;
use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::audit::{AUTHZ_DENIED, rfc3339};
use kartenn_core::family::ALL;
use kartenn_core::objects::{AddressUsage, Nature, RangeUsage, Slug};
use kartenn_core::period::Instant;
use kartenn_service::audit::{AuditJournal, Outcome, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::authz::Authorizer;
use kartenn_service::expected::Sources;
use kartenn_service::module::{ModuleRegistry, ObjectType};
use kartenn_service::reads::{ReadAudit, objects_read, read_object, read_permission};
use kartenn_service::{RequestError, StoreError};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::filter::{self as engine, Condition, FilterError};
use kartenn_store_pg::list::{self, Criteria, Key, Kind, Version};
use kartenn_store_pg::property_read;
use kartenn_store_pg::rbac::{PgAuthorizer, ReadScope};
use kartenn_store_pg::read;
use kartenn_store_pg::write::PgStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::OffsetDateTime;
use time::format_description::well_known::{Iso8601, Rfc3339};
use utoipa::ToSchema;
use utoipa::openapi::path::HttpMethod;
use utoipa::openapi::{RefOr, Schema};
use utoipa_axum::router::OpenApiRouter;
use uuid::Uuid;

use crate::changesets::{ChangesetApplied, ChangesetPending, ChangesetRef};
use crate::cursor::{Page, PageParams, next_href, respond_page};
use crate::dated::{Dates, DatesParams, ETag, Moment, respond};
use crate::filter::{self, Field, ListQuery};
use crate::openapi::enums::{AddressUsageSchema, NatureSchema, RangeUsageSchema};
use crate::openapi::{documented, listing_of, op, queryable, schemas_of};
use crate::problem::{Problem, ProblemKind};
use crate::resources;

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
    /// The sources of expected values, which the suggestions ask (spec 003, REQ-003-23).
    pub sources: Sources,
    /// The names of the technical outputs configured, whose filter may change at run time
    /// (012/REQ-012-27).
    pub outputs: std::sync::Arc<[String]>,
    /// The modules of the build (ADR 0026, REQ-008-1).
    pub modules: std::sync::Arc<kartenn_service::module::ModuleRegistry>,
}

impl Api {
    /// The authorizer of the requests, which knows the types of the modules of the build
    /// (REQ-008-6).
    #[must_use]
    pub fn authorizer(&self) -> PgAuthorizer {
        PgAuthorizer::new(self.pool.clone()).with_modules(self.modules.clone())
    }
}

/// What a route by object serves: a kind of the core, or a type of object of a module
/// (ADR 0026, clause 9), whose transversal routes the core provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A kind of the core.
    Core(Kind),
    /// A type of object of a module.
    Module(&'static ObjectType),
}

impl Target {
    /// Its name: the kind of the core, or the qualified type (`vlan.vlan`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core(kind) => kind.as_str(),
            Self::Module(ty) => ty.kind,
        }
    }

    /// Its collection under `/v1`.
    #[must_use]
    pub const fn collection(self) -> &'static str {
        match self {
            Self::Core(kind) => collection(kind),
            Self::Module(ty) => ty.collection,
        }
    }

    /// The URL of its object `id`.
    #[must_use]
    pub fn href(self, id: Uuid) -> String {
        format!("/v1/{}/{id}", self.collection())
    }

    /// The permission that reads its objects.
    #[must_use]
    pub fn read_permission(self) -> String {
        read_permission(self.as_str())
    }

    /// Its name in an operation identifier (`vlan_vlan`).
    #[must_use]
    pub fn operation_name(self) -> String {
        self.as_str().replace('.', "_")
    }

    /// The kind or the type named `name`, among the core and the modules of the build.
    #[must_use]
    pub fn named(name: &str, modules: &ModuleRegistry) -> Option<Self> {
        Kind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == name)
            .map(Self::Core)
            .or_else(|| modules.object_type(name).map(Self::Module))
    }
}

impl From<Kind> for Target {
    fn from(kind: Kind) -> Self {
        Self::Core(kind)
    }
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

/// The schema of an object of `kind` and those it refers to, its name first.
fn object_schemas(kind: Kind) -> (String, Vec<(String, RefOr<Schema>)>) {
    fn of<T: ToSchema + 'static>() -> (String, Vec<(String, RefOr<Schema>)>) {
        let mut schemas = Vec::new();
        schemas_of::<T>(&mut schemas);
        schemas.push(listing_of::<T>());
        (T::name().into_owned(), schemas)
    }
    match kind {
        Kind::Domain => of::<DomainDto>(),
        Kind::Entity | Kind::Site => of::<NodeDto>(),
        Kind::Prefix => of::<PrefixDto>(),
        Kind::Range => of::<RangeDto>(),
        Kind::Address => of::<AddressDto>(),
    }
}

/// The routes of the six collections, documented one by one: the handlers serve every
/// collection, which one `#[utoipa::path]` cannot name.
#[allow(clippy::too_many_lines)]
pub fn routes() -> OpenApiRouter<Api> {
    let mut router = OpenApiRouter::new();
    for kind in Kind::ALL {
        let name = collection(kind);
        let one = kind.as_str();
        let (dto, mut schemas) = object_schemas(kind);
        let [(creation, _), (modification, _)] = resources::bodies(kind);
        schemas.extend(resources::bodies(kind));
        resources::referred(&mut schemas);
        schemas_of::<ChangesetApplied>(&mut schemas);
        schemas_of::<ChangesetPending>(&mut schemas);
        schemas_of::<VersionDto>(&mut schemas);
        schemas.push(listing_of::<VersionDto>());
        let listing = format!("Listing_{dto}");
        let mut list_op = op(
            &format!("list_{name}"),
            "core",
            &format!(
                "The {name} the caller may read, by natural key; `prop.<key>` filters by \
                 property (`:between`, `:exists`)."
            ),
        );
        for field in key_fields(kind) {
            list_op = list_op.query(field, "Equal to (repeated: one of).");
        }
        let list_op = list_op
            .params::<DatesParams>()
            .params::<PageParams>()
            .answers("200", "A page of the collection.", &listing)
            .build();
        let create_op = op(
            &format!("create_{one}"),
            "core",
            &format!("Creates a {one}."),
        )
        .header(
            "Idempotency-Key",
            false,
            "Makes the write idempotent for 24 h.",
        )
        .body("application/json", &creation, true)
        .answers("201", "Created.", "ChangesetApplied")
        .answers("202", "Held for a decision.", "ChangesetPending")
        .build();
        router = router.routes(documented(
            &format!("/{name}"),
            vec![(HttpMethod::Get, list_op), (HttpMethod::Post, create_op)],
            schemas,
            filter::collection(
                move |state: State<Api>,
                      principal: Extension<Principal>,
                      context: Extension<RequestContext>,
                      query: ListQuery,
                      headers: HeaderMap| {
                    list(kind, state, principal, context, query, headers)
                },
            )
            .post(
                move |state: State<Api>,
                      principal: Extension<Principal>,
                      context: Extension<RequestContext>,
                      headers: HeaderMap,
                      raw: Bytes| {
                    resources::create(kind, state, principal, context, headers, raw)
                },
            ),
        ));
        router = queryable(router, &format!("/{name}"), &format!("query_{name}"));
        let the = format!("The {one}.");
        let show_op = op(&format!("show_{one}"), "core", &format!("The {one}."))
            .path("id", &the, true)
            .params::<DatesParams>()
            .answers("200", "The object; its version is its ETag.", &dto)
            .build();
        let modify_op = op(
            &format!("modify_{one}"),
            "core",
            &format!("Modifies the {one} (JSON Merge Patch)."),
        )
        .path("id", &the, true)
        .header("If-Match", true, "The version read, as its ETag.")
        .body(resources::MERGE_PATCH, &modification, true)
        .answers("200", "Modified.", "ChangesetApplied")
        .answers("202", "Held for a decision.", "ChangesetPending")
        .build();
        let close_op = op(
            &format!("close_{one}"),
            "core",
            &format!("Closes the {one}."),
        )
        .path("id", &the, true)
        .header("If-Match", true, "The version read, as its ETag.")
        .body("application/json", "Closure", false)
        .answers("200", "Closed.", "ChangesetApplied")
        .answers("202", "Held for a decision.", "ChangesetPending")
        .build();
        router = router.routes(documented(
            &format!("/{name}/{{id}}"),
            vec![
                (HttpMethod::Get, show_op),
                (HttpMethod::Patch, modify_op),
                (HttpMethod::Delete, close_op),
            ],
            Vec::new(),
            get(
                move |state: State<Api>,
                      context: Extension<RequestContext>,
                      id: Path<Uuid>,
                      dates: Dates,
                      headers: HeaderMap| {
                    object(kind, state, context, id, dates, headers)
                },
            )
            .patch(
                move |state: State<Api>,
                      principal: Extension<Principal>,
                      context: Extension<RequestContext>,
                      id: Path<Uuid>,
                      headers: HeaderMap,
                      raw: Bytes| {
                    resources::modify(kind, state, principal, context, id, headers, raw)
                },
            )
            .delete(
                move |state: State<Api>,
                      principal: Extension<Principal>,
                      context: Extension<RequestContext>,
                      id: Path<Uuid>,
                      headers: HeaderMap,
                      raw: Bytes| {
                    resources::close(kind, state, principal, context, id, headers, raw)
                },
            ),
        ));
        let history_op = op(
            &format!("{one}_history"),
            "core",
            &format!("The versions of the {one}, on both axes (REQ-004-18)."),
        )
        .path("id", &the, true)
        .params::<DatesParams>()
        .params::<PageParams>()
        .answers("200", "A page of versions.", "Listing_Version")
        .build();
        router = router.routes(documented(
            &format!("/{name}/{{id}}/history"),
            vec![(HttpMethod::Get, history_op)],
            Vec::new(),
            get(
                move |state: State<Api>,
                      context: Extension<RequestContext>,
                      id: Path<Uuid>,
                      query: RawQuery,
                      headers: HeaderMap| {
                    history(kind, state, context, id, query, headers)
                },
            ),
        ));
    }
    router
}

// ---------------------------------------------------------------- representations

/// The summary of a linked object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Summary {
    /// Identifier.
    pub id: Uuid,
    /// Slug; `null` if the linked object did not exist at the dates of the read.
    pub slug: Option<String>,
    /// Its URL.
    pub href: String,
}

/// The links of an object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = ObjectLinks)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = Domain)]
pub struct DomainDto {
    /// Identifier.
    pub id: Uuid,
    /// Slug.
    pub slug: String,
    /// Links.
    pub links: Links,
}

/// An entity or a site.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = Node)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = Prefix)]
pub struct PrefixDto {
    /// Identifier.
    pub id: Uuid,
    /// Domain.
    pub domain: Summary,
    /// Canonical CIDR (REQ-002-6).
    pub value: String,
    /// Nature.
    #[schema(value_type = NatureSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = Range)]
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
    #[schema(value_type = RangeUsageSchema)]
    pub usage: RangeUsage,
    /// Public addressing.
    pub public: bool,
    /// Links.
    pub links: Links,
}

/// An address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = Address)]
pub struct AddressDto {
    /// Identifier.
    pub id: Uuid,
    /// Domain.
    pub domain: Summary,
    /// Canonical address (REQ-002-6).
    pub value: String,
    /// Usage.
    #[schema(value_type = AddressUsageSchema)]
    pub usage: AddressUsage,
    /// Public addressing.
    pub public: bool,
    /// Links.
    pub links: Links,
}

/// An object of the core, of any kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(untagged)]
#[schema(as = CoreObject)]
pub enum ObjectDto {
    /// An addressing domain.
    Domain(DomainDto),
    /// An entity or a site.
    Node(NodeDto),
    /// A prefix.
    Prefix(PrefixDto),
    /// A range.
    Range(RangeDto),
    /// An address.
    Address(AddressDto),
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
fn represent(kind: Kind, fields: &Value, link: Option<&str>) -> Result<ObjectDto, Problem> {
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
    Ok(match kind {
        Kind::Domain => ObjectDto::Domain(DomainDto {
            id: c.id,
            slug: slug()?,
            links,
        }),
        Kind::Entity | Kind::Site => ObjectDto::Node(NodeDto {
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
            ObjectDto::Prefix(PrefixDto {
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
            ObjectDto::Range(RangeDto {
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
            ObjectDto::Address(AddressDto {
                id: c.id,
                domain: domain()?,
                value,
                usage: serde_json::from_value(Value::String(usage()?))
                    .map_err(|_| malformed(kind, "usage"))?,
                public: c.public,
                links,
            })
        }
    })
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
    /// Conditions on properties, by key.
    properties: Vec<(String, Condition)>,
}

fn wanted(kind: Kind, params: &[(String, String)]) -> Result<Wanted, Problem> {
    let mut wanted = Wanted::default();
    for found in filter::parse(params, key_fields(kind))? {
        let name = match found.field {
            Field::Core(name) => name,
            Field::Property(key) => {
                wanted.properties.push((key, found.condition));
                continue;
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
            _ => {
                wanted.values.extend(canonical(name, values, |v| {
                    Addr::parse_any(&ALL, v).ok().map(|a| a.to_string())
                })?);
            }
        }
    }
    Ok(wanted)
}

/// Checks the values of the condition `name` on a property of type `value_type` as the
/// filter engine of spec 003 reads them, so that a refusal names the parameter.
fn typed(name: &str, value_type: &str, condition: &Condition) -> Result<(), Problem> {
    let fits = |v: &str| match value_type {
        "int" => v.parse::<i64>().is_ok(),
        "date" => time::Date::parse(v, &Iso8601::DATE).is_ok(),
        "bool" => matches!(v, "true" | "false"),
        "ref" => Uuid::parse_str(v).is_ok(),
        _ => true,
    };
    if !matches!(
        value_type,
        "enum" | "ref" | "text" | "fqdn" | "int" | "date" | "bool"
    ) {
        return Err(bad(format!(
            "{name}: a property of type {value_type} is not a filter"
        )));
    }
    let values: Vec<&str> = match condition {
        Condition::Equals(v) => vec![v],
        Condition::OneOf(v) => v.iter().map(String::as_str).collect(),
        Condition::Between(low, high) => {
            if !matches!(value_type, "int" | "date" | "text" | "fqdn") {
                return Err(bad(format!(
                    "{name}: a property of type {value_type} has no bounds"
                )));
            }
            low.iter().chain(high).map(String::as_str).collect()
        }
        Condition::Present | Condition::Absent => Vec::new(),
    };
    match values.into_iter().find(|v| !fits(v)) {
        Some(v) => Err(bad(format!(
            "{name}: {v:?} is not a value of type {value_type}"
        ))),
        None => Ok(()),
    }
}

/// The identifiers of the page of the objects of `kind` whose properties meet those of
/// `wanted`, sorted by identifier (design « Filtres de propriétés »): the filter engine
/// of spec 003 filters within one domain at the current system time. Its moment, and
/// the identifiers of the page plus one when there is a next page.
async fn filtered(
    api: &Api,
    kind: Kind,
    page: &Page,
    wanted: &Wanted,
    scope: &ReadScope,
    now: OffsetDateTime,
) -> Result<(Moment, Vec<Uuid>), Problem> {
    let first = wanted
        .properties
        .first()
        .map(|(key, _)| format!("prop.{key}"))
        .unwrap_or_default();
    if !kind.in_domain() {
        return Err(bad(format!(
            "{first}: properties filter the prefixes, ranges and addresses of a domain"
        )));
    }
    let [domain] = wanted.links.as_slice() else {
        return Err(bad(format!("{first} requires one domain")));
    };
    // The engine reads the current state; a next page holds no `as_of` (REQ-006-7).
    if page.dates.as_of.is_some() {
        return Err(bad(format!(
            "as_of does not go with {first}: properties filter the current state"
        )));
    }
    let after = page
        .after()
        .map(|key| serde_json::from_value::<Uuid>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?;
    let moment = page.dates.at(now, api.bound)?;
    let mut conditions = Vec::with_capacity(wanted.properties.len());
    for (key, condition) in &wanted.properties {
        let name = format!("prop.{key}");
        let declaration = property_read::declaration(&api.pool, key, moment.valid_at, None)
            .await
            .map_err(|e| store(&e))?
            .ok_or_else(|| bad(format!("{name}: no such property at that date")))?;
        typed(&name, &declaration.value_type, condition)?;
        conditions.push((declaration.id, condition.clone()));
    }
    let domains = list::objects(
        &api.pool,
        Kind::Domain,
        &Criteria {
            slugs: std::slice::from_ref(domain),
            ..Criteria::default()
        },
        moment.valid_at,
        None,
        None,
        1,
        &ReadScope::all(),
    )
    .await
    .map_err(|e| store(&e))?;
    let Some(domain) = domains.first().map(|row| row.key.id) else {
        return Ok((moment, Vec::new()));
    };
    let ids = engine::filter_all_in(
        &api.pool,
        domain,
        &conditions,
        Some(kind.as_str()),
        &wanted.values,
        moment.valid_at,
        after,
        i64::from(page.size) + 1,
        scope,
    )
    .await
    .map_err(|e| match e {
        FilterError::Database(e) => store(&e),
        e => bad(format!("{first}: {e}")),
    })?;
    Ok((moment, ids))
}

/// `GET`/`QUERY /v1/<collection>`: a page of the objects of `kind` the caller may read,
/// filtered by natural key, sorted by it; filtered by properties, sorted by identifier
/// and read at the current system time, a next page included.
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
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let scope = api
        .authorizer()
        .read_scope(&principal.subject(), now)
        .await?;
    let size = usize::from(page.size);
    let (moment, rows, next) = if wanted.properties.is_empty() {
        let after = page
            .after()
            .map(|key| serde_json::from_value::<(Option<Uuid>, String, Uuid)>(key.clone()))
            .transpose()
            .map_err(|_| Problem::new(ProblemKind::InvalidCursor))?
            .map(|(domain, value, id)| Key { domain, value, id });
        let moment = page.dates.at(now, api.bound)?;
        let mut rows = list::objects(
            &api.pool,
            kind,
            &Criteria {
                slugs: &wanted.slugs,
                links: &wanted.links,
                values: &wanted.values,
                ids: &[],
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
            page.next(
                &api.ring,
                name,
                &moment,
                json!([last.domain, last.value, last.id]),
            )
        });
        (moment, rows, next)
    } else {
        let (moment, mut ids) = filtered(&api, kind, &page, &wanted, &scope, now).await?;
        let next = (ids.len() > size).then(|| {
            ids.truncate(size);
            page.next_current(&api.ring, name, &moment, json!(ids[size - 1]))
        });
        let mut rows = if ids.is_empty() {
            Vec::new()
        } else {
            list::objects(
                &api.pool,
                kind,
                &Criteria {
                    ids: &ids,
                    ..Criteria::default()
                },
                moment.valid_at,
                None,
                None,
                i64::from(page.size),
                &scope,
            )
            .await
            .map_err(|e| store(&e))?
        };
        rows.sort_by_key(|row| row.key.id);
        (moment, rows, next)
    };
    let next = next.map(|cursor| next_href(&format!("/v1/{name}"), &cursor));
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
        &api.authorizer(),
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

/// A version of an object, on both axes (REQ-004-18).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Version)]
struct VersionDto {
    /// The start of its validity, RFC 3339.
    valid_from: String,
    /// The end of its validity, RFC 3339; `null` while open.
    valid_to: Option<String>,
    /// The start of its assertion, RFC 3339.
    asserted_from: String,
    /// The end of its assertion, RFC 3339; `null` while asserted.
    asserted_to: Option<String>,
    /// The changeset that wrote it.
    changeset: ChangesetRef,
    /// The object, as it was.
    object: ObjectDto,
}

/// A version as served.
fn version_of(kind: Kind, version: &Version) -> Result<VersionDto, Problem> {
    let at = |t: OffsetDateTime| rfc3339(instant(t));
    Ok(VersionDto {
        valid_from: at(version.valid_from),
        valid_to: version.valid_to.map(at),
        asserted_from: at(version.asserted_from),
        asserted_to: version.asserted_to.map(at),
        changeset: crate::changesets::summary(version.changeset),
        object: represent(kind, &version.fields, version.link.as_deref())?,
    })
}

/// 404, with `authz.access.denied` recorded, unless the subject of `context` may read the
/// object `id` of `kind` now: a sub-resource answers as its object (REQ-006-23).
pub(crate) async fn gate(
    api: &Api,
    context: &RequestContext,
    kind: Kind,
    id: Uuid,
) -> Result<(), Problem> {
    gate_target(api, context, Target::Core(kind), id).await
}

/// [`gate`], for a kind of the core or a type of a module: 404 for an object the caller
/// may not read, the refusal journaled.
///
/// # Errors
///
/// 404 `not-found`; the authorizer or the journal unavailable.
pub async fn gate_target(
    api: &Api,
    context: &RequestContext,
    target: Target,
    id: Uuid,
) -> Result<(), Problem> {
    if let Some(subject) = context.subject() {
        let permission = target.read_permission();
        let denied = api
            .authorizer()
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
pub(crate) type VersionKey = ((OffsetDateTime, OffsetDateTime), Option<OffsetDateTime>);

pub(crate) fn version_key(key: &Value) -> Result<VersionKey, Problem> {
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

    fn json_of(kind: Kind, fields: &Value, link: Option<&str>) -> Value {
        serde_json::to_value(represent(kind, fields, link).unwrap()).unwrap()
    }

    #[test]
    fn a_range_reads_as_its_bounds() {
        let id = Uuid::now_v7();
        let domain = Uuid::now_v7();
        let fields = json!({
            "id": id, "domain_id": domain, "range": "[10.0.0.10,10.0.0.20]",
            "usage": "dhcp", "public": false,
        });
        let body = json_of(Kind::Range, &fields, Some("lab"));
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
        let body = json_of(Kind::Address, &fields, None);
        assert_eq!(body["value"], "2001:db8::1");
        let params = |q: &str| vec![("value".to_owned(), q.to_owned())];
        let found = wanted(Kind::Prefix, &params("2001:DB8::/32")).unwrap();
        assert_eq!(found.values, ["2001:db8::/32"]);
        assert!(wanted(Kind::Prefix, &params("10.0.0.1/24")).is_err());
        let unknown = wanted(Kind::Domain, &[("value".into(), "x".into())]).unwrap_err();
        assert!(format!("{unknown:?}").contains("value"), "{unknown:?}");
    }
}
