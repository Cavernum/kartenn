//! The routes of the registry through the router (task 011-9; REQ-011-14 to 18, 25): a
//! request answered with the flow and its evaluations, the list and the read under
//! `flow.flow.read` or as the requester, the evaluations reduced for the requester, the
//! flows of an address, the non-conformities.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use kartenn_auth::keys::KeyRing;
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_rest::health::PgReadiness;
use kartenn_rest::objects::Api;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{AuditStatus, Instance};
use kartenn_service::authn::{IdentityKind, Method as AuthMethod, Principal};
use kartenn_service::reads::ReadAudit;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use tower::ServiceExt;

use crate::requests::Env;

/// Any `Bearer` token is this principal.
struct As(Principal);

impl Authenticator for As {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(self.0.clone().into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

impl Env {
    /// The router of an instance with the modules, as the person `identity`.
    fn app(&self, identity: Uuid) -> Router {
        let api = Api {
            pool: self.pool.clone(),
            ring: KeyRing::parse(&KeyRing::generate_line("k1")).unwrap(),
            bound: Duration::from_secs(60),
            journal: PgAuditJournal::new(self.pool.clone(), Instance::new("test")),
            audit: ReadAudit::default(),
            sources: kartenn_service::expected::Sources::none(),
            outputs: Arc::from([]),
            modules: self.registry.clone(),
        };
        kartenn_rest::router(
            kartenn_rest::v1_with(api, vec![kartenn_flow::rest::routes()]),
            Arc::new(PgReadiness::new(self.pool.clone(), AuditStatus::default())),
            Settings::default(),
            Arc::new(Quotas::new(Rates::default())),
            Arc::new(As(Principal {
                identity,
                kind: IdentityKind::Person,
                issuer: None,
                groups: Vec::new(),
                method: AuthMethod::Bearer,
            })),
        )
    }

    /// A person granted `entries` everywhere.
    async fn granted(&self, entries: &[&str]) -> Uuid {
        let pool = &self.pool;
        let identity: Uuid = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 'p') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let role = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO role (id, key, label, valid_period, changeset_id)
             VALUES ($1, $2, 'r', tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
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
        sqlx::query(
            "INSERT INTO binding (id, subject_kind, subject_id, role_id, scope_kind, scope_id,
                                  valid_period, changeset_id)
             VALUES (gen_random_uuid(), 'identity', $1, $2, 'global', NULL,
                     tstzrange(now() - interval '1 day', NULL), gen_random_uuid())",
        )
        .bind(identity)
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
        identity
    }
}

async fn call(app: &Router, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, "Bearer t");
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        None => request.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// The body of a request `prod → dev` for `ssh`.
async fn body(env: &Env, prod: Uuid, dev: Uuid) -> Value {
    let mut fields = env.fields(prod, dev).await;
    fields.as_object_mut().unwrap().remove("local");
    fields["motive"] = json!("m");
    fields
}

fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect()
}

/// REQ-011-18, 21: `POST /v1/flows` triages the request and, applied, answers with the
/// flow (its state) and its evaluations; the requester reads it, a stranger does not.
#[kartenn_testing::pg_test]
async fn a_request_answers_with_the_flow_and_its_evaluations(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env
        .granted(&["flow.flow.request", "prop.core.owner.write"])
        .await;
    let app = env.app(requester);
    let (status, answer) = call(
        &app,
        Method::POST,
        "/v1/flows",
        Some(body(&env, prod, dev).await),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    assert_eq!(answer["flow"]["state"], "active", "{answer}");
    assert_eq!(answer["flow"]["summary"], "allowed");
    let evaluations = answer["evaluations"].as_array().unwrap();
    assert_eq!(evaluations.len(), 1);
    assert_eq!(evaluations[0]["verdict"], "allowed");
    let id = answer["flow"]["id"].as_str().unwrap().to_owned();

    // The requester reads its own request; its evaluations are reduced (REQ-010-25).
    let (status, flow) = call(&app, Method::GET, &format!("/v1/flows/{id}"), None).await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    let (status, page) = call(
        &app,
        Method::GET,
        &format!("/v1/flows/{id}/evaluations"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["items"][0]["source"].is_null(), "{page}");
    assert_eq!(page["items"][0]["verdict"], "allowed");

    // A stranger: the 404 of an absent flow, an empty list.
    let stranger = env.app(env.granted(&[]).await);
    let (status, _) = call(&stranger, Method::GET, &format!("/v1/flows/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, page) = call(&stranger, Method::GET, "/v1/flows", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ids(&page).is_empty(), "{page}");
}

/// REQ-011-14, 15, 17: a reader under `flow.flow.read` lists, filters and reads the flow
/// with its full evaluations, and finds it among the flows of its source address.
#[kartenn_testing::pg_test]
async fn a_reader_lists_filters_and_reads_flows(pool: PgPool) {
    let env = Env::new(&pool).await;
    let prod = env.address("10.1.0.0/24", "10.1.0.1", "prod").await;
    let dev = env.address("10.2.0.0/24", "10.2.0.1", "dev").await;
    let requester = env
        .granted(&["flow.flow.request", "prop.core.owner.write"])
        .await;
    let (status, answer) = call(
        &env.app(requester),
        Method::POST,
        "/v1/flows",
        Some(body(&env, prod, dev).await),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{answer}");
    let id = answer["flow"]["id"].as_str().unwrap().to_owned();

    let reader = env.app(env.granted(&["flow.flow.read"]).await);
    let (status, page) = call(&reader, Method::GET, "/v1/flows", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(ids(&page), std::slice::from_ref(&id));
    let (_, page) = call(&reader, Method::GET, "/v1/flows?summary=denied", None).await;
    assert!(ids(&page).is_empty(), "{page}");
    let (_, page) = call(
        &reader,
        Method::GET,
        &format!("/v1/flows?endpoint=address:{prod}&state=active"),
        None,
    )
    .await;
    assert_eq!(ids(&page), std::slice::from_ref(&id));
    let (status, _) = call(&reader, Method::GET, "/v1/flows?colour=blue", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, flow) = call(&reader, Method::GET, &format!("/v1/flows/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(flow["state"], "active");
    let (_, page) = call(
        &reader,
        Method::GET,
        &format!("/v1/flows/{id}/evaluations"),
        None,
    )
    .await;
    assert_eq!(page["items"][0]["source"], prod.to_string(), "{page}");

    let (status, page) = call(
        &reader,
        Method::GET,
        &format!("/v1/addresses/{prod}/flows"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(ids(&page), [id]);

    let (status, page) = call(&reader, Method::GET, "/v1/flow-nonconformities", None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page["items"].as_array().unwrap().is_empty());
}
