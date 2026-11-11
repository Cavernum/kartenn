//! The routes of the module under `/v1` (feature `rest`; spec 019, design « Routes »),
//! mounted by kartenn-server: so far the batch of observations of a cluster
//! (`POST /v1/kube-clusters/{id}/observations`, REQ-019-16; ADR 0043, decisions 2, 3, 5;
//! Q-019-21 to 23), outside the changesets. Its right, `kube.observation.record`, is
//! evaluated on the pod domain of the cluster; its author is a service account member of
//! the connector group of the cluster; the batch is refused whole by its lines, or
//! written and audited in the same transaction. And the reads of that journal
//! (`GET /v1/kube-clusters/{id}/observations?ip=&at=`, `GET /v1/workloads/{id}/observations`;
//! REQ-019-16, 17; ADR 0043, decisions 6, 8; Q-019-25), under `kube.observation.read` on
//! the same pod domain, paged, each journaled.

use std::net::IpAddr;

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::header::{CONTENT_TYPE, LINK};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use kartenn_core::addr::{Addr, Prefix};
use kartenn_core::audit::seal::hex;
use kartenn_core::audit::{
    AUTHZ_DENIED, AuditEvent, INPUT_REJECTED, OBSERVATION_BATCH_RECORDED, Outcome,
};
use kartenn_core::authz::type_read_permission;
use kartenn_core::family::ALL;
use kartenn_rest::ModuleRouter;
use kartenn_rest::cursor::ListingLinks;
use kartenn_rest::kit::{
    self, Api, BatchViolation, ETag, Moment, Page, PageParams, Problem, ProblemKind, documented,
    next_href, op, respond, schemas_of,
};
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::authz::Authorizer;
use kartenn_service::{RequestError, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::openapi::path::HttpMethod;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::module::{
    CLUSTER, OBSERVATION_READ_PERMISSION, OBSERVATION_RECORD_PERMISSION, WORKLOAD,
};
use crate::observations::{self, Batch, Closing, Invalid, Line, Opening, RecordError};
use crate::read::{self, Cut, Key, Observation};

/// The journal a batch of the module writes, as its audit event names it.
const JOURNAL: &str = "kube.pod_observation";
/// The code of a batch refused, in the extension `code` of its `invalid-batch` (Q-019-21).
const INVALID: &str = "kube.observation.invalid";
/// The `reason` of `abuse.input.rejected` for a body that does not read: the class of
/// its 400.
const BAD_REQUEST: &str = "bad-request";

// ---------------------------------------------------------------- representations

/// The identifier of an object of the cluster (006/H3).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = KubeObjectRef)]
struct IdRef {
    id: Uuid,
}

/// A pod seen, which opens an observation.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = KubeObservationOpening)]
struct OpeningBody {
    /// Its namespace, by name.
    namespace: String,
    /// Its name.
    pod_name: String,
    /// Its Kubernetes UID.
    pod_uid: Uuid,
    /// The workload of the cluster its `ownerReferences` climb to.
    #[serde(default)]
    workload: Option<IdRef>,
    /// The node of the cluster it runs on.
    #[serde(default)]
    node: Option<IdRef>,
    /// One of its IPs, in a pod container or a pod range of the cluster or an address of
    /// one of its nodes; none in `hostNetwork`.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    ip: Option<IpAddr>,
    /// Whether it shares the network of its node.
    #[serde(default)]
    host_network: bool,
    /// Since when it is seen (RFC 3339, with an offset); an instant to come is the
    /// instant of the batch.
    from: String,
}

/// The end of the period of an open observation of the cluster, by pod and IP.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = KubeObservationClosing)]
struct ClosingBody {
    /// The UID of the pod.
    pod_uid: Uuid,
    /// Its IP; none in `hostNetwork`.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    ip: Option<IpAddr>,
    /// When it was gone (RFC 3339, with an offset); an instant to come is the instant of
    /// the batch.
    at: String,
}

/// A batch of the connector of a cluster: at most `batch_max_operations` lines of the
/// cluster in all, accepted or refused whole.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = KubeObservationBatch)]
struct BatchBody {
    /// The observations it opens.
    opened: Vec<OpeningBody>,
    /// The observations it closes.
    closed: Vec<ClosingBody>,
}

/// A batch written.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeObservationBatchRecorded)]
struct RecordedDto {
    /// Its identifier.
    id: Uuid,
    /// When it was received: the instant of its transaction.
    received_at: String,
    /// The observations it opened; an opening of an observation open with the same values
    /// writes nothing and is not counted.
    opened: u32,
    /// The observations it closed.
    closed: u32,
    /// SHA-256 of its canonical content (RFC 8785), in lowercase hexadecimal, as its
    /// audit event `observation.batch.recorded` keeps it.
    hash: String,
}

/// A pod observed, over its period: a row of the journal.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeObservation)]
struct ObservationDto {
    /// Its namespace, by name.
    namespace: String,
    /// Its name.
    pod_name: String,
    /// Its Kubernetes UID.
    pod_uid: Uuid,
    /// The workload its `ownerReferences` climb to.
    workload: Option<IdRef>,
    /// The node it ran on.
    node: Option<IdRef>,
    /// Its IP; `null` in `hostNetwork`, where the address is the node's.
    #[schema(value_type = Option<String>)]
    ip: Option<IpAddr>,
    /// Whether it shared the network of its node.
    host_network: bool,
    /// Since when it was seen (RFC 3339).
    from: String,
    /// When it was gone (RFC 3339); `null` while it is still seen.
    until: Option<String>,
    /// The batch that opened the observation, as its event `observation.batch.recorded`
    /// names it.
    opened_batch: Uuid,
    /// The batch that closed it.
    closed_batch: Option<Uuid>,
}

/// Why a page of the journal is empty though rows may still be stored.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = KubeObservationReason)]
enum ReasonDto {
    /// The instant asked about is older than the retention of the cluster.
    Retention,
}

/// A page of the journal.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Listing_KubeObservation)]
struct ListingDto<'a> {
    /// The observations of the page, by start, pod and IP.
    items: Vec<ObservationDto>,
    /// The links of the page.
    links: ListingLinks<'a>,
    /// `retention` when the instant asked about is older than the retention of the
    /// cluster, the page then empty (Q-019-25); `null` otherwise.
    reason: Option<ReasonDto>,
}

