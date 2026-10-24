//! The contract (spec 006, REQ-006-4, 33 to 35; task 006-22): the OpenAPI 3.1 document of
//! the build names every operation of the surface, keeps its reference of the repository
//! (a member removed fails the comparison), carries `QUERY` in its extension; it is served
//! with the capabilities; a retired route announces its successor.

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::contract::breaches;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::kit::{commanded, documented, op, presented};
use kartenn_rest::objects::Api;
use kartenn_rest::openapi::{CLIENT_EXTENSION, QUERY_EXTENSION, REFERENCE, json as produced};
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_rest::versions::{Retirement, retire};
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::module::{CORE_COLLECTIONS, Module, ModuleRegistry};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::Value;
use sqlx::PgPool;
use time::macros::datetime;
use tower::ServiceExt;
use utoipa::openapi::path::HttpMethod;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

/// The reference contract of the repository.
fn reference() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(REFERENCE);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// The reference is the contract of the build, its modules included (spec 008,
/// REQ-008-9): the core alone is a part of it.
#[test]
fn the_contract_keeps_its_reference() {
    let registry = kartenn_server::modules::registry().unwrap();
    let produced = kartenn_rest::openapi::json_with(&registry, kartenn_server::modules::routers());
    let reference = reference();
    let found = breaches(&reference, &produced);
    assert!(
        found.is_empty(),
        "the contract of /v1 is broken:\n{}",
        found
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        produced == reference,
        "the contract grew: write its reference again with \
         `cargo run -p kartenn-server --all-features -- openapi`"
    );
}

/// The operations REQ-006-4 asks for, by method and path.
fn surface() -> Vec<(&'static str, String)> {
    let mut wanted = Vec::new();
    for collection in [
        "entities",
        "sites",
        "domains",
        "prefixes",
        "ranges",
        "addresses",
    ] {
        let one = format!("/v1/{collection}/{{id}}");
        wanted.extend([
            ("get", format!("/v1/{collection}")),
            (QUERY_EXTENSION, format!("/v1/{collection}")),
            ("post", format!("/v1/{collection}")),
            ("get", one.clone()),
            ("patch", one.clone()),
            ("delete", one.clone()),
            ("get", format!("{one}/history")),
            ("get", format!("{one}/properties")),
            ("get", format!("{one}/properties/{{key}}/explanation")),
            ("get", format!("{one}/properties/{{key}}/suggestion")),
        ]);
    }
    let fixed = [
        // The tree, the anomalies.
        ("get", "/v1/prefixes/{id}/children"),
        ("get", "/v1/prefixes/{id}/ancestors"),
        ("get", "/v1/prefixes/{id}/occupancy"),
        ("get", "/v1/prefixes/{id}/next-free"),
        ("get", "/v1/anomalies"),
        ("post", "/v1/anomalies/{id}/acknowledgement"),
        // The properties.
        ("get", "/v1/property-declarations"),
        ("get", "/v1/property-declarations/{key}/values"),
        ("get", "/v1/property-gaps"),
        // The changesets.
        ("post", "/v1/changesets"),
        ("get", "/v1/changesets"),
        ("get", "/v1/changesets/{id}"),
        ("get", "/v1/changesets/{id}/diff"),
        ("post", "/v1/changesets/{id}/validation"),
        ("post", "/v1/changesets/{id}/rejection"),
        ("post", "/v1/changesets/{id}/withdrawal"),
        ("post", "/v1/changesets/{id}/undo"),
        ("get", "/v1/proposals"),
        // The imports.
        ("post", "/v1/imports"),
        ("get", "/v1/imports/{id}"),
        ("put", "/v1/imports/{id}/chunks/{n}"),
        ("post", "/v1/imports/{id}/start"),
        ("post", "/v1/imports/{id}/resume"),
        ("post", "/v1/imports/{id}/cancel"),
        // The rights.
        ("get", "/v1/roles"),
        ("get", "/v1/roles/{key}"),
        ("put", "/v1/roles/{key}"),
        ("delete", "/v1/roles/{key}"),
        ("get", "/v1/bindings"),
        ("post", "/v1/bindings"),
        ("get", "/v1/bindings/{id}"),
        ("delete", "/v1/bindings/{id}"),
        ("get", "/v1/issuer-attachments"),
        ("post", "/v1/issuer-attachments"),
        ("delete", "/v1/issuer-attachments"),
        ("get", "/v1/service-accounts"),
        ("post", "/v1/service-accounts"),
        ("get", "/v1/service-accounts/{id}"),
        ("post", "/v1/service-accounts/{id}/secrets"),
        ("delete", "/v1/service-accounts/{id}/secrets/{secret}"),
        ("get", "/v1/identities"),
        ("get", "/v1/identities/{id}"),
        ("patch", "/v1/identities/{id}"),
        ("get", "/v1/me"),
        // The audit.
        ("get", "/v1/audit-events"),
        (QUERY_EXTENSION, "/v1/audit-events"),
        ("post", "/v1/identities/{id}/pseudonymisation"),
        // The authentication.
        ("get", "/v1/auth/login"),
        ("get", "/v1/auth/callback"),
        ("post", "/v1/auth/logout"),
        ("post", "/v1/auth/breakglass"),
        // The operations.
        ("get", "/v1/capabilities"),
        ("get", "/v1/openapi.json"),
        ("put", "/v1/log-filters/{output}"),
        ("get", "/health/live"),
        ("get", "/health/ready"),
    ];
    wanted.extend(fixed.into_iter().map(|(m, p)| (m, p.to_owned())));
    wanted
}

