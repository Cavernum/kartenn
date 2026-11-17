//! Properties (spec 006, REQ-006-4; task 006-13a; spec 003): the declarations and the
//! values of the closed lists, which any authenticated caller reads (a schema, not data);
//! for an object, its resolved values, the explanation of one of them and the value its
//! source expects; the deviations between resolved and expected values. All are dated by
//! `valid_at` and `as_of`, but the suggestion, computed on the current state. The value of
//! a sensitive property needs `prop.<key>.read`, or it is masked (REQ-005-21); an object
//! the explanation walks that the caller may not read is shown as `null` (REQ-005-19).

use std::collections::{BTreeMap, BTreeSet};

use axum::Extension;
use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use kartenn_core::audit::rfc3339;
use kartenn_core::period::Period;
use kartenn_core::properties::{CORE_CARRIERS, is_module_kind};
use kartenn_core::rules::PROP_EXPECTED_MISMATCH;
use kartenn_service::RequestError;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::authz::Authorizer;
use kartenn_service::expected::suggest;
use kartenn_service::operations::RawInput;
use kartenn_service::reads::{objects_read, read_object, resolved_many, sensitive_disclosed};
use kartenn_store_pg::anomalies::{self, Anomaly, AnomalyFilter};
use kartenn_store_pg::explain::explain;
use kartenn_store_pg::list::Kind;
use kartenn_store_pg::property_read::{self, DeclarationFilter, DeclarationRead};
use kartenn_store_pg::read;
use kartenn_store_pg::write::PgStore;
use serde::Serialize;
use serde_json::{Value, json};
use utoipa::ToSchema;
use utoipa::openapi::path::HttpMethod;
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};
use utoipa_axum::routes;
use uuid::Uuid;

use crate::anomalies::ObjectRef;
use crate::cursor::{Listing, Page, PageParams, next_href, respond_page};
use crate::dated::{Dates, DatesParams, ETag, Moment, respond};
use crate::objects::{Api, Target, audited, bad, instant, store};
use crate::openapi::{documented, op, schemas_of};
use crate::problem::{Problem, ProblemKind};
use crate::quota::{Charge, ids_weight};
use crate::summary::{Summaries, Summary};

/// Most objects of a read of the resolved values of several objects (REQ-006-40).
pub const BATCH_MAX: usize = 200;

/// The routes of the properties.
pub fn routes() -> OpenApiRouter<Api> {
    let mut router = OpenApiRouter::new()
        .routes(routes!(declarations))
        .routes(routes!(values))
        .routes(routes!(gaps));
    for kind in Kind::ALL {
        router = object_routes(router, Target::Core(kind));
    }
    router
}

/// The properties of the objects of `target`, their explanation and their suggestion
/// (REQ-003-14, 23), added to `router`: for the kinds of the core and the types of the
/// modules (REQ-008-9).
pub(crate) fn object_routes(mut router: OpenApiRouter<Api>, target: Target) -> OpenApiRouter<Api> {
    let name = target.collection();
    let object = format!("The {}.", target.as_str());
    let mut schemas = Vec::new();
    schemas_of::<Resolved>(&mut schemas);
    schemas_of::<ResolvedBatch>(&mut schemas);
    schemas_of::<ExplanationDto>(&mut schemas);
    schemas_of::<SuggestionDto>(&mut schemas);
    router = router
        .routes(batch_route(target))
        .routes(documented(
            &format!("/{name}/{{id}}/properties"),
            vec![(
                HttpMethod::Get,
                op(
                    &format!("{}_properties", target.operation_name()),
                    "properties",
                    "The resolved values of the properties of the object.",
                )
                .path("id", &object, true)
                .params::<DatesParams>()
                .answers("200", "The resolved values.", "Resolved")
                .build(),
            )],
            schemas.clone(),
            get(
                move |state: State<Api>,
                      context: Extension<RequestContext>,
                      id: Path<Uuid>,
                      dates: Dates,
                      headers: HeaderMap| {
                    resolved(target, state, context, id, dates, headers)
                },
            ),
        ))
        .routes(documented(
            &format!("/{name}/{{id}}/properties/{{key}}/explanation"),
            vec![(
                HttpMethod::Get,
                op(
                    &format!("{}_property_explanation", target.operation_name()),
                    "properties",
                    "How the value of a property resolves on the object (REQ-003-14).",
                )
                .path("id", &object, true)
                .path("key", "The property.", false)
                .params::<DatesParams>()
                .answers("200", "The explanation.", "Explanation")
                .build(),
            )],
            Vec::new(),
            get(
                move |state: State<Api>,
                      context: Extension<RequestContext>,
                      path: Path<(Uuid, String)>,
                      dates: Dates,
                      headers: HeaderMap| {
                    explanation(target, state, context, path, dates, headers)
                },
            ),
        ))
        .routes(documented(
            &format!("/{name}/{{id}}/properties/{{key}}/suggestion"),
            vec![(
                HttpMethod::Get,
                op(
                    &format!("{}_property_suggestion", target.operation_name()),
                    "properties",
                    "The value the source of a property expects for the object (REQ-003-23).",
                )
                .path("id", &object, true)
                .path("key", "The property.", false)
                .params::<DatesParams>()
                .answers("200", "The suggestion.", "Suggestion")
                .build(),
            )],
            Vec::new(),
            get(
                move |state: State<Api>,
                      context: Extension<RequestContext>,
                      path: Path<(Uuid, String)>,
                      dates: Dates,
                      headers: HeaderMap| {
                    suggestion(target, state, context, path, dates, headers)
                },
            ),
        ));
    router
}

/// The resolved values of several objects of `target` (REQ-006-40): a static segment,
/// matched before the identifier of `/{name}/{id}` (task 006-31).
fn batch_route(target: Target) -> UtoipaMethodRouter<Api> {
    let description = format!(
        "An object, repeated, {BATCH_MAX} at most; one that does not exist at the dates, of \
         another collection or that the caller may not read is left out."
    );
    documented(
        &format!("/{}/properties", target.collection()),
        vec![(
            HttpMethod::Get,
            op(
                &format!("{}_properties_batch", target.operation_name()),
                "properties",
                "The resolved values of the properties of several objects (REQ-006-40).",
            )
            .repeated("id", &description, true, true)
            .repeated(
                "key",
                "A property, repeated: those only; every one declared by default.",
                false,
                false,
            )
            .params::<DatesParams>()
            .answers(
                "200",
                "The resolved values of each object, in the order of `id`.",
                "ResolvedBatch",
            )
            .build(),
        )],
        Vec::new(),
        get(
            move |state: State<Api>,
                  context: Extension<RequestContext>,
                  charge: Option<Extension<Charge>>,
                  query: RawQuery| {
                resolved_batch(target, state, context, charge, query)
            },
        ),
    )
}

