//! The read port of the module (spec 019, design « Port de lecture »): the reads of the
//! journal of the pods observed (REQ-019-16, 17; ADR 0043, decisions 6, 8; Q-019-23, 25;
//! task 019-6d), the chain of execution of a workload, framed as those reads (REQ-019-11;
//! Q-019-31; task 019-7b), and the workloads of a namespace, a service or a cluster with
//! the values of their axes (REQ-019-32, 019/X11; task 019-7a). The journal has no dated read
//! (`as_of`): it answers what is stored, within the retention of each cluster, and a
//! question about an instant past that retention answers nothing, with the reason
//! [`Cut::Retention`], whether or not the purge has dropped the rows yet. The workloads
//! and their values are read at (`valid_at`, `as_of`), as the reads of the referential.

use std::net::IpAddr;

use kartenn_core::addr::Prefix;
use kartenn_core::period::Instant;
use kartenn_service::StoreError;
use kartenn_service::audit::{AuditJournal, RequestContext};
use kartenn_service::authz::{Authorizer, Subject};
use kartenn_service::reads::{ReadAudit, objects_read};
use serde_json::Value;
use sqlx::PgConnection;
use std::time::Duration;

use sqlx::types::time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::edges::{snapshot, time};
use crate::module::OBSERVATION_READ_PERMISSION;
use crate::observations::JOURNAL;
use crate::pg::{CLUSTER_TABLE, NAMESPACE_TABLE, SERVICE_TABLE, WORKLOAD_TABLE, store};

/// The edge from a node to its machine (REQ-019-11).
const RUNS_ON: &str = "kube.runs_on";
/// The edge from a machine to the machine that hosts it (REQ-009-17).
const HOSTED_ON: &str = "machines.hosted_on";
/// The edge from a machine to the cluster of machines that hosts it (REQ-009-17).
const HOSTED_IN: &str = "machines.hosted_in";

/// A row of the journal: a pod seen with an IP, or without one in `hostNetwork`, over a
/// period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// The cluster that observed it.
    pub cluster: Uuid,
    /// Its namespace, by name.
    pub namespace: String,
    /// Its name.
    pub pod_name: String,
    /// Its Kubernetes UID.
    pub pod_uid: Uuid,
    /// The workload its `ownerReferences` climb to.
    pub workload: Option<Uuid>,
    /// The node it ran on.
    pub node: Option<Uuid>,
    /// Its IP; none in `hostNetwork`, where the address is the node's.
    pub ip: Option<IpAddr>,
    /// Whether it shared the network of its node.
    pub host_network: bool,
    /// Since when it was seen.
    pub from: OffsetDateTime,
    /// When it was gone; `None` while it is still seen.
    pub until: Option<OffsetDateTime>,
    /// The batch that opened the observation.
    pub opened_batch: Uuid,
    /// The batch that closed it.
    pub closed_batch: Option<Uuid>,
    /// Its IP as the database writes it (`host`), empty without one: the text the order
    /// of the reads compares, which the text of [`IpAddr`] may not be (rare IPv6 forms).
    ip_text: String,
}

impl Observation {
    /// Its key in the order of the reads: its start, its pod, its IP.
    #[must_use]
    pub fn key(&self) -> Key {
        Key {
            from: self.from,
            pod_uid: self.pod_uid,
            ip: self.ip_text.clone(),
        }
    }
}

/// The position of an observation in the order of the reads, after which a page starts:
/// its start, its pod, then its IP (empty in `hostNetwork`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// The start of its period.
    pub from: OffsetDateTime,
    /// The UID of its pod.
    pub pod_uid: Uuid,
    /// Its IP as text, empty without one.
    pub ip: String,
}

/// Why an answer is empty or partial, though rows may still be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    /// The instant asked about is older than the retention of the cluster (ADR 0043,
    /// decision 8).
    Retention,
    /// The cluster is closed: its rows may still be stored, within its retention, but a
    /// closed cluster is not read (Q-019-28).
    Closed,
    /// The journal holds no observation of the workload within the retention of its
    /// cluster: [`execution_chain`] only (Q-019-31).
    Unobserved,
}

