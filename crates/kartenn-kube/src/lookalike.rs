//! `kube.machine.looks_like_pod` (REQ-019-3; design « Règles »; Q-019-32 and its
//! amendments; task 019-7b): a machine `container` hosted by the machine of a node, and
//! named as a pod the journal sees running on that node, is the trace of an earlier
//! inventory of the pods. The module evaluates it on the machines the changeset writes,
//! once it carries a write of a machine (`machines.create_machine`,
//! `machines.update_machine`, among its operations), with no hook in the module
//! `machines`. Only the open observations count.
//!
//! The warning never names the pod: it becomes an anomaly that any reader of the machine
//! sees. It is raised only for an author who holds `kube.observation.read` on the pod
//! domain of the cluster of the node: to anyone else, a warning or its absence would
//! answer whether a pod of that name runs there. A machine that no longer looks like a
//! pod is evaluated anew whoever the author, so that its anomaly closes: that reveals
//! nothing. One that still does is left as it is to an author without the right.

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

/// The findings of `kube.machine.looks_like_pod` on the machines the changeset of
/// `context` writes (see the module).
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
    // node); with the pod domain of that cluster in its last version begun.
    let rows: Vec<(Uuid, String, Uuid)> = sqlx::query_as(
        "SELECT DISTINCT m.id, m.name, c.pod_domain_id
           FROM machines.machine m
           JOIN kube.node n ON n.machine_id = m.host_machine_id
                           AND n.valid_period && m.valid_period
                           AND n.valid_period && tstzrange(now(), NULL)
          CROSS JOIN LATERAL (
                SELECT pod_domain_id FROM kube.cluster
                 WHERE id = n.cluster_id AND lower(valid_period) <= now()
                 ORDER BY lower(valid_period) DESC LIMIT 1) c
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
    let mut domains: Vec<Uuid> = rows.iter().map(|row| row.2).collect();
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
    let mut violations: Vec<Violation> = Vec::new();
    let mut silent = Vec::new();
    for (machine, name, domain) in rows {
        if denied.contains(&domain) {
            silent.push(machine);
        } else if !violations.iter().any(|v| v.subjects == [machine]) {
            violations.push(
                Violation::new(
                    &MACHINE_LOOKS_LIKE_POD,
                    format!("the container machine {name} looks like a pod of a cluster"),
                )
                .with_subjects([machine]),
            );
        }
    }
    // A machine that warns is evaluated anew; one that still looks like a pod only on a
    // domain the author may not read keeps its anomaly as it is.
    let warned: Vec<Uuid> = violations.iter().flat_map(|v| v.subjects.clone()).collect();
    let reevaluated = machines
        .into_iter()
        .filter(|m| warned.contains(m) || !silent.contains(m))
        .map(|m| (m, MACHINE_LOOKS_LIKE_POD.id))
        .collect();
    Ok(Findings {
        violations,
        reevaluated,
    })
}
