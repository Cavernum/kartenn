//! `placements` in a simulation (spec 021, A12; task 021-24a): the objects whose parent the
//! changeset changes at the present, with their parents before and after, on the
//! simulation of a changeset and on that of a draft.

use crate::common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// The router, as a person granted everything everywhere.
async fn app(pool: &PgPool) -> Router {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role: Uuid = sqlx::query_scalar(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'all', '{\"fr\": \"tout\"}',
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, '*', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(kartenn_testing::BIND_IDENTITY)
        .bind(identity)
        .bind(role)
        .bind("global")
        .bind(None::<Uuid>)
        .execute(pool)
        .await
        .unwrap();
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: Arc::default(),
    };
    kartenn_rest::router(
        kartenn_rest::v1(api),
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

/// `body` posted to `uri`: the status and the answer.
async fn post(app: &Router, uri: &str, body: &Value) -> (StatusCode, Value) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app
        .clone()
        .oneshot(common::applied(common::bearer(request)))
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

/// `operations` applied.
async fn apply(app: &Router, operations: Value) {
    let body = json!({"motive": "lab", "operations": operations});
    let (status, answer) = post(app, "/v1/changesets", &body).await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
}

/// The simulation of `operations`.
async fn simulate(app: &Router, operations: Value) -> Value {
    let body = json!({"motive": "lab", "operations": operations});
    let (status, answer) = post(app, "/v1/changesets?simulate=true", &body).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["valid"], true, "{answer}");
    answer
}

/// The identifier of the prefix of value `value`.
async fn prefix(pool: &PgPool, value: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = $1::cidr AND valid_period @> now()")
        .bind(value)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// The placements of a simulation, `[object, kind, before, after]` sorted by object.
fn placements(answer: &Value) -> Vec<Value> {
    assert_eq!(answer["placements_truncated"], false, "{answer}");
    let mut found: Vec<Value> = answer["placements"]
        .as_array()
        .unwrap_or_else(|| panic!("placements in {answer}"))
        .iter()
        .map(|p| json!([p["object"], p["kind"], p["before"], p["after"]]))
        .collect();
    found.sort_by_key(ToString::to_string);
    found
}

fn sorted(mut rows: Vec<Value>) -> Vec<Value> {
    rows.sort_by_key(ToString::to_string);
    rows
}

/// A domain `lab` with a container `10.0.0.0/16` holding two networks, a range and an
/// address.
async fn lab(pool: &PgPool, app: &Router) -> (Uuid, [Uuid; 5]) {
    apply(
        app,
        json!([
            {"op": "create_domain", "local": 1, "slug": "lab"},
            {"op": "create_prefix", "local": null, "domain": {"local": 1},
             "prefix": "10.0.0.0/16", "nature": "container"},
            {"op": "create_prefix", "local": null, "domain": {"local": 1},
             "prefix": "10.0.1.0/24", "nature": "network"},
            {"op": "create_prefix", "local": null, "domain": {"local": 1},
             "prefix": "10.0.2.0/24", "nature": "network"},
            {"op": "create_prefix", "local": null, "domain": {"local": 1},
             "prefix": "10.0.9.0/24", "nature": "network"},
        ]),
    )
    .await;
    let domain: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = 'lab'")
        .fetch_one(pool)
        .await
        .unwrap();
    apply(
        app,
        json!([
            {"op": "create_range", "local": null, "domain": {"id": domain},
             "first": "10.0.1.10", "last": "10.0.1.20", "usage": "dhcp"},
            {"op": "create_address", "local": null, "domain": {"id": domain},
             "address": "10.0.2.5", "usage": "host"},
        ]),
    )
    .await;
    (
        domain,
        [
            prefix(pool, "10.0.0.0/16").await,
            prefix(pool, "10.0.1.0/24").await,
            prefix(pool, "10.0.2.0/24").await,
            prefix(pool, "10.0.9.0/24").await,
            sqlx::query_scalar("SELECT id FROM address WHERE address = '10.0.2.5'")
                .fetch_one(pool)
                .await
                .unwrap(),
        ],
    )
}

/// The placement of the object the simulation creates: its identifier and its row.
fn created(answer: &Value) -> (Uuid, Value) {
    let new = answer["placements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["before"].is_null() && p["kind"] == "prefix")
        .unwrap_or_else(|| panic!("a created prefix in {answer}"));
    let id = new["object"].as_str().unwrap().parse().unwrap();
    (
        id,
        json!([new["object"], new["kind"], new["before"], new["after"]]),
    )
}

#[kartenn_testing::pg_test]
async fn an_insert_takes_its_children_from_their_parent(pool: PgPool) {
    let app = app(&pool).await;
    let (domain, [wide, one, two, nine, _]) = lab(&pool, &app).await;
    let insert = json!([{"op": "create_prefix", "local": null, "domain": {"id": domain},
                         "prefix": "10.0.0.0/22", "nature": "container"}]);

    // Inserted between the /16 and its children: those it holds change parent, the one
    // outside stays; the range and the address keep their networks.
    let answer = simulate(&app, insert.clone()).await;
    let (new, row) = created(&answer);
    assert_eq!(
        placements(&answer),
        sorted(vec![
            row,
            json!([one, "prefix", wide, new]),
            json!([two, "prefix", wide, new]),
        ]),
        "{answer}"
    );
    assert!(!answer["placements"].to_string().contains(&nine.to_string()));
    // The objects of the operations first: the prefix created, under the /16.
    assert_eq!(answer["placements"][0]["after"], json!(wide), "{answer}");

    // An address created is placed in its network, from no parent.
    let answer = simulate(
        &app,
        json!([{"op": "create_address", "local": null, "domain": {"id": domain},
                "address": "10.0.9.7", "usage": "host"}]),
    )
    .await;
    assert_eq!(answer["placements"][0]["kind"], "address", "{answer}");
    assert_eq!(answer["placements"][0]["before"], Value::Null, "{answer}");
    assert_eq!(answer["placements"][0]["after"], json!(nine), "{answer}");
    assert_eq!(
        answer["placements"].as_array().unwrap().len(),
        1,
        "{answer}"
    );

    // The same from a draft (POST /v1/drafts/{id}/simulation).
    let body = json!({"title": "Insert", "operations": insert});
    let (status, draft) = post(&app, "/v1/drafts", &body).await;
    assert_eq!(status, StatusCode::CREATED, "{draft}");
    let uri = format!("/v1/drafts/{}/simulation", draft["id"].as_str().unwrap());
    let (status, answer) = post(&app, &uri, &json!({})).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let (new, row) = created(&answer);
    assert_eq!(
        placements(&answer),
        sorted(vec![
            row,
            json!([one, "prefix", wide, new]),
            json!([two, "prefix", wide, new]),
        ]),
        "{answer}"
    );
}

#[kartenn_testing::pg_test]
async fn a_prefix_removed_or_resized_moves_its_children(pool: PgPool) {
    let app = app(&pool).await;
    let (domain, [wide, one, two, nine, _]) = lab(&pool, &app).await;

    // Closed: its children rise to the root; it leaves the tree, unlisted.
    let answer = simulate(
        &app,
        json!([{"op": "close", "kind": "prefix", "id": {"id": wide}}]),
    )
    .await;
    assert_eq!(
        placements(&answer),
        sorted(vec![
            json!([one, "prefix", wide, null]),
            json!([two, "prefix", wide, null]),
            json!([nine, "prefix", wide, null]),
        ]),
        "{answer}"
    );

    // REQ-021-17: a container 10.0.0.0/23 over 10.0.1.0/24, resized to 10.0.0.0/22, takes
    // 10.0.2.0/24 under it; 10.0.9.0/24 resized to 10.0.8.0/22 keeps its place.
    apply(
        &app,
        json!([{"op": "create_prefix", "local": null, "domain": {"id": domain},
                "prefix": "10.0.0.0/23", "nature": "container"}]),
    )
    .await;
    let block = prefix(&pool, "10.0.0.0/23").await;
    let answer = simulate(
        &app,
        json!([
            {"op": "update_prefix", "id": {"id": nine}, "prefix": "10.0.8.0/22"},
            {"op": "update_prefix", "id": {"id": block}, "prefix": "10.0.0.0/22"},
        ]),
    )
    .await;
    assert_eq!(
        placements(&answer),
        vec![json!([two, "prefix", wide, block])],
        "{answer}"
    );

    // Closed, the container gives its network back to the /16.
    let answer = simulate(
        &app,
        json!([{"op": "close", "kind": "prefix", "id": {"id": block}}]),
    )
    .await;
    assert_eq!(
        placements(&answer),
        vec![json!([one, "prefix", block, wide])],
        "{answer}"
    );
}

#[kartenn_testing::pg_test]
async fn a_changeset_without_effect_on_the_trees_places_nothing(pool: PgPool) {
    let app = app(&pool).await;
    let (_, [_, one, ..]) = lab(&pool, &app).await;
    for operations in [
        json!([{"op": "create_domain", "local": null, "slug": "other"}]),
        json!([{"op": "update_prefix", "id": {"id": one}, "point_to_point": false}]),
    ] {
        let answer = simulate(&app, operations).await;
        assert_eq!(answer["placements"], json!([]), "{answer}");
        assert_eq!(answer["placements_truncated"], false, "{answer}");
    }
}

#[kartenn_testing::pg_test]
async fn past_a_thousand_the_placements_are_truncated(pool: PgPool) {
    let app = app(&pool).await;
    let (domain, _) = lab(&pool, &app).await;
    // 1001 networks /28 in 10.2.0.0/16.
    let mut operations = vec![json!({"op": "create_prefix", "local": null,
        "domain": {"id": domain}, "prefix": "10.2.0.0/16", "nature": "container"})];
    operations.extend((0..1001_u32).map(|n| {
        json!({"op": "create_prefix", "local": null, "domain": {"id": domain},
               "prefix": format!("10.2.{}.{}/28", n / 16, (n % 16) * 16), "nature": "network"})
    }));
    apply(&app, Value::Array(operations)).await;
    let insert = json!([{"op": "create_prefix", "local": null, "domain": {"id": domain},
                         "prefix": "10.2.0.0/17", "nature": "container"}]);
    let answer = simulate(&app, insert).await;
    assert_eq!(answer["placements_truncated"], true);
    let found = answer["placements"].as_array().unwrap();
    assert_eq!(found.len(), 1000);
    // The prefix created first, then its children in address order.
    assert_eq!(found[0]["before"], Value::Null);
    let new = &found[0]["object"];
    assert!(found[1..].iter().all(|p| &p["after"] == new), "{answer}");
}
