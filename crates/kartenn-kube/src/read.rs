//! The read port of the module (spec 019, design « Port de lecture »): so far the reads
//! of the journal of the pods observed (REQ-019-16, 17; ADR 0043, decisions 6, 8;
//! Q-019-23, 25; task 019-6d). The journal has no dated read (`as_of`): it answers what
//! is stored, within the retention of each cluster, and a question about an instant past
//! that retention answers nothing, with the reason [`Cut::Retention`], whether or not the
//! purge has dropped the rows yet.

use std::net::IpAddr;

use kartenn_core::addr::Prefix;
use kartenn_service::StoreError;
use kartenn_service::authz::{Authorizer, Subject};
use sqlx::PgConnection;
use std::time::Duration;

use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::module::OBSERVATION_READ_PERMISSION;
use crate::pg::store;

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
}

impl Observation {
    /// Its key in the order of the reads: its start, its pod, its IP.
    #[must_use]
    pub fn key(&self) -> Key {
        Key {
            from: self.from,
            pod_uid: self.pod_uid,
            ip: self.ip.map(|ip| ip.to_string()).unwrap_or_default(),
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
}

impl Cut {
    /// Its name in the answers of the routes (`reason`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Retention => "retention",
        }
    }
}

/// What [`who_had`] answers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answer {
    /// The observations, in the order of [`Key`].
    pub observations: Vec<Observation>,
    /// [`Cut::Retention`] when the instant is past the retention of a cluster of the
    /// domain the reader sees, which therefore answers nothing.
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
        ip: ip.and_then(|ip| ip.parse().ok()),
        host_network,
        from: row.8,
        until: row.9,
        opened_batch: row.10,
        closed_batch: row.11,
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

/// Who had an address of `network` at `at` among the clusters whose pod domain is
/// `domain` (design « Port de lecture »): the observations of those clusters, provided
/// `reader` holds `kube.observation.read` on that domain (Q-019-23; ADR 0043, decision 6),
/// nothing otherwise. A cluster for which `at` is past its retention answers nothing and
/// sets [`Cut::Retention`] (Q-019-25).
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
    let clusters: Vec<(Uuid, i16, OffsetDateTime)> = sqlx::query_as(
        "SELECT id, retention_days, now() FROM kube.cluster
          WHERE pod_domain_id = $1 AND valid_period @> now() ORDER BY id",
    )
    .bind(domain)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let (kept, past): (Vec<&(Uuid, i16, OffsetDateTime)>, Vec<_>) = clusters
        .iter()
        .partition(|(_, days, now)| within_retention(at, *now, *days));
    let kept: Vec<Uuid> = kept.into_iter().map(|c| c.0).collect();
    let observations = if kept.is_empty() {
        Vec::new()
    } else {
        held(conn, &kept, network, at, None, None)
            .await
            .map_err(|e| store(&e))?
    };
    Ok(Answer {
        observations,
        reason: (!past.is_empty()).then_some(Cut::Retention),
    })
}
