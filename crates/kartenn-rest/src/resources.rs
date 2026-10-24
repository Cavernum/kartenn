//! Writes by resource (spec 006, REQ-006-12 to 14; task 006-16): `POST` on a collection
//! creates an object, `PATCH` (JSON Merge Patch, RFC 7396) modifies one, `DELETE` closes
//! one; each wraps a changeset submitted by the same pipeline as `POST /v1/changesets`,
//! the motive and the period in the reserved members of the body. `PATCH` and `DELETE`
//! require `If-Match` with the version of the object (428 without it, 412 when stale).

use std::collections::BTreeMap;

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use kartenn_service::operations::{Named, RawInput, Ref};
use kartenn_service::{ChangesetRequest, ObjectKind, Operation};
use kartenn_store_pg::list::Kind;
use kartenn_store_pg::property_read;
use kartenn_store_pg::read;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::changesets::{Answer, Location, Pointers, idempotency_key, period, submit_answered};
use crate::dated::ETag;
use crate::objects::{Api, bad, gate, store};
use crate::openapi::enums::{AddressUsageSchema, NatureSchema, RangeUsageSchema};
use crate::operation::RawDto;
use crate::problem::{Problem, ProblemKind};

/// The media type of a `PATCH` (RFC 7396 §4).
pub const MERGE_PATCH: &str = "application/merge-patch+json";

/// The pointer to the whole body (RFC 6901 §5).
pub(crate) const WHOLE: &str = "";

/// The kind of the pipeline of a kind of collection.
const fn object_kind(kind: Kind) -> ObjectKind {
    match kind {
        Kind::Entity => ObjectKind::Entity,
        Kind::Site => ObjectKind::Site,
        Kind::Domain => ObjectKind::Domain,
        Kind::Prefix => ObjectKind::Prefix,
        Kind::Range => ObjectKind::Range,
        Kind::Address => ObjectKind::Address,
    }
}

/// The essence of the media type of the body, lowercase.
fn media_type(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(|v| v.trim().to_ascii_lowercase())
}

/// The body as a JSON object of media type `expected` (another → 415), or 400.
pub(crate) fn members(
    headers: &HeaderMap,
    raw: &[u8],
    expected: &str,
) -> Result<Map<String, Value>, Problem> {
    if media_type(headers).as_deref() != Some(expected) {
        return Err(Problem::new(ProblemKind::UnsupportedMediaType)
            .detail(format!("the body must be {expected}")));
    }
    match serde_json::from_slice(raw) {
        Ok(Value::Object(members)) => Ok(members),
        Ok(_) => Err(bad("the body must be a JSON object")),
        Err(e) => Err(bad(format!("the body is not JSON: {e}"))),
    }
}

/// A member as `T`, or 400 naming it.
pub(crate) fn typed<T: DeserializeOwned>(name: &str, value: Value) -> Result<T, Problem> {
    serde_json::from_value(value).map_err(|e| bad(format!("{name}: {e}")))
}

/// The reserved members of a write (design « Représentations »): `motive`, `valid_from`,
/// `valid_to`, removed from `members`; `null` dates are absent.
#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct Reserved {
    /// Why; required by the rules.
    pub(crate) motive: String,
    /// The start of the period of the write, RFC 3339; now by default.
    pub(crate) valid_from: Option<String>,
    /// The end of the period of the write, RFC 3339; unbounded by default.
    pub(crate) valid_to: Option<String>,
}

pub(crate) fn reserved(members: &mut Map<String, Value>) -> Result<Reserved, Problem> {
    let mut take = |name: &str| -> Result<Option<String>, Problem> {
        members
            .remove(name)
            .map(|v| typed::<Option<String>>(name, v))
            .transpose()
            .map(Option::flatten)
    };
    Ok(Reserved {
        motive: take("motive")?.unwrap_or_default(),
        valid_from: take("valid_from")?,
        valid_to: take("valid_to")?,
    })
}

/// The body of a write as the contract describes it: the reserved members, read by
/// [`reserved`], beside the members of the resource, read once they are taken out.
macro_rules! written {
    ($name:ident, $members:ty, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
        #[allow(dead_code)]
        pub(crate) struct $name {
            #[serde(flatten)]
            reserved: $crate::resources::Reserved,
            #[serde(flatten)]
            members: $members,
        }
    };
}
pub(crate) use written;

