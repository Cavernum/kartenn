//! `kube.machine.looks_like_pod` (REQ-019-3; design « Règles »; Q-019-32 and its
//! amendment; task 019-7b): a machine `container` hosted by the machine of a node, and
//! named as a pod the journal sees running on that node, is the trace of an earlier
//! inventory of the pods. The module evaluates it on the machines the changeset writes,
//! once it carries a write of a machine (`machines.create_machine`,
//! `machines.update_machine`, among its operations), with no hook in the module
//! `machines`. Only the open observations count. The warning never names the pod, whoever
//! the author: it becomes an anomaly that any reader of the machine sees; a reader who may
//! read the journal finds the pod by `read::execution_chain` or the journal.

use kartenn_core::rules::Violation;
use kartenn_service::ObjectKind;
use kartenn_service::module::{CheckContext, Findings};
use kartenn_service::operations::Operation;
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::module::MACHINE;
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
    // version that meets a version of a node of that machine living from now on, whose
    // name is that of a pod of an open observation of that node (in the cluster of the
    // node).
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT DISTINCT m.id, m.name
           FROM machines.machine m
           JOIN kube.node n ON n.machine_id = m.host_machine_id
                           AND n.valid_period && m.valid_period
                           AND n.valid_period && tstzrange(now(), NULL)
          WHERE m.id = ANY($1) AND m.kind = 'container'
            AND m.valid_period && tstzrange(now(), NULL)
            AND EXISTS (SELECT 1 FROM kube.pod_observation o
                         WHERE o.cluster_id = n.cluster_id AND upper_inf(o.observed)
                           AND o.node_id = n.id AND o.pod_name = m.name)
          ORDER BY m.id",
    )
    .bind(&machines)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let violations = rows
        .into_iter()
        .map(|(machine, name)| {
            Violation::new(
                &MACHINE_LOOKS_LIKE_POD,
                format!("the container machine {name} looks like a pod of a cluster"),
            )
            .with_subjects([machine])
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