impl Cut {
    /// Its name in the answers of the routes (`reason`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Retention => "retention",
            Self::Closed => "closed",
            Self::Unobserved => "unobserved",
        }
    }
}

/// What [`who_had`] answers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answer {
    /// The observations, in the order of [`Key`].
    pub observations: Vec<Observation>,
    /// Why clusters of the domain the reader sees answered nothing (Q-019-28): a cluster
    /// for which the instant is past its retention gives [`Cut::Retention`]; a closed
    /// cluster that still lived at the instant, within its retention, gives
    /// [`Cut::Closed`]. [`Cut::Closed`] prevails over [`Cut::Retention`] when both occur:
    /// it says that rows are kept but not served.
    pub reason: Option<Cut>,
}

/// Whether `at` is within `retention_days` days before `now`: the observations that
/// held at `at` are then all kept, their retention running from their end.
#[must_use]
pub fn within_retention(at: OffsetDateTime, now: OffsetDateTime, retention_days: i16) -> bool {
    let days = u64::from(retention_days.unsigned_abs());
    at >= now - Duration::from_secs(days * 86_400)
}

type Row = (
    Uuid,
    String,
    String,
    Uuid,
    Option<Uuid>,
    Option<Uuid>,
    Option<String>,
    bool,
    OffsetDateTime,
    Option<OffsetDateTime>,
    Uuid,
    Option<Uuid>,
);

const COLUMNS: &str = "o.cluster_id, o.namespace, o.pod_name, o.pod_uid, o.workload_id, \
    o.node_id, host(o.ip), o.host_network, lower(o.observed), upper(o.observed), \
    o.opened_batch, o.closed_batch";

/// The keyset condition of a page, on the parameters `$a`, `$b`, `$c` of [`Key`].
fn after(a: u8, b: u8, c: u8) -> String {
    format!(
        "(${a}::timestamptz IS NULL OR (lower(o.observed), o.pod_uid, coalesce(host(o.ip), ''))
              > (${a}::timestamptz, ${b}::uuid, ${c}::text))"
    )
}

fn observation(row: Row) -> Observation {
    let (cluster, namespace, pod_name, pod_uid, workload, node, ip, host_network) =
        (row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7);
    Observation {
        cluster,
        namespace,
        pod_name,
        pod_uid,
        workload,
        node,
        // `host` of an `inet` of the journal always reads back as an address.
        ip: ip.as_deref().and_then(|ip| ip.parse().ok()),
        host_network,
        from: row.8,
        until: row.9,
        opened_batch: row.10,
        closed_batch: row.11,
        ip_text: ip.unwrap_or_default(),
    }
}

/// The observations of the clusters `clusters` that held an address of `network` at
/// `at`, in the order of [`Key`], after `after`, at most `limit` of them (all without
/// one): a pod with one of these IPs, or a pod in `hostNetwork` on a node that had one of
/// them among its addresses at `at`. The retention is the caller's to check
/// ([`within_retention`]).
///
/// # Errors
///
/// The database.
pub async fn held(
    conn: &mut PgConnection,
    clusters: &[Uuid],
    network: Prefix,
    at: OffsetDateTime,
    after_key: Option<&Key>,
    limit: Option<u16>,
) -> Result<Vec<Observation>, sqlx::Error> {
    let keyset = after(4, 5, 6);
    // Two branches, each on the index (cluster, IP, period): the IPs of the pods, then
    // the pods in `hostNetwork` (no IP) whose node had an address of the network.
    let sql = format!(
        "SELECT * FROM (
             SELECT {COLUMNS} FROM kube.pod_observation o
              WHERE o.cluster_id = ANY($1) AND o.ip <<= $2::inet AND o.observed @> $3
                AND {keyset}
             UNION ALL
             SELECT {COLUMNS} FROM kube.pod_observation o
              WHERE o.cluster_id = ANY($1) AND o.ip IS NULL AND o.observed @> $3
                AND {keyset}
                AND EXISTS (SELECT 1 FROM kube.node n, jsonb_array_elements(n.addresses) a
                             WHERE n.id = o.node_id AND n.valid_period @> $3
                               AND CASE WHEN pg_input_is_valid(a->>'value', 'inet')
                                        THEN (a->>'value')::inet <<= $2::inet END)
         ) o (cluster_id, namespace, pod_name, pod_uid, workload_id, node_id, ip,
              host_network, from_, until, opened_batch, closed_batch)
         ORDER BY from_, pod_uid, coalesce(ip, '')
         LIMIT $7"
    );
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(clusters)
        .bind(network.to_string())
        .bind(at)
        .bind(after_key.map(|k| k.from))
        .bind(after_key.map(|k| k.pod_uid))
        .bind(after_key.map(|k| k.ip.as_str()))
        .bind(limit.map(i64::from))
        .fetch_all(conn)
        .await?;
    Ok(rows.into_iter().map(observation).collect())
}