/// `body` as a dated read, with the weak tag of its content.
fn respond_json<T: Serialize>(headers: &HeaderMap, moment: &Moment, body: &T) -> Response {
    let bytes = serde_json::to_vec(body).expect("JSON values serialise");
    let etag = ETag::page(&bytes);
    let response = ([(CONTENT_TYPE, "application/json")], bytes).into_response();
    respond(headers, moment, &etag, response)
}

async fn moment_of(api: &Api, dates: Dates) -> Result<Moment, Problem> {
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    dates.at(now, api.bound)
}

// ---------------------------------------------------------------- declarations

/// The source of the expected values of a property.
#[derive(Debug, Serialize, ToSchema)]
struct ExpectedItem {
    /// The source.
    source: String,
    /// `suggest`, `default` or `check`.
    mode: String,
}

/// The links of a declaration.
#[derive(Debug, Serialize, ToSchema)]
struct DeclarationLinks {
    /// The values of its closed list.
    values: String,
}

/// The declaration of a property (003).
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct DeclarationItem {
    /// Identifier.
    id: Uuid,
    /// The key.
    key: String,
    /// Its type.
    #[serde(rename = "type")]
    value_type: String,
    /// The parameters of its type.
    #[schema(value_type = Object)]
    params: Value,
    /// `single` or `set`.
    cardinality: String,
    /// The kinds of object that carry it.
    carriers: Vec<String>,
    /// The tree it is inherited along.
    inherit: Option<String>,
    /// Its values are sensitive (REQ-003-27).
    sensitive: bool,
    /// The source of its expected values.
    expected: Option<ExpectedItem>,
    /// Its labels by language, `{<tag>: <text>}` (spec 018, R20): the dictionary
    /// `GET /v1/labels` serves them resolved.
    #[schema(value_type = std::collections::BTreeMap<String, String>)]
    labels: Value,
    /// Links.
    links: DeclarationLinks,
}

pub(crate) fn declaration_of(found: &DeclarationRead) -> DeclarationItem {
    DeclarationItem {
        id: found.id,
        key: found.key.clone(),
        value_type: found.value_type.clone(),
        params: found.params.clone(),
        cardinality: found.cardinality.clone(),
        carriers: found.carriers.clone(),
        inherit: found.inherit.clone(),
        sensitive: found.sensitive,
        expected: found.expected.as_ref().map(|(source, mode)| ExpectedItem {
            source: source.clone(),
            mode: mode.clone(),
        }),
        labels: found.labels.clone(),
        links: DeclarationLinks {
            values: format!("/v1/property-declarations/{}/values", found.key),
        },
    }
}

/// The key after which a page starts, as [`Page::next`] wrote it.
fn after_key(page: &Page) -> Result<Option<String>, Problem> {
    page.after()
        .map(|key| serde_json::from_value::<String>(key.clone()))
        .transpose()
        .map_err(|_| Problem::new(ProblemKind::InvalidCursor))
}

/// The filters of the list of declarations among the parameters left by the page (task
/// 021-11p3, Q-021-27): the prefix `q` (see [`crate::filter::prefix`]) and `carrier`, a
/// kind of the core or a qualified type of a module, each at most once; anything else
/// → 400.
fn declaration_filter(
    params: &[(String, String)],
) -> Result<(Option<String>, Option<String>), Problem> {
    let mut rest = params.to_vec();
    let prefix = crate::filter::prefix(&mut rest)?;
    let mut carrier = None;
    for (name, value) in rest {
        if name != "carrier" {
            return Err(bad(format!("unknown parameter {name}")));
        }
        if carrier.is_some() {
            return Err(bad("carrier is given more than once"));
        }
        if !(CORE_CARRIERS.contains(&value.as_str()) || is_module_kind(&value)) {
            return Err(bad(format!(
                "carrier {value} is neither a kind of the core nor a qualified type of a module"
            )));
        }
        carrier = Some(value);
    }
    Ok((carrier, prefix))
}

/// `GET /v1/property-declarations`: the declarations at the dates of the read, by key;
/// `carrier` keeps those a kind carries, `q` those whose key or a label starts with it
/// (task 021-11p3, Q-021-27).
#[utoipa::path(
    get,
    path = "/property-declarations",
    operation_id = "list_property_declarations",
    tag = "properties",
    params(
        ("carrier" = Option<String>, Query, description = "A kind of the core (`prefix`) or a qualified type of a module (`<module>.<type>`): the declarations it carries; given once."),
        ("q" = Option<String>, Query, description = "Prefix of the key or of a label, in any language, ignoring case; 100 characters at most. With it, 20 a page by default."),
        DatesParams,
        PageParams,
    ),
    responses((status = 200, description = "The declarations, by key.", body = Listing<DeclarationItem>))
)]
async fn declarations(
    State(api): State<Api>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = "property-declarations";
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, name)?;
    let (carrier, prefix) = declaration_filter(&page.filters)?;
    let page = if prefix.is_some() {
        page.suggesting()
    } else {
        page
    };
    let after = after_key(&page)?;
    let moment = moment_of(&api, page.dates).await?;
    let size = usize::from(page.size);
    let filter = DeclarationFilter {
        carrier: carrier.as_deref(),
        prefix: prefix.as_deref(),
    };
    let mut found = property_read::declarations(
        &api.pool,
        moment.valid_at,
        Some(moment.as_of()),
        filter,
        after.as_deref(),
        i64::from(page.size) + 1,
    )
    .await
    .map_err(|e| store(&e))?;
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next(&api.ring, name, &moment, json!(found[size - 1].key));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let items: Vec<DeclarationItem> = found.iter().map(declaration_of).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// A value of a closed list: its key and its period (spec 018, REQ-018-22); its labels
/// come from the dictionary `GET /v1/labels` (space `value`).
#[derive(Debug, Serialize, ToSchema)]
struct EnumValueItem {
    /// Its key.
    key: String,
    /// Start of its validity (RFC 3339).
    valid_from: Option<String>,
    /// End of its validity, excluded (RFC 3339); `null` while it lasts.
    valid_to: Option<String>,
}