/// The optional body of a closure: its motive and its start, no `valid_to`.
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
pub(crate) struct Closure {
    /// Why; required by the rules.
    motive: String,
    /// The start of the closure, RFC 3339; now by default.
    valid_from: Option<String>,
}

/// A reference to another object: the summary without `href` (design « Routes par
/// ressource »).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Reference {
    /// Its identifier.
    id: Uuid,
}

impl From<Reference> for Ref {
    fn from(r: Reference) -> Self {
        Self::Id(r.id)
    }
}

/// A new addressing domain.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct NewDomain {
    /// Technical identifier.
    slug: String,
}

/// A new entity or site.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct NewNode {
    /// Technical identifier.
    slug: String,
    /// The parent; none for a root.
    #[serde(default)]
    parent: Option<Reference>,
}

/// A new prefix.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct NewPrefix {
    /// The domain.
    domain: Reference,
    /// `address/length`.
    value: String,
    /// Container or network.
    #[schema(value_type = NatureSchema)]
    nature: Nature,
    /// Point-to-point network.
    #[serde(default)]
    point_to_point: bool,
    /// Anycast network.
    #[serde(default)]
    anycast: bool,
}

/// A new range.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct NewRange {
    /// The domain.
    domain: Reference,
    /// First address.
    first: String,
    /// Last address, included.
    last: String,
    /// Usage.
    #[schema(value_type = RangeUsageSchema)]
    usage: RangeUsage,
}

/// A new address.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct NewAddress {
    /// The domain.
    domain: Reference,
    /// Address.
    value: String,
    /// Usage.
    #[schema(value_type = AddressUsageSchema)]
    usage: AddressUsage,
}

written!(
    DomainCreation,
    NewDomain,
    "The body that creates an addressing domain."
);
written!(
    NodeCreation,
    NewNode,
    "The body that creates an entity or a site."
);
written!(PrefixCreation, NewPrefix, "The body that creates a prefix.");
written!(RangeCreation, NewRange, "The body that creates a range.");
written!(
    AddressCreation,
    NewAddress,
    "The body that creates an address."
);

/// What a merge patch writes in a property: a value or the elements of a set assign it,
/// `null` removes the assignment (inheritance resumes), `{"clear": true}` clears it
/// explicitly (REQ-003-13).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum PropertyWrite {
    /// The elements of a set.
    Values(Vec<RawDto>),
    /// One value.
    Value(RawDto),
    /// An explicit clearing.
    Clear(ClearMark),
    /// `null`: the assignment removed.
    Remove(()),
}

/// An explicit clearing of a property.
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
struct ClearMark {
    /// `true`.
    clear: bool,
}

/// The members of a merge patch of a domain, read by [`fields`] (`writable`).
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
struct DomainPatch {
    /// New slug.
    slug: Option<String>,
    /// The properties to write, by key.
    properties: Option<BTreeMap<String, PropertyWrite>>,
}

/// The members of a merge patch of an entity or a site.
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
struct NodePatch {
    /// New slug.
    slug: Option<String>,
    /// New parent; `null`: to the root.
    parent: Option<Reference>,
    /// The properties to write, by key.
    properties: Option<BTreeMap<String, PropertyWrite>>,
}

/// The members of a merge patch of a prefix.
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
struct PrefixPatch {
    /// The domain never changes (REQ-002-12): given another, the write is refused.
    domain: Option<Reference>,
    /// New value, `address/length` (resizing; same identifier).
    value: Option<String>,
    /// New nature.
    #[schema(value_type = Option<NatureSchema>)]
    nature: Option<Nature>,
    /// Point-to-point network or not.
    point_to_point: Option<bool>,
    /// Anycast network or not.
    anycast: Option<bool>,
    /// The properties to write, by key.
    properties: Option<BTreeMap<String, PropertyWrite>>,
}

/// The members of a merge patch of a range.
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
struct RangePatch {
    /// The domain never changes (REQ-002-12): given another, the write is refused.
    domain: Option<Reference>,
    /// New usage.
    #[schema(value_type = Option<RangeUsageSchema>)]
    usage: Option<RangeUsage>,
    /// The properties to write, by key.
    properties: Option<BTreeMap<String, PropertyWrite>>,
}

/// The members of a merge patch of an address.
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)]
struct AddressPatch {
    /// The domain never changes (REQ-002-12): given another, the write is refused.
    domain: Option<Reference>,
    /// New usage.
    #[schema(value_type = Option<AddressUsageSchema>)]
    usage: Option<AddressUsage>,
    /// The properties to write, by key.
    properties: Option<BTreeMap<String, PropertyWrite>>,
}

