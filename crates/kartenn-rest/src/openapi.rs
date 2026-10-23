//! The contract of the API (spec 006, REQ-006-4, 33, 34; task 006-22): the OpenAPI 3.1
//! document of the build, drawn from the routes themselves ([`OpenApiRouter`]), served at
//! `GET /v1/openapi.json`. OpenAPI 3.1 has no `QUERY`: its operation is built by hand
//! and placed in the extension `x-query` of its path item (design « Versions et
//! contrat »). Every operation answers a problem (RFC 9457) by default.

use std::sync::LazyLock;

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::MethodRouter;
use serde_json::Value;
use utoipa::openapi::path::{
    HttpMethod, Operation, OperationBuilder, ParameterBuilder, ParameterIn, PathItem, PathsBuilder,
};
use utoipa::openapi::request_body::RequestBodyBuilder;
use utoipa::openapi::security::{
    ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityRequirement, SecurityScheme,
};
use utoipa::openapi::{
    Content, ContentBuilder, InfoBuilder, OpenApi, OpenApiBuilder, Ref, RefOr, Required,
    ResponseBuilder, Schema,
};
use utoipa::{IntoParams, PartialSchema, ToSchema};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};
use utoipa_axum::routes;

use sha2::{Digest, Sha256};

use crate::dated::ETag;
use crate::objects::Api;
use crate::problem::Problem;
use kartenn_service::module::ModuleRegistry;

/// The reference contract, from the directory of this crate (design « Versions et
/// contrat »).
pub const REFERENCE: &str = "../../specs/006-api-rest/openapi-v1.json";

/// The extension of a path item that holds its `QUERY` operation.
pub const QUERY_EXTENSION: &str = "x-query";

/// The extension that tells a client how to name and show a collection (on its path
/// item: `key`, `columns`) or how to call an operation (`command`) (023/A5, design
/// « Versions et contrat »).
pub const CLIENT_EXTENSION: &str = "x-kartenn";

/// The media type of a problem (RFC 9457 §3).
const PROBLEM_TYPE: &str = "application/problem+json";

/// The schemes of the authentication (REQ-006-24 to 26).
const SCHEMES: [&str; 3] = ["session", "bearer", "basic"];

/// The operations open to anonymous callers: the login routes and the probes.
const PUBLIC: [&str; 6] = [
    "/v1/auth/login",
    "/v1/auth/callback",
    "/v1/auth/breakglass",
    "/v1/auth/issuers",
    "/health/live",
    "/health/ready",
];

/// The document of the core without business module: the routes of `/v1`, those of the
/// login, the probes.
#[must_use]
pub fn document() -> OpenApi {
    assemble(crate::api().into_openapi(), ModuleRegistry::empty())
}

/// The document of a build: the core, the modules of `modules` and their routes,
/// `routers` (ADR 0026, clause 9).
#[must_use]
pub fn document_with(modules: &ModuleRegistry, routers: Vec<OpenApiRouter<Api>>) -> OpenApi {
    assemble(crate::api_with(modules, routers).into_openapi(), modules)
}

/// Writes the contract of a build, [`json_with`], to `path`: the reference of the
/// repository is that of the server with every module (`kartenn-server openapi`).
///
/// # Errors
///
/// When the file cannot be written.
///
/// # Panics
///
/// Never: an OpenAPI document serialises.
pub fn write_with(
    path: &std::path::Path,
    modules: &ModuleRegistry,
    routers: Vec<OpenApiRouter<Api>>,
) -> std::io::Result<()> {
    let mut text = serde_json::to_string_pretty(&json_with(modules, routers))
        .expect("an OpenAPI document serialises");
    text.push('\n');
    std::fs::write(path, text)
}

/// [`document_with`] as JSON, the form of the reference contract.
///
/// # Panics
///
/// Never: an OpenAPI document serialises.
#[must_use]
pub fn json_with(modules: &ModuleRegistry, routers: Vec<OpenApiRouter<Api>>) -> Value {
    serde_json::to_value(document_with(modules, routers)).expect("an OpenAPI document serialises")
}