/// The observations of the workload `workload` still within the retention of its
/// cluster at `now` (open, or closed less than `retention_days` days before `now`): the
/// nodes and the addresses it was seen on (REQ-019-17), in the order of [`Key`], after
/// `after`, at most `limit` of them.
///
/// # Errors
///
/// The database.
pub async fn of_workload(
    conn: &mut PgConnection,
    workload: Uuid,
    now: OffsetDateTime,
    retention_days: i16,
    after_key: Option<&Key>,
    limit: Option<u16>,
) -> Result<Vec<Observation>, sqlx::Error> {
    let keyset = after(4, 5, 6);
    let sql = format!(
        "SELECT {COLUMNS} FROM kube.pod_observation o
          WHERE o.workload_id = $1
            AND (upper_inf(o.observed)
                 OR upper(o.observed) >= $2::timestamptz - make_interval(days => $3))
            AND {keyset}
          ORDER BY lower(o.observed), o.pod_uid, coalesce(host(o.ip), '')
          LIMIT $7"
    );
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(workload)
        .bind(now)
        .bind(i32::from(retention_days))
        .bind(after_key.map(|k| k.from))
        .bind(after_key.map(|k| k.pod_uid))
        .bind(after_key.map(|k| k.ip.as_str()))
        .bind(limit.map(i64::from))
        .fetch_all(conn)
        .await?;
    Ok(rows.into_iter().map(observation).collect())
}

/// The observations of the workload `workload` that held at `at`, in the order of
/// [`Key`]: the index `(workload_id, observed)` of each partition finds the rows of the
/// workload, filtered on their period. Such an observation ends after `at`: when `at` is
/// within the retention of the cluster, so is it. The retention is the caller's to check
/// ([`within_retention`]).
///
/// # Errors
///
/// The database.
pub async fn workload_at(
    conn: &mut PgConnection,
    workload: Uuid,
    at: OffsetDateTime,
) -> Result<Vec<Observation>, sqlx::Error> {
    let sql = format!(
        "SELECT {COLUMNS} FROM kube.pod_observation o
          WHERE o.workload_id = $1 AND o.observed @> $2::timestamptz
          ORDER BY lower(o.observed), o.pod_uid, coalesce(host(o.ip), '')"
    );
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(workload)
        .bind(at)
        .fetch_all(conn)
        .await?;
    Ok(rows.into_iter().map(observation).collect())
}

