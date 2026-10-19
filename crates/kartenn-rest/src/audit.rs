//! Audit and operations (spec 006, REQ-006-4; task 006-19; design « Audit et
//! exploitation »): the audit journal by pages or exported as NDJSON, under
//! `core.audit.read` (012/REQ-012-17); the pseudonymisation of a person at once (004/
//! REQ-004-33), under `core.identity.pseudonymise`; the filter of a technical output
//! changed at run time (012/REQ-012-27), under `core.observability.admin`.

use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use futures_util::stream;
use kartenn_core::audit::rfc3339;
use kartenn_service::RequestError;
use kartenn_service::audit::RequestContext;
use kartenn_service::observability::set_log_filter;
use kartenn_service::port::{AuditQuery, Reader};
use kartenn_service::reads::{export_audit, pseudonymisable, read_audit};
use kartenn_store_pg::audit::{Pseudonymisation, pseudonymise_identity};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::rbac_read;
use kartenn_store_pg::read;
use kartenn_store_pg::write::PgStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::anomalies::{Motive, motive};
use crate::cursor::{Listing, Page, PageParams, next_href, respond_page};
use crate::dated::Dates;
use crate::filter::{self, ListQuery};
use crate::objects::{Api, bad, instant, store};
use crate::openapi::queryable;
use crate::problem::{Problem, ProblemKind};
use crate::rights::{IdentityItem, identity_item};

/// The media type of an export: one JSON event per line.
const NDJSON: &str = "application/x-ndjson";

/// Events read at a time by an export.
const EXPORT_BATCH: u32 = 1_000;

/// The collection of the journal.
const EVENTS: &str = "audit-events";

/// The routes of the audit and of the operations.
pub fn routes() -> OpenApiRouter<Api> {
    // `GET` and `QUERY` by the same handler: the route of `filter::collection`, the
    // contract of the annotation.
    let (schemas, paths, _) = routes!(events);
    let router = OpenApiRouter::new()
        .routes((schemas, paths, filter::collection(events)))
        .routes(routes!(pseudonymise))
        .routes(routes!(log_filter));
    queryable(router, "/audit-events", "query_audit_events")
}

/// A store that reads the journal and records as the server's instance.
fn pg_store(api: &Api) -> PgStore {
    PgStore::new(api.pool.clone()).with_instance(api.journal.instance().clone())
}

/// An event of the audit journal, as journaled (spec 012, REQ-012-17).
#[derive(Debug, Serialize, ToSchema)]
#[serde(transparent)]
#[schema(value_type = Object)]
struct AuditEvent(Value);

/// The criteria of a consultation, as the contract names them.
#[derive(Debug, IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)]
struct EventsParams {
    /// The first instant, RFC 3339 with an offset.
    from: Option<String>,
    /// The last instant, RFC 3339 with an offset.
    to: Option<String>,
    /// The name of the event.
    name: Option<String>,
    /// The actor.
    actor: Option<Uuid>,
    /// An object the event names.
    object: Option<Uuid>,
}

/// The criteria of a consultation: `from` and `to` (RFC 3339 with an offset), `name`,
/// `actor` and `object` (UUIDs), each at most once.
fn criteria(filters: &[(String, String)]) -> Result<AuditQuery, Problem> {
    fn once<T>(slot: &mut Option<T>, name: &str, value: Result<T, Problem>) -> Result<(), Problem> {
        if slot.is_some() {
            return Err(bad(format!("{name} is given more than once")));
        }
        *slot = Some(value?);
        Ok(())
    }
    let uuid = |name: &str, value: &str| {
        value
            .parse::<Uuid>()
            .map_err(|_| bad(format!("{name} must be a UUID")))
    };
    let date = |name: &str, value: &str| {
        OffsetDateTime::parse(value, &Rfc3339)
            .map(instant)
            .map_err(|_| bad(format!("{name} must be an RFC 3339 date with an offset")))
    };
    let mut query = AuditQuery::default();
    for (name, value) in filters {
        match name.as_str() {
            "from" => once(&mut query.from, name, date(name, value))?,
            "to" => once(&mut query.to, name, date(name, value))?,
            "name" => once(&mut query.name, name, Ok(value.clone()))?,
            "actor" => once(&mut query.actor, name, uuid(name, value))?,
            "object" => once(&mut query.object, name, uuid(name, value))?,
            other => return Err(bad(format!("unknown parameter {other}"))),
        }
    }
    Ok(query)
}

