//! The event stream `GET /v1/events` (spec 022, ADR 0034): server-sent events fed by the
//! outbox, read once per instance ([`reader`]), filtered by the rights of each
//! subscriber ([`hub`]), written with an encrypted cursor ([`cursor`], [`stream`]).

pub mod audit;
pub mod cursor;
pub mod hub;
pub mod reader;
pub mod stream;

use std::sync::Arc;

use axum::Extension;
use axum::body::Body as HttpBody;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::{Method, Principal};
use kartenn_service::events::{FilterError, Filters};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::objects::Api;
use crate::problem::{Problem, ProblemKind};
use cursor::{Cursor, Position};
use hub::{Hub, Refused, SubjectKey, Watched};
use stream::{Body, Report, Start};

const LAST_EVENT_ID: HeaderName = HeaderName::from_static("last-event-id");
const SEC_FETCH_SITE: HeaderName = HeaderName::from_static("sec-fetch-site");
const X_ACCEL_BUFFERING: HeaderName = HeaderName::from_static("x-accel-buffering");
const INCREMENTAL: HeaderName = HeaderName::from_static("incremental");

/// The media type of the stream.
pub const EVENT_STREAM: &str = "text/event-stream";

/// The end of a cookie session if it is not seen again before (Unix seconds), as the
/// cookie read at the opening tells it: the stream is not an activity (REQ-022-18).
#[derive(Debug, Clone, Copy)]
pub struct QuietEnd(pub i64);

/// The end of the credentials of a request (Unix seconds): the `exp` of an access token.
#[derive(Debug, Clone, Copy)]
pub struct CredentialsEnd(pub i64);

/// The secret of a service account a request presented (`Basic`).
#[derive(Debug, Clone, Copy)]
pub struct ServiceSecret(pub uuid::Uuid);

/// A response that is no activity of its session: its cookie is not sent again.
#[derive(Debug, Clone, Copy)]
pub struct NotActivity;

/// The routes of the stream, under `/v1`.
pub(crate) fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new().routes(routes!(events))
}

/// Whether `Accept` admits `text/event-stream` (absent: anything).
fn accepts(headers: &HeaderMap) -> bool {
    let values: Vec<&str> = headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    if values.is_empty() {
        return true;
    }
    values.iter().flat_map(|v| v.split(',')).any(|range| {
        let mut parts = range.split(';');
        let media = parts.next().unwrap_or_default().trim();
        let refused = parts.any(|p| {
            p.trim()
                .strip_prefix("q=")
                .and_then(|q| q.trim().parse::<f32>().ok())
                .is_some_and(|q| q <= 0.0)
        });
        !refused
            && (media.eq_ignore_ascii_case(EVENT_STREAM)
                || media == "*/*"
                || media.eq_ignore_ascii_case("text/*"))
    })
}

/// REQ-022-15: a cookie session opens the stream only from the public origin: `Origin`,
/// when present, equal to it; `Sec-Fetch-Site`, when present, `same-origin`.
#[must_use]
pub fn same_origin(headers: &HeaderMap, public_origin: Option<&str>) -> bool {
    if let Some(site) = headers.get(SEC_FETCH_SITE)
        && site.as_bytes() != b"same-origin"
    {
        return false;
    }
    match headers.get(header::ORIGIN) {
        None => true,
        Some(origin) => public_origin.is_some_and(|public| {
            origin
                .to_str()
                .is_ok_and(|o| o.eq_ignore_ascii_case(public))
        }),
    }
}

/// The types a filter may name: those of the core and of the modules of the build.
fn known(hub: &Hub, name: &str) -> bool {
    kartenn_store_pg::rbac::READ_PERMISSIONS
        .iter()
        .any(|(kind, _)| name.strip_prefix("core.") == Some(kind))
        || hub.modules().object_types().any(|t| t.kind == name)
}

fn filter_problem(e: &FilterError) -> Problem {
    Problem::new(ProblemKind::BadRequest)
        .detail(e.to_string())
        .extension("code", serde_json::json!("events.filter.invalid"))
}