/// `GET /v1/property-declarations/{key}/values`: the values of a closed list valid at
/// `valid_at`, sorted by key, 20 by default; `q` keeps those whose key or a label starts
/// with it, ignoring case (spec 018, REQ-018-22); 404 for a key not declared at the
/// dates, 409 for a property of another type.
#[utoipa::path(
    get,
    path = "/property-declarations/{key}/values",
    operation_id = "list_property_values",
    tag = "properties",
    params(
        ("key" = String, Path, description = "The property."),
        ("q" = Option<String>, Query, description = "Prefix of the key or of a label, in any language, ignoring case; 100 characters at most."),
        DatesParams,
        PageParams,
    ),
    responses((status = 200, description = "The values of the closed list, sorted by key; 20 a page by default.", body = Listing<EnumValueItem>))
)]
async fn values(
    State(api): State<Api>,
    Path(key): Path<String>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let name = format!("property-declarations/{key}/values");
    let page =
        Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &name)?.suggesting();
    let prefix = page.only_prefix()?;
    let after = after_key(&page)?;
    let moment = moment_of(&api, page.dates).await?;
    let as_of = Some(moment.as_of());
    let declaration = property_read::declaration(&api.pool, &key, moment.valid_at, as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    if declaration.value_type != "enum" {
        return Err(Problem::new(ProblemKind::Conflict).detail(format!(
            "{key} is of type {}, not a closed list",
            declaration.value_type
        )));
    }
    let size = usize::from(page.size);
    let mut found = property_read::enum_values(
        &api.pool,
        declaration.id,
        moment.valid_at,
        as_of,
        after.as_deref(),
        prefix.as_deref(),
        i64::from(page.size) + 1,
    )
    .await
    .map_err(|e| store(&e))?;
    let next = (found.len() > size).then(|| {
        found.truncate(size);
        let cursor = page.next(&api.ring, &name, &moment, json!(found[size - 1].key));
        next_href(&format!("/v1/{name}"), &cursor)
    });
    let items: Vec<EnumValueItem> = found
        .into_iter()
        .map(|value| EnumValueItem {
            key: value.key,
            valid_from: value.valid_from.map(crate::cursor::rfc3339),
            valid_to: value.valid_to.map(crate::cursor::rfc3339),
        })
        .collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

// ---------------------------------------------------------------- one object

/// The links of the resolved values of an object.
#[derive(Debug, Serialize, ToSchema)]
struct ResolvedLinks {
    /// These values.
    #[serde(rename = "self")]
    this: String,
    /// The object.
    object: String,
}

/// The resolved values of the properties of an object.
#[derive(Debug, Serialize, ToSchema)]
struct Resolved {
    /// The values by key; `null`: « no value » explicitly. A value of type reference is
    /// the summary of its object, `{id}` alone if the caller may not read it or if it was
    /// absent then (REQ-006-42); an array of them for a set.
    #[schema(value_type = BTreeMap<String, Object>)]
    properties: BTreeMap<String, Option<Value>>,
    /// Where each value of `properties` comes from, by the same keys; a key of `masked`
    /// has none (task 021-11p1).
    sources: BTreeMap<String, SourceDto>,
    /// The keys of the values the caller may not see (REQ-005-21).
    masked: BTreeSet<String>,
    /// Links.
    links: ResolvedLinks,
}

/// Where a resolved value comes from (task 021-11p1): its explanation in short.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = ValueSource)]
struct SourceDto {
    /// The object that carries the assignment, the object itself or the origin it
    /// inherits from; `null` if the caller may not read it (REQ-005-19).
    #[schema(required = true)]
    origin: Option<Node>,
    /// The value is inherited: the object that carries it is another one.
    inherited: bool,
    /// How the value was obtained, as stored: `input` (typed), `source:<id>@<version>`
    /// (the default of a source of expected values, REQ-003-22), or
    /// `module:<module>#<motif>` (derived by a module).
    #[schema(required = true)]
    provenance: Option<String>,
}

/// `GET /v1/<collection>/{id}/properties`: the resolved values of the object and where each
/// comes from, its holder named only when the caller may read it; those the caller may not
/// see named in `masked`; 404 as the object.
async fn resolved(
    kind: Target,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = moment_of(&api, dates).await?;
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
    let holders: Vec<Uuid> = found
        .sources
        .values()
        .map(|source| source.holder)
        .chain([id])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    // The object itself among them, read already: its key with the others'.
    let mut nodes = visible_nodes(&api, &context, &holders, &moment).await?;
    nodes.entry(id).or_insert_with(|| Node {
        kind: kind.as_str(),
        id,
        label: None,
        href: kind.href(id),
    });
    let sources = found
        .sources
        .into_iter()
        .map(|(key, source)| {
            let shown = SourceDto {
                origin: nodes.get(&source.holder).cloned(),
                inherited: source.holder != id,
                provenance: source.provenance,
            };
            (key, shown)
        })
        .collect();
    let refs = ref_keys(&declared(&api, &moment).await?);
    let referred: Vec<Uuid> = found
        .properties
        .iter()
        .filter(|(key, _)| refs.contains(*key))
        .filter_map(|(_, value)| value.as_ref())
        .flat_map(references)
        .collect();
    let summaries =
        Summaries::read(&api, &context, referred, moment.valid_at, moment.as_of).await?;
    let body = Resolved {
        properties: with_summaries(found.properties, &refs, &summaries),
        sources,
        masked: found.masked,
        links: ResolvedLinks {
            this: format!("{}/properties", kind.href(id)),
            object: kind.href(id),
        },
    };
    Ok(respond_json(&headers, &moment, &body))
}

/// The declarations of the properties at the dates of `moment`.
async fn declared(api: &Api, moment: &Moment) -> Result<Vec<DeclarationRead>, Problem> {
    let all = property_read::DeclarationFilter::default();
    property_read::declarations(
        &api.pool,
        moment.valid_at,
        moment.as_of,
        all,
        None,
        i64::MAX,
    )
    .await
    .map_err(|e| store(&e))
}

/// The keys of the properties of type reference declared at `valid_at` as asserted at
/// `as_of`.
pub(crate) async fn declared_refs(
    api: &Api,
    valid_at: time::OffsetDateTime,
    as_of: Option<time::OffsetDateTime>,
) -> Result<BTreeSet<String>, Problem> {
    let all = property_read::DeclarationFilter::default();
    let found = property_read::declarations(&api.pool, valid_at, as_of, all, None, i64::MAX)
        .await
        .map_err(|e| store(&e))?;
    Ok(ref_keys(&found))
}

/// The keys of the properties of type reference among `declarations`.
fn ref_keys(declarations: &[DeclarationRead]) -> BTreeSet<String> {
    declarations
        .iter()
        .filter(|d| d.value_type == "ref")
        .map(|d| d.key.clone())
        .collect()
}

/// The objects a value of type reference names: its identifier, or those of a set.
pub(crate) fn references(value: &Value) -> Vec<Uuid> {
    let one = |v: &Value| v.as_str().and_then(|s| s.parse::<Uuid>().ok());
    match value {
        Value::Array(values) => values.iter().filter_map(one).collect(),
        other => one(other).into_iter().collect(),
    }
}

/// A value of type reference with each identifier it holds replaced by the summary of
/// its object among `summaries` (REQ-006-42): `{id}` alone for an object the caller may
/// not read or that was absent at the dates of the read.
pub(crate) fn summarised(value: Value, summaries: &Summaries) -> Value {
    let one = |v: Value| match v.as_str().and_then(|s| s.parse::<Uuid>().ok()) {
        Some(id) => serde_json::to_value(summaries.of(id)).expect("a summary serialises"),
        None => v,
    };
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(one).collect()),
        other => one(other),
    }
}

