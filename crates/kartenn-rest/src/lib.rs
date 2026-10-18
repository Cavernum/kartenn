//! The REST API of kartenn (spec 006): router, problems, layers. No business logic: each
//! route calls the service, or the store after a rights check.

pub mod error;
pub mod health;
pub mod problem;
pub mod request_id;

use std::any::Any;
use std::sync::Arc;

use axum::Router;
use axum::response::{IntoResponse, Response};
use tower_http::catch_panic::CatchPanicLayer;

use crate::health::Readiness;
use crate::problem::{Problem, ProblemKind};

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

/// The application: `api` under `/v1`, the probes, and the layers around them, the
/// request identifier outermost so that every response carries it.
pub fn router(api: Router, readiness: Arc<dyn Readiness>) -> Router {
    Router::new()
        .nest("/v1", api)
        .merge(health::routes(readiness))
        .fallback(not_found)
        .layer(CatchPanicLayer::custom(panicked))
        .layer(axum::middleware::from_fn(request_id::layer))
}