/// The parameters of the question « who had this address at this instant ».
#[derive(Debug, IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)]
struct HeldParams {
    /// An address, or a network `address/length` without host bits (RFC 4632 §3.1): the
    /// pods that had one of its addresses, and those in `hostNetwork` on a node that had
    /// one.
    ip: String,
    /// The instant, RFC 3339 with its offset.
    at: String,
}

// ---------------------------------------------------------------- handler

/// An instant of the body, or 400.
fn instant(pointer: &str, text: &str) -> Result<OffsetDateTime, Problem> {
    OffsetDateTime::parse(text, &Rfc3339)
        .map_err(|_| kit::bad(format!("{pointer}: an instant RFC 3339 with an offset")))
}

/// The batch of the body, or 400.
fn batch_of(body: BatchBody) -> Result<Batch, Problem> {
    let opened = body
        .opened
        .into_iter()
        .enumerate()
        .map(|(i, o)| {
            Ok(Opening {
                namespace: o.namespace,
                pod_name: o.pod_name,
                pod_uid: o.pod_uid,
                workload: o.workload.map(|r| r.id),
                node: o.node.map(|r| r.id),
                ip: o.ip,
                host_network: o.host_network,
                from: instant(&format!("/opened/{i}/from"), &o.from)?,
            })
        })
        .collect::<Result<Vec<_>, Problem>>()?;
    let closed = body
        .closed
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            Ok(Closing {
                pod_uid: c.pod_uid,
                ip: c.ip,
                at: instant(&format!("/closed/{i}/at"), &c.at)?,
            })
        })
        .collect::<Result<Vec<_>, Problem>>()?;
    Ok(Batch { opened, closed })
}