/// Whether the request asks for the export.
fn exported(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|v| v.split(';').next())
        .any(|v| v.trim().eq_ignore_ascii_case(NDJSON))
}

/// `GET`/`QUERY /v1/audit-events?from=…&to=…&name=…&actor=…&object=…`: the events, the
/// most recent first, by pages keyed by their sequence number; under `Accept:
/// application/x-ndjson`, the whole selection exported.
#[utoipa::path(
    get,
    path = "/audit-events",
    operation_id = "list_audit_events",
    tag = "audit",
    params(EventsParams, PageParams),
    responses((
        status = 200,
        description = "A page of events, or under `Accept: application/x-ndjson` the whole \
                       selection, one event per line.",
        content(
            (Listing<AuditEvent> = "application/json"),
            (AuditEvent = "application/x-ndjson"),
        )
    ))
)]
async fn events(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    ListQuery(query): ListQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    if exported(&headers) {
        return export(&api, &context, &query).await;
    }
    let page = Page::from_query(&query, &api.ring, EVENTS)?;
    if page.after().is_none() && page.dates != Dates::default() {
        return Err(bad("the audit events take neither valid_at nor as_of"));
    }
    let before = page
        .after()
        .map(|key| {
            serde_json::from_value::<i64>(key.clone())
                .map_err(|_| Problem::new(ProblemKind::InvalidCursor))
        })
        .transpose()?;
    let wanted = AuditQuery {
        before,
        limit: u32::from(page.size),
        ..criteria(&page.filters)?
    };
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = Dates::default().at(now, api.bound)?;
    let (found, more) = read_audit(
        &pg_store(&api),
        &PgAuthorizer::new(api.pool.clone()),
        &api.journal,
        &context,
        &wanted,
    )
    .await?;
    let next = match found.last().and_then(|e| e.get("seq")) {
        Some(seq) if more => {
            let cursor = page.next_current(&api.ring, EVENTS, &moment, seq.clone());
            Some(next_href(&format!("/v1/{EVENTS}"), &cursor))
        }
        _ => None,
    };
    let items: Vec<AuditEvent> = found.into_iter().map(AuditEvent).collect();
    Ok(respond_page(&headers, &moment, &items, next.as_deref()))
}

/// The export of the selection of `query`, audited once before the first event is read,
/// then read by batches into the body. A failure past the first byte cuts the body: the
/// client sees an incomplete transfer, not a partial export taken as whole.
async fn export(api: &Api, context: &RequestContext, query: &str) -> Result<Response, Problem> {
    let mut params = Vec::new();
    for (name, value) in form_urlencoded::parse(query.as_bytes()) {
        match name.as_ref() {
            "cursor" | "page_size" => {
                return Err(bad(format!("an export takes no {name}: it is whole")));
            }
            "valid_at" | "as_of" => {
                return Err(bad("the audit events take neither valid_at nor as_of"));
            }
            _ => params.push((name.into_owned(), value.into_owned())),
        }
    }
    let wanted = criteria(&params)?;
    let reader = pg_store(api);
    let bounded = export_audit(
        &reader,
        &PgAuthorizer::new(api.pool.clone()),
        &api.journal,
        context,
        &wanted,
    )
    .await?;
    let batches = stream::try_unfold(Some(bounded), move |next| {
        let reader = reader.clone();
        async move {
            let Some(query) = next else {
                return Ok(None);
            };
            let found = reader
                .audit_events(&AuditQuery {
                    limit: EXPORT_BATCH,
                    ..query.clone()
                })
                .await
                .inspect_err(|e| tracing::warn!(error = %e, "audit export cut short"))?;
            if found.is_empty() {
                return Ok(None);
            }
            let mut bytes = Vec::new();
            for event in &found {
                serde_json::to_writer(&mut bytes, event).expect("JSON values serialise");
                bytes.push(b'\n');
            }
            let last = found
                .last()
                .and_then(|e| e.get("seq"))
                .and_then(Value::as_i64);
            let after = (found.len() == EXPORT_BATCH as usize)
                .then_some(last)
                .flatten()
                .map(|seq| AuditQuery {
                    before: Some(seq),
                    ..query
                });
            Ok::<_, kartenn_service::StoreError>(Some((Bytes::from(bytes), after)))
        }
    });
    Ok((
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(NDJSON)),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        Body::from_stream(batches),
    )
        .into_response())
}