/// `properties` with the values of the keys among `refs` (of type reference) summarised.
fn with_summaries(
    properties: BTreeMap<String, Option<Value>>,
    refs: &BTreeSet<String>,
    summaries: &Summaries,
) -> BTreeMap<String, Option<Value>> {
    properties
        .into_iter()
        .map(|(key, value)| {
            let value = if refs.contains(&key) {
                value.map(|v| summarised(v, summaries))
            } else {
                value
            };
            (key, value)
        })
        .collect()
}

// ---------------------------------------------------------------- several objects

/// The resolved values of the properties of one object of a batch: those of the read of
/// the object, its links aside, and its identifier (REQ-006-40).
#[derive(Debug, Serialize, ToSchema)]
struct ResolvedItem {
    /// The object.
    id: Uuid,
    /// The values by key; `null`: « no value » explicitly; a value of type reference as
    /// in the read of one object (REQ-006-42).
    #[schema(value_type = BTreeMap<String, Object>)]
    properties: BTreeMap<String, Option<Value>>,
    /// Where each value of `properties` comes from, by the same keys; a key of `masked`
    /// has none.
    sources: BTreeMap<String, SourceDto>,
    /// The keys of the values the caller may not see (REQ-005-21).
    masked: BTreeSet<String>,
}

/// The resolved values of the properties of several objects (REQ-006-40).
#[derive(Debug, Serialize, ToSchema)]
struct ResolvedBatch {
    /// One per object read, in the order of the parameters `id`.
    items: Vec<ResolvedItem>,
}

/// What a read of several objects asks for.
#[derive(Debug, PartialEq, Eq)]
struct BatchQuery {
    /// The objects, once each, in the order given.
    ids: Vec<Uuid>,
    /// How many `id` were given: the weight of the read.
    given: usize,
    /// The properties asked for, once each; empty: all of them.
    keys: Vec<String>,
    /// Its dates.
    dates: Dates,
}