/// An audit event that could not be built or written: 503, nothing answered.
fn audited(e: impl std::fmt::Display) -> Problem {
    Problem::from(RequestError::Audit(StoreError(e.to_string())))
}

/// Journals `event` outside any transaction, the refusals.
async fn journal(
    api: &Api,
    event: Result<AuditEvent, impl std::fmt::Display>,
) -> Result<(), Problem> {
    let event = event.map_err(audited)?;
    api.journal.record(&event).await.map_err(audited)
}

/// 403 for the object `target` (a cluster or a workload) refused by `permission`, the
/// refusal journaled (`authz.access.denied`).
async fn denied(
    api: &Api,
    context: &RequestContext,
    target: Uuid,
    permission: &str,
    detail: &str,
) -> Problem {
    let event = context
        .event(&AUTHZ_DENIED, Outcome::Denied)
        .map(|e| e.target("object", [target]).reason(permission))
        .and_then(|e| e.detail("permission", permission));
    match journal(api, event).await {
        Ok(()) => Problem::new(ProblemKind::Forbidden).detail(detail),
        Err(unavailable) => unavailable,
    }
}

/// `problem` for the body of a batch of `cluster` refused at the entry, the refusal
/// journaled (`abuse.input.rejected`, its `reason`): 400 for a body that does not read,
/// 422 `invalid-batch` for lines that refuse it.
async fn rejected(
    api: &Api,
    context: &RequestContext,
    cluster: Uuid,
    reason: &str,
    problem: Problem,
) -> Result<Response, Problem> {
    let event = context
        .event(&INPUT_REJECTED, Outcome::Failure)
        .map(|e| e.target("object", [cluster]).reason(reason))
        .and_then(|e| e.detail("reason", reason));
    journal(api, event).await?;
    Err(problem)
}

/// 422 `invalid-batch` for the batch of `cluster` refused by `violations`, journaled.
async fn refused(
    api: &Api,
    context: &RequestContext,
    cluster: Uuid,
    violations: &[BatchViolation],
) -> Result<Response, Problem> {
    let problem = Problem::invalid_batch(INVALID, violations);
    rejected(api, context, cluster, INVALID, problem).await
}

/// The JSON pointer of a line of the body (RFC 6901).
fn pointer(line: Line) -> String {
    match line {
        Line::Opened(i) => format!("/opened/{i}"),
        Line::Closed(i) => format!("/closed/{i}"),
    }
}

/// Once the caller holds `permission` on the pod domain `pods` of the cluster of `id`, an
/// object of the type `ty` (the cluster itself, or one of its workloads). The object is
/// seen by whoever reads it (`<ty>.read`, ADR 0021) or holds `permission` on that pod
/// domain (Q-019-23), where the connector role is bound, the cluster itself being anchored
/// in no domain: to anyone else it answers as an absent one, 404 unjournaled; seen
/// without the right, 403 journaled.
async fn allowed(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    (ty, id): (&str, Uuid),
    pods: Uuid,
    permission: &str,
) -> Result<(), Problem> {
    let subject = principal.subject();
    let authorizer = api.authorizer();
    let held = authorizer
        .authorize_many(&subject, permission, &[pods])
        .await?
        .is_empty();
    if held {
        return Ok(());
    }
    let readable = authorizer
        .authorize_many(&subject, &type_read_permission(ty), &[id])
        .await?
        .is_empty();
    if !readable {
        return Err(Problem::new(ProblemKind::NotFound));
    }
    let detail = format!("{permission} is not held on the pod domain of the cluster");
    Err(denied(api, context, id, permission, &detail).await)
}

