//! Who may write the mirror objects (019/X5, X10, ADR 0044; task 019-5a): their tracked
//! fields only by a service account member of the connector group of their cluster
//! (`kube.object.managed`), and a member of a connector group only by `kube.sync`
//! (`kube.connector.direct`). Both read the author of the changeset, as the pipeline set
//! it, and the operations it wrote: the plans `kube.sync` adds itself (a selection set to
//! nothing, the addresses and networks it creates or lets go) are not among them, and pass.

use kartenn_core::rules::Violation;
use kartenn_service::module::CheckContext;
use kartenn_service::operations::{ModuleKind, Operation};
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::pg::store;
use crate::rules::{CONNECTOR_DIRECT, OBJECT_MANAGED};

/// Whether `operation` is a `kube.sync`, on its own period or the changeset's.
fn is_sync(operation: &Operation) -> bool {
    match operation {
        Operation::Within { operation, .. } => is_sync(operation),
        Operation::Module(m) => m.module == "kube" && m.name == "sync",
        _ => false,
    }
}

/// The violations of authorship of the changeset of `author` (`None`: kartenn itself, never
/// a connector): its mirrors are the objects of the module it writes but its clusters.
pub(crate) async fn check(
    conn: &mut PgConnection,
    author: Option<Uuid>,
    context: &CheckContext<'_>,
    cluster: ModuleKind,
) -> Result<Vec<Violation>, StoreError> {
    let mut found = Vec::new();
    let mirrors: Vec<Uuid> = context
        .touched
        .iter()
        .filter(|(kind, _)| *kind != cluster)
        .map(|(_, id)| *id)
        .collect();
    // Only `kube.sync` writes the tracked fields of a mirror: a closure or a restoration
    // (an administrator undoing a batch, X5) and the properties stay under ordinary rights.
    if !mirrors.is_empty() && context.operations.iter().any(is_sync) {
        for object in unmanaged(conn, author, &mirrors).await? {
            found.push(
                Violation::new(
                    &OBJECT_MANAGED,
                    format!(
                        "the object {object} is a mirror of its cluster: its tracked fields are written only by a service account member of the connector group of the cluster"
                    ),
                )
                .with_subjects([object]),
            );
        }
    }
    let foreign: Vec<usize> = context
        .operations
        .iter()
        .enumerate()
        .filter(|(_, o)| !is_sync(o))
        .map(|(i, _)| i)
        .collect();
    if let (Some(author), Some(first)) = (author, foreign.first()) {
        let groups = connector_groups(conn, author).await?;
        if !groups.is_empty() {
            found.push(
                Violation::new(
                    &CONNECTOR_DIRECT,
                    format!(
                        "the author is a member of a connector group, who writes only through kube.sync: {} operation(s) of the changeset are not, the first at position {first}",
                        foreign.len()
                    ),
                )
                .with_subjects(groups),
            );
        }
    }
    Ok(found)
}

/// The objects among `mirrors` whose cluster does not designate, from now on, a connector
/// group that `author`, a service account, is a direct member of now: all of them for a
/// person, a person member of the group included (REQ-019-12).
async fn unmanaged(
    conn: &mut PgConnection,
    author: Option<Uuid>,
    mirrors: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "WITH mirror (id, cluster_id) AS (
             SELECT id, cluster_id FROM kube.node WHERE id = ANY($1)
             UNION SELECT id, cluster_id FROM kube.namespace WHERE id = ANY($1)
             UNION SELECT o.id, n.cluster_id
                     FROM kube.workload o JOIN kube.namespace n ON n.id = o.namespace_id
                    WHERE o.id = ANY($1)
             UNION SELECT o.id, n.cluster_id
                     FROM kube.service o JOIN kube.namespace n ON n.id = o.namespace_id
                    WHERE o.id = ANY($1)
             UNION SELECT o.id, n.cluster_id
                     FROM kube.ingress o JOIN kube.namespace n ON n.id = o.namespace_id
                    WHERE o.id = ANY($1))
         SELECT DISTINCT m.id FROM mirror m
          WHERE NOT EXISTS (
                SELECT 1
                  FROM kube.cluster c
                  JOIN public.group_member g
                    ON g.group_id = c.connector_group_id AND g.member_kind = 'identity'
                   AND g.identity_id = $2 AND g.valid_period @> now()
                  JOIN public.person p ON p.id = g.identity_id AND p.kind = 'service_account'
                 WHERE c.id = m.cluster_id AND c.valid_period && tstzrange(now(), NULL)
                   AND EXISTS (SELECT 1 FROM public.rbac_group r
                                WHERE r.id = g.group_id AND r.valid_period @> now()))
          ORDER BY m.id",
    )
    .bind(mirrors)
    .bind(author)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The groups that a cluster designates, from now on, as its connector group and that
/// `author` is a direct member of now, whatever its kind (019/X10).
async fn connector_groups(conn: &mut PgConnection, author: Uuid) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT DISTINCT g.group_id
           FROM public.group_member g
           JOIN kube.cluster c
             ON c.connector_group_id = g.group_id AND c.valid_period && tstzrange(now(), NULL)
          WHERE g.member_kind = 'identity' AND g.identity_id = $1 AND g.valid_period @> now()
          ORDER BY g.group_id",
    )
    .bind(author)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

#[cfg(test)]
mod tests {
    use kartenn_service::operations::{ModuleOperation, PeriodSpec};
    use serde_json::Map;

    use super::*;

    fn module(module: &str, name: &str) -> Operation {
        Operation::Module(ModuleOperation {
            module: module.into(),
            name: name.into(),
            body: Map::new(),
        })
    }

    #[test]
    fn only_kube_sync_is_sync() {
        assert!(is_sync(&module("kube", "sync")));
        assert!(!is_sync(&module("kube", "update_cluster")));
        assert!(!is_sync(&module("machines", "sync")));
        assert!(!is_sync(&Operation::CreateDomain {
            local: None,
            slug: "d".into(),
        }));
        let within = |operation| Operation::Within {
            period: PeriodSpec {
                start: 1_767_225_600_000_000,
                end: None,
            },
            operation: Box::new(operation),
        };
        assert!(is_sync(&within(module("kube", "sync"))));
        assert!(!is_sync(&within(module("kube", "create_cluster"))));
    }
}
