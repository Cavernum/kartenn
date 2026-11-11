//! The routes of the module under `/v1` (feature `rest`; spec 019, design « Routes »),
//! mounted by kartenn-server: so far the batch of observations of a cluster
//! (`POST /v1/kube-clusters/{id}/observations`, REQ-019-16; ADR 0043, decisions 2, 3, 5;
//! Q-019-21 to 23), outside the changesets. Its right, `kube.observation.record`, is
//! evaluated on the pod domain of the cluster; its author is a service account member of
//! the connector group of the cluster; the batch is refused whole by its lines, or
//! written and audited in the same transaction.

use std::net::IpAddr;

use axum::Extension;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use kartenn_core::audit::seal::hex;
use kartenn_core::audit::{
    AUTHZ_DENIED, AuditEvent, INPUT_REJECTED, OBSERVATION_BATCH_RECORDED, Outcome,
};
use kartenn_rest::ModuleRouter;
use kartenn_rest::kit::{self, Api, Problem, ProblemKind, documented, op, schemas_of};
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authn::Principal;
use kartenn_service::authz::Authorizer;
use kartenn_service::{RequestError, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;
use utoipa::openapi::path::HttpMethod;
use uuid::Uuid;

use crate::module::OBSERVATION_RECORD_PERMISSION;
use crate::observations::{self, Batch, Closing, Invalid, Line, Opening, RecordError};

/// The journal a batch of the module writes, as its audit event names it.
const JOURNAL: &str = "kube.pod_observation";
/// The code of a batch refused, in the extension `code` of its `invalid-batch` (Q-019-21).
const INVALID: &str = "kube.observation.invalid";

// ---------------------------------------------------------------- representations

/// The identifier of an object of the cluster (006/H3).
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
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

/// 403 for the batch of `cluster` refused by the right `kube.observation.record`, the
/// refusal journaled (`authz.access.denied`).
async fn denied(
    api: &Api,
    context: &RequestContext,
    cluster: Uuid,
    detail: &str,
) -> Result<Response, Problem> {
    let event = context
        .event(&AUTHZ_DENIED, Outcome::Denied)
        .map(|e| {
            e.target("object", [cluster])
                .reason(OBSERVATION_RECORD_PERMISSION)
        })
        .and_then(|e| e.detail("permission", OBSERVATION_RECORD_PERMISSION));
    journal(api, event).await?;
    Err(Problem::new(ProblemKind::Forbidden).detail(detail))
}

/// 422 `invalid-batch` for the batch of `cluster` refused by `violations`, the refusal
/// journaled (`abuse.input.rejected`).
async fn refused(
    api: &Api,
    context: &RequestContext,
    cluster: Uuid,
    violations: Vec<Value>,
) -> Result<Response, Problem> {
    let event = context
        .event(&INPUT_REJECTED, Outcome::Failure)
        .map(|e| e.target("object", [cluster]).reason(INVALID))
        .and_then(|e| e.detail("reason", INVALID));
    journal(api, event).await?;
    Err(Problem::new(ProblemKind::InvalidBatch)
        .detail("the batch is refused whole, by its violations")
        .extension("code", json!(INVALID))
        .extension("violations", Value::Array(violations)))
}

/// The JSON pointer of a line of the body (RFC 6901).
fn pointer(line: Line) -> String {
    match line {
        Line::Opened(i) => format!("/opened/{i}"),
        Line::Closed(i) => format!("/closed/{i}"),
    }
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
    // The right on the pod domain of the cluster (Q-019-23), where the connector role is
    // bound.
    let outside = api
        .authorizer()
        .authorize_many(&principal.subject(), OBSERVATION_RECORD_PERMISSION, &[pods])
        .await?;
    if !outside.is_empty() {
        return denied(
            &api,
            &context,
            id,
            "kube.observation.record is not held on the pod domain of the cluster",
        )
        .await;
    }
    let body: BatchBody = serde_json::from_slice(&raw).map_err(|e| kit::bad(e.to_string()))?;
    let batch = batch_of(body)?;
    let lines = batch.opened.len() + batch.closed.len();
    if usize::try_from(max).is_ok_and(|max| lines > max) {
        return refused(
            &api,
            &context,
            id,
            vec![json!({
                "pointer": "",
                "detail": format!(
                    "{lines} lines, over the {max} of the cluster (batch_max_operations)"
                ),
            })],
        )
        .await;
    }
    let mut tx = api.pool.begin().await.map_err(|e| kit::store(&e))?;
    let recorded = match observations::record(&mut tx, id, principal.identity, &batch).await {
        Ok(recorded) => recorded,
        Err(RecordError::UnknownCluster(_)) => return Err(Problem::new(ProblemKind::NotFound)),
        Err(RecordError::NotConnector) => {
            drop(tx);
            return denied(
                &api,
                &context,
                id,
                "the caller is not a service account member of the connector group of the cluster",
            )
            .await;
        }
        Err(RecordError::Invalid(lines)) => {
            drop(tx);
            let violations = lines
                .iter()
                .map(|Invalid { line, reason }| {
                    json!({ "pointer": pointer(*line), "detail": reason.to_string() })
                })
                .collect();
            return refused(&api, &context, id, violations).await;
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

/// The routes of the module, mounted under `/v1` (ADR 0026, clause 9).
#[must_use]
pub fn routes() -> ModuleRouter {
    let mut schemas = Vec::new();
    schemas_of::<BatchBody>(&mut schemas);
    schemas_of::<RecordedDto>(&mut schemas);
    let record_op = op(
        "record_kube_observations",
        "kube",
        "Writes a batch of observations of the pods of the cluster, outside the changesets \
         (REQ-019-16): under `kube.observation.record` on the pod domain of the cluster, by \
         a service account member of its connector group (403 otherwise); at most \
         `batch_max_operations` lines; accepted whole, audited in its transaction \
         (`observation.batch.recorded`, an empty batch too), or refused whole with 422 \
         `invalid-batch` (`code` `kube.observation.invalid`, `violations` by JSON pointer).",
    )
    .path("id", "The cluster.", true)
    .body("application/json", "KubeObservationBatch", true)
    .answers("201", "Written.", "KubeObservationBatchRecorded")
    .build();
    ModuleRouter::new().routes(documented(
        "/kube-clusters/{id}/observations",
        vec![(HttpMethod::Post, record_op)],
        schemas,
        post(record_batch),
    ))
}