/// The `batch_max_operations` of the living cluster `id`, once the caller may write a
/// batch there (see [`allowed`]).
async fn admitted(
    api: &Api,
    principal: &Principal,
    context: &RequestContext,
    id: Uuid,
) -> Result<i32, Problem> {
    let cluster: Option<(Uuid, i32)> = sqlx::query_as(
        "SELECT pod_domain_id, batch_max_operations FROM kube.cluster
          WHERE id = $1 AND valid_period @> now()",
    )
    .bind(id)
    .fetch_optional(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    let Some((pods, max)) = cluster else {
        return Err(Problem::new(ProblemKind::NotFound));
    };
    let target = (CLUSTER, id);
    allowed(
        api,
        principal,
        context,
        target,
        pods,
        OBSERVATION_RECORD_PERMISSION,
    )
    .await?;
    Ok(max)
}

/// `POST /v1/kube-clusters/{id}/observations`: the batch of the connector of the cluster,
/// written with its audit event in one transaction, or refused whole without writing
/// anything (REQ-019-16; ADR 0043, decisions 2, 3, 5).
async fn record_batch(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    raw: Bytes,
) -> Result<Response, Problem> {
    let max = admitted(&api, &principal, &context, id).await?;
    let batch = match serde_json::from_slice(&raw)
        .map_err(|e| kit::bad(e.to_string()))
        .and_then(batch_of)
    {
        Ok(batch) => batch,
        Err(problem) => return rejected(&api, &context, id, BAD_REQUEST, problem).await,
    };
    let lines = batch.opened.len() + batch.closed.len();
    if usize::try_from(max).is_ok_and(|max| lines > max) {
        let whole = BatchViolation {
            pointer: String::new(),
            detail: format!("{lines} lines, over the {max} of the cluster (batch_max_operations)"),
        };
        return refused(&api, &context, id, &[whole]).await;
    }
    let mut tx = api.pool.begin().await.map_err(|e| kit::store(&e))?;
    let recorded = match observations::record(&mut tx, id, principal.identity, &batch).await {
        Ok(recorded) => recorded,
        Err(RecordError::UnknownCluster(_)) => return Err(Problem::new(ProblemKind::NotFound)),
        Err(RecordError::NotConnector) => {
            drop(tx);
            return Err(denied(
                &api,
                &context,
                id,
                OBSERVATION_RECORD_PERMISSION,
                "the caller is not a service account member of the connector group of the cluster",
            )
            .await);
        }
        Err(RecordError::Invalid(lines)) => {
            drop(tx);
            let violations: Vec<BatchViolation> = lines
                .iter()
                .map(|Invalid { line, reason }| BatchViolation {
                    pointer: pointer(*line),
                    detail: reason.to_string(),
                })
                .collect();
            return refused(&api, &context, id, &violations).await;
        }
        Err(RecordError::Store(e)) => return Err(kit::store(&e)),
        // A canonical form that failed: a fault of the server, never of the batch.
        Err(RecordError::Canonical(_)) => return Err(Problem::new(ProblemKind::Internal)),
    };
    let hash = hex(&recorded.hash);
    let event = context
        .event(&OBSERVATION_BATCH_RECORDED, Outcome::Success)
        .map(|e| e.target("object", [id]))
        .and_then(|e| e.detail("journal", JOURNAL))
        .and_then(|e| e.detail("cluster", id.to_string()))
        .and_then(|e| e.detail("batch", recorded.id.to_string()))
        .and_then(|e| e.detail("opened", recorded.opened))
        .and_then(|e| e.detail("closed", recorded.closed))
        .and_then(|e| e.detail("hash", hash.as_str()))
        .and_then(|e| e.detail("clamped", recorded.clamped))
        .and_then(|e| e.detail("skew_max", recorded.skew_max_ms))
        .map_err(audited)?;
    // In the transaction of the batch (ADR 0043, decision 5): no batch without its event.
    api.journal
        .record_in(&mut tx, &event)
        .await
        .map_err(audited)?;
    tx.commit().await.map_err(|e| kit::store(&e))?;
    let received_at = recorded
        .received_at
        .format(&Rfc3339)
        .map_err(|_| Problem::new(ProblemKind::Internal))?;
    let answer = RecordedDto {
        id: recorded.id,
        received_at,
        opened: recorded.opened,
        closed: recorded.closed,
        hash,
    };
    Ok((StatusCode::CREATED, axum::Json(answer)).into_response())
}

// ---------------------------------------------------------------- reads

/// 400 for a date of the model on the first page of a read of the journal, which has no
/// dated read (REQ-019-16): its cursors freeze only the instant of the first page.
fn undated(page: &Page) -> Result<(), Problem> {
    if page.after().is_none() && (page.dates.valid_at.is_some() || page.dates.as_of.is_some()) {
        return Err(kit::bad(
            "the journal of the pods observed has no dated read: no valid_at, no as_of",
        ));
    }
    Ok(())
}

/// The network of `ip=`: an address is its full-length prefix; a network is strict, its
/// host bits refused (REQ-002-4, 5).
fn network(text: &str) -> Result<Prefix, Problem> {
    let parsed = if text.contains('/') {
        Prefix::parse_any(&ALL, text)
    } else {
        Addr::parse_any(&ALL, text).map(Prefix::host)
    };
    parsed.map_err(|e| kit::bad(format!("ip: {e}")))
}

/// The address or network and the instant of the question, its only filters, required.
fn question(page: &Page) -> Result<(Prefix, OffsetDateTime), Problem> {
    let (mut ip, mut at) = (None, None);
    for (name, value) in &page.filters {
        let slot = match name.as_str() {
            "ip" => &mut ip,
            "at" => &mut at,
            _ => return Err(kit::bad(format!("unknown parameter {name}"))),
        };
        if slot.is_some() {
            return Err(kit::bad(format!("{name} is given more than once")));
        }
        *slot = Some(value.as_str());
    }
    let ip = ip.ok_or_else(|| kit::bad("ip is required: an address or a network"))?;
    let at = at.ok_or_else(|| kit::bad("at is required: an instant RFC 3339 with an offset"))?;
    Ok((network(ip)?, instant("at", at)?))
}

/// The key after which the page starts, from its cursor: `[from, pod_uid, ip]`.
fn after_key(page: &Page) -> Result<Option<Key>, Problem> {
    let Some(after) = page.after() else {
        return Ok(None);
    };
    let invalid = || Problem::new(ProblemKind::InvalidCursor);
    let (from, pod_uid, ip): (String, Uuid, String) =
        serde_json::from_value(after.clone()).map_err(|_| invalid())?;
    let from = OffsetDateTime::parse(&from, &Rfc3339).map_err(|_| invalid())?;
    Ok(Some(Key { from, pod_uid, ip }))
}

fn rfc3339(at: OffsetDateTime) -> Result<String, Problem> {
    at.format(&Rfc3339)
        .map_err(|_| Problem::new(ProblemKind::Internal))
}

fn observation_dto(o: Observation) -> Result<ObservationDto, Problem> {
    Ok(ObservationDto {
        namespace: o.namespace,
        pod_name: o.pod_name,
        pod_uid: o.pod_uid,
        workload: o.workload.map(|id| IdRef { id }),
        node: o.node.map(|id| IdRef { id }),
        ip: o.ip,
        host_network: o.host_network,
        from: rfc3339(o.from)?,
        until: o.until.map(rfc3339).transpose()?,
        opened_batch: o.opened_batch,
        closed_batch: o.closed_batch,
    })
}

/// A read of the journal: the page asked for in `collection`, read at `moment`.
struct Asked<'a> {
    page: &'a Page,
    collection: &'a str,
    moment: &'a Moment,
}