#[test]
fn every_operation_of_the_surface_is_in_the_contract() {
    let document = produced();
    let missing: Vec<String> = surface()
        .into_iter()
        .filter(|(method, path)| {
            document["paths"]
                .get(path)
                .and_then(|item| item.get(*method))
                .and_then(|op| op.get("responses"))
                .is_none()
        })
        .map(|(method, path)| format!("{method} {path}"))
        .collect();
    assert!(missing.is_empty(), "missing: {missing:?}");
}

#[test]
fn a_member_removed_fails_the_comparison() {
    let reference = produced();
    let mut produced = reference.clone();
    produced["components"]["schemas"]["Prefix"]["properties"]
        .as_object_mut()
        .unwrap()
        .remove("nature")
        .unwrap();
    let found = breaches(&reference, &produced);
    assert!(
        found
            .iter()
            .any(|b| b.at == "components.schemas.Prefix.nature" && b.what == "member removed"),
        "{found:?}"
    );
    // A member of a request made required, an operation removed.
    let mut produced = reference.clone();
    produced["components"]["schemas"]["Submission"]["required"]
        .as_array_mut()
        .unwrap()
        .push("valid_from".into());
    produced["paths"]["/v1/prefixes"]
        .as_object_mut()
        .unwrap()
        .remove(QUERY_EXTENSION);
    let found: Vec<String> = breaches(&reference, &produced)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        found.contains(&"components.schemas.Submission.valid_from: member now required".into()),
        "{found:?}"
    );
    assert!(
        found.contains(&format!(
            "paths./v1/prefixes.{QUERY_EXTENSION}: operation removed"
        )),
        "{found:?}"
    );
}

#[test]
fn the_contract_is_openapi_3_1_its_query_in_an_extension() {
    let document = produced();
    assert_eq!(document["openapi"], "3.1.0");
    let item = &document["paths"]["/v1/prefixes"];
    assert!(item.get("query").is_none(), "a 3.1 path item has no query");
    let query = &item[QUERY_EXTENSION];
    assert_eq!(query["operationId"], "query_prefixes");
    assert_eq!(
        query["requestBody"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/Search"
    );
    assert_eq!(query["responses"]["200"], item["get"]["responses"]["200"]);
    // Every operation answers a problem; the login routes and the probes are public.
    for (path, item) in document["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            if method == CLIENT_EXTENSION {
                continue;
            }
            assert!(
                operation["responses"].get("default").is_some(),
                "{method} {path} answers no problem"
            );
        }
    }
    assert_eq!(
        document["paths"]["/health/live"]["get"]["security"],
        serde_json::json!([])
    );
    assert!(document["paths"]["/v1/me"]["get"].get("security").is_none());
}

