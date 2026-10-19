//! The REST API of kartenn (spec 006): router, problems, layers. No business logic: each
//! route calls the service, or the store after a rights check.

pub mod anomalies;
pub mod audit;
pub mod auth;
pub mod authn;
pub mod changesets;
pub mod compression;
pub mod cookies;
pub mod csrf;
pub mod cursor;
mod cycle;
pub mod dated;
pub mod error;
pub mod filter;
pub mod health;
pub mod objects;
pub mod problem;
pub mod properties;
pub mod quota;
pub mod request_id;
pub mod resources;
pub mod rights;
pub mod transport;
pub mod tree;

use std::any::Any;
use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::timeout::RequestBodyTimeoutLayer;

use crate::authn::Authenticator;
use crate::health::Readiness;
use crate::problem::{Problem, ProblemKind};
use crate::quota::{InFlight, Quotas};
use crate::transport::Settings;

/// The routes under `/v1`.
pub fn v1(api: objects::Api) -> Router {
    objects::routes(api.clone())
        .merge(tree::routes(api.clone()))
        .merge(anomalies::routes(api.clone()))
        .merge(changesets::routes(api.clone()))
        .merge(properties::routes(api.clone()))
        .merge(rights::routes(api.clone()))
        .merge(audit::routes(api))
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
    let transport = Arc::new(transport);
    let in_flight = Arc::new(InFlight::new(transport.max_in_flight));
    Router::new()
        .nest("/v1", api.merge(auth::routes(authenticator.clone())))
        .merge(health::routes(readiness))
        .fallback(not_found)
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
