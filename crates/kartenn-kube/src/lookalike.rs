//! `kube.machine.looks_like_pod` (REQ-019-3; design « Règles »; Q-019-32; task 019-7b):
//! a machine `container` hosted by the machine of a node, and named as a pod the journal
//! sees running on that node, is the trace of an earlier inventory of the pods. The module
//! evaluates it on the machines the changeset writes, once it carries a write of a machine
//! (`machines.create_machine`, `machines.update_machine`, among its operations), with no
//! hook in the module `machines`. Only the open observations count. The warning names the
//! pod to an author who holds `kube.observation.read` on the pod domain of the cluster of
//! the node; to anyone else, it says the machine looks like a pod of a cluster.

use kartenn_core::rules::Violation;
use kartenn_service::ObjectKind;
use kartenn_service::module::{CheckContext, Findings};
use kartenn_service::operations::Operation;
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::module::{MACHINE, OBSERVATION_READ_PERMISSION};
use crate::pg::store;
use crate::rules::MACHINE_LOOKS_LIKE_POD;

/// Whether `operation` writes the name, the kind or the host of a machine.
fn writes_machine(operation: &Operation) -> bool {
    match operation {
        Operation::Within { operation, .. } => writes_machine(operation),
        Operation::Module(m) => {
            m.module == "machines" && matches!(m.name.as_str(), "create_machine" | "update_machine")
        }
        _ => false,
    }
}

/// A machine that looks like a pod: its name, the node whose machine hosts it, the pod
/// (namespace and name) and the pod domain of the cluster of the node.
type Row = (Uuid, String, Uuid, String, String, Uuid);

/// The findings of `kube.machine.looks_like_pod` on the machines the changeset of
/// `context` writes: a warning for each one that looks like a pod, and each of them
/// evaluated anew, so that the anomaly closes once the machine is renamed, moved or
/// closed, or once the pod is gone.
pub(crate) async fn check(
    conn: &mut PgConnection,
    context: &CheckContext<'_>,
) -> Result<Findings, StoreError> {
    if !context.operations.iter().any(writes_machine) {
        return Ok(Findings::default());
    }
    let mut machines: Vec<Uuid> = context
        .objects
        .iter()
        .filter(|(kind, _)| matches!(kind, ObjectKind::Module(k) if k.as_str() == MACHINE))
        .map(|(_, id)| *id)
        .collect();
    machines.sort_unstable();
    machines.dedup();
    if machines.is_empty() {
        return Ok(Findings::default());
    }
    // Each machine living from now on, of kind `container` and hosted by a machine, on a
    // version that meets a version of a node of that machine living from now on; the
    // open observations of the cluster of the node, on the node, by the name of the
    // machine; the pod domain of the cluster in its last version begun. One pod per
    // machine, the first by namespace and name.
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT DISTINCT ON (m.id) m.id, m.name, n.id, o.namespace, o.pod_name, c.pod_domain_id
           FROM machines.machine m
           JOIN kube.node n ON n.machine_id = m.host_machine_id
                           AND n.valid_period && m.valid_period
                           AND n.valid_period && tstzrange(now(), NULL)
           JOIN kube.pod_observation o ON o.cluster_id = n.cluster_id
                                      AND upper_inf(o.observed)
                                      AND o.node_id = n.id
                                      AND o.pod_name = m.name
          CROSS JOIN LATERAL (
                SELECT pod_domain_id FROM kube.cluster
                 WHERE id = n.cluster_id AND lower(valid_period) <= now()
                 ORDER BY lower(valid_period) DESC LIMIT 1) c
          WHERE m.id = ANY($1) AND m.kind = 'container'
            AND m.valid_period && tstzrange(now(), NULL)
          ORDER BY m.id, o.namespace, o.pod_name",
    )
    .bind(&machines)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut domains: Vec<Uuid> = rows.iter().map(|row| row.5).collect();
    domains.sort_unstable();
    domains.dedup();
    let denied = if domains.is_empty() {
        Vec::new()
    } else {
        kartenn_store_pg::rbac::outside_now(
            conn,
            context.author,
            OBSERVATION_READ_PERMISSION,
            &domains,
        )
        .await?
    };
    let violations = rows
        .into_iter()
        .map(|(machine, name, node, namespace, pod, domain)| {
            let message = if denied.contains(&domain) {
                format!("the container machine {name} looks like a pod of a cluster")
            } else {
                format!(
                    "the container machine {name} is hosted by the machine of the node {node} and is named as the pod {namespace}/{pod} observed on it: the trace of an earlier inventory of the pods"
                )
            };
            Violation::new(&MACHINE_LOOKS_LIKE_POD, message).with_subjects([machine])
        })
        .collect();
    Ok(Findings {
        violations,
        reevaluated: machines
            .into_iter()
            .map(|m| (m, MACHINE_LOOKS_LIKE_POD.id))
            .collect(),
    })
}