/// `POST /v1/identities/{id}/pseudonymisation` `{motive}`: pseudonymises the person at once;
/// 200 with the identity. Without `core.identity.pseudonymise` → 403 (before any lookup);
/// no motive → 422; unknown → 404; not a person → 409; pseudonymised before → 200, nothing
/// done.
#[utoipa::path(
    post,
    path = "/identities/{id}/pseudonymisation",
    operation_id = "pseudonymise_identity",
    tag = "audit",
    params(("id" = Uuid, Path, description = "The person.")),
    request_body(content = Motive, content_type = "application/json"),
    responses((status = 200, description = "The identity, pseudonymised.", body = IdentityItem))
)]
async fn pseudonymise(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Problem> {
    pseudonymisable(&PgAuthorizer::new(api.pool.clone()), &api.journal, &context).await?;
    let motive = motive(&headers, &body, "a pseudonymisation needs a motive")?;
    match pseudonymise_identity(&api.pool, id, &context, &motive, api.journal.instance())
        .await
        .map_err(RequestError::Audit)?
    {
        Pseudonymisation::Unknown => return Err(Problem::new(ProblemKind::NotFound)),
        Pseudonymisation::NotAPerson => {
            return Err(
                Problem::new(ProblemKind::Conflict).detail("only a person is pseudonymised")
            );
        }
        Pseudonymisation::Already | Pseudonymisation::Done => {}
    }
    let found = rbac_read::identity(&api.pool, id)
        .await?
        .ok_or_else(|| Problem::new(ProblemKind::NotFound))?;
    Ok(Json(identity_item(&found)).into_response())
}

/// The body of `PUT /v1/log-filters/{output}`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct LogFilter {
    /// The filter, in the syntax of the output.
    filter: String,
    /// Seconds, 1 to 86 400; 3 600 by default. A value that is no positive integer is an
    /// invalid duration.
    #[schema(value_type = Option<u64>)]
    ttl: Option<Value>,
}

/// The filter of an output, set.
#[derive(Debug, Serialize, ToSchema)]
struct LogFilterSet {
    /// The output.
    output: String,
    /// Its filter.
    filter: String,
    /// The return to the configuration, RFC 3339.
    expires_at: String,
}

/// `PUT /v1/log-filters/{output}` `{filter, ttl}`: the filter of a configured output for
/// `ttl` seconds (1 hour by default, 24 at most), after which every instance returns to the
/// configuration; 200 `{output, filter, expires_at}`. An output not configured → 404; a
/// filter that does not parse or a duration out of bounds → 422 `invalid-override`.
#[utoipa::path(
    put,
    path = "/log-filters/{output}",
    operation_id = "set_log_filter",
    tag = "operations",
    params(("output" = String, Path, description = "A configured technical output.")),
    request_body = LogFilter,
    responses((status = 200, description = "The filter, set.", body = LogFilterSet))
)]
async fn log_filter(
    State(api): State<Api>,
    Extension(context): Extension<RequestContext>,
    Path(output): Path<String>,
    body: Bytes,
) -> Result<Response, Problem> {
    let wanted: LogFilter =
        serde_json::from_slice(&body).map_err(|e| bad(format!("the body: {e}")))?;
    if !api.outputs.contains(&output) {
        return Err(Problem::new(ProblemKind::NotFound).detail("no such output"));
    }
    let ttl = match &wanted.ttl {
        None => None,
        Some(ttl) => Some(Duration::from_secs(
            ttl.as_u64().ok_or(RequestError::InvalidOverride)?,
        )),
    };
    let set = set_log_filter(
        &pg_store(&api),
        &PgAuthorizer::new(api.pool.clone()),
        &api.journal,
        &context,
        &output,
        &wanted.filter,
        ttl,
        kartenn_telemetry::capture::valid_filter,
    )
    .await?;
    Ok(Json(LogFilterSet {
        output: set.output,
        filter: set.filter,
        expires_at: rfc3339(set.expires_at),
    })
    .into_response())
}