/// Who had an address of `network` at `at` among the clusters whose pod domain is
/// `domain` (design « Port de lecture »): the observations of those clusters, provided
/// `reader` holds `kube.observation.read` on that domain (Q-019-23; ADR 0043, decision 6),
/// nothing otherwise. Each cluster is taken in its last version begun: a living one is
/// read; one for which `at` is past its retention answers nothing and sets
/// [`Cut::Retention`] (Q-019-25); a closed one that still lived at `at` answers nothing
/// and sets [`Cut::Closed`] (Q-019-28), which prevails. It journals nothing: the caller
/// journals the read (`record_read`, REQ-012-18), as the routes do.
///
/// # Errors
///
/// The database or the authorizer.
pub async fn who_had(
    conn: &mut PgConnection,
    authorizer: &impl Authorizer,
    reader: &Subject,
    network: Prefix,
    domain: Uuid,
    at: OffsetDateTime,
) -> Result<Answer, StoreError> {
    let denied = authorizer
        .authorize_many(reader, OBSERVATION_READ_PERMISSION, &[domain])
        .await?;
    if !denied.is_empty() {
        return Ok(Answer::default());
    }
    // Each cluster in its last version begun, if its pod domain is `domain`: whether it
    // lives, and when it was closed.
    let clusters: Vec<(Uuid, i16, bool, Option<OffsetDateTime>, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, retention_days, valid_period @> now(), upper(valid_period), now()
               FROM (SELECT DISTINCT ON (id) * FROM kube.cluster
                      WHERE id IN (SELECT id FROM kube.cluster WHERE pod_domain_id = $1)
                        AND lower(valid_period) <= now()
                      ORDER BY id, lower(valid_period) DESC) c
              WHERE pod_domain_id = $1
              ORDER BY id",
    )
    .bind(domain)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut kept = Vec::new();
    let mut reason = None;
    for (id, days, living, end, now) in clusters {
        if !within_retention(at, now, days) {
            reason = reason.or(Some(Cut::Retention));
        } else if living {
            kept.push(id);
        } else if end.is_some_and(|end| at < end) {
            reason = Some(Cut::Closed);
        }
    }
    let observations = if kept.is_empty() {
        Vec::new()
    } else {
        held(conn, &kept, network, at, None, None)
            .await
            .map_err(|e| store(&e))?
    };
    Ok(Answer {
        observations,
        reason,
    })
}

/// A workload as its journal is read: the pod domain and the retention of its cluster,
/// whether the workload and its cluster both live now (each in its last version begun),
/// and the instant of the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Framed {
    /// The pod domain of its cluster, where `kube.observation.read` is held.
    pub pod_domain: Uuid,
    /// The retention of its cluster, in days.
    pub retention_days: i16,
    /// Whether the workload and its cluster live now: a closed one is not read
    /// (Q-019-28).
    pub living: bool,
    /// Now, as the database reads it.
    pub now: OffsetDateTime,
}

/// The frame of the reads of the journal of the workload `workload` (see [`Framed`]);
/// `None` for a workload never begun. It reads under no right: the caller authorizes the
/// read on [`Framed::pod_domain`], and answers nothing of the frame before.
///
/// # Errors
///
/// The database.
pub async fn framed(
    executor: impl sqlx::PgExecutor<'_>,
    workload: Uuid,
) -> Result<Option<Framed>, sqlx::Error> {
    // The workload, its namespace and its cluster, each in its last version begun.
    let row: Option<(Uuid, i16, bool, OffsetDateTime)> = sqlx::query_as(
        "SELECT c.pod_domain_id, c.retention_days,
                w.valid_period @> now() AND c.valid_period @> now(), now()
           FROM (SELECT namespace_id, valid_period FROM kube.workload
                  WHERE id = $1 AND lower(valid_period) <= now()
                  ORDER BY lower(valid_period) DESC LIMIT 1) w
          CROSS JOIN LATERAL (
                SELECT cluster_id FROM kube.namespace
                 WHERE id = w.namespace_id AND lower(valid_period) <= now()
                 ORDER BY lower(valid_period) DESC LIMIT 1) n
          CROSS JOIN LATERAL (
                SELECT pod_domain_id, retention_days, valid_period FROM kube.cluster
                 WHERE id = n.cluster_id AND lower(valid_period) <= now()
                 ORDER BY lower(valid_period) DESC LIMIT 1) c",
    )
    .bind(workload)
    .fetch_optional(executor)
    .await?;
    Ok(row.map(|(pod_domain, retention_days, living, now)| Framed {
        pod_domain,
        retention_days,
        living,
        now,
    }))
}

/// A link of the chain of execution of a workload (REQ-019-11): a node it was seen
/// running on at the date, the machine the node runs on (`kube.runs_on`), and what hosts
/// that machine (`machines.hosted_on`, `machines.hosted_in`), all at the date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The node, from the journal.
    pub node: Uuid,
    /// Its machine; none for a node matched to no machine.
    pub machine: Option<Uuid>,
    /// The hypervisor that hosts the machine.
    pub hypervisor: Option<Uuid>,
    /// The virtualisation cluster that hosts the machine.
    pub cluster: Option<Uuid>,
}

