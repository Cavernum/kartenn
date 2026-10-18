//! Request identifier and trace (layer 1, REQ-006-37; 012/REQ-012-6, 7): every request
//! gets a new UUID v7, returned in `Request-Id` and as the `instance` of its problem, and
//! a span that follows or links the incoming `traceparent`.

use std::fmt;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use kartenn_core::audit::{Actor, ActorKind};
use kartenn_service::audit::RequestContext;
use kartenn_telemetry::otel;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::problem::Problem;
use crate::transport::{self, Settings};

/// Header of the request identifier in responses.
pub static REQUEST_ID: HeaderName = HeaderName::from_static("request-id");

/// The identifier of one request, drawn by kartenn: an incoming one is never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(Uuid);

impl RequestId {
    /// A new identifier (UUID v7).
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// The UUID.
    #[must_use]
    pub const fn uuid(self) -> Uuid {
        self.0
    }

    /// As the `instance` of a problem (RFC 9457 §3.1.5): `urn:uuid:…`.
    #[must_use]
    pub fn urn(self) -> String {
        self.0.urn().to_string()
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(f)
    }
}

/// The context of a request before authentication: no actor established yet.
fn context(id: RequestId) -> RequestContext {
    let mut context = RequestContext::system();
    context.actor = Actor {
        kind: ActorKind::Unknown,
        ..Actor::system()
    };
    context.correlation.request_id = Some(id.to_string());
    if let Some((trace_id, span_id)) = otel::current_ids() {
        context.correlation.trace_id = Some(trace_id);
        context.correlation.span_id = Some(span_id);
    }
    context
}

/// The layer: identifier and [`RequestContext`] in the request extensions, span around
/// the rest of the stack, `Request-Id` on the response and `instance` in its problem. The
/// incoming `traceparent` is followed only from a trusted proxy (REQ-012-7).
pub async fn layer(
    State(settings): State<Arc<Settings>>,
    mut request: Request,
    next: Next,
) -> Response {
    let id = RequestId::new();
    let method = request.method().clone();
    let trusted = transport::peer(&request).is_some_and(|peer| settings.trusts(peer));
    let incoming = otel::incoming(
        request
            .headers()
            .get("traceparent")
            .and_then(|v| v.to_str().ok()),
        trusted,
    );
    let span = tracing::info_span!("http.request", request_id = %id, method = %method);
    if let Some(parent) = incoming.parent {
        let _ = span.set_parent(parent);
    }
    if let Some(link) = incoming.link {
        span.add_link(link);
    }
    let context = span.in_scope(|| context(id));
    request.extensions_mut().insert(id);
    request.extensions_mut().insert(context);

    let mut response = next.run(request).instrument(span.clone()).await;

    let status = response.status();
    span.in_scope(|| {
        if status.is_server_error() {
            tracing::warn!(name: "http.request", request_id = %id, method = %method, status = status.as_u16(), "request failed");
        } else {
            tracing::debug!(name: "http.request", request_id = %id, method = %method, status = status.as_u16(), "request served");
        }
    });
    if let Some(problem) = response.extensions_mut().remove::<Problem>() {
        *response.body_mut() = Body::from(problem.instance(id.urn()).body());
        response.headers_mut().remove(header::CONTENT_LENGTH);
    }
    if let Ok(value) = HeaderValue::from_str(&id.to_string()) {
        response.headers_mut().insert(REQUEST_ID.clone(), value);
    }
    response
}
