//! The storage of the module in PostgreSQL (feature `pg`): the tables of the schema `kube`
//! (migrations `0001_kube.sql`, `0002_node_pod_cidrs.sql`, `0003_workload.sql`), on the
//! connection of the changeset.

use kartenn_core::period::{Instant, Period};
use kartenn_core::rules::{Rule, Violation};
use kartenn_service::port::{StoreError, WriteError};
use serde_json::Value;
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

/// The table of the clusters, as the generic writes of the core name it.
pub(crate) const CLUSTER_TABLE: &str = "kube.cluster";
/// The table of the nodes.
pub(crate) const NODE_TABLE: &str = "kube.node";
/// The table of the namespaces.
pub(crate) const NAMESPACE_TABLE: &str = "kube.namespace";
/// The table of the workloads.
pub(crate) const WORKLOAD_TABLE: &str = "kube.workload";

/// An instant as PostgreSQL reads it.
fn time(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

/// The bounds of `period` to bind; a `None` start is the instant of the transaction.
pub(crate) fn bounds(period: Option<Period>) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    period.map_or((None, None), |p| (Some(time(p.start())), p.end().map(time)))
}

pub(crate) fn store(e: &sqlx::Error) -> StoreError {
    StoreError(e.to_string())
}

/// A refusal of the module before writing: a failed statement would abort the
/// transaction of the changeset, whose other violations must still be found.
pub(crate) fn refused(rule: &'static Rule, message: String, subjects: &[Uuid]) -> WriteError {
    WriteError::Rule(Violation::new(rule, message).with_subjects(subjects.iter().copied()))
}

/// The identifiers of the properties `core.owner` and `core.site` declared at `at`.
pub(crate) async fn owner_and_site(
    conn: &mut PgConnection,
    at: Option<OffsetDateTime>,
) -> Result<(Option<Uuid>, Option<Uuid>), StoreError> {
    let rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT key, id FROM public.property_declaration
          WHERE key IN ('core.owner', 'core.site')
            AND valid_period @> coalesce($1::timestamptz, now())",
    )
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    let id = |key: &str| rows.iter().find(|(k, _)| k == key).map(|(_, id)| *id);
    Ok((id("core.owner"), id("core.site")))
}