/// The whole document around `v1`, the contract of the routes under `/v1`: the login
/// routes, the probes, the security; the operations name the types of the modules among
/// their kinds of object (REQ-008-5).
pub(crate) fn assemble(mut v1: OpenApi, modules: &ModuleRegistry) -> OpenApi {
    v1.merge(crate::auth::routes().into_openapi());
    let mut document = OpenApiBuilder::new()
        .info(
            InfoBuilder::new()
                .title("kartenn")
                .version(env!("CARGO_PKG_VERSION"))
                .description(Some("The REST API of kartenn (spec 006)."))
                .build(),
        )
        .build()
        .nest("/v1", v1)
        .merge_from(crate::health::routes().into_openapi());
    finish(&mut document);
    module_kinds(&mut document, modules);
    document
}

/// Adds the types of objects of `modules` to the kinds an operation names.
fn module_kinds(document: &mut OpenApi, modules: &ModuleRegistry) {
    let Some(RefOr::T(Schema::Object(kinds))) = document
        .components
        .as_mut()
        .and_then(|components| components.schemas.get_mut("TargetKind"))
    else {
        return;
    };
    let values = kinds.enum_values.get_or_insert_default();
    for ty in modules.object_types() {
        values.push(Value::from(ty.kind));
    }
}

/// The contract a router of `/v1` serves at `GET /v1/openapi.json`, its own routes, and
/// its strong tag (023/A5, REQ-023-2): the SHA-256 of the bytes served, computed once.
#[derive(Debug, Clone)]
pub struct Served {
    bytes: axum::body::Bytes,
    tag: ETag,
}

impl Served {
    /// The served form of `document`: compact JSON, its members sorted (through
    /// [`Value`], whose maps are ordered), so that two instances of the same build serve
    /// the same bytes under the same tag whatever the order of utoipa's hash maps.
    ///
    /// # Panics
    ///
    /// Never: an OpenAPI document serialises.
    #[must_use]
    pub fn of(document: &OpenApi) -> Self {
        let value = serde_json::to_value(document).expect("an OpenAPI document serialises");
        let bytes = serde_json::to_vec(&value).expect("JSON values serialise");
        let tag = ETag::fingerprint(crate::dated::hex(&Sha256::digest(&bytes)));
        Self {
            bytes: bytes.into(),
            tag,
        }
    }

    /// The strong tag of the document.
    #[must_use]
    pub fn etag(&self) -> &ETag {
        &self.tag
    }

    /// The document under `no-cache` and its tag; 304 without body when `If-None-Match`
    /// names the tag (RFC 9110 §13.1.2, §15.4.5).
    fn answer(&self, headers: &HeaderMap) -> Response {
        let tag = (ETAG, self.tag.header_value());
        let cache = (CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        if self.tag.none_match(headers) {
            return (StatusCode::NOT_MODIFIED, [tag, cache]).into_response();
        }
        let media = (CONTENT_TYPE, HeaderValue::from_static("application/json"));
        ([media, tag, cache], self.bytes.clone()).into_response()
    }
}

/// The document as JSON, its members sorted: the form of the reference contract.
///
/// # Panics
///
/// Never: an OpenAPI document serialises.
#[must_use]
pub fn json() -> Value {
    serde_json::to_value(document()).expect("an OpenAPI document serialises")
}

/// The problem by default of every operation, the security schemes and the public
/// operations.
fn finish(document: &mut OpenApi) {
    let components = document.components.get_or_insert_default();
    components
        .schemas
        .insert("Problem".to_owned(), Problem::schema());
    components.add_security_scheme(
        "session",
        SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(
            kartenn_auth::oidc::SESSION_COOKIE,
        ))),
    );
    components.add_security_scheme(
        "bearer",
        SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
    );
    components.add_security_scheme(
        "basic",
        SecurityScheme::Http(Http::new(HttpAuthScheme::Basic)),
    );
    document.security = Some(
        SCHEMES
            .iter()
            .map(|scheme| SecurityRequirement::new(*scheme, Vec::<String>::new()))
            .collect(),
    );
    for (path, item) in &mut document.paths.paths {
        let public = PUBLIC.contains(&path.as_str());
        for operation in operations(item) {
            operation
                .responses
                .responses
                .entry("default".to_owned())
                .or_insert_with(|| problem().into());
            if public {
                operation.security = Some(Vec::new());
            }
        }
    }
}