/// The extension `x-kartenn` of the reference (023/A5, task 023-5): every collection of
/// the core and of the business modules tells the clients its natural key, among the
/// filters of its list, and its columns; `kartenn policy check` is named.
#[test]
fn every_collection_tells_the_clients_its_key_and_its_columns() {
    const MODULES: [&str; 5] = ["core", "vlan", "machines", "policy", "flow"];
    let reference = reference();
    let mut described = 0;
    for (path, item) in reference["paths"].as_object().unwrap() {
        let Some(get) = item.get("get") else {
            continue;
        };
        let collection = path.strip_prefix("/v1/").is_some_and(|c| !c.contains('/'));
        let tag = get["tags"][0].as_str().unwrap_or_default();
        let paged = get["responses"]["200"]["content"]["application/json"]["schema"]["$ref"]
            .as_str()
            .is_some_and(|r| r.starts_with("#/components/schemas/Listing_"));
        if !(collection && paged && MODULES.contains(&tag)) {
            continue;
        }
        let extension = &item[CLIENT_EXTENSION];
        let columns = extension["columns"].as_array();
        assert!(
            columns.is_some_and(|c| !c.is_empty()),
            "{path} has no columns: {extension}"
        );
        let filters: Vec<&str> = get["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["in"] == "query")
            .filter_map(|p| p["name"].as_str())
            .collect();
        for member in extension["key"].as_array().unwrap() {
            assert!(
                filters.contains(&member.as_str().unwrap()),
                "{path}: the key {member} is not a filter"
            );
        }
        described += 1;
    }
    assert_eq!(described, 26, "the collections described");
    let prefixes = &reference["paths"]["/v1/prefixes"][CLIENT_EXTENSION];
    assert_eq!(prefixes["key"], serde_json::json!(["domain", "value"]));
    assert_eq!(
        reference["paths"]["/v1/verdicts"]["get"][CLIENT_EXTENSION]["command"],
        "check"
    );
}

/// A module receives the extension through the kit: `presented` on the path item of its
/// collection, `commanded` on an operation; a key that is not a filter is refused.
#[test]
fn the_kit_describes_the_collection_of_a_module() {
    let router = || {
        let list = op("list_gadgets", "gadget", "The gadgets.")
            .query("slug", "Equal to.")
            .answers("200", "A page.", "Problem")
            .build();
        let make = op("make_gadget", "gadget", "Makes a gadget.")
            .answers("201", "Made.", "Problem")
            .build();
        OpenApiRouter::<Api>::new().routes(documented(
            "/gadgets",
            vec![(HttpMethod::Get, list), (HttpMethod::Post, make)],
            Vec::new(),
            axum::routing::get(|| async { "[]" }).post(|| async { "{}" }),
        ))
    };
    let described = presented(router(), "/gadgets", &["slug"], &["slug", "size"]);
    let described = commanded(described, "/gadgets", &HttpMethod::Post, "make");
    let document = kartenn_rest::openapi::json_with(ModuleRegistry::empty(), vec![described]);
    let item = &document["paths"]["/v1/gadgets"];
    assert_eq!(
        item[CLIENT_EXTENSION],
        serde_json::json!({"key": ["slug"], "columns": ["slug", "size"]})
    );
    assert_eq!(
        item["post"][CLIENT_EXTENSION],
        // Spec 024, REQ-024-17: a write without a changeset pending does not simulate.
        serde_json::json!({"command": "make", "simulate": false})
    );
    assert!(item["get"].get(CLIENT_EXTENSION).is_none());
    // The extension is an addition: the contract without it breaks nothing.
    let bare = kartenn_rest::openapi::json_with(ModuleRegistry::empty(), vec![router()]);
    assert_eq!(breaches(&bare, &document), []);
    assert_eq!(breaches(&document, &bare), []);
    let refused = std::panic::catch_unwind(|| presented(router(), "/gadgets", &["size"], &[]));
    assert!(refused.is_err(), "a key that is not a filter");
}

/// A route of a test version, retired.
#[utoipa::path(get, path = "/v0/things", responses((status = 200, description = "Things.")))]
async fn things() -> &'static str {
    "[]"
}