/// `id` (1 to [`BATCH_MAX`]), `key`, `valid_at` and `as_of` of `query`; anything else, a
/// malformed identifier or date → 400 that names the parameter.
fn batch_query(query: &str) -> Result<BatchQuery, Problem> {
    let dates = Dates::from_query(query)?;
    let (mut ids, mut keys, mut given) = (Vec::new(), Vec::new(), 0);
    for (name, value) in form_urlencoded::parse(query.as_bytes()) {
        match name.as_ref() {
            "id" => {
                given += 1;
                let id = value
                    .parse::<Uuid>()
                    .map_err(|_| bad("id must be a UUID"))?;
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            "key" => {
                if !keys.iter().any(|k: &String| *k == value) {
                    keys.push(value.into_owned());
                }
            }
            "valid_at" | "as_of" => {}
            other => return Err(bad(format!("unknown parameter {other}"))),
        }
    }
    if given == 0 {
        return Err(bad("id is required, once per object"));
    }
    if given > BATCH_MAX {
        return Err(bad(format!(
            "id is given {given} times, {BATCH_MAX} at most"
        )));
    }
    Ok(BatchQuery {
        ids,
        given,
        keys,
        dates,
    })
}

/// `GET /v1/<collection>/properties?id=…&id=…&key=…`: the resolved values of several
/// objects, each item equal to what `GET /v1/<collection>/{id}/properties` answers at the
/// same dates, its links aside (REQ-006-40). An object that does not exist at the dates,
/// of another collection or that the caller may not read now is left out, alike: no
/// refusal is recorded, as for a list (005: invisible is absent). One `data.objects.read`
/// for the batch, naming the objects read; one `data.sensitive.read` per value disclosed,
/// as the read of one object. It weighs one read per hundred identifiers begun. No `ETag`
/// (H23).
async fn resolved_batch(
    kind: Target,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    charge: Option<Extension<Charge>>,
    RawQuery(query): RawQuery,
) -> Response {
    let asked = match batch_query(query.as_deref().unwrap_or_default()) {
        Ok(asked) => asked,
        Err(problem) => return problem.into_response(),
    };
    if let Some(Extension(charge)) = charge
        && let Err(refused) = charge.weigh(ids_weight(asked.given))
    {
        return refused.into_response();
    }
    batch(&api, &context, kind, asked)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

/// The body of [`resolved_batch`] once its parameters are read and weighed.
async fn batch(
    api: &Api,
    context: &RequestContext,
    kind: Target,
    asked: BatchQuery,
) -> Result<Response, Problem> {
    let BatchQuery {
        mut ids,
        keys: asked_keys,
        dates,
        ..
    } = asked;
    let moment = moment_of(api, dates).await?;
    // Of the collection at the dates, then readable now: an absent object and an
    // unreadable one take the same path, the same queries, without a trace of refusal.
    let kinds = property_read::kind_names(&api.pool, &ids, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?;
    ids.retain(|id| kinds.get(id).map(String::as_str) == Some(kind.as_str()));
    if let Some(subject) = context.subject() {
        let denied: BTreeSet<Uuid> = api
            .authorizer()
            .authorize_many(&subject, &kind.read_permission(), &ids)
            .await?
            .into_iter()
            .collect();
        ids.retain(|id| !denied.contains(id));
    }
    let declarations = declared(api, &moment).await?;
    let refs = ref_keys(&declarations);
    let keys = if asked_keys.is_empty() {
        declarations.into_iter().map(|d| d.key).collect()
    } else {
        asked_keys.clone()
    };
    let rows = resolved_many(
        &PgStore::new(api.pool.clone()),
        &api.authorizer(),
        &api.journal,
        context,
        &ids,
        &keys,
        instant(moment.valid_at),
        moment.as_of.map(instant),
    )
    .await?;
    let criteria = json!({
        "collection": kind.collection(),
        "keys": (!asked_keys.is_empty()).then_some(&asked_keys),
        "valid_at": rfc3339(instant(moment.valid_at)),
        "as_of": moment.as_of.map(|at| rfc3339(instant(at))),
    });
    let event = objects_read(context, "object", &ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    // The holders of the values shown, named when the caller may read them: the objects
    // themselves among them, for their keys.
    let holders: Vec<Uuid> = rows
        .iter()
        .filter(|r| !r.masked)
        .filter_map(|r| r.origin)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let nodes = visible_nodes(api, context, &holders, &moment).await?;
    // The objects the values of type reference name, summarised together.
    let referred: Vec<Uuid> = rows
        .iter()
        .filter(|r| refs.contains(&r.property))
        .filter_map(|r| r.value.as_ref())
        .flat_map(references)
        .collect();
    let summaries = Summaries::read(api, context, referred, moment.valid_at, moment.as_of).await?;
    let mut items = items_of(kind, &ids, rows, &nodes);
    for item in &mut items {
        item.properties = with_summaries(std::mem::take(&mut item.properties), &refs, &summaries);
    }
    let bytes = serde_json::to_vec(&ResolvedBatch { items }).expect("JSON values serialise");
    Ok((
        [
            (CONTENT_TYPE, "application/json"),
            (CACHE_CONTROL, moment.cache_control()),
        ],
        bytes,
    )
        .into_response())
}

/// The item of each of `ids` from the resolved `rows`, as [`resolved`] shows one object:
/// a value that applies with its source, its holder among `nodes` or the object itself;
/// a masked one by its key only; nothing for a property that does not apply.
fn items_of(
    kind: Target,
    ids: &[Uuid],
    rows: Vec<kartenn_service::port::Resolved>,
    nodes: &BTreeMap<Uuid, Node>,
) -> Vec<ResolvedItem> {
    let mut items: Vec<ResolvedItem> = ids
        .iter()
        .map(|id| ResolvedItem {
            id: *id,
            properties: BTreeMap::new(),
            sources: BTreeMap::new(),
            masked: BTreeSet::new(),
        })
        .collect();
    let place: BTreeMap<Uuid, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    for row in rows {
        let (Some(origin), Some(&i)) = (row.origin, place.get(&row.object)) else {
            continue;
        };
        let item = &mut items[i];
        if row.masked {
            item.masked.insert(row.property);
            continue;
        }
        let shown = nodes.get(&origin).cloned().or_else(|| {
            (origin == row.object).then(|| Node {
                kind: kind.as_str(),
                id: origin,
                label: None,
                href: kind.href(origin),
            })
        });
        item.sources.insert(
            row.property.clone(),
            SourceDto {
                origin: shown,
                inherited: origin != row.object,
                provenance: row.provenance,
            },
        );
        item.properties.insert(row.property, row.value);
    }
    items
}

/// 404 unless the object `id` of `kind` exists at the dates of `moment` and the caller
/// may read it now; then the declaration of `key` at those dates, 404 without one.
async fn object_and_declaration(
    api: &Api,
    context: &RequestContext,
    kind: Target,
    id: Uuid,
    key: &str,
    moment: &Moment,
) -> Result<DeclarationRead, Problem> {
    let kinds = property_read::kind_names(&api.pool, &[id], moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?;
    if kinds.get(&id).map(String::as_str) != Some(kind.as_str()) {
        return Err(Problem::new(ProblemKind::NotFound));
    }
    crate::objects::gate_target(api, context, kind, id).await?;
    property_read::declaration(&api.pool, key, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))
}

/// Whether the caller may see the value of `declaration` on `id`: always for a property
/// that is not sensitive; a disclosure of a sensitive one is audited.
async fn discloses(
    api: &Api,
    context: &RequestContext,
    declaration: &DeclarationRead,
    id: Uuid,
) -> Result<bool, Problem> {
    if !declaration.sensitive {
        return Ok(true);
    }
    let disclosed = sensitive_disclosed(
        &api.authorizer(),
        &api.journal,
        context,
        &declaration.key,
        &[id],
    )
    .await?;
    Ok(disclosed.contains(&id))
}

/// Records the read of the property `key` of `id` (`data.objects.read`).
async fn audit_read(
    api: &Api,
    context: &RequestContext,
    kind: Target,
    id: Uuid,
    criteria: Value,
) -> Result<(), Problem> {
    let event =
        objects_read(context, kind.as_str(), &[id], criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    Ok(())
}

/// An object named by its kind, identifier and URL: one an explanation walks, or the
/// holder of a resolved value.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = NodeRef)]
struct Node {
    /// Its kind.
    #[schema(value_type = crate::openapi::enums::KindSchema)]
    kind: &'static str,
    /// Identifier.
    id: Uuid,
    /// Its natural key as a human reads it (REQ-006-42), as in a summary; absent for an
    /// object whose module tells none.
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// Its URL.
    href: String,
}

/// The objects among `ids` the caller may read now, as nodes `{kind, id, label, href}`;
/// the others, and those absent at the dates, are left out.
async fn visible_nodes(
    api: &Api,
    context: &RequestContext,
    ids: &[Uuid],
    moment: &Moment,
) -> Result<BTreeMap<Uuid, Node>, Problem> {
    let kinds = property_read::kind_names(&api.pool, ids, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?;
    let kinds: Vec<(Uuid, Target)> = kinds
        .into_iter()
        .filter_map(|(id, name)| Target::named(&name, &api.modules).map(|t| (id, t)))
        .collect();
    let mut by_kind: BTreeMap<&'static str, (Target, Vec<Uuid>)> = BTreeMap::new();
    for (id, kind) in &kinds {
        by_kind
            .entry(kind.as_str())
            .or_insert_with(|| (*kind, Vec::new()))
            .1
            .push(*id);
    }
    let mut denied = BTreeSet::new();
    if let Some(subject) = context.subject() {
        let authorizer = api.authorizer();
        for (kind, ids) in by_kind.values() {
            denied.extend(
                authorizer
                    .authorize_many(&subject, &kind.read_permission(), ids)
                    .await?,
            );
        }
    }
    let shown: Vec<(Uuid, Target)> = kinds
        .into_iter()
        .filter(|(id, _)| !denied.contains(id))
        .collect();
    let mut labels = Summaries::labels(
        api,
        shown.iter().map(|(id, _)| *id),
        moment.valid_at,
        moment.as_of,
    )
    .await?;
    Ok(shown
        .into_iter()
        .map(|(id, kind)| {
            let node = Node {
                kind: kind.as_str(),
                id,
                label: labels.remove(&id),
                href: kind.href(id),
            };
            (id, node)
        })
        .collect())
}

/// A resolved value written as JSON by the store.
fn value_of(text: Option<&str>) -> Result<Value, Problem> {
    text.map(serde_json::from_str)
        .transpose()
        .map(Option::unwrap_or_default)
        .map_err(|_| Problem::new(ProblemKind::Internal))
}

/// The assignment an explicit one replaces.
#[derive(Debug, Serialize, ToSchema)]
struct Replaced {
    /// Where it comes from; `null` if the caller may not read it.
    #[schema(required = true)]
    origin: Option<Node>,
    /// Its value; `null` when masked.
    #[schema(value_type = Object)]
    value: Value,
}

/// The links of an explanation or a suggestion.
#[derive(Debug, Serialize, ToSchema)]
struct ObjectLinks {
    /// The object.
    object: String,
}

/// How the value of a property resolves on an object (REQ-003-14, 15).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Explanation)]
struct ExplanationDto {
    /// The property.
    property: String,
    /// The resolved value; `null` when masked or without value.
    #[schema(value_type = Object)]
    value: Value,
    /// The caller may not see the value (REQ-005-21).
    masked: bool,
    /// The object the value comes from; `null` if the caller may not read it.
    #[schema(required = true)]
    origin: Option<Node>,
    /// The objects walked, nearest first; `null` for one the caller may not read.
    chain: Vec<Option<Node>>,
    /// How the value was obtained, as stored: `input` (typed), `source:<id>@<version>`
    /// (the default of a source of expected values, REQ-003-22), or
    /// `module:<module>#<motif>` (derived by a module, as `module:kube#label:<key>` from
    /// a label of a workload; ADR 0026, amendment 14).
    #[schema(required = true)]
    provenance: Option<String>,
    /// The assignment it replaces; `null` when it replaces none.
    #[schema(required = true)]
    replaces: Option<Replaced>,
    /// Links.
    links: ObjectLinks,
}

/// `GET /v1/<collection>/{id}/properties/{key}/explanation`: how the value of `key`
/// resolves on the object at the dates of the read (REQ-003-14, 15).
async fn explanation(
    kind: Target,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path((id, key)): Path<(Uuid, String)>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let moment = moment_of(&api, dates).await?;
    let declaration = object_and_declaration(&api, &context, kind, id, &key, &moment).await?;
    let found = explain(&api.pool, id, declaration.id, moment.valid_at, moment.as_of)
        .await
        .map_err(|e| store(&e))?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    let mut walked = found.chain.clone();
    walked.extend(found.origin);
    walked.extend(found.replaces.as_ref().map(|(origin, _)| *origin));
    let nodes = visible_nodes(&api, &context, &walked, &moment).await?;
    let node = |id: &Uuid| nodes.get(id).cloned();
    let criteria = json!({
        "object": id,
        "property": key,
        "explanation": true,
        "valid_at": rfc3339(instant(moment.valid_at)),
        "as_of": rfc3339(instant(moment.as_of())),
    });
    audit_read(&api, &context, kind, id, criteria).await?;
    let shown = discloses(&api, &context, &declaration, id).await?;
    let value = if shown {
        value_of(found.value.as_deref())?
    } else {
        Value::Null
    };
    let mut replaces = match &found.replaces {
        Some((origin, replaced)) => Some(Replaced {
            origin: node(origin),
            value: if shown {
                value_of(replaced.as_deref())?
            } else {
                Value::Null
            },
        }),
        None => None,
    };
    // A reference, and the one it replaces, summarised (REQ-006-42, task 006-33f3).
    let value = if declaration.value_type == "ref" {
        let former = replaces.as_ref().map(|r| &r.value);
        let referred: Vec<Uuid> = [Some(&value), former]
            .into_iter()
            .flatten()
            .flat_map(references)
            .collect();
        let summaries =
            Summaries::read(&api, &context, referred, moment.valid_at, moment.as_of).await?;
        if let Some(replaced) = &mut replaces {
            replaced.value = summarised(std::mem::take(&mut replaced.value), &summaries);
        }
        summarised(value, &summaries)
    } else {
        value
    };
    let body = ExplanationDto {
        property: found.property,
        value,
        masked: !shown,
        origin: found.origin.as_ref().and_then(node),
        chain: found.chain.iter().map(node).collect(),
        provenance: found.provenance,
        replaces,
        links: ObjectLinks {
            object: kind.href(id),
        },
    };
    Ok(respond_json(&headers, &moment, &body))
}

fn raw_value(input: &RawInput) -> Value {
    match input {
        RawInput::Bool(b) => json!(b),
        RawInput::Int(i) => json!(i),
        RawInput::Text(t) => json!(t),
        RawInput::External { system, id, label } => {
            json!({ "system": system, "id": id, "label": label })
        }
        RawInput::Local { local } => json!({ "local": local }),
    }
}

/// One value a source expects: a summary (an object with `id`) for a property of type
/// reference (its declaration's `value_type` is `ref`) and for it only; a raw value
/// (`RawValue`) for any other type.
#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
enum ExpectedValue {
    /// For a property of type reference, the summary of the object it names (REQ-006-42):
    /// `{id}` alone for an object the caller may not read or that is absent.
    Summary(Summary),
    /// Any other value, raw.
    #[schema(value_type = crate::operation::RawDto)]
    Raw(Value),
}

/// The value a source expects, or its mask.
#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
enum Expected {
    /// The values and why.
    Shown {
        /// One value, or the elements of a set.
        values: Vec<ExpectedValue>,
        /// How the source reached them.
        explanation: String,
    },
    /// The caller may not see them (REQ-005-21).
    Masked {
        /// `true`.
        masked: bool,
    },
}

/// The value the source of a property expects for an object (REQ-003-23).
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Suggestion)]
struct SuggestionDto {
    /// The property.
    property: String,
    /// The suggestion; `null` when the source has none, or more than one (REQ-016-5).
    suggestion: Option<Expected>,
    /// Every candidate of the source, in its order (REQ-016-5).
    candidates: Vec<Expected>,
    /// Links.
    links: ObjectLinks,
}

/// `GET /v1/<collection>/{id}/properties/{key}/suggestion`: the value the source of the
/// property expects for the object at `valid_at`, written nowhere (REQ-003-23); computed
/// on the current state, so `as_of` is refused.
async fn suggestion(
    kind: Target,
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path((id, key)): Path<(Uuid, String)>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    if dates.as_of.is_some() {
        return Err(bad("as_of: a suggestion is computed on the current state"));
    }
    let moment = moment_of(&api, dates).await?;
    let declaration = object_and_declaration(&api, &context, kind, id, &key, &moment).await?;
    let found = suggest(
        &PgStore::new(api.pool.clone()),
        &api.sources,
        declaration.id,
        id,
        Some(Period::from(instant(moment.valid_at))),
    )
    .await
    .map_err(Problem::from)?;
    let criteria = json!({
        "object": id,
        "property": key,
        "suggestion": true,
        "valid_at": rfc3339(instant(moment.valid_at)),
    });
    audit_read(&api, &context, kind, id, criteria).await?;
    let disclosed =
        !found.candidates().is_empty() && discloses(&api, &context, &declaration, id).await?;
    // The objects a reference names, summarised (REQ-006-42, task 006-33f3).
    let refs = disclosed && declaration.value_type == "ref";
    let referred: Vec<Uuid> = if refs {
        found
            .candidates()
            .iter()
            .flat_map(|s| s.values.iter().map(raw_value))
            .flat_map(|v| references(&v))
            .collect()
    } else {
        Vec::new()
    };
    let summaries =
        Summaries::read(&api, &context, referred, moment.valid_at, moment.as_of).await?;
    let expected = |raw: &RawInput| {
        let value = raw_value(raw);
        match value.as_str().and_then(|s| s.parse::<Uuid>().ok()) {
            Some(id) if refs => ExpectedValue::Summary(summaries.of(id)),
            _ => ExpectedValue::Raw(value),
        }
    };
    let shown = |s: &kartenn_service::expected::Suggestion| {
        if disclosed {
            Expected::Shown {
                values: s.values.iter().map(expected).collect(),
                explanation: s.explanation.clone(),
            }
        } else {
            Expected::Masked { masked: true }
        }
    };
    let body = SuggestionDto {
        property: declaration.key,
        suggestion: found.single().map(shown),
        candidates: found.candidates().iter().map(shown).collect(),
        links: ObjectLinks {
            object: kind.href(id),
        },
    };
    Ok(respond_json(&headers, &moment, &body))
}

// ---------------------------------------------------------------- deviations

/// The property a deviation is about: the one its detail names, as the `property` filter
/// reads it (021/A39); failing a detail, the key its message starts with, so that the
/// message of a sensitive property stays masked.
fn gap_property(anomaly: &Anomaly) -> Option<&str> {
    if anomaly.rule_id != PROP_EXPECTED_MISMATCH.id {
        return None;
    }
    anomaly
        .detail
        .as_ref()
        .and_then(|d| d.get("property"))
        .and_then(Value::as_str)
        .or_else(|| anomaly.message.split_once(':').map(|(key, _)| key))
}

/// Replaces the message of each deviation of a sensitive property that the caller may not
/// see on its object: the message cites the resolved value (REQ-005-21). Each message left
/// is a disclosure, audited.
pub(crate) async fn mask_gaps(
    api: &Api,
    context: &RequestContext,
    found: &mut [(Anomaly, Option<Kind>)],
) -> Result<(), Problem> {
    let mut sensitive: BTreeMap<String, Vec<Uuid>> = BTreeMap::new();
    let keys = property_read::sensitive_keys(&api.pool)
        .await
        .map_err(|e| store(&e))?;
    for (anomaly, _) in found.iter() {
        if let Some(key) = gap_property(anomaly).filter(|k| keys.contains(*k)) {
            sensitive
                .entry(key.to_owned())
                .or_default()
                .push(anomaly.object_id);
        }
    }
    let authorizer = api.authorizer();
    for (key, objects) in sensitive {
        let disclosed =
            sensitive_disclosed(&authorizer, &api.journal, context, &key, &objects).await?;
        for (anomaly, _) in found.iter_mut() {
            if gap_property(anomaly) == Some(key.as_str())
                && !disclosed.contains(&anomaly.object_id)
            {
                anomaly.message = masked_gap(&key);
                // Its detail cites both values as well.
                if let Some(Value::Object(detail)) = &mut anomaly.detail {
                    detail.remove("resolved");
                    detail.remove("expected");
                }
            }
        }
    }
    Ok(())
}

/// The message of a deviation of sensitive property `key`, without its values.
pub(crate) fn masked_gap(key: &str) -> String {
    format!("{key}: the resolved value differs from the expected one")
}

/// The filter of the deviations.
#[derive(Debug, Default, PartialEq, Eq)]
struct GapFilter {
    /// The property.
    property: Option<String>,
    /// The domain of the object.
    domain: Option<Uuid>,
    /// The entity of the object.
    entity: Option<Uuid>,
    /// The object (task 021-11p1, as 021/A37 for the anomalies).
    object: Option<Uuid>,
}

/// `property`, `domain`, `entity` and `object` among the parameters left by the page,
/// each at most once; anything else → 400.
fn gap_filter(params: &[(String, String)]) -> Result<GapFilter, Problem> {
    let (mut property, mut domain, mut entity, mut object) = (None, None, None, None);
    for (name, value) in params {
        let slot = match name.as_str() {
            "property" => &mut property,
            "domain" => &mut domain,
            "entity" => &mut entity,
            "object" => &mut object,
            other => return Err(bad(format!("unknown parameter {other}"))),
        };
        if slot.replace(value.clone()).is_some() {
            return Err(bad(format!("{name} is given more than once")));
        }
    }
    let id = |name: &str, value: Option<String>| {
        value
            .map(|v| v.parse::<Uuid>())
            .transpose()
            .map_err(|_| bad(format!("{name} must be a UUID")))
    };
    Ok(GapFilter {
        property,
        domain: id("domain", domain)?,
        entity: id("entity", entity)?,
        object: id("object", object)?,
    })
}

/// A deviation between a resolved and an expected value.
#[derive(Debug, Serialize, ToSchema)]
struct Gap {
    /// Identifier of its anomaly.
    id: Uuid,
    /// The property.
    property: Option<String>,
    /// Its object.
    object: ObjectRef,
    /// The code of its rule, `prop.resolution.expected_mismatch` (021/A36).
    rule: String,
    /// The resolved value, raw (closed-list keys; for a reference, the summary of its
    /// object, REQ-006-42); `null` without a value, when masked, or for a deviation not
    /// evaluated since its values are stored.
    resolved: Value,
    /// The value the source expects, in the form of the resolved one (an array for a
    /// set); `null` when masked, or for a deviation not evaluated since its values are
    /// stored.
    expected: Value,
    /// The source that expects it; `null` for a deviation not evaluated since its values
    /// are stored.
    source: Option<String>,
    /// The revision of the source it was evaluated against; `null` as `source`.
    revision: Option<String>,
    /// A readable message; generic for a sensitive property the caller may not see.
    message: String,
    /// Its opening, RFC 3339.
    opened_at: String,
}

impl Gap {
    /// The values of type reference of a deviation of a property among `refs`, raw.
    fn referred<'a>(anomaly: &'a Anomaly, refs: &BTreeSet<String>) -> Vec<&'a Value> {
        match (gap_property(anomaly), anomaly.detail.as_ref()) {
            (Some(key), Some(detail)) if refs.contains(key) => ["resolved", "expected"]
                .into_iter()
                .filter_map(|name| detail.get(name))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The deviation of `anomaly`, its values taken from its detail (021/A39), its object
    /// and, for a property among `refs` (of type reference), its values summarised among
    /// `summaries` (REQ-006-42).
    fn of(
        anomaly: &Anomaly,
        kind: Option<Kind>,
        refs: &BTreeSet<String>,
        summaries: &Summaries,
    ) -> Self {
        let member = |name: &str| {
            anomaly
                .detail
                .as_ref()
                .and_then(|d| d.get(name))
                .cloned()
                .unwrap_or_default()
        };
        let text = |name: &str| member(name).as_str().map(str::to_owned);
        let value = |name: &str| {
            let raw = member(name);
            if gap_property(anomaly).is_some_and(|key| refs.contains(key)) {
                summarised(raw, summaries)
            } else {
                raw
            }
        };
        Self {
            id: anomaly.id,
            property: gap_property(anomaly).map(str::to_owned),
            object: ObjectRef::new(kind, anomaly.object_id, summaries),
            rule: anomaly.rule_id.clone(),
            resolved: value("resolved"),
            expected: value("expected"),
            source: text("source"),
            revision: text("revision"),
            message: anomaly.message.clone(),
            opened_at: rfc3339(instant(anomaly.opened_at)),
        }
    }
}

/// `GET /v1/property-gaps?property=…&domain=…&entity=…&object=…`: the open deviations
/// between resolved and expected values (REQ-003-24) whose object the caller may read now;
/// those of one object as well (task 021-11p1), under the same filter of reading: an
/// object the caller may not read gives an empty list, as an unknown one (ADR 0021).
#[utoipa::path(
    get,
    path = "/property-gaps",
    operation_id = "list_property_gaps",
    tag = "properties",
    params(
        ("property" = Option<String>, Query, description = "The property."),
        ("domain" = Option<Uuid>, Query, description = "The domain of the object."),
        ("entity" = Option<Uuid>, Query, description = "The entity of the object."),
        ("object" = Option<Uuid>, Query, description = "The object."),
        PageParams,
    ),
    responses((status = 200, description = "The open deviations.", body = Listing<Gap>))
)]
async fn gaps(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    const NAME: &str = "property-gaps";
    let (page, after) = crate::anomalies::anomaly_page(&api, query.as_deref(), NAME)?;
    let GapFilter {
        property,
        domain,
        entity,
        object,
    } = gap_filter(&page.filters)?;
    let moment = moment_of(&api, Dates::default()).await?;
    let scope = api
        .authorizer()
        .read_scope(&principal.subject(), moment.now)
        .await?;
    let wanted = AnomalyFilter {
        rule: Some(PROP_EXPECTED_MISMATCH.id),
        domain,
        entity,
        property: property.as_deref(),
        object,
    };
    let limit = i64::from(page.size) + 1;
    let mut found = anomalies::open_page(&api.pool, &wanted, &scope, after, Some(limit))
        .await
        .map_err(|e| store(&e))?;
    let next = crate::anomalies::anomaly_next(&api, &page, NAME, &moment, &mut found);
    let ids: Vec<Uuid> = found.iter().map(|(a, _)| a.object_id).collect();
    let criteria = json!({
        "property_gaps": {
            "property": property, "domain": domain, "entity": entity, "object": object,
        }
    });
    let event = objects_read(&context, "object", &ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    mask_gaps(&api, &context, &mut found).await?;
    let refs = ref_keys(&declared(&api, &moment).await?);
    let referred = found
        .iter()
        .flat_map(|(anomaly, _)| Gap::referred(anomaly, &refs))
        .flat_map(references);
    let summaries = Summaries::read(
        &api,
        &context,
        ids.iter().copied().chain(referred),
        moment.valid_at,
        moment.as_of,
    )
    .await?;
    let items: Vec<Gap> = found
        .iter()
        .map(|(anomaly, kind)| Gap::of(anomaly, *kind, &refs, &summaries))
        .collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::*;

    fn anomaly(rule: &str, message: &str) -> Anomaly {
        Anomaly {
            id: Uuid::nil(),
            rule_id: rule.to_owned(),
            object_id: Uuid::nil(),
            message: message.to_owned(),
            detail: None,
            opened_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn a_deviation_names_its_property() {
        let gap = anomaly(
            PROP_EXPECTED_MISMATCH.id,
            "dns.ttl: resolved 60, expected …",
        );
        assert_eq!(gap_property(&gap), Some("dns.ttl"));
        assert_eq!(gap_property(&anomaly("core.other", "dns.ttl: …")), None);
        // Its detail first, as the `property` filter reads it.
        let detailed = Anomaly {
            detail: Some(json!({ "property": "dns.zone" })),
            ..gap
        };
        assert_eq!(gap_property(&detailed), Some("dns.zone"));
    }

    #[test]
    fn the_gaps_filter_takes_a_property_a_domain_an_entity_and_an_object() {
        let pairs = |query: &str| -> Vec<(String, String)> {
            form_urlencoded::parse(query.as_bytes())
                .map(|(n, v)| (n.into_owned(), v.into_owned()))
                .collect()
        };
        let (entity, object) = (Uuid::now_v7(), Uuid::now_v7());
        assert_eq!(
            gap_filter(&pairs(&format!(
                "property=dns.ttl&entity={entity}&object={object}"
            )))
            .unwrap(),
            GapFilter {
                property: Some("dns.ttl".to_owned()),
                entity: Some(entity),
                object: Some(object),
                ..GapFilter::default()
            }
        );
        assert!(gap_filter(&pairs("object=x")).is_err());
        assert!(gap_filter(&pairs(&format!("object={object}&object={object}"))).is_err());
        assert!(gap_filter(&pairs("rule=x")).is_err());
    }

    #[test]
    fn a_stored_value_reads_as_json() {
        assert_eq!(value_of(Some("[\"a\",\"b\"]")).unwrap(), json!(["a", "b"]));
        assert_eq!(value_of(None).unwrap(), Value::Null);
    }
}