/// The page of `rows` (one more than its size when another page follows), journaled as
/// read (`data.objects.read`, its pods by UID; REQ-012-18, Q-019-25), then answered with
/// its `reason`, the next page in `Link` and in the body, a weak `ETag` of the content.
async fn answered(
    api: &Api,
    context: &RequestContext,
    headers: &HeaderMap,
    asked: &Asked<'_>,
    mut rows: Vec<Observation>,
    reason: Option<Cut>,
) -> Result<Response, Problem> {
    let Asked {
        page,
        collection,
        moment,
    } = *asked;
    let size = usize::from(page.size);
    let mut next = None;
    if rows.len() > size {
        rows.truncate(size);
        if let Some(last) = rows.last() {
            let key = last.key();
            let after = json!([rfc3339(key.from)?, key.pod_uid, key.ip]);
            let cursor = page.next_current(&api.ring, collection, moment, after);
            next = Some(next_href(&format!("/v1/{collection}"), &cursor));
        }
    }
    let mut pods: Vec<Uuid> = rows.iter().map(|o| o.pod_uid).collect();
    pods.sort_unstable();
    pods.dedup();
    let criteria = json!({
        "collection": collection,
        "filters": page.filters,
        "valid_at": rfc3339(moment.valid_at)?,
    });
    kit::record_read(api, context, JOURNAL, &pods, criteria).await?;
    let body = ListingDto {
        items: rows
            .into_iter()
            .map(observation_dto)
            .collect::<Result<_, _>>()?,
        links: ListingLinks {
            next: next.as_deref(),
        },
        reason: reason.map(|Cut::Retention| ReasonDto::Retention),
    };
    let bytes = serde_json::to_vec(&body).map_err(|_| Problem::new(ProblemKind::Internal))?;
    let etag = ETag::page(&bytes);
    let mut response = ([(CONTENT_TYPE, "application/json")], bytes).into_response();
    if let Some(next) = &next {
        let link = HeaderValue::try_from(format!("<{next}>; rel=\"next\""))
            .map_err(|_| Problem::new(ProblemKind::Internal))?;
        response.headers_mut().insert(LINK, link);
    }
    Ok(respond(headers, moment, &etag, response))
}