/// The operations of a path item, but its `QUERY`, built whole.
fn operations(item: &mut PathItem) -> impl Iterator<Item = &mut Operation> {
    [
        &mut item.get,
        &mut item.put,
        &mut item.post,
        &mut item.delete,
        &mut item.patch,
    ]
    .into_iter()
    .flatten()
}

/// The response of a problem.
pub(crate) fn problem() -> utoipa::openapi::Response {
    ResponseBuilder::new()
        .description("A problem (RFC 9457).")
        .content(
            PROBLEM_TYPE,
            ContentBuilder::new()
                .schema(Some(Ref::from_schema_name("Problem")))
                .build(),
        )
        .build()
}

/// A JSON content of the schema named after `T`.
pub(crate) fn json_of<T: ToSchema>() -> Content {
    ContentBuilder::new()
        .schema(Some(Ref::from_schema_name(T::name())))
        .build()
}

/// The schema of `T` and those it refers to, for the components.
pub fn schemas_of<T: ToSchema>(schemas: &mut Vec<(String, RefOr<Schema>)>) {
    schemas.push((T::name().into_owned(), T::schema()));
    T::schemas(schemas);
}

/// An operation built by hand, for the routes one handler serves for each collection
/// (`#[utoipa::path]` names one path), and for the routes of the modules.
pub struct Op(OperationBuilder);

impl std::fmt::Debug for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Op").finish_non_exhaustive()
    }
}

/// An operation `id` of `tag`.
#[must_use]
pub fn op(id: &str, tag: &str, summary: &str) -> Op {
    Op(OperationBuilder::new()
        .operation_id(Some(id))
        .tag(tag)
        .summary(Some(summary)))
}

impl Op {
    /// A parameter of the path, a UUID or a string.
    #[must_use]
    pub fn path(self, name: &str, description: &str, uuid: bool) -> Self {
        let schema: RefOr<Schema> = if uuid {
            utoipa::openapi::ObjectBuilder::new()
                .schema_type(utoipa::openapi::schema::Type::String)
                .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
                    utoipa::openapi::KnownFormat::Uuid,
                )))
                .into()
        } else {
            String::schema()
        };
        Self(
            self.0.parameter(
                ParameterBuilder::new()
                    .name(name)
                    .parameter_in(ParameterIn::Path)
                    .required(Required::True)
                    .description(Some(description))
                    .schema(Some(schema))
                    .build(),
            ),
        )
    }

    /// A parameter of the query string.
    #[must_use]
    pub fn query(self, name: &str, description: &str) -> Self {
        Self(
            self.0.parameter(
                ParameterBuilder::new()
                    .name(name)
                    .parameter_in(ParameterIn::Query)
                    .required(Required::False)
                    .description(Some(description))
                    .schema(Some(String::schema()))
                    .build(),
            ),
        )
    }

    /// The parameters of `P`.
    #[must_use]
    pub fn params<P: IntoParams>(self) -> Self {
        Self(
            self.0
                .parameters(Some(P::into_params(|| Some(ParameterIn::Query)))),
        )
    }

    /// A header.
    #[must_use]
    pub fn header(self, name: &str, required: bool, description: &str) -> Self {
        Self(
            self.0.parameter(
                ParameterBuilder::new()
                    .name(name)
                    .parameter_in(ParameterIn::Header)
                    .required(if required {
                        Required::True
                    } else {
                        Required::False
                    })
                    .description(Some(description))
                    .schema(Some(String::schema()))
                    .build(),
            ),
        )
    }

    /// A body of media type `media`, of the schema named `schema`.
    #[must_use]
    pub fn body(self, media: &str, schema: &str, required: bool) -> Self {
        Self(
            self.0.request_body(Some(
                RequestBodyBuilder::new()
                    .required(Some(if required {
                        Required::True
                    } else {
                        Required::False
                    }))
                    .content(
                        media,
                        ContentBuilder::new()
                            .schema(Some(Ref::from_schema_name(schema)))
                            .build(),
                    )
                    .build(),
            )),
        )
    }

    /// A JSON response `status`, of the schema named `schema`.
    #[must_use]
    pub fn answers(self, status: &str, description: &str, schema: &str) -> Self {
        Self(
            self.0.response(
                status,
                ResponseBuilder::new()
                    .description(description)
                    .content(
                        "application/json",
                        ContentBuilder::new()
                            .schema(Some(Ref::from_schema_name(schema)))
                            .build(),
                    )
                    .build(),
            ),
        )
    }

    /// The operation.
    #[must_use]
    pub fn build(self) -> Operation {
        self.0.build()
    }
}

