//! The edges of the module at an object (REQ-019-11; ADR 0026, clause 11), read from its
//! tables at the dates of a read: a node runs on its machine and holds its pod ranges, a
//! service selects its workloads and holds its addresses, an ingress routes to the
//! services its routes name and holds its addresses.

use kartenn_core::period::{Instant, Period};
use kartenn_service::module::{Edge, Module};
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::module::Kube;
use crate::pg::{INGRESS_TABLE, NODE_TABLE, SERVICE_TABLE, store};

/// The versions of `table` valid at `$1`, as asserted at `$2` (now when null): the current
/// rows, or the history for a past `$2` (as `kartenn_machines::read`). `table` and `cols`
/// are constants of this module.
fn snapshot(table: &str, cols: &str) -> String {
    format!(
        "(SELECT {cols} FROM {table}
           WHERE valid_period @> $1::timestamptz
             AND ($2::timestamptz IS NULL OR sys_start <= $2::timestamptz)
          UNION ALL
          SELECT {cols} FROM {table}_history
           WHERE $2::timestamptz IS NOT NULL AND valid_period @> $1::timestamptz
             AND sys_start <= $2::timestamptz AND sys_end > $2::timestamptz)"
    )
}

fn time(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

fn instant(t: OffsetDateTime) -> Instant {
    Instant(i64::try_from(t.unix_timestamp_nanos() / 1_000).unwrap_or(i64::MAX))
}

/// The edges of the module that start or end at `object` at (`valid_at`, `as_of`), each
/// with the period of the version it comes from; a route, with the common period of the
/// ingress and of the service it names. Each branch is filtered on `object` by the key or
/// the index of its table, then each edge on its ends.
///
/// # Errors
///
/// Database errors.
pub(crate) async fn of(
    conn: &mut PgConnection,
    object: Uuid,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<Edge>, StoreError> {
    type Row = (String, Uuid, Uuid, OffsetDateTime, Option<OffsetDateTime>);
    let node = snapshot(NODE_TABLE, "id, machine_id, range_ids, valid_period");
    let service = snapshot(
        SERVICE_TABLE,
        "id, namespace_id, name, workload_ids, cluster_address_ids, external_address_ids, \
         valid_period",
    );
    let ingress = snapshot(
        INGRESS_TABLE,
        "id, namespace_id, rules, address_ids, valid_period",
    );
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH e (name, origin, target, period) AS (
             SELECT 'kube.runs_on'::text, id, machine_id, valid_period
               FROM {node} n
              WHERE machine_id IS NOT NULL AND (id = $3 OR machine_id = $3)
             UNION ALL
             SELECT 'kube.node_range', n.id, r.range, n.valid_period
               FROM {node} n, unnest(n.range_ids) AS r (range)
              WHERE n.id = $3 OR n.range_ids @> ARRAY[$3::uuid]
             UNION ALL
             SELECT 'kube.selects', s.id, w.workload, s.valid_period
               FROM {service} s, unnest(s.workload_ids) AS w (workload)
              WHERE s.id = $3 OR s.workload_ids @> ARRAY[$3::uuid]
             UNION ALL
             -- One address may be a cluster IP and an external address at once (019-4b).
             SELECT DISTINCT 'kube.service_address'::text, s.id, a.address, s.valid_period
               FROM {service} s,
                    unnest(s.cluster_address_ids || s.external_address_ids) AS a (address)
              WHERE s.id = $3 OR s.cluster_address_ids @> ARRAY[$3::uuid]
                 OR s.external_address_ids @> ARRAY[$3::uuid]
             UNION ALL
             -- A route names a service of the namespace of the ingress by its name; the
             -- routes of an ingress to one service make one edge.
             SELECT DISTINCT 'kube.routes_to'::text, i.id, s.id, i.valid_period * s.valid_period
               FROM {ingress} i
               CROSS JOIN LATERAL jsonb_array_elements(i.rules) AS r (route)
               JOIN {service} s ON s.namespace_id = i.namespace_id
                               AND s.name = r.route ->> 'service'
              WHERE i.id = $3
             UNION ALL
             SELECT 'kube.routes_to', i.id, s.id, i.valid_period * s.valid_period
               FROM {service} s
               JOIN {ingress} i
                 ON i.namespace_id = s.namespace_id
                AND i.rules @> jsonb_build_array(jsonb_build_object('service', s.name))
              WHERE s.id = $3
             UNION ALL
             SELECT 'kube.ingress_address', i.id, a.address, i.valid_period
               FROM {ingress} i, unnest(i.address_ids) AS a (address)
              WHERE i.id = $3 OR i.address_ids @> ARRAY[$3::uuid])
         -- The other elements of the array of a row the index found are not edges at
         -- `object`.
         SELECT name, origin, target, lower(period), upper(period) FROM e
          WHERE origin = $3 OR target = $3
          ORDER BY name, origin, target"
    )))
    .bind(time(valid_at))
    .bind(as_of.map(time))
    .bind(object)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    let named = |name: &str| Kube.edges().iter().find(|t| t.name == name).map(|t| t.name);
    Ok(rows
        .into_iter()
        .filter_map(|(name, from, to, start, end)| {
            let period = Period::new(instant(start), end.map(instant)).ok()?;
            Some(Edge {
                name: named(&name)?,
                from,
                to,
                period,
            })
        })
        .collect())
}