/// The filters and the resumption of `GET /v1/events` (REQ-022-2, 7).
#[derive(utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)]
struct StreamParams {
    /// Qualified types of object (`core.prefix`, `vlan.vlan`), comma separated, 32 at
    /// most.
    types: Option<String>,
    /// Addressing domains, comma separated, 32 at most.
    domains: Option<String>,
    /// Objects watched, comma separated, 100 at most.
    objects: Option<String>,
    /// Kinds of event among `changes`, `changeset`, `labels`; `ready` and `reset` always
    /// come.
    kinds: Option<String>,
    /// The cursor to resume after, when the header `Last-Event-ID` cannot be set (a new
    /// `EventSource`).
    last_event_id: Option<String>,
}

/// The data of `ready`.
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) struct EventReady {
    /// The instance that serves the stream.
    instance: uuid::Uuid,
    /// Seconds of silence before a comment.
    heartbeat: u64,
    /// How long the ring of the instance keeps an event, in seconds.
    ring_seconds: u64,
}

/// An object named by `changes`.
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) struct EventObject {
    /// Its qualified type.
    #[schema(rename = "type")]
    kind: String,
    /// Its identifier.
    id: uuid::Uuid,
}

/// The data of `changes`: keys and revisions, never values (REQ-022-4, 5).
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) struct EventChanges {
    /// The changeset, the revision of every object named; absent for a background
    /// task.
    changeset: Option<uuid::Uuid>,
    /// `<module>:<task>`, for an event of a background task.
    task: Option<String>,
    /// The CloudEvents identifier of an event of a module.
    event: Option<String>,
    /// The objects written; absent when `truncated` names types alone.
    objects: Option<Vec<EventObject>>,
    /// A broad invalidation: every object of `types` may have changed.
    truncated: Option<bool>,
    /// The types of a broad invalidation.
    types: Option<Vec<String>>,
    /// The addressing domains written, those the subscriber may read.
    domains: Vec<uuid::Uuid>,
}

/// The data of `changeset`: a pending changeset changed state (REQ-022-17).
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) struct EventChangeset {
    /// The changeset.
    changeset: uuid::Uuid,
    /// `pending`, `applied`, `rejected`, `withdrawn`, `expired`, `failed_revalidation`.
    state: String,
}

/// The data of `labels`: the labels changed (spec 018).
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) struct EventLabels {
    /// The new revision of the labels.
    revision: String,
}

/// The data of `reset`: invalidate everything (REQ-022-8, 18, 21).
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) struct EventReset {
    /// `too_old`, `invalid_cursor`, `overflow`, `rights_changed`.
    reason: String,
}

/// The extension of the operation that names the schema of the data of each kind of
/// event (REQ-022-23): OpenAPI 3.1 does not describe the items of a stream.
pub const EVENTS_EXTENSION: &str = "x-kartenn-events";

/// The schemas of the data of the events, by kind.
pub(crate) fn event_schemas() -> Vec<(
    &'static str,
    String,
    utoipa::openapi::RefOr<utoipa::openapi::Schema>,
)> {
    use utoipa::{PartialSchema, ToSchema};
    vec![
        (
            "ready",
            EventReady::name().into_owned(),
            EventReady::schema(),
        ),
        (
            "changes",
            EventChanges::name().into_owned(),
            EventChanges::schema(),
        ),
        (
            "changeset",
            EventChangeset::name().into_owned(),
            EventChangeset::schema(),
        ),
        (
            "labels",
            EventLabels::name().into_owned(),
            EventLabels::schema(),
        ),
        (
            "reset",
            EventReset::name().into_owned(),
            EventReset::schema(),
        ),
    ]
}

/// The schema of an object named by `changes`.
pub(crate) fn event_object_schema() -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
    <EventObject as utoipa::PartialSchema>::schema()
}

