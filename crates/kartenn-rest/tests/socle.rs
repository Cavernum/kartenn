//! The base of the API: problems, request identifier, probes (spec 006, REQ-006-3, 20,
//! 36, 37).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::Extension;
use axum::http::{Request, Response, StatusCode};
use axum::routing::get;
use http_body_util::BodyExt;
use kartenn_core::audit::AUTHZ_DENIED;
use kartenn_rest::health::{PgReadiness, Readiness};
use kartenn_rest::problem::{Problem, ProblemKind};
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::audit::{
    AuditJournal, AuditStatus, MemoryJournal, Outcome, RequestContext, SinkHealth,
};
use kartenn_telemetry::capture::{CaptureLayer, Output};
use sqlx::PgPool;
use tower::ServiceExt;

mod common;
use tracing_subscriber::layer::SubscriberExt;
use uuid::Uuid;

struct Ready(bool);

impl Readiness for Ready {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(self.0))
    }
}

fn app(api: Router) -> Router {
    kartenn_rest::router(
        api,
        Arc::new(Ready(true)),
        Settings::default(),
        quotas(),
        common::anyone(),
    )
}

fn quotas() -> Arc<Quotas> {
    Arc::new(Quotas::new(Rates::default()))
}

async fn call(app: Router, uri: &str) -> Response<Body> {
    app.oneshot(common::bearer(
        Request::get(uri).body(Body::empty()).unwrap(),
    ))
    .await
    .unwrap()
}

fn request_id(response: &Response<Body>) -> Uuid {
    response.headers()["request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap()
}

async fn problem(response: Response<Body>) -> serde_json::Value {
    assert_eq!(
        response.headers()["content-type"],
        "application/problem+json"
    );
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn a_panic_answers_a_generic_500_with_the_request_identifier() {
    async fn boom() -> StatusCode {
        panic!("secret detail in the panic")
    }
    let api = Router::new().route("/boom", get(boom));
    let response = call(app(api), "/v1/boom").await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let id = request_id(&response);
    let body = problem(response).await;
    assert_eq!(body["type"], "urn:kartenn:problem:internal");
    assert_eq!(body["status"], 500);
    assert_eq!(body["instance"], id.urn().to_string());
    assert!(!body.to_string().contains("secret"), "{body}");
}

#[tokio::test]
async fn an_unknown_route_is_a_not_found_problem() {
    let response = call(app(kartenn_rest::v1()), "/v1/nothing").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let id = request_id(&response);
    let body = problem(response).await;
    assert_eq!(body["type"], "urn:kartenn:problem:not-found");
    assert_eq!(body["instance"], id.urn().to_string());
}

#[tokio::test]
async fn the_same_identifier_is_in_the_response_the_log_and_the_audit() {
    let (output, mut logs) = Output::new("test", "debug", 64).unwrap();
    let subscriber = tracing_subscriber::registry().with(CaptureLayer::new(vec![output]));
    let _guard = tracing::subscriber::set_default(subscriber);
    let journal = MemoryJournal::new();
    let api = Router::new().route(
        "/denied",
        get({
            let journal = journal.clone();
            move |Extension(context): Extension<RequestContext>| async move {
                let mut context = context;
                context.actor = RequestContext::person(Uuid::now_v7()).actor;
                let event = context.event(&AUTHZ_DENIED, Outcome::Denied).unwrap();
                journal.record(&event).await.unwrap();
                Problem::new(ProblemKind::Forbidden)
            }
        }),
    );

    let incoming = Uuid::now_v7().to_string();
    let response = app(api)
        .oneshot(
            Request::get("/v1/denied")
                .header("authorization", "Bearer test")
                .header("request-id", &incoming)
                .header("traceparent", "not a traceparent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let id = request_id(&response);
    assert_ne!(
        id.to_string(),
        incoming,
        "an incoming identifier is not reused"
    );
    assert_eq!(id.get_version_num(), 7);
    assert_eq!(problem(response).await["instance"], id.urn().to_string());

    let events = journal.events();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].correlation.request_id.as_deref(),
        Some(id.to_string().as_str())
    );
    let mut logged = Vec::new();
    while let Ok(record) = logs.try_recv() {
        logged.push(record);
    }
    let served = logged
        .iter()
        .find(|r| r.name == "http.request")
        .expect("the request is logged");
    assert_eq!(served.fields["request_id"], id.to_string());
    assert_eq!(served.fields["status"], "403");
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn ready_follows_the_database_and_the_audit_outputs(pool: PgPool) {
    let audit = AuditStatus::default();
    let app = kartenn_rest::router(
        kartenn_rest::v1(),
        Arc::new(PgReadiness::new(pool.clone(), audit.clone())),
        Settings::default(),
        quotas(),
        common::anyone(),
    );
    assert_eq!(
        call(app.clone(), "/health/ready").await.status(),
        StatusCode::OK
    );

    let mut down = SinkHealth::default();
    let s = |secs: i64| kartenn_core::period::Instant(secs * 1_000_000);
    down.observe(false, s(0));
    down.observe(false, s(60));
    audit.update([&down]);
    let response = call(app.clone(), "/health/ready").await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .is_empty(),
        "no internal detail"
    );
    audit.update([]);

    pool.close().await;
    assert_eq!(
        call(app.clone(), "/health/ready").await.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "database stopped"
    );
    assert_eq!(
        call(app, "/health/live").await.status(),
        StatusCode::OK,
        "live while the process answers"
    );
}