written!(
    DomainModification,
    DomainPatch,
    "A merge patch of an addressing domain."
);
written!(
    NodeModification,
    NodePatch,
    "A merge patch of an entity or a site."
);
written!(
    PrefixModification,
    PrefixPatch,
    "A merge patch of a prefix."
);
written!(RangeModification, RangePatch, "A merge patch of a range.");
written!(
    AddressModification,
    AddressPatch,
    "A merge patch of an address."
);

/// The schemas of the bodies of the writes of a kind, by the names the contract gives
/// them: creation, then modification.
pub(crate) fn bodies(kind: Kind) -> [(String, utoipa::openapi::RefOr<utoipa::openapi::Schema>); 2] {
    fn named<T: ToSchema>() -> (String, utoipa::openapi::RefOr<utoipa::openapi::Schema>) {
        (T::name().into_owned(), T::schema())
    }
    match kind {
        Kind::Domain => [named::<DomainCreation>(), named::<DomainModification>()],
        Kind::Entity | Kind::Site => [named::<NodeCreation>(), named::<NodeModification>()],
        Kind::Prefix => [named::<PrefixCreation>(), named::<PrefixModification>()],
        Kind::Range => [named::<RangeCreation>(), named::<RangeModification>()],
        Kind::Address => [named::<AddressCreation>(), named::<AddressModification>()],
    }
}

/// The schemas the bodies of the writes refer to.
pub(crate) fn referred(
    schemas: &mut Vec<(String, utoipa::openapi::RefOr<utoipa::openapi::Schema>)>,
) {
    crate::openapi::schemas_of::<Reserved>(schemas);
    crate::openapi::schemas_of::<Closure>(schemas);
    crate::openapi::schemas_of::<Reference>(schemas);
    crate::openapi::schemas_of::<NewDomain>(schemas);
    crate::openapi::schemas_of::<NewNode>(schemas);
    crate::openapi::schemas_of::<NewPrefix>(schemas);
    crate::openapi::schemas_of::<NewRange>(schemas);
    crate::openapi::schemas_of::<NewAddress>(schemas);
    crate::openapi::schemas_of::<DomainPatch>(schemas);
    crate::openapi::schemas_of::<NodePatch>(schemas);
    crate::openapi::schemas_of::<PrefixPatch>(schemas);
    crate::openapi::schemas_of::<RangePatch>(schemas);
    crate::openapi::schemas_of::<AddressPatch>(schemas);
    crate::openapi::schemas_of::<PropertyWrite>(schemas);
    crate::openapi::schemas_of::<ClearMark>(schemas);
}

/// The creation of an object of `kind` from the members of the body, or 400.
fn creation(kind: Kind, members: Map<String, Value>) -> Result<Operation, Problem> {
    let body = Value::Object(members);
    let new = |what: &str| format!("the body is not {what}");
    Ok(match kind {
        Kind::Domain => {
            let d: NewDomain = typed(&new("a domain"), body)?;
            Operation::CreateDomain {
                local: None,
                slug: d.slug,
            }
        }
        Kind::Entity => {
            let n: NewNode = typed(&new("an entity"), body)?;
            Operation::CreateEntity {
                local: None,
                slug: n.slug,
                parent: n.parent.map(Ref::from),
            }
        }
        Kind::Site => {
            let n: NewNode = typed(&new("a site"), body)?;
            Operation::CreateSite {
                local: None,
                slug: n.slug,
                parent: n.parent.map(Ref::from),
            }
        }
        Kind::Prefix => {
            let p: NewPrefix = typed(&new("a prefix"), body)?;
            Operation::CreatePrefix {
                local: None,
                domain: p.domain.into(),
                prefix: p.value,
                nature: p.nature,
                point_to_point: p.point_to_point,
                anycast: p.anycast,
            }
        }
        Kind::Range => {
            let r: NewRange = typed(&new("a range"), body)?;
            Operation::CreateRange {
                local: None,
                domain: r.domain.into(),
                first: r.first,
                last: r.last,
                usage: r.usage,
            }
        }
        Kind::Address => {
            let a: NewAddress = typed(&new("an address"), body)?;
            Operation::CreateAddress {
                local: None,
                domain: a.domain.into(),
                address: a.value,
                usage: a.usage,
            }
        }
    })
}