/// The pod domain of the cluster `id` at `at`, if it lives then.
pub(crate) async fn pod_domain(
    conn: &mut PgConnection,
    cluster: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT pod_domain_id FROM kube.cluster
          WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(cluster)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The pod containers of `cluster` at `at`, as prefixes (`address/length`).
pub(crate) async fn pod_containers(
    conn: &mut PgConnection,
    cluster: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<String>, StoreError> {
    sqlx::query_scalar(
        "SELECT p.prefix::text
           FROM kube.cluster c
           JOIN public.prefix p
             ON p.id = ANY(c.pod_container_ids)
            AND p.valid_period @> coalesce($2::timestamptz, now())
          WHERE c.id = $1 AND c.valid_period @> coalesce($2::timestamptz, now())
          ORDER BY p.prefix",
    )
    .bind(cluster)
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The networks the node `node` cites at `at` (`range_ids`), with their prefixes: the
/// pod ranges it holds a network for.
pub(crate) async fn node_ranges(
    conn: &mut PgConnection,
    node: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<(String, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT p.prefix::text, p.id
           FROM kube.node n
           JOIN public.prefix p
             ON p.id = ANY(n.range_ids)
            AND p.valid_period @> coalesce($2::timestamptz, now())
          WHERE n.id = $1 AND n.valid_period @> coalesce($2::timestamptz, now())
          ORDER BY p.prefix",
    )
    .bind(node)
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The object of `table` (`node` or `namespace`) named `name` in `cluster`, living at `at`:
/// its identity in the cluster (019/X9).
pub(crate) async fn named(
    conn: &mut PgConnection,
    table: &str,
    cluster: Uuid,
    name: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    let sql = format!(
        "SELECT id FROM {table}
          WHERE cluster_id = $1 AND name = $2 AND valid_period @> coalesce($3::timestamptz, now())"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(cluster)
        .bind(name)
        .bind(at)
        .fetch_optional(conn)
        .await
        .map_err(|e| store(&e))
}

/// The workload of `kind` named `name` in `namespace`, living at `at`: its identity in the
/// namespace (019/X3).
pub(crate) async fn workload_named(
    conn: &mut PgConnection,
    namespace: Uuid,
    kind: &str,
    name: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT id FROM kube.workload
          WHERE namespace_id = $1 AND kind = $2 AND name = $3
            AND valid_period @> coalesce($4::timestamptz, now())",
    )
    .bind(namespace)
    .bind(kind)
    .bind(name)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// The workload of `kind` named `name` in `namespace` closed last before `at`, if none of
/// its versions lives from `at` on: the one a recreation in Kubernetes opens again
/// (019/X3).
pub(crate) async fn workload_closed(
    conn: &mut PgConnection,
    namespace: Uuid,
    kind: &str,
    name: &str,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT w.id FROM kube.workload w
          WHERE w.namespace_id = $1 AND w.kind = $2 AND w.name = $3
            AND upper(w.valid_period) <= coalesce($4::timestamptz, now())
            AND NOT EXISTS (SELECT 1 FROM kube.workload l
                             WHERE l.id = w.id
                               AND l.valid_period && tstzrange(coalesce($4::timestamptz, now()), NULL))
          ORDER BY upper(w.valid_period) DESC
          LIMIT 1",
    )
    .bind(namespace)
    .bind(kind)
    .bind(name)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// Whether another object of `table` than `id` matches `condition` (on the texts `$1`
/// to `$n` of `keys`) over `period`.
async fn taken(
    conn: &mut PgConnection,
    table: &str,
    condition: &str,
    keys: &[&str],
    id: Uuid,
    period: Option<Period>,
) -> Result<bool, StoreError> {
    let (from, to) = bounds(period);
    let n = keys.len();
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {table}
                         WHERE {condition} AND id <> ${}
                           AND valid_period && tstzrange(coalesce(${}::timestamptz, now()), ${}))",
        n + 1,
        n + 2,
        n + 3,
    );
    let mut query = sqlx::query_scalar(sqlx::AssertSqlSafe(sql));
    for key in keys {
        query = query.bind(*key);
    }
    query
        .bind(id)
        .bind(from)
        .bind(to)
        .fetch_one(conn)
        .await
        .map_err(|e| store(&e))
}

/// The refusals of the unique identities of `row` (the full row of `table`, or the
/// columns a patch changes merged over the current ones), before they hit the exclusion
/// constraints: a cluster slug, a node or namespace name in its cluster, a workload kind
/// and name in its namespace, a machine designated by one node.
pub(crate) async fn identities(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    row: &Value,
    period: Option<Period>,
) -> Result<(), WriteError> {
    let field = |name: &str| row.get(name).and_then(Value::as_str);
    if table == CLUSTER_TABLE {
        if let Some(slug) = field("slug")
            && taken(conn, table, "slug = $1", &[slug], id, period).await?
        {
            return Err(refused(
                &kartenn_core::rules::SLUG_DUPLICATE,
                format!("the slug {slug} of a Kubernetes cluster is taken over the period"),
                &[id],
            ));
        }
        return Ok(());
    }
    if table == WORKLOAD_TABLE {
        if let (Some(namespace), Some(kind), Some(name)) =
            (field("namespace_id"), field("kind"), field("name"))
            && taken(
                conn,
                table,
                "namespace_id = $1::uuid AND kind = $2 AND name = $3",
                &[namespace, kind, name],
                id,
                period,
            )
            .await?
        {
            return Err(refused(
                &crate::rules::WORKLOAD_DUPLICATE,
                format!("the {kind} {name} is taken in the namespace over the period"),
                &[id],
            ));
        }
        return Ok(());
    }
    if let (Some(cluster), Some(name)) = (field("cluster_id"), field("name"))
        && taken(
            conn,
            table,
            "cluster_id = $1::uuid AND name = $2",
            &[cluster, name],
            id,
            period,
        )
        .await?
    {
        let rule = if table == NODE_TABLE {
            &crate::rules::NODE_DUPLICATE
        } else {
            &crate::rules::NAMESPACE_DUPLICATE
        };
        return Err(refused(
            rule,
            format!("the name {name} is taken in the cluster over the period"),
            &[id],
        ));
    }
    if table == NODE_TABLE
        && let Some(machine) = field("machine_id")
        && taken(conn, table, "machine_id = $1::uuid", &[machine], id, period).await?
    {
        return Err(refused(
            &crate::rules::NODE_MACHINE_KIND,
            format!("the machine {machine} is designated by another living node"),
            &[id],
        ));
    }
    Ok(())
}

/// The current state of `id` in `table` at the start of `period`, as JSON.
pub(crate) async fn current(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    period: Option<Period>,
) -> Result<Option<Value>, StoreError> {
    let (from, _) = bounds(period);
    let sql = format!(
        "SELECT to_jsonb(t) FROM {table} t
          WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(from)
        .fetch_optional(conn)
        .await
        .map_err(|e| store(&e))
}

/// Whether changing the pod domain of `cluster` to `domain` over `period` moves it under
/// living nodes or namespaces of the cluster: a version of the cluster over the period has
/// another pod domain, and an object of the cluster lives over the period.
pub(crate) async fn pod_domain_moved(
    conn: &mut PgConnection,
    cluster: Uuid,
    domain: &Value,
    period: Option<Period>,
) -> Result<bool, StoreError> {
    let (from, to) = bounds(period);
    sqlx::query_scalar(
        "WITH portion AS (SELECT tstzrange(coalesce($3::timestamptz, now()), $4) AS p)
         SELECT EXISTS (SELECT 1 FROM kube.cluster c, portion
                         WHERE c.id = $1 AND c.valid_period && portion.p
                           AND c.pod_domain_id <> ($2 #>> '{}')::uuid)
            AND EXISTS (SELECT 1 FROM (SELECT cluster_id, valid_period FROM kube.node
                                       UNION ALL
                                       SELECT cluster_id, valid_period FROM kube.namespace) o,
                               portion
                         WHERE o.cluster_id = $1 AND o.valid_period && portion.p)",
    )
    .bind(cluster)
    .bind(domain)
    .bind(from)
    .bind(to)
    .fetch_one(conn)
    .await
    .map_err(|e| store(&e))
}

/// The parent of the node, namespace or workload `id` of `table` (its cluster, or the
/// namespace of a workload) and the pod domain of its cluster, at the start of `period`:
/// its place in the tree of the module and its anchor.
pub(crate) async fn placement(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    period: Option<Period>,
) -> Result<Option<(Uuid, Option<Uuid>)>, StoreError> {
    let (from, _) = bounds(period);
    let sql = if table == WORKLOAD_TABLE {
        "SELECT w.namespace_id, c.pod_domain_id
           FROM kube.workload w
           LEFT JOIN kube.namespace n
             ON n.id = w.namespace_id AND n.valid_period @> coalesce($2::timestamptz, now())
           LEFT JOIN kube.cluster c
             ON c.id = n.cluster_id AND c.valid_period @> coalesce($2::timestamptz, now())
          WHERE w.id = $1 AND w.valid_period @> coalesce($2::timestamptz, now())"
            .to_owned()
    } else {
        format!(
            "SELECT o.cluster_id, c.pod_domain_id
               FROM {table} o
               LEFT JOIN kube.cluster c
                 ON c.id = o.cluster_id AND c.valid_period @> coalesce($2::timestamptz, now())
              WHERE o.id = $1 AND o.valid_period @> coalesce($2::timestamptz, now())"
        )
    };
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(from)
        .fetch_optional(conn)
        .await
        .map_err(|e| store(&e))
}

/// A new row of `table` over `period`: `row` holds every column but those of time.
pub(crate) async fn insert(
    conn: &mut PgConnection,
    table: &str,
    row: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let sql = format!(
        "INSERT INTO {table}
         SELECT * FROM jsonb_populate_record(NULL::{table},
             $1::jsonb || jsonb_build_object(
                 'valid_period', tstzrange(coalesce($2::timestamptz, now()), $3),
                 'sys_start', now(), 'changeset_id', $4::uuid))"
    );
    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(row)
        .bind(from)
        .bind(to)
        .bind(changeset)
        .execute(conn)
        .await
        .map(drop)
        .map_err(|e| WriteError::Store(store(&e)))
}

/// Applies `patch` (columns and their values) to the row `id` of `table` over `period`
/// ("FOR PORTION OF", migration 0018 of the core).
pub(crate) async fn portion(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    patch: &Value,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    let (from, to) = bounds(period);
    let versions: i32 = sqlx::query_scalar(
        "SELECT public.kartenn_portion($1::regclass, $2, coalesce($3::timestamptz, now()), $4,
                                       $5::jsonb, $6)",
    )
    .bind(table)
    .bind(id)
    .bind(from)
    .bind(to)
    .bind(patch)
    .bind(changeset)
    .fetch_one(conn)
    .await
    .map_err(|e| WriteError::Store(store(&e)))?;
    if versions == 0 {
        return Err(WriteError::NoVersion(id));
    }
    Ok(())
}

/// The clusters among `ids` whose designated ranges are not of their domains over one of
/// their versions (`kube.cluster.range_domain`): `(cluster, range)`.
pub(crate) async fn ranges_outside(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT c.id, r.range
           FROM kube.cluster c
           CROSS JOIN LATERAL (
               SELECT unnest(c.pod_container_ids), c.pod_domain_id, 'container'
               UNION ALL
               SELECT unnest(c.service_network_ids), c.service_domain_id, 'network'
           ) AS r(range, domain, nature)
          WHERE c.id = ANY($1)
            AND NOT EXISTS (
                SELECT 1 FROM public.prefix p
                 WHERE p.id = r.range AND p.domain_id = r.domain AND p.nature = r.nature
                 GROUP BY p.id
                HAVING range_agg(p.valid_period) @> c.valid_period)
          ORDER BY c.id, r.range",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The clusters among `ids` whose designated domains do not live over one of their
/// versions: `(cluster, domain)`.
pub(crate) async fn dead_domains(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT c.id, d.domain
           FROM kube.cluster c
           CROSS JOIN LATERAL (VALUES (c.pod_domain_id), (c.service_domain_id),
                                      (c.external_domain_id)) AS d(domain)
          WHERE c.id = ANY($1) AND d.domain IS NOT NULL
            AND NOT EXISTS (
                SELECT 1 FROM public.addressing_domain a
                 WHERE a.id = d.domain
                 GROUP BY a.id
                HAVING range_agg(a.valid_period) @> c.valid_period)
          ORDER BY c.id, d.domain",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The clusters among `ids` whose connector group does not live over one of their
/// versions (`kube.cluster.connector_group`, 025/G1): `(cluster, group)`.
pub(crate) async fn dead_groups(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT c.id, c.connector_group_id
           FROM kube.cluster c
          WHERE c.id = ANY($1) AND c.connector_group_id IS NOT NULL
            AND NOT EXISTS (
                SELECT 1 FROM public.rbac_group g
                 WHERE g.id = c.connector_group_id
                 GROUP BY g.id
                HAVING range_agg(g.valid_period) @> c.valid_period)
          ORDER BY c.id",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The nodes among `ids` whose machine is not `physical` or `virtual`, or does not live,
/// over one of their versions (`kube.node.machine_kind`): `(node, machine)`.
pub(crate) async fn bad_machines(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, Uuid)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT n.id, n.machine_id
           FROM kube.node n
          WHERE n.id = ANY($1) AND n.machine_id IS NOT NULL
            AND NOT EXISTS (
                SELECT 1 FROM machines.machine m
                 WHERE m.id = n.machine_id AND m.kind IN ('physical', 'virtual')
                 GROUP BY m.id
                HAVING range_agg(m.valid_period) @> n.valid_period)
          ORDER BY n.id",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The pod ranges of the nodes among `ids`, living from now on, that no pod container of
/// their cluster strictly contains over their version (`kube.node.range_outside`):
/// `(node, range)`.
pub(crate) async fn pod_ranges_outside(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT n.id, r.cidr::text
           FROM kube.node n
           CROSS JOIN LATERAL unnest(n.pod_cidrs) AS r(cidr)
          WHERE n.id = ANY($1) AND n.valid_period && tstzrange(now(), NULL)
            AND NOT EXISTS (
                SELECT 1 FROM kube.cluster c
                  JOIN public.prefix p ON p.id = ANY(c.pod_container_ids)
                 WHERE c.id = n.cluster_id
                   AND c.valid_period && n.valid_period
                   AND p.valid_period && n.valid_period
                   AND p.prefix >> r.cidr)
          ORDER BY 1, 2",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The living objects of the module that cite `target` by the reference `from` → `to`
/// from `at` on.
pub(crate) async fn referrers(
    conn: &mut PgConnection,
    from: &str,
    to: &str,
    target: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<Uuid>, StoreError> {
    use crate::module::{CLUSTER, DOMAIN, MACHINE, NAMESPACE, NODE, PREFIX, WORKLOAD};
    let sql = match (from, to) {
        (NODE, CLUSTER) => "SELECT DISTINCT id FROM kube.node WHERE cluster_id = $1 AND {live}",
        (NAMESPACE, CLUSTER) => {
            "SELECT DISTINCT id FROM kube.namespace WHERE cluster_id = $1 AND {live}"
        }
        (WORKLOAD, NAMESPACE) => {
            "SELECT DISTINCT id FROM kube.workload WHERE namespace_id = $1 AND {live}"
        }
        (NODE, MACHINE) => "SELECT DISTINCT id FROM kube.node WHERE machine_id = $1 AND {live}",
        (NODE, PREFIX) => {
            "SELECT DISTINCT id FROM kube.node WHERE range_ids @> ARRAY[$1::uuid] AND {live}"
        }
        (CLUSTER, DOMAIN) => {
            "SELECT DISTINCT id FROM kube.cluster
              WHERE (pod_domain_id = $1 OR service_domain_id = $1 OR external_domain_id = $1)
                AND {live}"
        }
        (CLUSTER, PREFIX) => {
            "SELECT DISTINCT id FROM kube.cluster
              WHERE ($1 = ANY(pod_container_ids) OR $1 = ANY(service_network_ids)) AND {live}"
        }
        _ => return Ok(Vec::new()),
    };
    let sql = format!(
        "{} ORDER BY id",
        sql.replace(
            "{live}",
            "valid_period && tstzrange(coalesce($2::timestamptz, now()), NULL)",
        )
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(target)
        .bind(at)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))
}