/// What [`execution_chain`] answers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chain {
    /// One link per node, by node.
    pub links: Vec<Link>,
    /// Why the chain is empty though the workload may have run (Q-019-31):
    /// [`Cut::Closed`] for a closed workload or cluster, [`Cut::Retention`] for a date
    /// past the retention of its cluster, [`Cut::Unobserved`] for a workload the journal
    /// never saw within that retention.
    pub reason: Option<Cut>,
}

/// Who reads, and where the read is journaled: the request, its journal and the bound of
/// the identifiers it records (REQ-012-18).
#[derive(Debug)]
pub struct Reading<'a, J> {
    /// The request; its subject is the reader.
    pub context: &'a RequestContext,
    /// The journal of the read.
    pub journal: &'a J,
    /// The bound of the identifiers recorded.
    pub audit: ReadAudit,
}

/// The chain of execution of the workload `workload` at `valid_at` (REQ-019-11; design
/// « Port de lecture »; Q-019-31): the nodes the journal saw it running on at that date
/// (its observations within the retention of its cluster, [`of_workload`]), then the
/// machine of each node and what hosts that machine, by the edges of the modules at that
/// date. Framed as [`who_had`]: `None` for a workload never begun, or when the reader does
/// not hold `kube.observation.read` on the pod domain of its cluster; a closed workload or
/// cluster answers an empty chain with [`Cut::Closed`], unjournaled, as the routes answer
/// 410 without journaling; a date past the retention, [`Cut::Retention`]; a workload
/// never observed within the retention, [`Cut::Unobserved`]. Every other read, empty or
/// not, is journaled as the routes do (`data.objects.read`, the pods it relied on).
///
/// # Errors
///
/// The database, the authorizer, or the journal: then nothing is answered.
pub async fn execution_chain(
    conn: &mut PgConnection,
    authorizer: &impl Authorizer,
    reading: Reading<'_, impl AuditJournal>,
    workload: Uuid,
    valid_at: OffsetDateTime,
) -> Result<Option<Chain>, StoreError> {
    let Some(frame) = framed(&mut *conn, workload).await.map_err(|e| store(&e))? else {
        return Ok(None);
    };
    // Kartenn itself is no reader: it holds no right on the journal.
    let Some(reader) = reading.context.subject() else {
        return Ok(None);
    };
    let denied = authorizer
        .authorize_many(&reader, OBSERVATION_READ_PERMISSION, &[frame.pod_domain])
        .await?;
    if !denied.is_empty() {
        return Ok(None);
    }
    if !frame.living {
        return Ok(Some(Chain {
            links: Vec::new(),
            reason: Some(Cut::Closed),
        }));
    }
    let (pods, chain) = if within_retention(valid_at, frame.now, frame.retention_days) {
        // Whether the journal saw it at all within the retention: one row is enough.
        let any = of_workload(
            conn,
            workload,
            frame.now,
            frame.retention_days,
            None,
            Some(1),
        )
        .await
        .map_err(|e| store(&e))?;
        if any.is_empty() {
            let chain = Chain {
                links: Vec::new(),
                reason: Some(Cut::Unobserved),
            };
            (Vec::new(), chain)
        } else {
            let held = workload_at(conn, workload, valid_at)
                .await
                .map_err(|e| store(&e))?;
            linked(conn, &held, valid_at).await?
        }
    } else {
        let chain = Chain {
            links: Vec::new(),
            reason: Some(Cut::Retention),
        };
        (Vec::new(), chain)
    };
    let at = valid_at
        .format(&Rfc3339)
        .map_err(|e| StoreError(e.to_string()))?;
    let criteria = serde_json::json!({
        "read": "kube.execution_chain",
        "workload": workload,
        "valid_at": at,
    });
    let event = objects_read(reading.context, JOURNAL, &pods, criteria, reading.audit)
        .map_err(|e| StoreError(e.to_string()))?;
    reading.journal.record(&event).await?;
    Ok(Some(chain))
}

