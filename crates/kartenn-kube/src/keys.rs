//! The natural keys of the objects of the module as a human reads them, the `label` of
//! their summaries (spec 006, REQ-006-42, task 33e; `ModuleHandler::keys`), read at the
//! dates of a read in one statement for them all. A key names its object alone, never
//! another one: the slug of a cluster; the name of a node or of a namespace, without its
//! cluster; the name of a workload, a service or an ingress, without its namespace nor its
//! cluster (a workload is unique by kind in its namespace: its kind is not in its key, the
//! `href` of its summary says it is a workload); the label key of a tracked label; the
//! axis of a set of admitted values, without its namespace; `<label>=<value> → <axis>`
//! for a mapping of a label to a value of an axis.

use kartenn_core::period::Instant;
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::edges::{snapshot, time};
use crate::pg::{
    ADMITTED_VALUES_TABLE, CLUSTER_TABLE, INGRESS_TABLE, LABEL_MAPPING_TABLE, NAMESPACE_TABLE,
    NODE_TABLE, SERVICE_TABLE, TRACKED_LABEL_TABLE, WORKLOAD_TABLE, store,
};

/// Each table of the module, the columns its key reads and that key.
const KEYS: &[(&str, &str, &str)] = &[
    (CLUSTER_TABLE, "id, slug", "x.slug"),
    (NODE_TABLE, "id, name", "x.name"),
    (NAMESPACE_TABLE, "id, name", "x.name"),
    (WORKLOAD_TABLE, "id, name", "x.name"),
    (SERVICE_TABLE, "id, name", "x.name"),
    (INGRESS_TABLE, "id, name", "x.name"),
    (TRACKED_LABEL_TABLE, "id, label_key", "x.label_key"),
    (ADMITTED_VALUES_TABLE, "id, axis_key", "x.axis_key"),
    (
        LABEL_MAPPING_TABLE,
        "id, label_key, label_value, axis_key",
        "x.label_key || '=' || x.label_value || ' → ' || x.axis_key",
    ),
];

/// The key of each of `objects` that exists at `valid_at` as asserted at `as_of` (now
/// when `None`); an object of another module, or absent then, is left out.
///
/// # Errors
///
/// Database errors.
pub(crate) async fn of(
    conn: &mut PgConnection,
    objects: &[Uuid],
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    let sql = KEYS
        .iter()
        .map(|(table, columns, key)| {
            format!(
                "SELECT x.id, {key} FROM {} x WHERE x.id = ANY($3)",
                snapshot(table, columns)
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(time(valid_at))
        .bind(as_of.map(time))
        .bind(objects)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}