/// `GET /v1/kube-clusters/{id}/observations?ip=&at=`: who had an address of `ip` at `at`
/// in the cluster (REQ-019-16, 17; Q-019-25), under `kube.observation.read` on its pod
/// domain (see [`allowed`]); past its retention, an empty page with the reason
/// `retention`, whether or not the purge dropped the rows yet.
async fn cluster_observations(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let cluster: Option<(Uuid, i16)> = sqlx::query_as(
        "SELECT pod_domain_id, retention_days FROM kube.cluster
          WHERE id = $1 AND valid_period @> now()",
    )
    .bind(id)
    .fetch_optional(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    let Some((pods, retention)) = cluster else {
        return Err(Problem::new(ProblemKind::NotFound));
    };
    let target = (CLUSTER, id);
    allowed(
        &api,
        &principal,
        &context,
        target,
        pods,
        OBSERVATION_READ_PERMISSION,
    )
    .await?;
    let collection = format!("kube-clusters/{id}/observations");
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &collection)?;
    undated(&page)?;
    let (network, at) = question(&page)?;
    let after = after_key(&page)?;
    let moment = page.dates.at(kit::now(&api).await?, api.bound)?;
    let (rows, reason) = if read::within_retention(at, moment.valid_at, retention) {
        let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
        let limit = Some(page.size + 1);
        let rows = read::held(&mut conn, &[id], network, at, after.as_ref(), limit)
            .await
            .map_err(|e| kit::store(&e))?;
        (rows, None)
    } else {
        (Vec::new(), Some(Cut::Retention))
    };
    let asked = Asked {
        page: &page,
        collection: &collection,
        moment: &moment,
    };
    answered(&api, &context, &headers, &asked, rows, reason).await
}

/// `GET /v1/workloads/{id}/observations`: where the workload ran, its pods with their
/// nodes and addresses still within the retention of its cluster (REQ-019-17), under
/// `kube.observation.read` on the pod domain of its cluster (see [`allowed`]).
async fn workload_observations(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let workload: Option<(Uuid, i16)> = sqlx::query_as(
        "SELECT c.pod_domain_id, c.retention_days
           FROM kube.workload w
           JOIN kube.namespace n ON n.id = w.namespace_id AND n.valid_period @> now()
           JOIN kube.cluster c ON c.id = n.cluster_id AND c.valid_period @> now()
          WHERE w.id = $1 AND w.valid_period @> now()",
    )
    .bind(id)
    .fetch_optional(&api.pool)
    .await
    .map_err(|e| kit::store(&e))?;
    let Some((pods, retention)) = workload else {
        return Err(Problem::new(ProblemKind::NotFound));
    };
    let target = (WORKLOAD, id);
    allowed(
        &api,
        &principal,
        &context,
        target,
        pods,
        OBSERVATION_READ_PERMISSION,
    )
    .await?;
    let collection = format!("workloads/{id}/observations");
    let page = Page::from_query(query.as_deref().unwrap_or_default(), &api.ring, &collection)?;
    undated(&page)?;
    page.unfiltered()?;
    let after = after_key(&page)?;
    let moment = page.dates.at(kit::now(&api).await?, api.bound)?;
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let limit = Some(page.size + 1);
    let rows = read::of_workload(
        &mut conn,
        id,
        moment.valid_at,
        retention,
        after.as_ref(),
        limit,
    )
    .await
    .map_err(|e| kit::store(&e))?;
    drop(conn);
    let asked = Asked {
        page: &page,
        collection: &collection,
        moment: &moment,
    };
    answered(&api, &context, &headers, &asked, rows, None).await
}