#[tokio::test]
async fn a_retired_route_announces_its_successor() {
    let retirement = Retirement {
        deprecated: datetime!(2026-07-01 00:00 UTC),
        sunset: datetime!(2027-01-04 12:30:05 +02:00),
        successor: "/v1/things".to_owned(),
    };
    let (router, document) = OpenApiRouter::<()>::new()
        .routes(retire(routes!(things), retirement))
        .split_for_parts();
    let response = router
        .oneshot(Request::get("/v0/things").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let header = |name: &str| response.headers()[name].to_str().unwrap().to_owned();
    assert_eq!(header("deprecation"), "@1782864000");
    assert_eq!(header("sunset"), "Mon, 04 Jan 2027 10:30:05 GMT");
    assert_eq!(header("link"), "</v1/things>; rel=\"successor-version\"");
    let document = serde_json::to_value(document).unwrap();
    assert_eq!(document["paths"]["/v0/things"]["get"]["deprecated"], true);
}

fn app(pool: &PgPool, identity: Uuid, settings: Settings) -> axum::Router {
    app_with(pool, identity, settings, None, Arc::default())
}

fn app_with(
    pool: &PgPool,
    identity: Uuid,
    settings: Settings,
    web: Option<Arc<kartenn_rest::web::Web>>,
    modules: Arc<ModuleRegistry>,
) -> axum::Router {
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules,
    };
    kartenn_rest::router_with_web(
        kartenn_rest::v1(api),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        settings,
        Arc::new(Quotas::new(Rates::default())),
        Arc::new(common::As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
        web,
    )
}

/// `ui_plugins` publishes, for each plugin of the directory, what the shell needs to load
/// it (spec 007, REQ-007-7; task 007-5).
#[kartenn_testing::pg_test]
async fn the_capabilities_publish_the_plugins_of_the_directory(pool: PgPool) {
    let dir = std::env::temp_dir().join(format!("kartenn-capabilities-{}", Uuid::now_v7()));
    let version = dir.join("example/1.1.0");
    std::fs::create_dir_all(version.join("i18n")).unwrap();
    let manifest = serde_json::json!({
        "name": "example", "version": "1.1.0", "sdk": "^1.0", "entry": "index.js",
        "ui": "^1.0.0", "framework": "^3.5.0", "space": "referential",
        "catalogs": { "en": "i18n/en.json", "fr": "i18n/fr.json" },
        "extensions": [{ "point": "tab", "id": "notes", "element": "example-notes",
                         "object_types": ["prefix"], "label": "tab.notes",
                         "permission": "core.prefix.read" },
                       { "point": "view", "id": "all", "element": "example-all", "go": "n" }]
    });
    std::fs::write(version.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(version.join("index.js"), "export {};").unwrap();
    std::fs::write(version.join("i18n/en.json"), r#"{"tab":{"notes":"Notes"}}"#).unwrap();
    std::fs::write(version.join("i18n/fr.json"), r#"{"tab":{"notes":"Notes"}}"#).unwrap();
    let plugins = kartenn_rest::web::Plugins::open(
        &dir,
        kartenn_rest::capabilities::module_names(&ModuleRegistry::default()),
    )
    .unwrap();
    let web = Arc::new(kartenn_rest::web::Web::new(None, Some(Arc::new(plugins))));
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'reader') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let app = app_with(
        &pool,
        identity,
        Settings::default(),
        Some(web),
        Arc::default(),
    );
    let (status, capabilities) = read(&app, "/v1/capabilities").await;
    assert_eq!(status, StatusCode::OK, "{capabilities}");
    let plugin = &capabilities["ui_plugins"][0];
    assert_eq!(plugin["name"], "example");
    assert_eq!(plugin["version"], "1.1.0");
    assert_eq!(plugin["sdk"], "^1.0");
    assert_eq!(plugin["ui"], "^1.0.0");
    assert_eq!(plugin["framework"], "^3.5.0");
    assert_eq!(plugin["space"], "referential");
    assert_eq!(plugin["base"], "/plugins/example/1.1.0/");
    assert_eq!(plugin["entry"], "/plugins/example/1.1.0/index.js");
    for file in ["index.js", "manifest.json", "i18n/en.json", "i18n/fr.json"] {
        assert!(
            plugin["integrity"][file]
                .as_str()
                .is_some_and(|sri| sri.starts_with("sha384-")),
            "{file}: {plugin}"
        );
    }
    assert_eq!(plugin["catalogs"]["fr"], "i18n/fr.json");
    assert_eq!(plugin["extensions"], manifest["extensions"]);
    assert_eq!(capabilities["ui_plugins"].as_array().unwrap().len(), 1);
}

async fn read(app: &axum::Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(common::bearer(
            Request::get(path).body(Body::empty()).unwrap(),
        ))
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// A business module of the build, without routes.
#[derive(Debug)]
struct Inventory;

impl Module for Inventory {
    fn name(&self) -> &'static str {
        "inventory"
    }
    fn version(&self) -> &'static str {
        "1.2.3"
    }
}

#[kartenn_testing::pg_test]
async fn capabilities_list_registered_modules(pool: PgPool) {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'reader') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let module = |name: &str, capabilities: &Value| {
        capabilities["modules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == name)
            .cloned()
    };
    let without = app(&pool, identity, Settings::default());
    let (_, capabilities) = read(&without, "/v1/capabilities").await;
    assert_eq!(module("inventory", &capabilities), None);
    let registry = ModuleRegistry::new(vec![Arc::new(Inventory)]).unwrap();
    let with = app_with(
        &pool,
        identity,
        Settings::default(),
        None,
        Arc::new(registry),
    );
    let (status, capabilities) = read(&with, "/v1/capabilities").await;
    assert_eq!(status, StatusCode::OK, "{capabilities}");
    assert_eq!(
        module("inventory", &capabilities),
        Some(serde_json::json!({"name": "inventory", "version": "1.2.3"}))
    );
    assert!(module("core", &capabilities).is_some(), "{capabilities}");
}

/// REQ-018-12: the capabilities publish the default language of the instance.
#[kartenn_testing::pg_test]
async fn the_capabilities_publish_the_default_locale(pool: PgPool) {
    kartenn_core::labels::set_default_locale(kartenn_core::labels::Locale::parse("en").unwrap())
        .unwrap();
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'reader') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, capabilities) = read(
        &app(&pool, identity, Settings::default()),
        "/v1/capabilities",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{capabilities}");
    assert_eq!(capabilities["default_locale"], "en");
}

/// The collections the registry reserves for the core are those of the contract: a module
/// can take none of them (REQ-008-1).
#[test]
fn core_collections_are_those_of_the_contract() {
    let document = produced();
    let mut served: Vec<&str> = document["paths"]
        .as_object()
        .unwrap()
        .keys()
        .filter_map(|path| path.strip_prefix("/v1/"))
        .map(|rest| rest.split('/').next().unwrap())
        .collect();
    served.sort_unstable();
    served.dedup();
    let mut reserved = CORE_COLLECTIONS.to_vec();
    reserved.sort_unstable();
    assert_eq!(served, reserved);
}

#[kartenn_testing::pg_test]
async fn the_contract_and_the_capabilities_are_served(pool: PgPool) {
    let settings = Settings {
        max_body_bytes: 4 * 1024 * 1024,
        ..Settings::default()
    };
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'reader') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let app = app(&pool, identity, settings);
    let (status, document) = read(&app, "/v1/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(document, produced());
    let (status, capabilities) = read(&app, "/v1/capabilities").await;
    assert_eq!(status, StatusCode::OK, "{capabilities}");
    assert_eq!(capabilities["api_versions"], serde_json::json!(["v1"]));
    let modules: Vec<&str> = capabilities["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    for module in [
        "audit",
        "changesets",
        "core",
        "imports",
        "properties",
        "rights",
    ] {
        assert!(modules.contains(&module), "{modules:?}");
    }
    assert!(capabilities["properties"].is_array());
    assert_eq!(capabilities["ui_plugins"], serde_json::json!([]));
    let limits = &capabilities["limits"];
    assert_eq!(limits["page_size_default"], 100);
    assert_eq!(limits["page_size_max"], 1_000);
    assert_eq!(limits["changeset_operations"], 10_000);
    assert_eq!(limits["chunk_elements"], 10_000);
    assert_eq!(limits["body_bytes"], 4 * 1024 * 1024);
    assert_eq!(limits["idempotency_retention_seconds"], 86_400);
}

/// The contract is served under a strong `ETag`, the SHA-256 of its bytes, and
/// `no-cache`; `If-None-Match` naming it answers 304 without body (023/A5, task 023-4).
#[kartenn_testing::pg_test]
async fn the_contract_is_revalidated_by_its_tag(pool: PgPool) {
    use sha2::Digest as _;
    let app = app(&pool, Uuid::now_v7(), Settings::default());
    let get = |tag: Option<&str>| {
        let mut request = Request::get("/v1/openapi.json");
        if let Some(tag) = tag {
            request = request.header("if-none-match", tag);
        }
        app.clone()
            .oneshot(common::bearer(request.body(Body::empty()).unwrap()))
    };
    let response = get(None).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let tag = response.headers()["etag"].to_str().unwrap().to_owned();
    assert_eq!(response.headers()["cache-control"], "no-cache");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let hex = sha2::Sha256::digest(&bytes)
        .iter()
        .fold(String::new(), |hex, b| hex + &format!("{b:02x}"));
    assert_eq!(
        tag,
        format!("\"{hex}\""),
        "a strong tag, the SHA-256 of the body"
    );
    let response = get(Some(&format!("\"other\", {tag}"))).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers()["etag"], tag.as_str());
    assert_eq!(response.headers()["cache-control"], "no-cache");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(body.is_empty());
    let response = get(Some("\"other\"")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Two instances of the same build serve the same tag: the document is serialised with
/// its members sorted, whatever the order of the hash maps of each instance.
#[test]
fn the_tag_of_the_contract_is_that_of_the_build() {
    let served = || {
        let registry = kartenn_server::modules::registry().unwrap();
        kartenn_rest::openapi::Served::of(&kartenn_rest::openapi::document_with(
            &registry,
            kartenn_server::modules::routers(),
        ))
    };
    let first = served();
    for _ in 0..4 {
        assert_eq!(served().etag(), first.etag());
    }
    let core = kartenn_rest::openapi::Served::of(&kartenn_rest::openapi::document());
    assert_ne!(core.etag(), first.etag(), "another contract, another tag");
}