/// `GET /v1/events`: the stream of the changes the caller may read (REQ-022-1).
#[utoipa::path(
    get,
    path = "/events",
    operation_id = "events",
    tag = "operations",
    params(
        StreamParams,
        ("Last-Event-ID" = Option<String>, Header, description = "The cursor to resume after."),
    ),
    responses(
        (status = 200, description = "The stream of events (`text/event-stream`, WHATWG HTML \
            § 9.2): `ready`, then `changes`, `changeset`, `labels`, `reset` and resumption \
            points; each `id:` an opaque cursor. The data of each kind is named by the \
            extension `x-kartenn-events`.", content_type = "text/event-stream", body = String),
        (status = 400, description = "A filter unknown or out of bounds (`events.filter.invalid`).", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "A cookie session from another origin.", body = Problem, content_type = "application/problem+json"),
        (status = 406, description = "`Accept` does not admit `text/event-stream`.", body = Problem, content_type = "application/problem+json"),
        (status = 429, description = "Too many streams for the identity (`Retry-After`).", body = Problem, content_type = "application/problem+json"),
        (status = 503, description = "Too many streams for the instance, or it stops (`Retry-After`).", body = Problem, content_type = "application/problem+json"),
    )
)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // the extractors of one route, then its steps
async fn events(
    State(api): State<Api>,
    hub: Option<Extension<Arc<Hub>>>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    quiet: Option<Extension<QuietEnd>>,
    session: Option<Extension<kartenn_auth::oidc::Session>>,
    secret: Option<Extension<ServiceSecret>>,
    kill: Option<Extension<crate::transport::ConnectionKill>>,
    credentials: Option<Extension<CredentialsEnd>>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    let Some(Extension(hub)) = hub else {
        return Problem::new(ProblemKind::NotFound).into_response();
    };
    if !accepts(&headers) {
        return Problem::new(ProblemKind::NotAcceptable)
            .detail("the stream is text/event-stream")
            .into_response();
    }
    let by_cookie = matches!(principal.method, Method::OidcSession | Method::Breakglass);
    if by_cookie && !same_origin(&headers, hub.settings.public_origin.as_deref()) {
        return Problem::new(ProblemKind::CrossSiteRequest).into_response();
    }
    let query = query.unwrap_or_default();
    let pairs: Vec<(String, String)> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let mut filters = match Filters::parse(
        pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        &|name| known(&hub, name),
    ) {
        Ok(filters) => filters,
        Err(e) => return filter_problem(&e).into_response(),
    };
    let last = headers
        .get(LAST_EVENT_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .or_else(|| {
            pairs
                .iter()
                .find(|(k, _)| k == "last_event_id")
                .map(|(_, v)| v.clone())
        })
        .filter(|v| !v.is_empty());
    let resumed = last
        .as_deref()
        .map(|text| Cursor::decode(&hub.ring_keys, text, principal.identity));
    let watched = Watched {
        session_started: session.map(|Extension(s)| s.started),
        secret: secret.map(|Extension(ServiceSecret(s))| s),
        kill: kill.map(|Extension(k)| k),
        closing: by_cookie.then(|| (principal.clone(), context.source.clone())),
        service: principal.kind == kartenn_service::authn::IdentityKind::ServiceAccount,
    };
    let criteria = filters.to_json();
    let opened = match hub.open(
        SubjectKey::of(&principal.subject()),
        std::mem::take(&mut filters),
        watched,
        resumed.map(|r| r.map(|c| c.position.floor).map_err(|_| ())),
    ) {
        Ok(opened) => opened,
        Err(refused) => {
            let (kind, detail) = match refused {
                Refused::Instance => (ProblemKind::Unavailable, "too many event streams"),
                Refused::Identity => (
                    ProblemKind::RateLimited,
                    "too many event streams for this identity",
                ),
            };
            return (
                [(header::RETRY_AFTER, HeaderValue::from_static("5"))],
                Problem::new(kind).detail(detail),
            )
                .into_response();
        }
    };
    hub.journal.get_or_init(|| api.journal.clone());
    if let Err(e) = hub.prime(&opened.conn.subject).await {
        tracing::warn!(name: "events.scope.failed", error = %e.0, "the rights of a subscriber could not be read");
        hub.release(opened.conn.id);
        return Problem::new(ProblemKind::Unavailable).into_response();
    }
    // REQ-022-24: the opening recorded, or no stream (INV-50).
    let recorded = match audit::opened(&context, criteria, resumed.is_some()) {
        Ok(event) => api.journal.record(&event).await.map_err(|e| e.0),
        Err(e) => Err(e.to_string()),
    };
    if let Err(e) = recorded {
        tracing::error!(name: "audit.write.failed", error = %e, "the opening of an event stream could not be recorded");
        hub.release(opened.conn.id);
        return Problem::new(ProblemKind::Unavailable).into_response();
    }
    let report: Arc<dyn Report> = Arc::new(audit::Audited {
        journal: api.journal.clone(),
        context,
        ids_max: api.audit.ids_max,
    });
    let position = match (resumed, opened.reset) {
        (Some(Ok(c)), None) => c.position,
        _ => Position {
            floor: opened.floor,
            last: opened.floor,
        },
    };
    let (replay, reset) = match hub.replay(&opened.conn, &opened.replay, opened.floor).await {
        Ok(replay) => (replay, opened.reset.map(|r| (r, opened.floor))),
        Err(e) => {
            tracing::warn!(name: "events.replay.failed", error = %e.0, "a resumption could not be replayed");
            (
                Vec::new(),
                Some((kartenn_service::events::ResetReason::TooOld, opened.floor)),
            )
        }
    };
    let expires = match (quiet, credentials) {
        (Some(Extension(QuietEnd(a))), Some(Extension(CredentialsEnd(b)))) => Some(a.min(b)),
        (Some(Extension(QuietEnd(a))), None) | (None, Some(Extension(CredentialsEnd(a)))) => {
            Some(a)
        }
        (None, None) => None,
    };
    let body = Body::new(
        hub.clone(),
        opened.conn,
        opened.rx,
        Start {
            position,
            reset,
            replay,
            expires,
        },
        Some(report),
    );
    let mut response = Response::new(HttpBody::from_stream(stream::stream(body)));
    *response.status_mut() = StatusCode::OK;
    let h = response.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(EVENT_STREAM));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(X_ACCEL_BUFFERING, HeaderValue::from_static("no"));
    if hub.settings.incremental {
        h.insert(INCREMENTAL, HeaderValue::from_static("?1"));
    }
    response.extensions_mut().insert(NotActivity);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.append(HeaderName::from_static(k), HeaderValue::from_static(v));
        }
        h
    }

    #[test]
    fn accept_admits_the_stream_or_anything() {
        assert!(accepts(&headers(&[])));
        assert!(accepts(&headers(&[("accept", "text/event-stream")])));
        assert!(accepts(&headers(&[(
            "accept",
            "application/json, */*;q=0.1"
        )])));
        assert!(!accepts(&headers(&[("accept", "application/json")])));
        assert!(!accepts(&headers(&[("accept", "text/event-stream;q=0")])));
    }

    /// REQ-022-15: the matrix of `Origin` × `Sec-Fetch-Site` for a cookie session.
    #[test]
    fn a_cookie_opens_the_stream_from_the_public_origin_only() {
        const PUBLIC: &str = "https://kartenn.example";
        for site in [
            Some("same-origin"),
            Some("same-site"),
            Some("cross-site"),
            Some("none"),
            None,
        ] {
            for origin in [Some(PUBLIC), Some("https://other.example"), None] {
                let mut h = HeaderMap::new();
                if let Some(site) = site {
                    h.insert(SEC_FETCH_SITE, HeaderValue::from_static(site));
                }
                if let Some(origin) = origin {
                    h.insert(header::ORIGIN, HeaderValue::from_static(origin));
                }
                let expected = matches!(site, None | Some("same-origin"))
                    && matches!(origin, None | Some(PUBLIC));
                assert_eq!(
                    same_origin(&h, Some(PUBLIC)),
                    expected,
                    "{site:?} × {origin:?}"
                );
            }
        }
    }
}