/// The opaque part of the one strong entity tag `If-Match` names (design « Écritures »):
/// absent or `*` → 428 with `required` as detail, several or malformed → 400, weak → 412.
pub(crate) fn entity_tag(headers: &HeaderMap, required: &str) -> Result<String, Problem> {
    let required = || Problem::new(ProblemKind::PreconditionRequired).detail(required);
    let mut values = headers.get_all(header::IF_MATCH).iter();
    let Some(value) = values.next() else {
        return Err(required());
    };
    if values.next().is_some() {
        return Err(bad("If-Match is given more than once"));
    }
    let value = value
        .to_str()
        .map_err(|_| bad("If-Match is not an entity tag"))?
        .trim();
    if value == "*" {
        return Err(required());
    }
    if value.contains(',') {
        return Err(bad("If-Match names one version"));
    }
    let (weak, quoted) = match value.strip_prefix("W/") {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    let opaque = quoted
        .strip_prefix('"')
        .and_then(|q| q.strip_suffix('"'))
        .filter(|o| !o.contains('"'))
        .ok_or_else(|| bad("If-Match is not an entity tag"))?;
    if weak {
        return Err(stale());
    }
    Ok(opaque.to_owned())
}

/// 412: `If-Match` names no current version.
pub(crate) fn stale() -> Problem {
    Problem::new(ProblemKind::PreconditionFailed).detail("If-Match names no version")
}

/// The version `If-Match` names (REQ-006-14, H19, design « Routes par ressource »): the
/// own part of a composite tag, or a bare version; absent or `*` → 428, several or
/// malformed → 400, weak or of another form → 412.
pub(crate) fn if_match(headers: &HeaderMap) -> Result<Uuid, Problem> {
    let opaque = entity_tag(
        headers,
        "read the object, then send its ETag in If-Match to modify or close it",
    )?;
    ETag::own_version(&opaque).ok_or_else(stale)
}

/// 422 `invalid-member` pointing at `pointer`.
pub(crate) fn invalid(pointer: &str, detail: impl Into<String>) -> Problem {
    Problem::new(ProblemKind::InvalidMember)
        .detail(detail)
        .extension("pointer", Value::String(pointer.to_owned()))
}

/// A JSON Pointer token (RFC 6901 §3).
fn token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// The members of a kind the service does not change.
fn read_only(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Domain | Kind::Entity | Kind::Site => &["id", "links", "version"],
        Kind::Prefix => &["id", "links", "version", "public"],
        Kind::Range => &["id", "links", "version", "public", "first", "last"],
        Kind::Address => &["id", "links", "version", "public", "value"],
    }
}

/// The members a `PATCH` of `kind` may write, besides `properties`.
fn writable(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Domain => &["slug"],
        Kind::Entity | Kind::Site => &["slug", "parent"],
        Kind::Prefix => &["domain", "value", "nature", "point_to_point", "anycast"],
        Kind::Range | Kind::Address => &["domain", "usage"],
    }
}

/// The operations of a merge patch, each with the pointer of its member: the guard, the
/// fields, then the properties by key (design « Routes par ressource »).
struct Patch {
    operations: Vec<Operation>,
    pointers: Vec<String>,
}

impl Patch {
    fn push(&mut self, operation: Operation, pointer: impl Into<String>) {
        self.operations.push(operation);
        self.pointers.push(pointer.into());
    }
}

/// The current usage of `fields`, when the patch leaves it.
fn current<T: DeserializeOwned>(fields: &Value) -> Result<T, Problem> {
    serde_json::from_value(fields["usage"].clone()).map_err(|_| {
        tracing::error!(name: "http.resource.usage", "an object read has no usage");
        Problem::new(ProblemKind::Internal)
    })
}

/// The field operations of a merge patch of a domain, an entity or a site: `slug` renames,
/// `parent` moves (`null`: to the root).
fn node(
    kind: Kind,
    reference: Ref,
    members: &mut Map<String, Value>,
    patch: &mut Patch,
) -> Result<(), Problem> {
    if let Some(slug) = members.remove("slug") {
        let named = match kind {
            Kind::Domain => Named::Domain,
            Kind::Entity => Named::Entity,
            _ => Named::Site,
        };
        patch.push(
            Operation::Rename {
                kind: named,
                id: reference,
                slug: typed("slug", slug)?,
                expected: None,
            },
            "/slug",
        );
    }
    if kind != Kind::Domain
        && let Some(parent) = members.remove("parent")
    {
        let parent = typed::<Option<Reference>>("parent", parent)?.map(Ref::from);
        let operation = if kind == Kind::Entity {
            Operation::MoveEntity {
                id: reference,
                parent,
                expected: None,
            }
        } else {
            Operation::MoveSite {
                id: reference,
                parent,
                expected: None,
            }
        };
        patch.push(operation, "/parent");
    }
    Ok(())
}