/// The links of the observations `held` at `valid_at`, and the pods they come from,
/// sorted: a pod seen without a node counts among the pods read, but makes no link.
async fn linked(
    conn: &mut PgConnection,
    held: &[Observation],
    valid_at: OffsetDateTime,
) -> Result<(Vec<Uuid>, Chain), StoreError> {
    let mut pods: Vec<Uuid> = held.iter().map(|o| o.pod_uid).collect();
    pods.sort_unstable();
    pods.dedup();
    let mut nodes: Vec<Uuid> = held.iter().filter_map(|o| o.node).collect();
    nodes.sort_unstable();
    nodes.dedup();
    let at = crate::edges::instant(valid_at);
    let mut links = Vec::with_capacity(nodes.len());
    for node in nodes {
        let machine = crate::edges::of(conn, node, at, None)
            .await?
            .into_iter()
            .find(|e| e.name == RUNS_ON && e.from == node)
            .map(|e| e.to);
        let (mut hypervisor, mut cluster) = (None, None);
        if let Some(machine) = machine {
            for edge in kartenn_machines::read::edges_of(conn, machine, at, None).await? {
                if edge.from != machine {
                    continue;
                }
                match edge.name {
                    HOSTED_ON => hypervisor = Some(edge.to),
                    HOSTED_IN => cluster = Some(edge.to),
                    _ => {}
                }
            }
        }
        links.push(Link {
            node,
            machine,
            hypervisor,
            cluster,
        });
    }
    Ok((
        pods,
        Chain {
            links,
            reason: None,
        },
    ))
}

/// What [`workloads_of`] lists the workloads of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The workloads of a namespace.
    Namespace(Uuid),
    /// The workloads a service selects.
    Service(Uuid),
    /// The workloads of the namespaces of a cluster.
    Cluster(Uuid),
}

/// A workload, as [`workloads_of`] lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workload {
    /// The workload.
    pub id: Uuid,
    /// Its namespace.
    pub namespace: Uuid,
    /// The cluster of its namespace.
    pub cluster: Uuid,
    /// Its kind (`deployment`, `statefulset`…).
    pub kind: String,
    /// Its name.
    pub name: String,
}

/// The workloads of `scope` at (`valid_at`, `as_of`), by namespace name, then name and
/// kind: none for an object that is not of the kind `scope` names, or not living then.
///
/// # Errors
///
/// The database.
pub async fn workloads_of(
    conn: &mut PgConnection,
    scope: Scope,
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<Workload>, StoreError> {
    let workload = snapshot(WORKLOAD_TABLE, "id, namespace_id, kind, name");
    let namespace = snapshot(NAMESPACE_TABLE, "id, cluster_id, name");
    let (filter, id) = match scope {
        Scope::Namespace(id) => ("w.namespace_id = $3".to_owned(), id),
        Scope::Cluster(id) => ("n.cluster_id = $3".to_owned(), id),
        Scope::Service(id) => (
            format!(
                "w.id IN (SELECT unnest(s.workload_ids) FROM {} s WHERE s.id = $3)",
                snapshot(SERVICE_TABLE, "id, workload_ids")
            ),
            id,
        ),
    };
    let rows: Vec<(Uuid, Uuid, Uuid, String, String)> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT w.id, w.namespace_id, n.cluster_id, w.kind, w.name
               FROM {workload} w JOIN {namespace} n ON n.id = w.namespace_id
              WHERE {filter}
              ORDER BY n.name, w.name, w.kind"
        )))
        .bind(time(valid_at))
        .bind(as_of.map(time))
        .bind(id)
        .fetch_all(conn)
        .await
        .map_err(|e| store(&e))?;
    Ok(rows
        .into_iter()
        .map(|(id, namespace, cluster, kind, name)| Workload {
            id,
            namespace,
            cluster,
            kind,
            name,
        })
        .collect())
}

/// The carrier a value of an axis of a workload comes from (REQ-019-32): the workload,
/// its namespace or its cluster along the tree of the module, else the addressing tree
/// on the pod containers of its cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An assignment of the workload.
    Workload,
    /// Inherited from its namespace.
    Namespace,
    /// Inherited from its cluster.
    Cluster,
    /// The value of the addressing tree on a pod container of its cluster, or on its pod
    /// domain when it declares no container.
    Tree,
}

