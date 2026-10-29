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
use kartenn_service::RequestError;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::{Method, Principal};
use kartenn_service::events::{FilterError, Filters};
use kartenn_service::reads::objects_read;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::cursor::ListingLinks;
use crate::objects::{Api, audited};
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
/// The media type of a page of the catch-up (REQ-022-28).
const JSON: &str = "application/json";

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

/// What `Accept` asks of `GET /v1/events`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wanted {
    /// The stream: `text/event-stream` named, or admitted by a range (absent: anything).
    Stream,
    /// A page of the catch-up (REQ-022-28): `application/json` named, the stream not.
    Page,
    /// Neither: 406.
    Neither,
}

/// What `Accept` asks: the stream when it names `text/event-stream`, a page when it
/// names `application/json`, the stream when a range admits it.
fn wanted(headers: &HeaderMap) -> Wanted {
    let values: Vec<&str> = headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    if values.is_empty() {
        return Wanted::Stream;
    }
    let admitted: Vec<&str> = values
        .iter()
        .flat_map(|v| v.split(','))
        .filter_map(|range| {
            let mut parts = range.split(';');
            let media = parts.next().unwrap_or_default().trim();
            let refused = parts.any(|p| {
                p.trim()
                    .strip_prefix("q=")
                    .and_then(|q| q.trim().parse::<f32>().ok())
                    .is_some_and(|q| q <= 0.0)
            });
            (!refused).then_some(media)
        })
        .collect();
    let named = |media: &str| admitted.iter().any(|m| m.eq_ignore_ascii_case(media));
    if named(EVENT_STREAM) {
        Wanted::Stream
    } else if named(JSON) {
        Wanted::Page
    } else if admitted
        .iter()
        .any(|m| *m == "*/*" || m.eq_ignore_ascii_case("text/*"))
    {
        Wanted::Stream
    } else {
        Wanted::Neither
    }
}

/// Whether a request asks a page of the catch-up rather than the stream (REQ-022-28):
/// a page is an ordinary read, counted by the quota.
#[must_use]
pub fn wants_page(headers: &HeaderMap) -> bool {
    wanted(headers) == Wanted::Page
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
    /// Qualified types of object (`core.prefix`, `core.vlan`, `machines.machine`), comma separated, 32 at
    /// most.
    types: Option<String>,
    /// Addressing domains, comma separated, 32 at most.
    domains: Option<String>,
    /// Objects watched, comma separated, 100 at most.
    objects: Option<String>,
    /// Kinds of event among `changes`, `changeset`, `labels`, `draft`, `stats`; `ready` and
    /// `reset` always come.
    kinds: Option<String>,
    /// The cursor to resume after, when the header `Last-Event-ID` cannot be set (a new
    /// `EventSource`).
    last_event_id: Option<String>,
    /// Under `Accept: application/json`, the events of a page, 1 to 1 000; 100 by
    /// default (REQ-022-28).
    page_size: Option<u16>,
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

/// The data of `draft`: a draft of the subscriber changed (spec 024, REQ-024-6); its
/// author alone receives it, never its content.
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) struct EventDraft {
    /// The draft.
    id: uuid::Uuid,
    /// Its new version; absent when it is deleted.
    version: Option<i64>,
    /// `true`: deleted, submitted or taken over by someone else.
    deleted: Option<bool>,
}

/// The data of `stats`, empty: a counter of `GET /v1/stats` may have changed, read it
/// again (spec 024, REQ-024-16). At most one per 30 s, without `id:` (the cursor stays).
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(crate) struct EventStats {}

/// An event of a page of the catch-up (REQ-022-28).
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct EventItem {
    /// Its kind: `changes`, `changeset`, `labels`, `draft`, `reset`.
    event: &'static str,
    /// The cursor to resume just after it.
    id: String,
    /// Its data, as the extension `x-kartenn-events` names it by kind.
    #[schema(value_type = Object)]
    data: serde_json::Value,
}

