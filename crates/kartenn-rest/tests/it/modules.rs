//! The routes of the modules through the router (spec 008, REQ-008-9, 11; task 008-8):
//! the routes of a module mounted under `/v1` and in the contract it serves, the
//! transversal routes of its types of objects, the edges of the graph filtered by the
//! rights of the caller on both ends.

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::Violation;
use kartenn_rest::ModuleRouter;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::module::{
    BoxFuture, Edge, EdgeType, Module, ModuleHandler, ModulePlan, ModuleRead, ModuleRegistry,
    ModuleTx, ObjectType,
};
use kartenn_service::operations::{ModuleOperation, Ref};
use kartenn_service::port::{StoreError, WriteError};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

/// The type of object of the test module.
static THING: ObjectType = ObjectType {
    kind: "kit.thing",
    collection: "things",
    parent: None,
    owner_required: false,
    carries_site: false,
    domain_anchored: false,
};

/// An edge between two entities, which the test module declares.
static NEAR: EdgeType = EdgeType {
    name: "kit.near",
    from: "entity",
    to: "entity",
};

/// The edges the handler serves: its own state, read through the adapter's connection.
#[derive(Debug, Default)]
struct Kit {
    edges: Vec<(Uuid, Uuid)>,
}

impl ModuleHandler for Kit {
    fn plan(
        &self,
        _: &ModuleOperation,
        _: Uuid,
        _: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        unreachable!("the test module has no operation")
    }

    fn apply<'a>(
        &'a self,
        _: &'a mut dyn ModuleTx,
        _: &'a ModulePlan,
        _: Option<Period>,
        _: Uuid,
    ) -> BoxFuture<'a, Result<(), WriteError>> {
        unreachable!("the test module has no operation")
    }

    fn edges<'a>(
        &'a self,
        read: &'a mut dyn ModuleRead,
        object: Uuid,
    ) -> BoxFuture<'a, Result<Vec<Edge>, StoreError>> {
        Box::pin(async move {
            // The read reaches the database of the adapter.
            let connection = read
                .connection()
                .downcast_mut::<sqlx::PgConnection>()
                .expect("a PostgreSQL connection");
            let one: i32 = sqlx::query_scalar("SELECT 1")
                .fetch_one(connection)
                .await
                .map_err(|e| StoreError(e.to_string()))?;
            assert_eq!(one, 1);
            let start = Instant(read.valid_at().0 - 1);
            Ok(self
                .edges
                .iter()
                .filter(|(from, to)| *from == object || *to == object)
                .map(|&(from, to)| Edge {
                    name: NEAR.name,
                    from,
                    to,
                    period: Period::from(start),
                })
                .collect())
        })
    }
}

#[derive(Debug)]
struct KitModule(Arc<Kit>);

impl Module for KitModule {
    fn name(&self) -> &'static str {
        "kit"
    }
    fn version(&self) -> &'static str {
        "0.1.0"
    }
    fn object_types(&self) -> &'static [ObjectType] {
        std::slice::from_ref(&THING)
    }
    fn edges(&self) -> &'static [EdgeType] {
        std::slice::from_ref(&NEAR)
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        Some(self.0.clone())
    }
}

/// A route of the module itself.
#[utoipa::path(
    get,
    path = "/kit-status",
    operation_id = "kit_status",
    tag = "kit",
    responses((status = 200, description = "The module answers."))
)]
async fn status() -> &'static str {
    "kit"
}

fn app(pool: &PgPool, identity: Uuid, kit: Kit) -> Router {
    let modules = ModuleRegistry::new(vec![Arc::new(KitModule(Arc::new(kit)))]).unwrap();
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: Arc::new(modules),
    };
    let routers: Vec<ModuleRouter> = vec![OpenApiRouter::new().routes(routes!(status))];
    kartenn_rest::router(
        kartenn_rest::v1_with(api, routers),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates::default())),
        Arc::new(common::As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

/// A person granted `entries` on `scope` (`None`: everywhere).
async fn person_on(pool: &PgPool, entries: &[&str], scope: Option<(&str, Uuid)>) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, $2, jsonb_build_object('fr', 'r'), tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .bind(format!("r_{}", role.simple()))
    .execute(pool)
    .await
    .unwrap();
    for entry in entries {
        sqlx::query(
            "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
             VALUES ($1, $2, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(role)
        .bind(entry)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(kartenn_testing::BIND_IDENTITY)
        .bind(identity)
        .bind(role)
        .bind(scope.map_or("global", |(kind, _)| kind))
        .bind(scope.map(|(_, id)| id))
        .execute(pool)
        .await
        .unwrap();
    identity
}

async fn entity(pool: &PgPool, slug: &str, parent: Option<Uuid>) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO entity (id, slug, parent_id, valid_period, changeset_id)
         VALUES ($1, $2, $3, tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(id)
    .bind(slug)
    .bind(parent)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn thing(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO module_object (id, module, kind, valid_period, changeset_id)
         VALUES ($1, 'kit', 'kit.thing', tstzrange(now() - interval '1 day', NULL),
                 gen_random_uuid())",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Declares the text property `kit.label` on things and gives `value` to `thing`.
async fn label(pool: &PgPool, thing: Uuid, value: &str) {
    let since = "tstzrange(now() - interval '1 day', NULL), gen_random_uuid()";
    let property: Uuid = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "INSERT INTO property_declaration (id, key, value_type, cardinality, carriers,
                                           type_params, inherit_tree, sensitive, valid_period,
                                           changeset_id)
         VALUES (gen_random_uuid(), 'kit.label', 'text', 'single', ARRAY['kit.thing'],
                 '{{\"min_len\": 1, \"max_len\": 64}}', NULL, false, {since})
         RETURNING id"
    )))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO property_assignment (object_id, object_type, property_id, kind, v_text,
                                          provenance, valid_period, changeset_id)
         VALUES ($1, 'kit.thing', $2, 'value', $3, 'input', {since})"
    )))
    .bind(thing)
    .bind(property)
    .bind(value)
    .execute(pool)
    .await
    .unwrap();
}

