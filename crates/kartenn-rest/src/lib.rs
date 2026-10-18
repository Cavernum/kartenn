//! The REST API of kartenn (spec 006): router, problems, layers. No business logic: each
//! route calls the service, or the store after a rights check.

pub mod error;
pub mod health;
pub mod problem;
pub mod request_id;
pub mod transport;

use std::any::Any;
use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::timeout::RequestBodyTimeoutLayer;

use crate::health::Readiness;
use crate::problem::{Problem, ProblemKind};
use crate::transport::Settings;

/// The routes under `/v1`.
pub fn v1() -> Router {
    Router::new()
}

/// A panic answers a generic 500: its message goes to the log (`sys.panic`), never to the
/// client (REQ-006-20).
fn panicked(_: Box<dyn Any + Send + 'static>) -> Response {
    Problem::new(ProblemKind::Internal).into_response()
}

async fn not_found() -> Problem {
    Problem::new(ProblemKind::NotFound)
}

/// The application: `api` under `/v1`, the probes, and the layers around them (design
/// « Pile de couches »), the request identifier outermost so that every response carries
/// it, then the delay, the client and the bounded body.
pub fn router(api: Router, readiness: Arc<dyn Readiness>, transport: Settings) -> Router {
    let transport = Arc::new(transport);
    Router::new()
        .nest("/v1", api)
        .merge(health::routes(readiness))
        .fallback(not_found)
        // The limit is that of layer 5, not the 2 MB of the extractors.
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyTimeoutLayer::new(transport.request_timeout))
        .layer(from_fn_with_state(transport.clone(), transport::limits))
        .layer(from_fn_with_state(transport.clone(), transport::forwarded))
        .layer(from_fn_with_state(transport.clone(), transport::delay))
        .layer(CatchPanicLayer::custom(panicked))
        .layer(from_fn_with_state(transport, request_id::layer))
}