impl Source {
    /// Its name in an explanation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workload => "workload",
            Self::Namespace => "namespace",
            Self::Cluster => "cluster",
            Self::Tree => "tree",
        }
    }
}

/// A value of an axis of a workload, with what explains it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sourced {
    /// The value as JSON (closed-list keys), a set for a property of several values.
    pub value: Value,
    /// The carrier it comes from.
    pub source: Source,
    /// The object carrying the assignment: the workload, its namespace or its cluster,
    /// or for [`Source::Tree`] the container or one of its ancestors.
    pub origin: Uuid,
    /// The object it was resolved for: the workload, or for [`Source::Tree`] the pod
    /// container (the pod domain without one).
    pub through: Uuid,
}

/// The values of an axis of a workload.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisValues {
    /// The key of the property.
    pub key: String,
    /// One value from the tree of the module; failing one, the value of each pod
    /// container of the cluster that has one, in the order of the cluster (their union is
    /// the reader's, as REQ-010-31 unites the machines of an address); else none.
    pub values: Vec<Sourced>,
}

/// A value of [`axis_values`] as read: workload, place (0 for the workload, then the
/// rank of a pod container or of the pod domain), object resolved, key, origin, value,
/// namespace, cluster.
type AxisRow = (
    Uuid,
    i64,
    Uuid,
    String,
    Option<Uuid>,
    Option<Value>,
    Uuid,
    Uuid,
);

/// The values of the properties `keys` of each of `workloads`, in their order, at
/// (`valid_at`, `as_of`) (REQ-019-32, design « Propriétés »): the value resolved along the
/// tree of the module (the workload, else its namespace, else its cluster, as the origins
/// of the core give them for a heritable property; the workload alone otherwise), else
/// the value of the addressing tree on each pod container of its cluster, or on its pod
/// domain when it declares none: the referential, never the ranges the journal observes
/// the pods on (Q-019-30). A workload not
/// living then has no axis; a key not declared then is absent; a key without a value has
/// no value. One query for the batch.
///
/// # Errors
///
/// The database.
pub async fn axis_values(
    conn: &mut PgConnection,
    workloads: &[Uuid],
    keys: &[&str],
    valid_at: Instant,
    as_of: Option<Instant>,
) -> Result<Vec<(Uuid, Vec<AxisValues>)>, StoreError> {
    let workload = snapshot(WORKLOAD_TABLE, "id, namespace_id");
    let namespace = snapshot(NAMESPACE_TABLE, "id, cluster_id");
    let cluster = snapshot(CLUSTER_TABLE, "id, pod_domain_id, pod_container_ids");
    let declaration = snapshot(
        "public.property_declaration",
        "id, key, value_type, inherit_tree",
    );
    let origin = snapshot(
        "public.property_origin",
        "object_id, property_id, origin_object_id",
    );
    let assignment = snapshot(
        "public.property_assignment",
        "object_id, property_id, kind, v_uuid, v_uuids, v_text, v_texts, v_int, v_ints, \
         v_bool, v_date, v_dates, v_json, v_jsons",
    );
    let value = snapshot("public.property_enum_value", "id, key");
    let keys: Vec<String> = keys.iter().map(|&k| k.to_owned()).collect();
    // Place 0 is the workload itself, along the tree of the module; places 1… are the
    // pod containers of its cluster, or its pod domain without one, along the addressing
    // tree (Q-019-30).
    let rows: Vec<AxisRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "WITH living AS (
             SELECT w.id, w.namespace_id, n.cluster_id,
                    CASE WHEN cardinality(c.pod_container_ids) = 0 THEN ARRAY[c.pod_domain_id]
                         ELSE c.pod_container_ids END AS tree_ids
               FROM {workload} w
               JOIN {namespace} n ON n.id = w.namespace_id
               JOIN {cluster} c ON c.id = n.cluster_id
              WHERE w.id = ANY($3)),
         asked AS (
             SELECT l.id AS workload, l.id AS object, 0::bigint AS place FROM living l
             UNION ALL
             SELECT l.id, p.object, p.place
               FROM living l
               CROSS JOIN LATERAL unnest(l.tree_ids) WITH ORDINALITY p(object, place)),
         wanted AS (
             SELECT d.id, d.key, d.value_type = 'enum' AS closed_list, d.inherit_tree,
                    array_position($4::text[], d.key) AS rank
               FROM {declaration} d WHERE d.key = ANY($4))
         SELECT q.workload, q.place, q.object, w.key, src.origin,
                CASE
                    WHEN a.kind IS NULL OR a.kind = 'cleared' THEN NULL
                    WHEN w.closed_list AND a.v_uuid IS NOT NULL THEN
                        to_jsonb((SELECT e.key FROM {value} e WHERE e.id = a.v_uuid LIMIT 1))
                    WHEN w.closed_list THEN
                        (SELECT jsonb_agg(k ORDER BY k) FROM (
                            SELECT DISTINCT e.key AS k FROM {value} e
                             WHERE e.id = ANY(a.v_uuids)) x)
                    ELSE coalesce(to_jsonb(a.v_uuid), to_jsonb(a.v_uuids), to_jsonb(a.v_text),
                                  to_jsonb(a.v_texts), to_jsonb(a.v_int), to_jsonb(a.v_ints),
                                  to_jsonb(a.v_bool), to_jsonb(a.v_date), to_jsonb(a.v_dates),
                                  public.kartenn_external_json(a.v_json),
                                  (SELECT jsonb_agg(public.kartenn_external_json(x))
                                     FROM jsonb_array_elements(a.v_jsons) x))
                END,
                l.namespace_id, l.cluster_id
           FROM asked q
           JOIN living l ON l.id = q.workload
           JOIN wanted w ON true
           LEFT JOIN LATERAL (
               SELECT CASE WHEN w.inherit_tree IS NULL THEN q.object
                           ELSE (SELECT o.origin_object_id FROM {origin} o
                                  WHERE o.object_id = q.object AND o.property_id = w.id
                                  LIMIT 1) END AS origin) src ON true
           -- The assignment of the origin by its key, behind an `OFFSET 0` fence, as
           -- `kartenn_machines::read::machine_values_of_addresses`.
           LEFT JOIN LATERAL (
               SELECT * FROM {assignment} x
                WHERE x.object_id = src.origin AND x.property_id = w.id OFFSET 0) a ON true
          ORDER BY q.workload, w.rank, q.place"
    )))
    .bind(time(valid_at))
    .bind(as_of.map(time))
    .bind(workloads)
    .bind(&keys)
    .fetch_all(conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(gathered(workloads, rows))
}

