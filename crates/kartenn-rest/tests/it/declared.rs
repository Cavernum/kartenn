//! Every filter a list serves is declared in its `parameters` (spec 021, REQ-021-35;
//! task 021-28): the typed filters of the clients are drawn from the contract, so a
//! parameter the server accepts but does not declare is a filter no client can offer.
//!
//! Each list of the contract (a `GET` without path parameter that takes `cursor`) is
//! probed with every query parameter name of the whole contract and the three forms of a
//! property filter: a name the list does not refuse as unknown must be among its
//! parameters. An unknown name must be refused, else the probe proves nothing.

use crate::common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rate, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// The parameters of the dates and of the pages, declared once for all lists.
const PAGES: [&str; 4] = ["cursor", "page_size", "valid_at", "as_of"];

/// The three forms of a property filter, as the contract names them.
const PROPERTY: [(&str, &str); 3] = [
    ("prop.{key}", "prop.dns.ttl"),
    ("prop.{key}:between", "prop.dns.ttl:between"),
    ("prop.{key}:exists", "prop.dns.ttl:exists"),
];

/// A person granted everything everywhere.
async fn admin(pool: &PgPool) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let role = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO role (id, key, labels, valid_period, changeset_id)
         VALUES ($1, $2, '{\"fr\": \"r\"}', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    )
    .bind(role)
    .bind(format!("r_{}", role.simple()))
    .execute(pool)
    .await
    .unwrap();
    for statement in [
        "INSERT INTO role_permission (role_id, permission, valid_period, changeset_id)
         VALUES ($1, '*', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                              valid_period, changeset_id)
         VALUES (gen_random_uuid(), 'identity', $2, $1, 'global', NULL,
                 tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
    ] {
        sqlx::query(statement)
            .bind(role)
            .bind(identity)
            .execute(pool)
            .await
            .unwrap();
    }
    identity
}

/// The router of the build, every module included.
async fn app(pool: &PgPool) -> Router {
    let registry = kartenn_server::modules::registry().unwrap();
    kartenn_store_pg::modules::migrate(pool, &registry)
        .await
        .unwrap();
    let identity = admin(pool).await;
    let rate = Rate::new(1_000_000, 1_000_000).unwrap();
    let api = Api {
        pool: pool.clone(),
        ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
        bound: Duration::from_secs(60),
        journal: PgAuditJournal::new(pool.clone(), Instance::new("test")),
        audit: ReadAudit::default(),
        sources: kartenn_service::expected::Sources::none(),
        outputs: Arc::from([]),
        modules: registry,
    };
    kartenn_rest::router(
        kartenn_rest::v1_with(api, kartenn_server::modules::routers()),
        Arc::new(PgReadiness::new(pool.clone(), AuditStatus::default())),
        Settings::default(),
        Arc::new(Quotas::new(Rates {
            read: rate,
            write: rate,
            anonymous: rate,
        })),
        Arc::new(common::As(Principal {
            identity,
            kind: IdentityKind::Person,
            issuer: None,
            groups: Vec::new(),
            method: Method::Bearer,
        })),
    )
}

/// The query parameters of an operation, references followed.
fn parameters(document: &Value, operation: &Value) -> BTreeSet<String> {
    operation["parameters"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| match p["$ref"].as_str() {
            Some(r) => &document["components"]["parameters"][r.rsplit('/').next().unwrap()],
            None => p,
        })
        .filter(|p| p["in"] == "query")
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect()
}

/// Whether `GET path?name=x` refuses `name` as unknown; the answer otherwise.
async fn refused(app: &Router, path: &str, name: &str) -> Result<(), (StatusCode, String)> {
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair(name, "x")
        .finish();
    let response = app
        .clone()
        .oneshot(common::bearer(
            Request::get(format!("{path}?{query}"))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or_default();
    let detail = body["detail"].as_str().unwrap_or_default().to_owned();
    let unknown = format!("unknown parameter {name}");
    if status == StatusCode::BAD_REQUEST
        && (detail == unknown || detail.starts_with(&format!("{unknown}: ")))
    {
        Ok(())
    } else {
        Err((status, detail))
    }
}

#[kartenn_testing::pg_test]
async fn every_filter_a_list_serves_is_declared(pool: PgPool) {
    let app = app(&pool).await;
    let registry = kartenn_server::modules::registry().unwrap();
    let document = kartenn_rest::openapi::json_with(&registry, kartenn_server::modules::routers());
    let paths = document["paths"].as_object().unwrap();
    let lists: Vec<(&String, BTreeSet<String>)> = paths
        .iter()
        .filter(|(path, _)| !path.contains('{'))
        .filter_map(|(path, item)| Some((path, parameters(&document, item.get("get")?))))
        .filter(|(_, declared)| declared.contains("cursor"))
        .collect();
    let mut names: BTreeSet<String> = lists
        .iter()
        .flat_map(|(_, declared)| declared.iter().cloned())
        .filter(|name| !PAGES.contains(&name.as_str()) && !name.starts_with("prop."))
        .collect();
    names.extend(PROPERTY.iter().map(|(_, sent)| (*sent).to_owned()));
    let mut undeclared = Vec::new();
    let mut lax = Vec::new();
    for (path, declared) in &lists {
        if let Err(answer) = refused(&app, path, "zz_unknown").await {
            lax.push(format!("{path}: {answer:?}"));
            continue;
        }
        for name in &names {
            let declared_as = PROPERTY
                .iter()
                .find(|(_, sent)| sent == name)
                .map_or(name.as_str(), |(as_declared, _)| as_declared);
            if declared.contains(declared_as) {
                continue;
            }
            if let Err(answer) = refused(&app, path, name).await {
                undeclared.push(format!("{path} serves {name}: {answer:?}"));
            }
        }
    }
    assert!(
        lax.is_empty(),
        "lists that accept an unknown name:\n{}",
        lax.join("\n")
    );
    assert!(
        undeclared.is_empty(),
        "filters served but not declared:\n{}",
        undeclared.join("\n")
    );
}