/// The field operations of a merge patch of the object `id` of `kind`, read as `fields`.
fn fields(
    kind: Kind,
    id: Uuid,
    fields: &Value,
    members: &mut Map<String, Value>,
    patch: &mut Patch,
) -> Result<(), Problem> {
    let mut take = |name: &str| members.remove(name);
    let domain = |v: Option<Value>| {
        v.map(|v| typed::<Reference>("domain", v).map(Ref::from))
            .transpose()
    };
    let reference = Ref::Id(id);
    match kind {
        Kind::Domain | Kind::Entity | Kind::Site => node(kind, reference, members, patch)?,
        Kind::Prefix => {
            let domain = domain(take("domain"))?;
            let prefix = take("value").map(|v| typed("value", v)).transpose()?;
            let nature = take("nature").map(|v| typed("nature", v)).transpose()?;
            let point_to_point = take("point_to_point")
                .map(|v| typed("point_to_point", v))
                .transpose()?;
            let anycast = take("anycast").map(|v| typed("anycast", v)).transpose()?;
            if domain.is_some()
                || prefix.is_some()
                || nature.is_some()
                || point_to_point.is_some()
                || anycast.is_some()
            {
                patch.push(
                    Operation::UpdatePrefix {
                        id: reference,
                        domain,
                        prefix,
                        nature,
                        point_to_point,
                        anycast,
                        expected: None,
                    },
                    WHOLE,
                );
            }
        }
        Kind::Range | Kind::Address => {
            let domain = domain(take("domain"))?;
            let usage = take("usage");
            if domain.is_some() || usage.is_some() {
                let operation = if kind == Kind::Range {
                    Operation::UpdateRange {
                        id: reference,
                        domain,
                        usage: match usage {
                            Some(u) => typed("usage", u)?,
                            None => current(fields)?,
                        },
                        expected: None,
                    }
                } else {
                    Operation::UpdateAddress {
                        id: reference,
                        domain,
                        usage: match usage {
                            Some(u) => typed("usage", u)?,
                            None => current(fields)?,
                        },
                        expected: None,
                    }
                };
                patch.push(operation, WHOLE);
            }
        }
    }
    Ok(())
}

/// The property operations of a merge patch, by key: a value (scalar or array) assigns,
/// `null` removes the assignment, `{"clear": true}` clears explicitly (REQ-003-13).
async fn properties(
    api: &Api,
    kind: Kind,
    id: Uuid,
    table: Value,
    patch: &mut Patch,
) -> Result<(), Problem> {
    let Value::Object(table) = table else {
        return Err(bad("properties is a table of keys"));
    };
    let table: BTreeMap<String, Value> = table.into_iter().collect();
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let object = Ref::Id(id);
    let object_kind = object_kind(kind);
    for (key, value) in table {
        let pointer = format!("/properties/{}", token(&key));
        let declaration = property_read::declaration(&api.pool, &key, now, None)
            .await
            .map_err(|e| store(&e))?
            .ok_or_else(|| invalid(&pointer, format!("property {key} is not declared")))?;
        let property = Ref::Id(declaration.id);
        let name = format!("properties.{key}");
        let operation = match value {
            Value::Null => Operation::UnassignProperty {
                object,
                kind: object_kind,
                property,
            },
            // An external reference is an object naming its system (REQ-016-3).
            Value::Object(reference) if reference.contains_key("system") => {
                Operation::AssignProperty {
                    object,
                    kind: object_kind,
                    property,
                    values: vec![typed(&name, Value::Object(reference))?],
                }
            }
            Value::Object(clear) => {
                if clear.len() != 1 || clear.get("clear") != Some(&Value::Bool(true)) {
                    return Err(bad(format!("{name}: only {{\"clear\": true}} clears")));
                }
                Operation::ClearProperty {
                    object,
                    kind: object_kind,
                    property,
                }
            }
            Value::Array(values) => Operation::AssignProperty {
                object,
                kind: object_kind,
                property,
                values: values
                    .into_iter()
                    .map(|v| typed::<RawInput>(&name, v))
                    .collect::<Result<_, _>>()?,
            },
            scalar => Operation::AssignProperty {
                object,
                kind: object_kind,
                property,
                values: vec![typed(&name, scalar)?],
            },
        };
        patch.push(operation, pointer);
    }
    Ok(())
}