/// A route documented by hand: its path item at `path`, the schemas it names.
pub fn documented<S>(
    path: &str,
    operations: Vec<(HttpMethod, Operation)>,
    schemas: Vec<(String, RefOr<Schema>)>,
    router: MethodRouter<S>,
) -> UtoipaMethodRouter<S> {
    let mut item = PathItem::default();
    for (method, operation) in operations {
        item.merge_operations(PathItem::new(method, operation));
    }
    let paths = PathsBuilder::new().path(path, item).build();
    (schemas, paths, router)
}

/// The schema of a page of `T`, under the name `#[utoipa::path]` gives it.
pub fn listing_of<T: ToSchema + 'static>() -> (String, RefOr<Schema>) {
    (
        format!("Listing_{}", T::name()),
        <crate::cursor::Listing<'static, T> as PartialSchema>::schema(),
    )
}

/// Places the `QUERY` operation of the collection at `path` of `router`, read by
/// [`crate::filter::collection`], in the extension `x-query` of its path item: the filters
/// in a JSON body ([`crate::filter::Search`]), the responses of its `GET`.
///
/// # Panics
///
/// When `path` is not routed with a `GET` in `router`.
pub fn queryable<S>(mut router: OpenApiRouter<S>, path: &str, id: &str) -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    let mut schemas = Vec::new();
    schemas_of::<crate::filter::Search>(&mut schemas);
    let document = router.get_openapi_mut();
    document
        .components
        .get_or_insert_default()
        .schemas
        .extend(schemas);
    let item = document
        .paths
        .paths
        .get_mut(path)
        .expect("the path of a QUERY is routed");
    let get = item.get.as_ref().expect("a QUERY stands beside a GET");
    let mut operation = OperationBuilder::new()
        .operation_id(Some(id))
        .summary(get.summary.clone())
        .tags(get.tags.clone())
        .request_body(Some(
            RequestBodyBuilder::new()
                .required(Some(Required::True))
                .content("application/json", json_of::<crate::filter::Search>())
                .build(),
        ))
        .build();
    operation.responses = get.responses.clone();
    operation
        .responses
        .responses
        .entry("default".to_owned())
        .or_insert_with(|| problem().into());
    item.extensions.get_or_insert_default().merge(
        utoipa::openapi::extensions::ExtensionsBuilder::new()
            .add(
                QUERY_EXTENSION,
                serde_json::to_value(&operation).expect("an operation serialises"),
            )
            .build(),
    );
    router
}

