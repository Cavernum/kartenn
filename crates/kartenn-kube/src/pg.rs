//! The storage of the module in PostgreSQL (feature `pg`): the tables of the schema `kube`
//! (migrations `0001_kube.sql` to `0005_ingress.sql`), on the connection of the
//! changeset.

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
/// The table of the services.
pub(crate) const SERVICE_TABLE: &str = "kube.service";
/// The table of the ingresses.
pub(crate) const INGRESS_TABLE: &str = "kube.ingress";
/// The table of the mappings of labels to values of axes (migration `0008_settings.sql`).
pub(crate) const LABEL_MAPPING_TABLE: &str = "kube.label_mapping";
/// The table of the admitted values of an axis in a namespace.
pub(crate) const ADMITTED_VALUES_TABLE: &str = "kube.admitted_values";
/// The table of the label keys tracked in every cluster.
pub(crate) const TRACKED_LABEL_TABLE: &str = "kube.tracked_label";

/// An instant as PostgreSQL reads it.
fn time(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

/// The bounds of `period` to bind; a `None` start is the instant of the transaction.
pub(crate) fn bounds(period: Option<Period>) -> (Option<OffsetDateTime>, Option<OffsetDateTime>) {
    period.map_or((None, None), |p| (Some(time(p.start())), p.end().map(time)))
}

/// The start of `period`, else the instant of the transaction, as JSON: the instant of
/// observation of a write of the connector (REQ-019-24).
pub(crate) async fn observed_at(
    conn: &mut PgConnection,
    period: Option<Period>,
) -> Result<Value, StoreError> {
    let (from, _) = bounds(period);
    sqlx::query_scalar("SELECT to_jsonb(coalesce($1::timestamptz, now()))")
        .bind(from)
        .fetch_one(conn)
        .await
        .map_err(|e| store(&e))
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

/// Whether the object `id` of `table`, living at `at`, is absent from its cluster then.
pub(crate) async fn is_absent(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<bool, StoreError> {
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM {table}
                         WHERE id = $1 AND absent_since IS NOT NULL
                           AND valid_period @> coalesce($2::timestamptz, now()))"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(at)
        .fetch_one(conn)
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

/// The identity of an object of a namespace: the workload of a kind, the service or the
/// ingress, named `name` in `namespace` (019/X3).
#[derive(Debug, Clone, Copy)]
pub(crate) struct InNamespace<'a> {
    /// [`WORKLOAD_TABLE`], [`SERVICE_TABLE`] or [`INGRESS_TABLE`].
    pub(crate) table: &'static str,
    pub(crate) namespace: Uuid,
    /// The kind of a workload; `None` for a service or an ingress.
    pub(crate) kind: Option<&'a str>,
    pub(crate) name: &'a str,
}

impl InNamespace<'_> {
    /// The table of the object and the condition on `w` of its identity, on `$1` to `$3`.
    fn of(self) -> (&'static str, &'static str) {
        if self.table == WORKLOAD_TABLE {
            (
                WORKLOAD_TABLE,
                "w.namespace_id = $1 AND w.kind = $2 AND w.name = $3",
            )
        } else {
            (
                self.table,
                "w.namespace_id = $1 AND $2::text IS NULL AND w.name = $3",
            )
        }
    }
}

/// The object of `identity`, living at `at` (019/X3).
pub(crate) async fn namespaced(
    conn: &mut PgConnection,
    identity: InNamespace<'_>,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    let (table, condition) = identity.of();
    let sql = format!(
        "SELECT w.id FROM {table} w
          WHERE {condition} AND w.valid_period @> coalesce($4::timestamptz, now())"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(identity.namespace)
        .bind(identity.kind)
        .bind(identity.name)
        .bind(at)
        .fetch_optional(conn)
        .await
        .map_err(|e| store(&e))
}

/// The object of `identity` closed last before `at`, if none of its versions lives from
/// `at` on: the one a recreation in Kubernetes opens again (019/X3).
pub(crate) async fn namespaced_closed(
    conn: &mut PgConnection,
    identity: InNamespace<'_>,
    at: Option<OffsetDateTime>,
) -> Result<Option<Uuid>, StoreError> {
    let (table, condition) = identity.of();
    let sql = format!(
        "SELECT w.id FROM {table} w
          WHERE {condition}
            AND upper(w.valid_period) <= coalesce($4::timestamptz, now())
            AND NOT EXISTS (SELECT 1 FROM {table} l
                             WHERE l.id = w.id
                               AND l.valid_period && tstzrange(coalesce($4::timestamptz, now()), NULL))
          ORDER BY upper(w.valid_period) DESC
          LIMIT 1"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(identity.namespace)
        .bind(identity.kind)
        .bind(identity.name)
        .bind(at)
        .fetch_optional(conn)
        .await
        .map_err(|e| store(&e))
}

/// The service domain and the external domain of `cluster` at `at`.
pub(crate) async fn service_domains(
    conn: &mut PgConnection,
    cluster: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Option<(Uuid, Option<Uuid>)>, StoreError> {
    sqlx::query_as(
        "SELECT service_domain_id, external_domain_id FROM kube.cluster
          WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(cluster)
    .bind(at)
    .fetch_optional(conn)
    .await
    .map_err(|e| store(&e))
}

/// An address a service cites: as a cluster IP, as an external address (both when the two
/// domains are one), its domain, its value and its identifier.
pub(crate) type HeldAddress = (bool, bool, Uuid, String, Uuid);

/// The living addresses the service `service` cites at `at`.
pub(crate) async fn service_addresses(
    conn: &mut PgConnection,
    service: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<HeldAddress>, StoreError> {
    sqlx::query_as(
        "SELECT a.id = ANY(s.cluster_address_ids), a.id = ANY(s.external_address_ids),
                a.domain_id, host(a.address), a.id
           FROM kube.service s
           JOIN public.address a
             ON (a.id = ANY(s.cluster_address_ids) OR a.id = ANY(s.external_address_ids))
            AND a.valid_period @> coalesce($2::timestamptz, now())
          WHERE s.id = $1 AND s.valid_period @> coalesce($2::timestamptz, now())
          ORDER BY a.address",
    )
    .bind(service)
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The living addresses the ingress `ingress` cites at `at`, as [`HeldAddress`]es of the
/// external sort: the addresses of an ingress are of one sort only.
pub(crate) async fn ingress_addresses(
    conn: &mut PgConnection,
    ingress: Uuid,
    at: Option<OffsetDateTime>,
) -> Result<Vec<HeldAddress>, StoreError> {
    sqlx::query_as(
        "SELECT false, true, a.domain_id, host(a.address), a.id
           FROM kube.ingress i
           JOIN public.address a
             ON a.id = ANY(i.address_ids)
            AND a.valid_period @> coalesce($2::timestamptz, now())
          WHERE i.id = $1 AND i.valid_period @> coalesce($2::timestamptz, now())
          ORDER BY a.address",
    )
    .bind(ingress)
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// An address a `sync` may cite without creating it (019/Q-019-17): its domain, its value
/// and its identifier.
pub(crate) type SharedAddress = (Uuid, String, Uuid);

/// The living addresses at `at` among `wanted` (domain, value) that a living service or
/// ingress of `cluster` cites: those a second holder cites instead of creating them. An
/// address no object of the cluster cites (declared by hand, or of another cluster) is
/// not among them.
pub(crate) async fn shared_addresses(
    conn: &mut PgConnection,
    cluster: Uuid,
    wanted: &[(Uuid, String)],
    at: Option<OffsetDateTime>,
) -> Result<Vec<SharedAddress>, StoreError> {
    let (domains, values): (Vec<Uuid>, Vec<String>) = wanted.iter().cloned().unzip();
    sqlx::query_as(
        "SELECT a.domain_id, host(a.address), a.id
           FROM unnest($2::uuid[], $3::text[]) AS w(domain_id, address)
           JOIN public.address a
             ON a.domain_id = w.domain_id AND a.address = w.address::inet
            AND a.valid_period @> coalesce($4::timestamptz, now())
          WHERE EXISTS (
                SELECT 1 FROM kube.service s
                  JOIN kube.namespace n
                    ON n.id = s.namespace_id AND n.cluster_id = $1
                   AND n.valid_period @> coalesce($4::timestamptz, now())
                 WHERE (s.cluster_address_ids @> ARRAY[a.id]
                        OR s.external_address_ids @> ARRAY[a.id])
                   AND s.valid_period @> coalesce($4::timestamptz, now()))
             OR EXISTS (
                SELECT 1 FROM kube.ingress i
                  JOIN kube.namespace n
                    ON n.id = i.namespace_id AND n.cluster_id = $1
                   AND n.valid_period @> coalesce($4::timestamptz, now())
                 WHERE i.address_ids @> ARRAY[a.id]
                   AND i.valid_period @> coalesce($4::timestamptz, now()))
          ORDER BY a.id",
    )
    .bind(cluster)
    .bind(domains)
    .bind(values)
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The workloads of `namespace` living at `at` among `wanted` (kind, name), with their
/// identifiers: the selection a `sync` of a service names.
pub(crate) async fn workloads_named(
    conn: &mut PgConnection,
    namespace: Uuid,
    wanted: &[(String, String)],
    at: Option<OffsetDateTime>,
) -> Result<Vec<(String, String, Uuid)>, StoreError> {
    let (kinds, names): (Vec<&str>, Vec<&str>) =
        wanted.iter().map(|(k, n)| (k.as_str(), n.as_str())).unzip();
    sqlx::query_as(
        "SELECT w.kind, w.name, w.id
           FROM kube.workload w
           JOIN unnest($2::text[], $3::text[]) AS r(kind, name)
             ON w.kind = r.kind AND w.name = r.name
          WHERE w.namespace_id = $1 AND w.valid_period @> coalesce($4::timestamptz, now())
          ORDER BY w.kind, w.name",
    )
    .bind(namespace)
    .bind(kinds)
    .bind(names)
    .bind(at)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// Whether another object of `table` than `id` matches `condition` (on the texts `$1`
/// to `$n` of `keys`) over `period`.
pub(crate) async fn taken(
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
    if table == SERVICE_TABLE || table == INGRESS_TABLE {
        let (rule, what) = if table == SERVICE_TABLE {
            (&crate::rules::SERVICE_DUPLICATE, "service")
        } else {
            (&crate::rules::INGRESS_DUPLICATE, "ingress")
        };
        if let (Some(namespace), Some(name)) = (field("namespace_id"), field("name"))
            && taken(
                conn,
                table,
                "namespace_id = $1::uuid AND name = $2",
                &[namespace, name],
                id,
                period,
            )
            .await?
        {
            return Err(refused(
                rule,
                format!("the {what} {name} is taken in the namespace over the period"),
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

/// Whether changing the service or external domain `column` (`service_domain_id` or
/// `external_domain_id`) of `cluster` to `domain` (`null` for none) over `period` moves it
/// under living services or ingresses of the cluster: they are anchored in these domains,
/// and so are their addresses (design « Types d'objets »). As `pod_domain_moved`.
pub(crate) async fn service_domain_moved(
    conn: &mut PgConnection,
    cluster: Uuid,
    column: &'static str,
    domain: &Value,
    period: Option<Period>,
) -> Result<bool, StoreError> {
    debug_assert!(matches!(column, "service_domain_id" | "external_domain_id"));
    let (from, to) = bounds(period);
    let sql = format!(
        "WITH portion AS (SELECT tstzrange(coalesce($3::timestamptz, now()), $4) AS p)
         SELECT EXISTS (SELECT 1 FROM kube.cluster c, portion
                         WHERE c.id = $1 AND c.valid_period && portion.p
                           AND c.{column} IS DISTINCT FROM ($2 #>> '{{}}')::uuid)
            AND EXISTS (SELECT 1 FROM (SELECT namespace_id, valid_period FROM {SERVICE_TABLE}
                                       UNION ALL
                                       SELECT namespace_id, valid_period FROM {INGRESS_TABLE}) o,
                               portion
                         WHERE o.valid_period && portion.p
                           AND EXISTS (SELECT 1 FROM {NAMESPACE_TABLE} n
                                        WHERE n.id = o.namespace_id AND n.cluster_id = $1))"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(cluster)
        .bind(domain)
        .bind(from)
        .bind(to)
        .fetch_one(conn)
        .await
        .map_err(|e| store(&e))
}

/// The parent of the node, namespace, workload, service or ingress `id` of `table` (its
/// cluster, or its namespace) and the domain of its cluster it is anchored in, at the
/// start of `period`: its place in the tree of the module and its anchor.
pub(crate) async fn placement(
    conn: &mut PgConnection,
    table: &str,
    id: Uuid,
    period: Option<Period>,
) -> Result<Option<(Uuid, Option<Uuid>)>, StoreError> {
    let (from, _) = bounds(period);
    // A service is anchored in the service domain of its cluster, an ingress in its
    // external domain, else its service domain (design « Types d'objets »), the other
    // objects in its pod domain.
    let anchor = if table == SERVICE_TABLE {
        "c.service_domain_id"
    } else if table == INGRESS_TABLE {
        "coalesce(c.external_domain_id, c.service_domain_id)"
    } else {
        "c.pod_domain_id"
    };
    let sql = if table == WORKLOAD_TABLE || table == SERVICE_TABLE || table == INGRESS_TABLE {
        format!(
            "SELECT w.namespace_id, {anchor}
               FROM {table} w
               LEFT JOIN kube.namespace n
                 ON n.id = w.namespace_id AND n.valid_period @> coalesce($2::timestamptz, now())
               LEFT JOIN kube.cluster c
                 ON c.id = n.cluster_id AND c.valid_period @> coalesce($2::timestamptz, now())
              WHERE w.id = $1 AND w.valid_period @> coalesce($2::timestamptz, now())"
        )
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

/// The ingresses living from now on whose routes name one of the services among `ids`, in
/// any of their versions: the ingresses a write of these services checks again
/// (`kube.ingress.backend_missing`).
pub(crate) async fn ingresses_routing_to(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT DISTINCT i.id
           FROM kube.service s
           JOIN kube.ingress i
             ON i.namespace_id = s.namespace_id
            AND i.rules @> jsonb_build_array(jsonb_build_object('service', s.name))
            AND i.valid_period && tstzrange(now(), NULL)
          WHERE s.id = ANY($1)
          ORDER BY i.id",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The routes of the ingresses among `ids`, living from now on, whose service no service
/// of their namespace living from now on, and present in the cluster (REQ-019-22), bears
/// over their version (`kube.ingress.backend_missing`): `(ingress, service name)`.
pub(crate) async fn backends_missing(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, StoreError> {
    sqlx::query_as(
        "SELECT DISTINCT i.id, r.route->>'service'
           FROM kube.ingress i
           CROSS JOIN LATERAL jsonb_array_elements(i.rules) AS r(route)
          WHERE i.id = ANY($1) AND i.valid_period && tstzrange(now(), NULL)
            AND NOT EXISTS (
                SELECT 1 FROM kube.service s
                 WHERE s.namespace_id = i.namespace_id
                   AND s.name = r.route->>'service'
                   AND s.absent_since IS NULL
                   AND s.valid_period && i.valid_period
                   AND s.valid_period && tstzrange(now(), NULL))
          ORDER BY 1, 2",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))
}

/// The mirror a `kube.sync` marked absent (REQ-019-22): its type, the instant of its
/// absence and its dependents, `(type, identifier)`.
pub(crate) type Absent = (Uuid, String, String, Vec<(String, Uuid)>);

/// The mirrors among `ids` and those they cite in one of their versions (the workloads a
/// service selects, the services an ingress routes to, the namespace of a workload, a
/// service or an ingress), absent now, with the objects living from now on that cite them:
/// the services that select a workload, the ingresses that route to a service by its name,
/// the workloads, services and ingresses of a namespace, and the objects of every type that
/// carry a reference property to it (core, 004).
pub(crate) async fn absent(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<Absent>, StoreError> {
    // An absent mirror, its type and instant, once per dependent (none: one row).
    type Row = (Uuid, String, String, Option<String>, Option<Uuid>);
    let rows: Vec<Row> = sqlx::query_as(
        "WITH cited (id) AS (
             SELECT unnest($1::uuid[])
             UNION SELECT unnest(workload_ids) FROM kube.service WHERE id = ANY($1)
             UNION SELECT s.id FROM kube.ingress i JOIN kube.service s
                      ON s.namespace_id = i.namespace_id
                     AND i.rules @> jsonb_build_array(jsonb_build_object('service', s.name))
                   WHERE i.id = ANY($1)
             UNION SELECT namespace_id FROM kube.workload WHERE id = ANY($1)
             UNION SELECT namespace_id FROM kube.service WHERE id = ANY($1)
             UNION SELECT namespace_id FROM kube.ingress WHERE id = ANY($1)),
         mirror (id, kind, since, namespace_id, name) AS (
             SELECT id, 'kube.node', absent_since, NULL::uuid, name FROM kube.node
              WHERE id IN (SELECT id FROM cited)
                AND valid_period @> now() AND absent_since IS NOT NULL
             UNION ALL SELECT id, 'kube.namespace', absent_since, NULL, name FROM kube.namespace
              WHERE id IN (SELECT id FROM cited)
                AND valid_period @> now() AND absent_since IS NOT NULL
             UNION ALL SELECT id, 'kube.workload', absent_since, namespace_id, name
               FROM kube.workload
              WHERE id IN (SELECT id FROM cited)
                AND valid_period @> now() AND absent_since IS NOT NULL
             UNION ALL SELECT id, 'kube.service', absent_since, namespace_id, name
               FROM kube.service
              WHERE id IN (SELECT id FROM cited)
                AND valid_period @> now() AND absent_since IS NOT NULL
             UNION ALL SELECT id, 'kube.ingress', absent_since, namespace_id, name
               FROM kube.ingress
              WHERE id IN (SELECT id FROM cited)
                AND valid_period @> now() AND absent_since IS NOT NULL)
         SELECT m.id, m.kind, m.since::text, d.kind, d.id
           FROM mirror m
           LEFT JOIN LATERAL (
                SELECT 'kube.service' AS kind, s.id FROM kube.service s
                 WHERE m.kind = 'kube.workload' AND s.workload_ids @> ARRAY[m.id]
                   AND s.valid_period && tstzrange(now(), NULL)
                UNION SELECT 'kube.ingress', i.id FROM kube.ingress i
                 WHERE m.kind = 'kube.service' AND i.namespace_id = m.namespace_id
                   AND i.rules @> jsonb_build_array(jsonb_build_object('service', m.name))
                   AND i.valid_period && tstzrange(now(), NULL)
                UNION SELECT 'kube.workload', w.id FROM kube.workload w
                 WHERE m.kind = 'kube.namespace' AND w.namespace_id = m.id
                   AND w.valid_period && tstzrange(now(), NULL)
                UNION SELECT 'kube.service', s.id FROM kube.service s
                 WHERE m.kind = 'kube.namespace' AND s.namespace_id = m.id
                   AND s.valid_period && tstzrange(now(), NULL)
                UNION SELECT 'kube.ingress', i.id FROM kube.ingress i
                 WHERE m.kind = 'kube.namespace' AND i.namespace_id = m.id
                   AND i.valid_period && tstzrange(now(), NULL)
                UNION SELECT a.object_type, a.object_id FROM public.property_assignment a
                  JOIN public.property_declaration p
                    ON p.id = a.property_id AND p.value_type = 'ref'
                 WHERE a.kind = 'value' AND (a.v_uuid = m.id OR m.id = ANY(a.v_uuids))
                   AND a.valid_period && tstzrange(now(), NULL)) d ON true
          ORDER BY m.id, d.kind, d.id",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    let mut found: Vec<Absent> = Vec::new();
    for (id, kind, since, dependent, of) in rows {
        if found.last().is_none_or(|(last, ..)| *last != id) {
            found.push((id, kind, since, Vec::new()));
        }
        if let (Some(dependent), Some(of), Some((.., dependents))) =
            (dependent, of, found.last_mut())
        {
            dependents.push((dependent, of));
        }
    }
    Ok(found)
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
    use crate::module::{
        ADDRESS, ADMITTED_VALUES, CLUSTER, DOMAIN, INGRESS, MACHINE, NAMESPACE, NODE, PREFIX,
        SERVICE, WORKLOAD,
    };
    let sql = match (from, to) {
        (ADMITTED_VALUES, NAMESPACE) => {
            "SELECT DISTINCT id FROM kube.admitted_values WHERE namespace_id = $1 AND {live}"
        }
        (INGRESS, NAMESPACE) => {
            "SELECT DISTINCT id FROM kube.ingress WHERE namespace_id = $1 AND {live}"
        }
        (INGRESS, ADDRESS) => {
            "SELECT DISTINCT id FROM kube.ingress WHERE address_ids @> ARRAY[$1::uuid] AND {live}"
        }
        (SERVICE, NAMESPACE) => {
            "SELECT DISTINCT id FROM kube.service WHERE namespace_id = $1 AND {live}"
        }
        (SERVICE, ADDRESS) => {
            "SELECT DISTINCT id FROM kube.service
              WHERE (cluster_address_ids @> ARRAY[$1::uuid]
                     OR external_address_ids @> ARRAY[$1::uuid])
                AND {live}"
        }
        (SERVICE, WORKLOAD) => {
            "SELECT DISTINCT id FROM kube.service WHERE workload_ids @> ARRAY[$1::uuid] AND {live}"
        }
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

/// The mirrors of the section `kube.objects`, in the order of its rows: their type, their
/// table, and what a version of each shows (design « Section de différentiel », Q-019-19):
/// a closed list of fields under stable names, its identity, its tracked fields and
/// `absent_since` ([`ABSENT_SINCE`]), whatever the columns.
pub(crate) const MIRRORS: [(&str, &str, &str); 5] = [
    (
        crate::module::NODE,
        NODE_TABLE,
        "jsonb_build_object('cluster', cluster_id, 'name', name, 'uid', uid,
                            'machine', machine_id, 'addresses', addresses,
                            'pod_networks', range_ids, 'zone', zone,
                            'absent_since', {absent})",
    ),
    (
        crate::module::NAMESPACE,
        NAMESPACE_TABLE,
        "jsonb_build_object('cluster', cluster_id, 'name', name, 'uid', uid,
                            'labels', labels, 'absent_since', {absent})",
    ),
    (
        crate::module::WORKLOAD,
        WORKLOAD_TABLE,
        "jsonb_build_object('namespace', namespace_id, 'kind', kind, 'name', name, 'uid', uid,
                            'labels', labels, 'service_account', service_account,
                            'ports', ports, 'host_network', host_network,
                            'absent_since', {absent})",
    ),
    (
        crate::module::SERVICE,
        SERVICE_TABLE,
        "jsonb_build_object('namespace', namespace_id, 'name', name, 'uid', uid,
                            'service_type', type, 'headless', headless,
                            'external_name', external_name, 'ports', ports,
                            'cluster_addresses', cluster_address_ids,
                            'external_addresses', external_address_ids,
                            'workloads', workload_ids, 'absent_since', {absent})",
    ),
    (
        crate::module::INGRESS,
        INGRESS_TABLE,
        "jsonb_build_object('namespace', namespace_id, 'name', name, 'uid', uid,
                            'class', class, 'rules', rules, 'tls_hosts', tls_hosts,
                            'addresses', address_ids, 'absent_since', {absent})",
    ),
];

/// `absent_since` of a version of the section as the REST interface writes an instant:
/// RFC 3339 in UTC (`Z`), the fraction of a second without its trailing zeros, none when
/// whole, whatever the time zone of the session.
const ABSENT_SINCE: &str = "to_char(absent_since AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS')
     || coalesce('.' || nullif(rtrim(to_char(absent_since AT TIME ZONE 'UTC', 'US'), '0'), ''),
                 '')
     || 'Z'";

/// What the mirrors among `ids` were before the current transaction and are after it, as
/// rows of the section `kube.objects` (design « Section de différentiel »): per mirror,
/// each portion of time where its identity, tracked fields and absence differ, created
/// (`before` none), closed (`after` none), changed or marked absent or present again
/// (`absent_since`). Before is read from the versions asserted before the transaction,
/// after from the current ones, as `machines.attachments` does; the periods of each side
/// are aggregated once per mirror.
pub(crate) async fn object_changes(
    conn: &mut PgConnection,
    ids: &[Uuid],
) -> Result<Vec<kartenn_service::module::DiffRow>, StoreError> {
    type Change = (Uuid, String, Option<Value>, Option<Value>, i64, Option<i64>);
    let versions = MIRRORS
        .iter()
        .enumerate()
        .map(|(rank, (kind, table, state))| {
            let state = state.replace("{absent}", ABSENT_SINCE);
            format!(
                "SELECT {rank} AS rank, id, '{kind}' AS kind, {state} AS state, valid_period,
                        sys_start < now() AS before, true AS after
                   FROM {table} WHERE id = ANY($1)
                 UNION ALL
                 SELECT {rank}, id, '{kind}', {state}, valid_period, true, false
                   FROM {table}_history
                  WHERE id = ANY($1) AND sys_end = now() AND sys_start < now()"
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let sql = format!(
        "WITH versions AS ({versions}),
         b AS (SELECT rank, id, kind, state, valid_period FROM versions WHERE before),
         f AS (SELECT rank, id, kind, state, valid_period FROM versions WHERE after),
         br AS (SELECT id, range_agg(valid_period) AS r FROM b GROUP BY id),
         fr AS (SELECT id, range_agg(valid_period) AS r FROM f GROUP BY id),
         portions AS (
             SELECT b.rank, b.id, b.kind, b.state AS bs, f.state AS fs,
                    b.valid_period * f.valid_period AS period
               FROM b JOIN f ON f.id = b.id AND f.valid_period && b.valid_period
              WHERE b.state <> f.state
             UNION ALL
             SELECT b.rank, b.id, b.kind, b.state, NULL, x
               FROM b LEFT JOIN fr USING (id),
                    unnest(multirange(b.valid_period)
                           - coalesce(fr.r, '{{}}'::tstzmultirange)) AS x
             UNION ALL
             SELECT f.rank, f.id, f.kind, NULL, f.state, x
               FROM f LEFT JOIN br USING (id),
                    unnest(multirange(f.valid_period)
                           - coalesce(br.r, '{{}}'::tstzmultirange)) AS x)
         SELECT id, kind, bs, fs,
                (extract(epoch FROM lower(period)) * 1000000)::bigint,
                (extract(epoch FROM upper(period)) * 1000000)::bigint
           FROM portions
          ORDER BY rank, id, lower(period)"
    );
    let found: Vec<Change> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(ids)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))?;
    Ok(found
        .into_iter()
        .map(
            |(object, kind, before, after, start, end)| kartenn_service::module::DiffRow {
                object,
                kind,
                before,
                after,
                period: Period::new(Instant(start), end.map(Instant)).ok(),
            },
        )
        .collect())
}