/// The object `id` of `kind` as read now, 404 if it is absent, invisible or of another
/// kind (the read predicate, REQ-006-23).
async fn existing(
    api: &Api,
    context: &RequestContext,
    kind: Kind,
    id: Uuid,
) -> Result<Value, Problem> {
    gate(api, context, kind, id).await?;
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    read::object(&api.pool, id, now, None)
        .await
        .map_err(|e| store(&e))?
        .filter(|found| found.kind == kind.as_str())
        .map(|found| found.fields)
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))
}

/// `POST /v1/<collection>`: creates an object; 201 and `Location` to it.
pub(crate) async fn create(
    kind: Kind,
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let idempotency_key = idempotency_key(&headers)?;
    let mut members = members(&headers, &raw, "application/json")?;
    let reserved = reserved(&mut members)?;
    let operation = creation(kind, members)?;
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(
            &api,
            reserved.valid_from.as_deref(),
            reserved.valid_to.as_deref(),
        )
        .await?,
        operations: vec![operation],
        simulate: false,
        idempotency_key,
        apply_own: context.apply_own,
    };
    let pointers = [WHOLE.to_owned()];
    let answer = Answer {
        pointers: Pointers::Members(&pointers),
        applied: StatusCode::CREATED,
        location: Some(Location::Created(kind)),
        precondition: None,
        shown: None,
    };
    submit_answered(&api, &context, &request, &answer).await
}

/// `PATCH /v1/<collection>/{id}` in JSON Merge Patch with `If-Match`: one changeset of the
/// guard, the fields and the properties.
pub(crate) async fn modify(
    kind: Kind,
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let mut members = members(&headers, &raw, MERGE_PATCH)?;
    let reserved = reserved(&mut members)?;
    if let Some(unknown) = members.keys().find(|n| {
        n.as_str() != "properties"
            && !writable(kind).contains(&n.as_str())
            && !read_only(kind).contains(&n.as_str())
    }) {
        return Err(bad(format!("unknown member {unknown}")));
    }
    let expected = if_match(&headers)?;
    let current = existing(&api, &context, kind, id).await?;
    if let Some(name) = read_only(kind).iter().find(|n| members.contains_key(**n)) {
        return Err(invalid(
            &format!("/{name}"),
            format!("{name} is not modified by a PATCH"),
        ));
    }
    let table = members.remove("properties");
    let mut patch = Patch {
        operations: Vec::new(),
        pointers: Vec::new(),
    };
    patch.push(
        Operation::Expect {
            kind: object_kind(kind),
            id: Ref::Id(id),
            expected,
        },
        WHOLE,
    );
    fields(kind, id, &current, &mut members, &mut patch)?;
    if let Some(table) = table {
        properties(&api, kind, id, table, &mut patch).await?;
    }
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(
            &api,
            reserved.valid_from.as_deref(),
            reserved.valid_to.as_deref(),
        )
        .await?,
        operations: patch.operations,
        simulate: false,
        idempotency_key: None,
        apply_own: context.apply_own,
    };
    let answer = Answer {
        pointers: Pointers::Members(&patch.pointers),
        applied: StatusCode::OK,
        location: None,
        precondition: Some(&kartenn_core::rules::CHANGESET_CONFLICT),
        shown: None,
    };
    submit_answered(&api, &context, &request, &answer).await
}

