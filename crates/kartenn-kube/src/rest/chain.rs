//! The chain of execution of a workload (task 019-8c; REQ-019-11, 17; design « Port de
//! lecture », « Routes »; Q-019-31): `GET /v1/workloads/{id}/chain?valid_at=`, the nodes
//! the journal saw it running on at the date, the machine of each node and what hosts
//! that machine, as [`read::execution_chain`] answers them. Framed as the reads of the
//! journal: a workload invisible or absent answers 404 unjournaled, visible without
//! `kube.observation.read` on the pod domain of its cluster 403 journaled, closed (or of a
//! closed cluster) 410 (Q-019-28). The port journals the read itself
//! (`data.objects.read`): the route does not.

use axum::Extension;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use kartenn_rest::ModuleRouter;
use kartenn_rest::kit::{
    self, Api, Dates, ETag, Problem, ProblemKind, documented, op, respond, schemas_of,
};
use kartenn_service::audit::RequestContext;
use kartenn_service::authn::Principal;
use serde::Serialize;
use utoipa::openapi::path::HttpMethod;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{IdRef, allowed, closed, gone};
use crate::module::{OBSERVATION_READ_PERMISSION, WORKLOAD};
use crate::read::{self, Chain, Cut, Link, Reading};

/// What a closed chain names in its 410.
const WHAT: &str = "the workload or its cluster";

/// A link of the chain: a node the workload was seen running on at the date, its machine
/// (`kube.runs_on`) and what hosts that machine (`machines.hosted_on`,
/// `machines.hosted_in`), all at the date.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeExecutionLink)]
struct LinkDto {
    /// The node, from the journal.
    node: IdRef,
    /// Its machine (`machines.machine`); `null` for a node matched to no machine, or a
    /// machine the caller may not read.
    #[schema(required = true)]
    machine: Option<HostRef>,
    /// The hypervisor (`machines.machine`) that hosts the machine; `null` without one, or
    /// for one the caller may not read.
    #[schema(required = true)]
    hypervisor: Option<HostRef>,
    /// The cluster of virtualisation (`machines.cluster`) that hosts the machine; `null`
    /// without one, or for one the caller may not read.
    #[schema(required = true)]
    virtualization_cluster: Option<HostRef>,
}

/// An object of the module machines in a chain of execution, by its identifier.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[schema(as = ExecutionHostRef)]
struct HostRef {
    /// Identifier.
    id: Uuid,
}

/// Why a chain is empty though the workload may have run.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = KubeExecutionChainReason)]
enum ReasonDto {
    /// The date asked about is older than the retention of its cluster.
    Retention,
    /// The journal holds no observation of the workload within that retention.
    Unobserved,
}

/// The chain of execution of a workload at a date.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = KubeExecutionChain)]
struct ChainDto {
    /// One link per node, by node.
    links: Vec<LinkDto>,
    /// `retention` past the retention of its cluster, `unobserved` for a workload the
    /// journal never saw within it, the chain then empty (Q-019-31); `null` otherwise,
    /// an empty chain then saying that no pod of the workload ran at the date.
    #[schema(required = true)]
    reason: Option<ReasonDto>,
}

/// The date of the chain.
#[derive(Debug, IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)]
struct ChainParams {
    /// Valid time, RFC 3339 with its offset; now by default.
    valid_at: Option<String>,
}

fn link_dto(link: &Link) -> LinkDto {
    let reference = |id: Option<Uuid>| id.map(|id| HostRef { id });
    LinkDto {
        node: IdRef { id: link.node },
        machine: reference(link.machine),
        hypervisor: reference(link.hypervisor),
        virtualization_cluster: reference(link.cluster),
    }
}

/// `GET /v1/workloads/{id}/chain`: the chain of execution of the workload at `valid_at`
/// (now by default), under `kube.observation.read` on the pod domain of its cluster (see
/// [`allowed`]); closed, or its cluster closed, 410. No `as_of`: the chain reads the
/// current assertions of the edges, and the journal has no system time.
async fn chain(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    Extension(context): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    dates: Dates,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let frame = read::framed(&api.pool, id)
        .await
        .map_err(|e| kit::store(&e))?;
    let Some(read::Framed {
        pod_domain, living, ..
    }) = frame
    else {
        return Err(Problem::new(ProblemKind::NotFound));
    };
    let target = (WORKLOAD, id);
    allowed(
        &api,
        &principal,
        &context,
        target,
        pod_domain,
        OBSERVATION_READ_PERMISSION,
    )
    .await?;
    closed(living, WHAT)?;
    if dates.as_of.is_some() {
        return Err(kit::bad(
            "the chain of execution reads the current assertions: no as_of",
        ));
    }
    let moment = dates.at(kit::now(&api).await?, api.bound)?;
    let reading = Reading {
        context: &context,
        journal: &api.journal,
        audit: api.audit,
    };
    let mut conn = api.pool.acquire().await.map_err(|e| kit::store(&e))?;
    let answer =
        read::execution_chain(&mut conn, &api.authorizer(), reading, id, moment.valid_at).await?;
    drop(conn);
    // `None` only when, between the frame and the read, the right on the pod domain was
    // withdrawn: 404, its refusal not journaled, where the frame would have answered 403
    // journaled (a race, the read itself refused all the same).
    let Some(Chain { links, reason }) = answer else {
        return Err(Problem::new(ProblemKind::NotFound));
    };
    let reason = match reason {
        None => None,
        Some(Cut::Retention) => Some(ReasonDto::Retention),
        Some(Cut::Unobserved) => Some(ReasonDto::Unobserved),
        Some(Cut::Closed) => return Err(gone(WHAT)),
    };
    let body = ChainDto {
        links: links.iter().map(link_dto).collect(),
        reason,
    };
    let bytes = serde_json::to_vec(&body).map_err(|_| Problem::new(ProblemKind::Internal))?;
    let etag = ETag::page(&bytes);
    let response = ([(CONTENT_TYPE, "application/json")], bytes).into_response();
    Ok(respond(&headers, &moment, &etag, response))
}

/// The route of the chain, under `/v1`.
pub(super) fn routes(router: ModuleRouter) -> ModuleRouter {
    let mut schemas = Vec::new();
    schemas_of::<ChainDto>(&mut schemas);
    let chain_op = op(
        "kube_workload_chain",
        "kube",
        "The chain of execution of the workload at a date (REQ-019-11; Q-019-31): the nodes \
         the journal saw it running on then, still within the retention of its cluster, the \
         machine of each node and the hypervisor or the cluster of virtualisation that hosts \
         that machine, as they were then; under `kube.observation.read` on the pod domain of \
         its cluster (403 otherwise; 404, as an absent one, for a workload the caller neither \
         reads nor holds that right on). Past the retention, an empty chain with the reason \
         `retention`; a workload never observed within it, `unobserved`. No `as_of`. A closed \
         workload, or one of a closed cluster, is not read: 410 `gone` (Q-019-28). Each read \
         is journaled (`data.objects.read`, the pods it relied on).",
    )
    .path("id", "The workload.", true)
    .params::<ChainParams>()
    .answers("200", "The chain, by node.", "KubeExecutionChain")
    .build();
    router.routes(documented(
        "/workloads/{id}/chain",
        vec![(HttpMethod::Get, chain_op)],
        schemas,
        get(chain),
    ))
}