/// Describes the collection at `path` of `router` to the clients, in the extension
/// `x-kartenn` of its path item (023/A5, REQ-023-27): `key`, the filters of its list whose
/// equality names one object at a date (its natural key; empty when it has none), in the
/// order a reference gives them (`<domain>/<value>`); `columns`, the members a table shows
/// by default. The library and the CLI work without it, by convention (ADR 0035).
///
/// # Panics
///
/// When `path` is not routed with a `GET` in `router`, or a member of `key` is not a query
/// parameter of that `GET`: a mistake of the module, caught by its first test.
pub fn presented<S>(
    mut router: OpenApiRouter<S>,
    path: &str,
    key: &[&str],
    columns: &[&str],
) -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    let item = router
        .get_openapi_mut()
        .paths
        .paths
        .get_mut(path)
        .unwrap_or_else(|| panic!("the collection {path} is routed"));
    let get = item
        .get
        .as_ref()
        .unwrap_or_else(|| panic!("the collection {path} is listed by a GET"));
    let filters: Vec<&str> = get
        .parameters
        .iter()
        .flatten()
        .filter_map(|p| match p {
            RefOr::T(p) if matches!(p.parameter_in, ParameterIn::Query) => Some(p.name.as_str()),
            _ => None,
        })
        .collect();
    for member in key {
        assert!(
            filters.contains(member),
            "the key {member} of {path} is a filter of its list"
        );
    }
    extend(
        &mut item.extensions,
        serde_json::json!({ "key": key, "columns": columns }),
    );
    router
}

/// Names `method` at `path` of `router` as the command `command` of the clients, in the
/// extension `x-kartenn` of the operation (023/A5): `kartenn policy check` rather than
/// `kartenn verdicts list`.
///
/// # Panics
///
/// When `path` is not routed with `method` in `router`.
pub fn commanded<S>(
    mut router: OpenApiRouter<S>,
    path: &str,
    method: &HttpMethod,
    command: &str,
) -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    let item = router
        .get_openapi_mut()
        .paths
        .paths
        .get_mut(path)
        .unwrap_or_else(|| panic!("{path} is routed"));
    let operation = match method {
        HttpMethod::Get => item.get.as_mut(),
        HttpMethod::Put => item.put.as_mut(),
        HttpMethod::Post => item.post.as_mut(),
        HttpMethod::Delete => item.delete.as_mut(),
        HttpMethod::Patch => item.patch.as_mut(),
        HttpMethod::Options | HttpMethod::Head | HttpMethod::Trace => None,
    }
    .unwrap_or_else(|| panic!("{path} is routed with the method of {command}"));
    extend(
        &mut operation.extensions,
        serde_json::json!({ "command": command }),
    );
    router
}

/// Merges `members` into the extension `x-kartenn` of `extensions`.
fn extend(extensions: &mut Option<utoipa::openapi::extensions::Extensions>, members: Value) {
    let current = extensions
        .get_or_insert_default()
        .entry(CLIENT_EXTENSION.to_owned())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let (Value::Object(current), Value::Object(members)) = (current, members) {
        current.extend(members);
    }
}

// ---------------------------------------------------------------- the document itself

static DOCUMENT: LazyLock<Served> = LazyLock::new(|| Served::of(&document()));

/// The OpenAPI 3.1 document of this build (REQ-006-34).
#[utoipa::path(
    get,
    path = "/openapi.json",
    operation_id = "openapi",
    tag = "operations",
    responses(
        (status = 200, description = "The OpenAPI 3.1 document of the build, under a strong \
            `ETag` (the SHA-256 of its bytes) and `Cache-Control: no-cache`.", body = Object),
        (status = 304, description = "`If-None-Match` names the tag of the document.")
    )
)]
async fn openapi(served: Option<axum::Extension<Served>>, headers: HeaderMap) -> Response {
    match served {
        Some(axum::Extension(served)) => served.answer(&headers),
        None => DOCUMENT.answer(&headers),
    }
}

/// The route of the document, under `/v1`.
pub(crate) fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new().routes(routes!(openapi))
}

// ---------------------------------------------------------------- the enumerations of the core

