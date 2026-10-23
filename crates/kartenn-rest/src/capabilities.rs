//! The capabilities of the build (spec 006, REQ-006-35; task 006-22; D7, D17): its
//! modules and versions, the properties declared, the plugins of the interface and the
//! limits a client must respect, at `GET /v1/capabilities`.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use kartenn_service::module::ModuleRegistry;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_store_pg::property_read;
use kartenn_store_pg::read;
use kartenn_store_pg::write::IDEMPOTENCY_RETENTION_HOURS;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::cursor::{DEFAULT_SIZE, MAX_SIZE};
use crate::imports::MAX_CHUNK;
use crate::objects::{Api, store};
use crate::problem::Problem;
use crate::transport::Settings;
use crate::web::Web;
use crate::web::plugins::{Extension as UiExtension, Published};

/// The major versions of the API this build serves (REQ-006-33).
pub const API_VERSIONS: [&str; 1] = ["v1"];

/// The most bytes of a request body, as the transport bounds it (layer 5): set on every
/// request by [`crate::router`].
#[derive(Debug, Clone, Copy)]
pub struct BodyLimit(pub usize);

/// A module of the build: a tag of its contract.
#[derive(Debug, Serialize, ToSchema)]
struct Module {
    /// Its name, the tag of its operations in the contract.
    name: String,
    /// The version of the build that serves it.
    version: &'static str,
}

/// A property declared now.
#[derive(Debug, Serialize, ToSchema)]
struct DeclaredProperty {
    /// The key.
    key: String,
    /// Its type.
    #[serde(rename = "type")]
    value_type: String,
}

/// A plugin of the interface (D17; spec 007, REQ-007-7): what the shell needs to load it
/// with the integrity of each file.
#[derive(Debug, Serialize, ToSchema)]
struct UiPlugin {
    /// Its name.
    name: String,
    /// Its version.
    version: String,
    /// The range of versions of the SDK it expects (`^1.0`, `~1.2`, `1.2.3`).
    sdk: String,
    /// The URL of its directory, `/plugins/<name>/<version>/`.
    base: String,
    /// The URL of its entry module.
    entry: String,
    /// The SRI digest (`sha384-…`) of each module, style sheet and catalogue, by path
    /// relative to `base`.
    integrity: BTreeMap<String, String>,
    /// Its catalogues of messages by language, by path relative to `base`.
    catalogs: BTreeMap<String, String>,
    /// The points of extension it fills.
    extensions: Vec<UiExtension>,
}

impl From<Published> for UiPlugin {
    fn from(p: Published) -> Self {
        Self {
            name: p.name,
            version: p.version,
            sdk: p.sdk,
            base: p.base,
            entry: p.entry,
            integrity: p.integrity,
            catalogs: p.catalogs,
            extensions: p.extensions,
        }
    }
}

/// The limits a client must respect.
#[derive(Debug, Serialize, ToSchema)]
struct Limits {
    /// The size of a page by default.
    page_size_default: u16,
    /// The largest page.
    page_size_max: u16,
    /// The most operations of a changeset.
    changeset_operations: usize,
    /// The most elements of a chunk of an import.
    chunk_elements: usize,
    /// The most bytes of a request body.
    body_bytes: usize,
    /// How long an idempotency key holds, in seconds.
    idempotency_retention_seconds: u32,
}

/// The bounds of the filters of the event stream.
#[derive(Debug, Serialize, ToSchema)]
struct EventFilters {
    /// Most qualified types in `types`.
    types: usize,
    /// Most domains in `domains`.
    domains: usize,
    /// Most objects in `objects`.
    objects: usize,
}

/// The event stream `GET /v1/events` (spec 022, REQ-022-23).
#[derive(Debug, Serialize, ToSchema)]
struct EventsCapability {
    /// The bounds of its filters.
    filters: EventFilters,
    /// Most objects an event names before it names types.
    event_objects: usize,
    /// Open streams per identity.
    streams_per_identity: usize,
    /// How long an instance keeps an event for a resumption, in seconds.
    ring_seconds: u64,
    /// Seconds of silence before a comment.
    heartbeat_seconds: u64,
}