/// The rows of [`axis_values`], by workload in the order of `workloads`, then by key in
/// the order asked, each key by place.
fn gathered(workloads: &[Uuid], rows: Vec<AxisRow>) -> Vec<(Uuid, Vec<AxisValues>)> {
    let mut out: Vec<(Uuid, Vec<AxisValues>)> =
        workloads.iter().map(|&w| (w, Vec::new())).collect();
    for (workload, place, object, key, origin, value, namespace, cluster) in rows {
        let Some((_, keyed)) = out.iter_mut().find(|(w, _)| *w == workload) else {
            continue;
        };
        if keyed.last().is_none_or(|a| a.key != key) {
            keyed.push(AxisValues {
                key,
                values: Vec::new(),
            });
        }
        let Some(axis) = keyed.last_mut() else {
            continue;
        };
        // An assignment that does not apply gives no value.
        let (Some(value), Some(origin)) = (value, origin) else {
            continue;
        };
        let source = if place > 0 {
            // The tree serves only when the tree of the module gives nothing (the rows of
            // a key come by place, the workload first).
            if axis.values.iter().any(|v| v.source != Source::Tree) {
                continue;
            }
            Source::Tree
        } else if origin == workload {
            Source::Workload
        } else if origin == namespace {
            Source::Namespace
        } else if origin == cluster {
            Source::Cluster
        } else {
            // The tree of the module has no other level above a workload.
            continue;
        };
        axis.values.push(Sourced {
            value,
            source,
            origin,
            through: object,
        });
    }
    out
}