/// The schemas of the enumerations of the core, which knows nothing of OpenAPI: each
/// mirror has the variants of its model, and the matches of [`mirrors`] break the build
/// when the model gains one (a test checks that both serialise alike). They name schemas
/// only: nothing builds them but the test.
#[allow(dead_code)]
pub(crate) mod enums {
    use serde::Serialize;
    use utoipa::ToSchema;

    /// The nature of a prefix.
    #[derive(Debug, Serialize, ToSchema)]
    #[serde(rename_all = "snake_case")]
    #[schema(as = Nature)]
    pub(crate) enum NatureSchema {
        Container,
        Network,
    }

    /// The usage of a range.
    #[derive(Debug, Serialize, ToSchema)]
    #[serde(rename_all = "snake_case")]
    #[schema(as = RangeUsage)]
    pub(crate) enum RangeUsageSchema {
        Dhcp,
        Reserved,
        Static,
    }

    /// The usage of an address.
    #[derive(Debug, Serialize, ToSchema)]
    #[serde(rename_all = "snake_case")]
    #[schema(as = AddressUsage)]
    pub(crate) enum AddressUsageSchema {
        Host,
        Gateway,
        Vip,
        Anycast,
        Reserved,
    }

    /// The kind of an object of the core.
    #[derive(Debug, Serialize, ToSchema)]
    #[serde(rename_all = "snake_case")]
    #[schema(as = ObjectKind)]
    pub(crate) enum KindSchema {
        Entity,
        Site,
        Domain,
        Prefix,
        Range,
        Address,
    }

    /// The mirror of each value of the core, as JSON both.
    #[cfg(test)]
    pub(super) fn mirrors() -> Vec<(serde_json::Value, serde_json::Value)> {
        use kartenn_core::objects::{AddressUsage, Nature, RangeUsage};
        use kartenn_store_pg::list::Kind;

        fn json<T: Serialize>(value: &T) -> serde_json::Value {
            serde_json::to_value(value).expect("an enumeration serialises")
        }
        let mut pairs = Vec::new();
        for n in [Nature::Container, Nature::Network] {
            let m = match n {
                Nature::Container => NatureSchema::Container,
                Nature::Network => NatureSchema::Network,
            };
            pairs.push((json(&n), json(&m)));
        }
        for u in [RangeUsage::Dhcp, RangeUsage::Reserved, RangeUsage::Static] {
            let m = match u {
                RangeUsage::Dhcp => RangeUsageSchema::Dhcp,
                RangeUsage::Reserved => RangeUsageSchema::Reserved,
                RangeUsage::Static => RangeUsageSchema::Static,
            };
            pairs.push((json(&u), json(&m)));
        }
        for u in [
            AddressUsage::Host,
            AddressUsage::Gateway,
            AddressUsage::Vip,
            AddressUsage::Anycast,
            AddressUsage::Reserved,
        ] {
            let m = match u {
                AddressUsage::Host => AddressUsageSchema::Host,
                AddressUsage::Gateway => AddressUsageSchema::Gateway,
                AddressUsage::Vip => AddressUsageSchema::Vip,
                AddressUsage::Anycast => AddressUsageSchema::Anycast,
                AddressUsage::Reserved => AddressUsageSchema::Reserved,
            };
            pairs.push((json(&u), json(&m)));
        }
        for k in Kind::ALL {
            let m = match k {
                Kind::Entity => KindSchema::Entity,
                Kind::Site => KindSchema::Site,
                Kind::Domain => KindSchema::Domain,
                Kind::Prefix => KindSchema::Prefix,
                Kind::Range => KindSchema::Range,
                Kind::Address => KindSchema::Address,
            };
            pairs.push((serde_json::Value::from(k.as_str()), json(&m)));
        }
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mirrors_serialise_as_the_core() {
        for (core, mirror) in enums::mirrors() {
            assert_eq!(core, mirror);
        }
    }
}