/// The routes of the module, mounted under `/v1` (ADR 0026, clause 9).
#[must_use]
pub fn routes() -> ModuleRouter {
    let mut schemas = Vec::new();
    schemas_of::<BatchBody>(&mut schemas);
    schemas_of::<RecordedDto>(&mut schemas);
    schemas_of::<ListingDto<'static>>(&mut schemas);
    let held_op = op(
        "kube_cluster_observations",
        "kube",
        "Who had an address, or an address of a network, at an instant in the cluster \
         (REQ-019-16, 17): the pods observed with it, their workload and node, and the pods \
         in `hostNetwork` on a node that had it; under `kube.observation.read` on the pod \
         domain of the cluster (403 otherwise; 404, as an absent one, for a cluster the \
         caller neither reads nor holds that right on). The journal has no dated read. Past \
         the retention of the cluster, an empty page with the reason `retention`. Each read \
         is journaled (`data.objects.read`).",
    )
    .path("id", "The cluster.", true)
    .params::<HeldParams>()
    .params::<PageParams>()
    .answers(
        "200",
        "A page of observations, by start, pod and IP.",
        "Listing_KubeObservation",
    )
    .build();
    let workload_op = op(
        "kube_workload_observations",
        "kube",
        "Where the workload ran (REQ-019-17): its pods observed with their nodes and \
         addresses, still within the retention of its cluster; under `kube.observation.read` \
         on the pod domain of its cluster (403 otherwise; 404, as an absent one, for a \
         workload the caller neither reads nor holds that right on). The journal has no \
         dated read. Each read is journaled (`data.objects.read`).",
    )
    .path("id", "The workload.", true)
    .params::<PageParams>()
    .answers(
        "200",
        "A page of observations, by start, pod and IP.",
        "Listing_KubeObservation",
    )
    .build();
    let record_op = op(
        "record_kube_observations",
        "kube",
        "Writes a batch of observations of the pods of the cluster, outside the changesets \
         (REQ-019-16): under `kube.observation.record` on the pod domain of the cluster, by \
         a service account member of its connector group (403 otherwise; 404, as an absent \
         one, for a cluster the caller neither reads nor holds that right on); at most \
         `batch_max_operations` lines; an instant to come is brought back to the instant of \
         the batch, counted in its event; accepted whole, audited in its transaction \
         (`observation.batch.recorded`, an empty batch too), or refused whole with 422 \
         `invalid-batch` (`code` `kube.observation.invalid`, `violations` by JSON pointer).",
    )
    .path("id", "The cluster.", true)
    .body("application/json", "KubeObservationBatch", true)
    .answers("201", "Written.", "KubeObservationBatchRecorded")
    .refuses_batches("Refused whole: the lines that refuse it, by JSON pointer.")
    .build();
    ModuleRouter::new()
        .routes(documented(
            "/kube-clusters/{id}/observations",
            vec![(HttpMethod::Post, record_op), (HttpMethod::Get, held_op)],
            schemas,
            post(record_batch).get(cluster_observations),
        ))
        .routes(documented(
            "/workloads/{id}/observations",
            vec![(HttpMethod::Get, workload_op)],
            Vec::new(),
            get(workload_observations),
        ))
}