/// The capabilities of the build.
#[derive(Debug, Serialize, ToSchema)]
struct Capabilities {
    /// The version of the build.
    version: &'static str,
    /// The major versions of the API served.
    api_versions: Vec<&'static str>,
    /// The modules of the build.
    modules: Vec<Module>,
    /// The properties declared now.
    properties: Vec<DeclaredProperty>,
    /// The plugins of the interface.
    ui_plugins: Vec<UiPlugin>,
    /// The limits.
    limits: Limits,
    /// The event stream; `null` when it is off.
    events: Option<EventsCapability>,
}

/// The routes of the capabilities, under `/v1`.
pub(crate) fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new().routes(routes!(capabilities))
}

/// The names of the modules of the build: a plugin of another module is not published
/// (REQ-007-11).
#[must_use]
pub fn module_names(registry: &ModuleRegistry) -> std::collections::BTreeSet<String> {
    modules(registry)
        .into_iter()
        .map(|module| module.name)
        .collect()
}

/// The modules of the build: the tags of its contract and the business modules of the
/// registry (ADR 0026, clause 1), each once.
fn modules(registry: &ModuleRegistry) -> Vec<Module> {
    let document = crate::openapi::json();
    let mut names: Vec<String> = document["paths"]
        .as_object()
        .into_iter()
        .flat_map(|paths| paths.values())
        .filter_map(serde_json::Value::as_object)
        .flat_map(|item| item.values())
        .filter_map(|operation| operation.get("tags").and_then(|t| t.as_array()))
        .flatten()
        .filter_map(|tag| tag.as_str().map(str::to_owned))
        .filter(|name| registry.get(name).is_none())
        .collect();
    names.sort();
    names.dedup();
    let mut modules: Vec<Module> = names
        .into_iter()
        .map(|name| Module {
            name,
            version: env!("CARGO_PKG_VERSION"),
        })
        .chain(registry.modules().map(|m| Module {
            name: m.name().to_owned(),
            version: m.version(),
        }))
        .collect();
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    modules
}

/// `GET /v1/capabilities`: what this build serves and the limits it sets (REQ-006-35).
#[utoipa::path(
    get,
    path = "/capabilities",
    operation_id = "capabilities",
    tag = "operations",
    responses((status = 200, description = "The capabilities of the build.", body = Capabilities))
)]
async fn capabilities(
    State(api): State<Api>,
    limit: Option<Extension<BodyLimit>>,
    web: Option<Extension<Arc<Web>>>,
    hub: Option<Extension<Arc<crate::events::hub::Hub>>>,
) -> Result<Response, Problem> {
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let declared = property_read::declarations(&api.pool, now, None, None, i64::MAX)
        .await
        .map_err(|e| store(&e))?;
    let body_bytes = limit.map_or_else(
        || Settings::default().max_body_bytes,
        |Extension(BodyLimit(bytes))| bytes,
    );
    Ok(Json(Capabilities {
        version: env!("CARGO_PKG_VERSION"),
        api_versions: API_VERSIONS.to_vec(),
        modules: modules(&api.modules),
        properties: declared
            .into_iter()
            .map(|d| DeclaredProperty {
                key: d.key,
                value_type: d.value_type,
            })
            .collect(),
        ui_plugins: web
            .as_ref()
            .and_then(|Extension(web)| web.plugins())
            .map(|plugins| {
                plugins
                    .published()
                    .into_iter()
                    .map(UiPlugin::from)
                    .collect()
            })
            .unwrap_or_default(),
        limits: Limits {
            page_size_default: DEFAULT_SIZE,
            page_size_max: MAX_SIZE,
            changeset_operations: MAX_OPERATIONS,
            chunk_elements: MAX_CHUNK,
            body_bytes,
            idempotency_retention_seconds: IDEMPOTENCY_RETENTION_HOURS.unsigned_abs() * 3_600,
        },
        events: hub.map(|Extension(hub)| EventsCapability {
            filters: EventFilters {
                types: kartenn_service::events::MAX_TYPES,
                domains: kartenn_service::events::MAX_DOMAINS,
                objects: kartenn_service::events::MAX_OBJECTS,
            },
            event_objects: hub.settings.bound,
            streams_per_identity: hub.settings.per_identity,
            ring_seconds: hub.settings.ring_age.as_secs(),
            heartbeat_seconds: hub.settings.heartbeat.as_secs(),
        }),
    })
    .into_response())
}