/// A page of the catch-up after a cursor (REQ-022-28).
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct EventPage<'a> {
    /// The events after the cursor, by number; a `reset` alone when the cursor is too old
    /// or invalid.
    items: Vec<EventItem>,
    /// The cursor to come back with later (`last_event_id` or `Last-Event-ID`).
    cursor: String,
    /// `next`: the rest at once, when the page is full; `null` when up to date.
    links: ListingLinks<'a>,
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
            "draft",
            EventDraft::name().into_owned(),
            EventDraft::schema(),
        ),
        (
            "stats",
            EventStats::name().into_owned(),
            EventStats::schema(),
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
            § 9.2): `ready`, then `changes`, `changeset`, `labels`, `draft`, `stats`, `reset` and resumption \
            points; each `id:` an opaque cursor. The data of each kind is named by the \
            extension `x-kartenn-events`. Under `Accept: application/json`, a page of the \
            events after the cursor (REQ-022-28): `links.next` when it is full, `cursor` \
            to come back with.", content(
                (String = "text/event-stream"),
                (EventPage = "application/json"),
            )),
        (status = 400, description = "A filter unknown or out of bounds (`events.filter.invalid`), or a `page_size` out of 1 to 1 000.", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "A cookie session from another origin.", body = Problem, content_type = "application/problem+json"),
        (status = 406, description = "`Accept` admits neither `text/event-stream` nor `application/json`.", body = Problem, content_type = "application/problem+json"),
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
    let wanted = wanted(&headers);
    if wanted == Wanted::Neither {
        return Problem::new(ProblemKind::NotAcceptable)
            .detail("the stream is text/event-stream, its catch-up application/json")
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
    let paged = wanted == Wanted::Page;
    let mut filters = match Filters::parse(
        pairs
            .iter()
            .filter(|(k, _)| !(paged && k == "page_size"))
            .map(|(k, v)| (k.as_str(), v.as_str())),
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
    if paged {
        let size = match page_size(&pairs) {
            Ok(size) => size,
            Err(problem) => return problem.into_response(),
        };
        let resume = resumed.map(|r| r.map(|c| c.position.floor).map_err(|_| ()));
        return page(
            &api, &hub, &principal, &context, &pairs, &filters, resume, size,
        )
        .await
        .unwrap_or_else(IntoResponse::into_response);
    }
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
    h.insert(header::VARY, HeaderValue::from_static("accept"));
    h.insert(X_ACCEL_BUFFERING, HeaderValue::from_static("no"));
    if hub.settings.incremental {
        h.insert(INCREMENTAL, HeaderValue::from_static("?1"));
    }
    response.extensions_mut().insert(NotActivity);
    response
}

/// The `page_size` of a page (REQ-022-28): once at most, 1 to 1 000, 100 by default.
fn page_size(pairs: &[(String, String)]) -> Result<u16, Problem> {
    let mut sizes = pairs.iter().filter(|(k, _)| k == "page_size");
    let size = sizes.next();
    if sizes.next().is_some() {
        return Err(
            Problem::new(ProblemKind::BadRequest).detail("page_size is given more than once")
        );
    }
    size.map_or(Ok(crate::cursor::DEFAULT_SIZE), |(_, v)| {
        crate::cursor::size(v)
    })
}

/// A page of the catch-up (REQ-022-28): the events of the ring after `resume` that the
/// caller may read now, an ordinary read audited as such (`data.objects.read`).
#[allow(clippy::too_many_arguments)] // the parts of one request
async fn page(
    api: &Api,
    hub: &Hub,
    principal: &Principal,
    context: &RequestContext,
    pairs: &[(String, String)],
    filters: &Filters,
    resume: Option<Result<i64, ()>>,
    size: u16,
) -> Result<Response, Problem> {
    let resumed = resume.is_some();
    let paged = hub
        .page(
            &SubjectKey::of(&principal.subject()),
            filters,
            resume,
            usize::from(size),
        )
        .await
        .map_err(|e| {
            tracing::warn!(name: "events.page.failed", error = %e.0, "a page of events could not be read");
            Problem::new(ProblemKind::Unavailable)
        })?;
    let cursor_at = |floor: i64| {
        Cursor {
            position: Position { floor, last: floor },
            identity: principal.identity,
            issued: stream::unix_now(),
        }
        .encode(&hub.ring_keys)
    };
    let mut items = Vec::with_capacity(paged.events.len() + 1);
    let mut ids = Vec::new();
    if let Some(reason) = paged.reset {
        items.push(EventItem {
            event: kartenn_service::events::Kind::Reset.as_str(),
            id: cursor_at(paged.floor),
            data: serde_json::json!({ "reason": reason.as_str() }),
        });
    }
    for message in paged.events {
        let hub::Message::Event {
            kind,
            data,
            objects,
            floor,
            ..
        } = message
        else {
            continue;
        };
        ids.extend(objects);
        items.push(EventItem {
            event: kind.as_str(),
            id: cursor_at(floor),
            data: serde_json::from_str(&data).expect("the data of an event is JSON"),
        });
    }
    let cursor = cursor_at(paged.floor);
    let next = paged.full.then(|| {
        let mut query = form_urlencoded::Serializer::new(String::new());
        for (k, v) in pairs.iter().filter(|(k, _)| k != "last_event_id") {
            query.append_pair(k, v);
        }
        query.append_pair("last_event_id", &cursor);
        format!("/v1/events?{}", query.finish())
    });
    // REQ-012-18: a page is a read, recorded with the objects it names.
    let mut criteria = filters.to_json();
    if let Some(c) = criteria.as_object_mut() {
        c.insert("page_size".into(), serde_json::json!(size));
        c.insert("resumed".into(), serde_json::json!(resumed));
    }
    let event = objects_read(context, "object", &ids, criteria, api.audit).map_err(audited)?;
    api.journal
        .record(&event)
        .await
        .map_err(RequestError::Audit)?;
    let body = EventPage {
        items,
        cursor,
        links: ListingLinks {
            next: next.as_deref(),
        },
    };
    let mut response = (
        [(header::CONTENT_TYPE, HeaderValue::from_static(JSON))],
        serde_json::to_vec(&body).expect("JSON values serialise"),
    )
        .into_response();
    let h = response.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(header::VARY, HeaderValue::from_static("accept"));
    if let Some(next) = &next {
        h.insert(
            header::LINK,
            HeaderValue::try_from(format!("<{next}>; rel=\"next\""))
                .expect("a path and a percent-encoded query make a header value"),
        );
    }
    response.extensions_mut().insert(NotActivity);
    Ok(response)
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
    fn accept_asks_the_stream_a_page_or_neither() {
        let asked = |accept: &'static str| wanted(&headers(&[("accept", accept)]));
        assert_eq!(wanted(&headers(&[])), Wanted::Stream);
        assert_eq!(asked("text/event-stream"), Wanted::Stream);
        assert_eq!(asked("text/*"), Wanted::Stream);
        assert_eq!(asked("*/*"), Wanted::Stream);
        assert_eq!(
            asked("application/json, text/event-stream;q=0.5"),
            Wanted::Stream
        );
        assert_eq!(asked("application/json"), Wanted::Page);
        assert_eq!(asked("application/json, */*;q=0.1"), Wanted::Page);
        assert_eq!(asked("application/json;q=0, */*"), Wanted::Stream);
        assert_eq!(asked("text/event-stream;q=0"), Wanted::Neither);
        assert_eq!(asked("text/html"), Wanted::Neither);
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