/// `DELETE /v1/<collection>/{id}` with `If-Match`: closes the object, from `valid_from`
/// of an optional body `{motive, valid_from}`.
pub(crate) async fn close(
    kind: Kind,
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    raw: Bytes,
) -> Result<Response, Problem> {
    let mut members = if raw.is_empty() {
        Map::new()
    } else {
        members(&headers, &raw, "application/json")?
    };
    if members.contains_key("valid_to") {
        return Err(bad("a closure has no valid_to"));
    }
    let reserved = reserved(&mut members)?;
    if let Some(unknown) = members.keys().next() {
        return Err(bad(format!("unknown member {unknown}")));
    }
    let expected = if_match(&headers)?;
    existing(&api, &context, kind, id).await?;
    let request = ChangesetRequest {
        author: principal.identity,
        motive: reserved.motive,
        period: period(&api, reserved.valid_from.as_deref(), None).await?,
        operations: vec![Operation::Close {
            kind: object_kind(kind),
            id: Ref::Id(id),
            expected: Some(expected),
            cascade: false,
        }],
        simulate: false,
        idempotency_key: None,
        apply_own: context.apply_own,
    };
    let pointers = [WHOLE.to_owned()];
    let answer = Answer {
        pointers: Pointers::Members(&pointers),
        applied: StatusCode::OK,
        location: None,
        precondition: Some(&kartenn_core::rules::CHANGESET_CONFLICT),
        shown: None,
    };
    submit_answered(&api, &context, &request, &answer).await
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use utoipa::PartialSchema;

    use super::*;

    fn with(name: header::HeaderName, value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(name, value.parse().unwrap());
        headers
    }

    #[test]
    fn if_match_names_one_strong_version() {
        let id = Uuid::now_v7();
        assert_eq!(
            if_match(&with(header::IF_MATCH, &format!("\"{id}\""))).unwrap(),
            id
        );
        assert_eq!(
            if_match(&with(
                header::IF_MATCH,
                &format!("\"{id}.0123456789abcdef\"")
            ))
            .unwrap(),
            id,
            "the version of a composite tag"
        );
        let kind = |value: &str| if_match(&with(header::IF_MATCH, value)).unwrap_err().kind();
        assert_eq!(
            if_match(&HeaderMap::new()).unwrap_err().kind(),
            ProblemKind::PreconditionRequired
        );
        assert_eq!(kind("*"), ProblemKind::PreconditionRequired);
        assert_eq!(
            kind(&format!("W/\"{id}\"")),
            ProblemKind::PreconditionFailed
        );
        assert_eq!(kind("\"abc\""), ProblemKind::PreconditionFailed);
        assert_eq!(kind("abc"), ProblemKind::BadRequest);
        assert_eq!(
            kind(&format!("\"{id}\", \"{id}\"")),
            ProblemKind::BadRequest
        );
    }

    #[test]
    fn a_patch_is_a_merge_patch_only() {
        let body = br#"{"slug": "lab"}"#;
        assert!(members(&with(header::CONTENT_TYPE, MERGE_PATCH), body, MERGE_PATCH).is_ok());
        for other in ["application/json-patch+json", "application/json"] {
            assert_eq!(
                members(&with(header::CONTENT_TYPE, other), body, MERGE_PATCH)
                    .unwrap_err()
                    .kind(),
                ProblemKind::UnsupportedMediaType,
                "{other}"
            );
        }
    }

    #[test]
    fn reserved_members_leave_the_object() {
        let Value::Object(mut members) = serde_json::json!({
            "motive": "m", "valid_from": null, "valid_to": "2999-01-01T00:00:00Z", "slug": "a"
        }) else {
            unreachable!()
        };
        let r = reserved(&mut members).unwrap();
        assert_eq!(r.motive, "m");
        assert_eq!(r.valid_from, None);
        assert_eq!(r.valid_to.as_deref(), Some("2999-01-01T00:00:00Z"));
        assert_eq!(members.keys().collect::<Vec<_>>(), ["slug"]);
    }

    #[test]
    fn a_pointer_escapes_its_key() {
        assert_eq!(token("a/b~c"), "a~1b~0c");
    }

    /// The members a schema names.
    fn named<T: PartialSchema>() -> BTreeSet<String> {
        let schema = serde_json::to_value(T::schema()).unwrap();
        schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn the_contract_of_a_patch_names_what_it_writes() {
        let patches = [
            (Kind::Domain, named::<DomainPatch>()),
            (Kind::Entity, named::<NodePatch>()),
            (Kind::Site, named::<NodePatch>()),
            (Kind::Prefix, named::<PrefixPatch>()),
            (Kind::Range, named::<RangePatch>()),
            (Kind::Address, named::<AddressPatch>()),
        ];
        for (kind, documented) in patches {
            let mut written: BTreeSet<String> =
                writable(kind).iter().map(|m| (*m).to_owned()).collect();
            written.insert("properties".to_owned());
            assert_eq!(documented, written, "{kind:?}");
        }
    }
}
