//! The REST API of kartenn (spec 006): router, problems, layers. No business logic: each
//! route calls the service, or the store after a rights check.

pub mod anomalies;
pub mod audit;
pub mod auth;
pub mod authn;
pub mod capabilities;
pub mod changesets;
pub mod compression;
pub mod contract;
pub mod cookies;
pub mod csrf;
pub mod cursor;
mod cycle;
pub mod dated;
pub mod error;
pub mod filter;
pub mod health;
pub mod imports;
pub mod objects;
pub mod openapi;
mod operation;
pub mod problem;
pub mod properties;
pub mod quota;
pub mod request_id;
pub mod resources;
pub mod rights;
pub mod transport;
pub mod tree;
pub mod versions;
pub mod web;

use std::any::Any;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::timeout::RequestBodyTimeoutLayer;
use utoipa_axum::router::OpenApiRouter;

use crate::authn::Authenticator;
use crate::capabilities::BodyLimit;
use crate::health::Readiness;
use crate::problem::{Problem, ProblemKind};
use crate::quota::{InFlight, Quotas};
use crate::transport::Settings;

/// The routes under `/v1` but the login ones, with their contract: each module adds its
/// routes and their schemas (design « Versions et contrat »).
pub fn api() -> OpenApiRouter<objects::Api> {
    objects::routes()
        .merge(tree::routes())
        .merge(anomalies::routes())
        .merge(changesets::routes())
        .merge(properties::routes())
        .merge(rights::routes())
        .merge(imports::routes())
        .merge(audit::routes())
        .merge(capabilities::routes())
        .merge(openapi::routes())
}

/// The routes under `/v1` but the login ones.
pub fn v1(api: objects::Api) -> Router {
    self::api().with_state(api).split_for_parts().0
}

/// A panic answers a generic 500: its message goes to the log (`sys.panic`), never to the
/// client (REQ-006-20).
fn panicked(_: Box<dyn Any + Send + 'static>) -> Response {
    Problem::new(ProblemKind::Internal).into_response()
}

async fn not_found() -> Problem {
    Problem::new(ProblemKind::NotFound)
}

/// The application: `api` and the login routes under `/v1`, the probes, and the layers
/// around them (design
/// « Pile de couches »), the request identifier outermost so that every response carries
/// it, then the load shedding, the delay, the client, the bounded body, the
/// authentication, the quotas, the CSRF check and, innermost, the compression.
pub fn router(
    api: Router,
    readiness: Arc<dyn Readiness>,
    transport: Settings,
    quotas: Arc<Quotas>,
    authenticator: Arc<dyn Authenticator>,
) -> Router {
    router_with_web(api, readiness, transport, quotas, authenticator, None)
}

/// [`router`], with the shell and its plugins outside `/v1` and `/health` when `web` is
/// given (spec 007, REQ-007-1).
pub fn router_with_web(
    api: Router,
    readiness: Arc<dyn Readiness>,
    transport: Settings,
    quotas: Arc<Quotas>,
    authenticator: Arc<dyn Authenticator>,
    web: Option<Arc<web::Web>>,
) -> Router {
    let transport = Arc::new(transport);
    let in_flight = Arc::new(InFlight::new(transport.max_in_flight));
    let routes = Router::new()
        .nest(
            "/v1",
            api.merge(
                auth::routes()
                    .with_state(authenticator.clone())
                    .split_for_parts()
                    .0,
            ),
        )
        .merge(health::routes().with_state(readiness).split_for_parts().0);
    let routes = match web {
        Some(web) => routes.fallback_service(Router::new().fallback(web::serve).with_state(web)),
        None => routes.fallback(not_found),
    };
    routes
        // What `GET /v1/capabilities` tells of layer 5.
        .layer(Extension(BodyLimit(transport.max_body_bytes)))
        // Layer 10, on the way back.
        .layer(compression::layer())
        // The limit is that of layer 5, not the 2 MB of the extractors.
        .layer(DefaultBodyLimit::disable())
        // Layer 8, then layer 7, then layer 6 outside them: the quota counts per identity.
        .layer(from_fn_with_state(transport.clone(), csrf::layer))
        .layer(from_fn_with_state(quotas, quota::layer))
        .layer(from_fn_with_state(authenticator, authn::layer))
        .layer(RequestBodyTimeoutLayer::new(transport.request_timeout))
        .layer(from_fn_with_state(transport.clone(), transport::limits))
        .layer(from_fn_with_state(transport.clone(), transport::forwarded))
        .layer(from_fn_with_state(transport.clone(), transport::delay))
        .layer(from_fn_with_state(in_flight, quota::shed))
        .layer(CatchPanicLayer::custom(panicked))
        .layer(from_fn_with_state(transport, request_id::layer))
}