async fn read(app: &Router, path: &str) -> (StatusCode, Value) {
    let request = Request::get(path).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(common::bearer(request)).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into_owned())),
    )
}

#[kartenn_testing::pg_test]
async fn module_routes_are_mounted(pool: PgPool) {
    let identity = person_on(&pool, &["*"], None).await;
    let app = app(&pool, identity, Kit::default());
    let (status, body) = read(&app, "/v1/kit-status").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, "kit");
    // The served contract is that of the routes mounted: the module's, the transversal
    // ones of its types, the kinds of its objects among those an operation names.
    let (status, contract) = read(&app, "/v1/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    let paths = contract["paths"].as_object().unwrap();
    for path in [
        "/v1/kit-status",
        "/v1/things/{id}/history",
        "/v1/things/{id}/properties",
        "/v1/things/{id}/properties/{key}/explanation",
        "/v1/things/{id}/properties/{key}/suggestion",
        "/v1/graph/edges",
    ] {
        assert!(paths.contains_key(path), "{path} missing");
    }
    let kinds = &contract["components"]["schemas"]["TargetKind"]["enum"];
    assert!(
        kinds
            .as_array()
            .unwrap()
            .contains(&Value::from("kit.thing")),
        "{kinds}"
    );
    // A build without the module serves neither.
    let base = kartenn_rest::openapi::json();
    assert!(
        !base["paths"]
            .as_object()
            .unwrap()
            .contains_key("/v1/kit-status")
    );
}

#[kartenn_testing::pg_test]
async fn transversal_routes_for_module_types(pool: PgPool) {
    let id = thing(&pool).await;
    label(&pool, id, "shelf").await;
    let reader = person_on(&pool, &["kit.thing.read"], None).await;
    let app = app(&pool, reader, Kit::default());
    let (status, history) = read(&app, &format!("/v1/things/{id}/history")).await;
    assert_eq!(status, StatusCode::OK, "{history}");
    let items = history["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{history}");
    assert_eq!(items[0]["object"]["kind"], "kit.thing");
    assert_eq!(items[0]["object"]["href"], format!("/v1/things/{id}"));
    let (status, properties) = read(&app, &format!("/v1/things/{id}/properties")).await;
    assert_eq!(status, StatusCode::OK, "{properties}");
    assert_eq!(properties["links"]["object"], format!("/v1/things/{id}"));
    assert_eq!(
        properties["properties"]["kit.label"], "shelf",
        "{properties}"
    );
    let (status, explanation) = read(
        &app,
        &format!("/v1/things/{id}/properties/kit.label/explanation"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{explanation}");
    assert_eq!(explanation["property"], "kit.label");
    assert_eq!(explanation["value"], "shelf");
    // An object of another kind, or one the caller may not read, is absent.
    let other = entity(&pool, "elsewhere", None).await;
    let (status, _) = read(&app, &format!("/v1/things/{other}/history")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let stranger = person_on(&pool, &["core.entity.read"], None).await;
    let denied = self::app(&pool, stranger, Kit::default());
    for path in [
        format!("/v1/things/{id}/history"),
        format!("/v1/things/{id}/properties"),
        format!("/v1/things/{id}/properties/kit.label/explanation"),
    ] {
        let (status, _) = read(&denied, &path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[kartenn_testing::pg_test]
async fn graph_edges_filtered_by_rights(pool: PgPool) {
    let a = entity(&pool, "a", None).await;
    let child = entity(&pool, "a-child", Some(a)).await;
    let b = entity(&pool, "b", None).await;
    let kit = || Kit {
        edges: vec![(a, child), (a, b)],
    };
    // Everywhere: both edges of `a`, sorted by end.
    let everywhere = person_on(&pool, &["core.entity.read"], None).await;
    let (status, edges) = read(
        &app(&pool, everywhere, kit()),
        &format!("/v1/graph/edges?object={a}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edges}");
    let mut ends: Vec<String> = edges["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["to"]["id"].as_str().unwrap().to_owned())
        .collect();
    ends.sort();
    let mut expected = vec![child.to_string(), b.to_string()];
    expected.sort();
    assert_eq!(ends, expected, "{edges}");
    assert_eq!(edges["items"][0]["type"], "kit.near");
    assert_eq!(
        edges["items"][0]["from"]["href"],
        format!("/v1/entities/{a}")
    );
    // On `a` only: the edge to `b`, which the caller may not read, is left out.
    let on_a = person_on(&pool, &["core.entity.read"], Some(("entity", a))).await;
    let app_a = app(&pool, on_a, kit());
    let (status, edges) = read(&app_a, &format!("/v1/graph/edges?object={a}")).await;
    assert_eq!(status, StatusCode::OK, "{edges}");
    let items = edges["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{edges}");
    assert_eq!(items[0]["to"]["id"], child.to_string());
    // An object the caller may not read is absent.
    let (status, _) = read(&app_a, &format!("/v1/graph/edges?object={b}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
