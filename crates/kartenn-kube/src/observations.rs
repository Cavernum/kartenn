//! The journal of the pods observed (REQ-019-16, 019/X7, ADR 0043; design « Données »;
//! task 019-6a): a batch of the connector opens observations and closes the period of
//! open ones, outside the model and its changesets. The batch is checked at the entry in
//! its transaction and refused whole, with its lines named; once accepted, its canonical
//! content (RFC 8785) has a SHA-256 fingerprint, kept in `kube.observation_batch`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::net::IpAddr;

use serde_json::{Value, json};
use sqlx::PgConnection;
use sqlx::types::time::{OffsetDateTime, UtcOffset};
use uuid::Uuid;

/// The journal a batch of the module writes, as its audit events name it.
pub const JOURNAL: &str = "kube.pod_observation";

/// A pod seen: every column of an observation but the end of its period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// Its namespace, by name.
    pub namespace: String,
    /// Its name.
    pub pod_name: String,
    /// Its Kubernetes UID.
    pub pod_uid: Uuid,
    /// The workload its `ownerReferences` climb to, of the cluster.
    pub workload: Option<Uuid>,
    /// The node it runs on, of the cluster.
    pub node: Option<Uuid>,
    /// One of its IPs; none in `hostNetwork` (the address is the node's).
    pub ip: Option<IpAddr>,
    /// Whether it shares the network of its node.
    pub host_network: bool,
    /// Since when it is seen; an instant to come is the instant of the batch.
    pub from: OffsetDateTime,
}

/// The end of the period of an open observation of the cluster, by pod and IP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closing {
    /// The UID of the pod.
    pub pod_uid: Uuid,
    /// Its IP; none in `hostNetwork`.
    pub ip: Option<IpAddr>,
    /// When it was gone; an instant to come is the instant of the batch.
    pub at: OffsetDateTime,
}

/// A batch of the connector of a cluster.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch {
    /// The observations it opens.
    pub opened: Vec<Opening>,
    /// The observations it closes.
    pub closed: Vec<Closing>,
}

/// A line of a batch, by its list and its position there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Line {
    /// The opening at this position.
    Opened(usize),
    /// The closing at this position.
    Closed(usize),
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Opened(i) => write!(f, "opened[{i}]"),
            Self::Closed(i) => write!(f, "closed[{i}]"),
        }
    }
}

/// Why a line refuses its batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// An IP without `hostNetwork`, or `hostNetwork` with an IP.
    HostNetwork,
    /// The IP is in no pod container of the cluster, no pod range of its nodes, and is
    /// not an address of its nodes.
    OutsideCluster,
    /// The workload is not one of the cluster.
    ForeignWorkload,
    /// The node is not one of the cluster.
    ForeignNode,
    /// The same pod and IP twice in the list.
    Repeated,
    /// An observation of the pod and IP is open with other values.
    OpenOtherwise,
    /// An observation of the pod and IP is closed: a period is never opened again.
    Closed,
    /// No observation of the pod and IP is open in the cluster.
    NotOpen,
    /// The end is not after the start of the period.
    EndNotAfterStart,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::HostNetwork => "a pod has an IP unless it is in hostNetwork",
            Self::OutsideCluster => {
                "the IP is outside the pod containers and ranges of the cluster and the addresses of its nodes"
            }
            Self::ForeignWorkload => "the workload is not one of the cluster",
            Self::ForeignNode => "the node is not one of the cluster",
            Self::Repeated => "the same pod and IP appear twice in the list",
            Self::OpenOtherwise => "an observation of this pod and IP is open with other values",
            Self::Closed => "the observation of this pod and IP is closed: it is not opened again",
            Self::NotOpen => "no observation of this pod and IP is open in the cluster",
            Self::EndNotAfterStart => "the end is not after the start of the observation",
        })
    }
}

/// A line of a batch refused, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid {
    /// The line.
    pub line: Line,
    /// Why.
    pub reason: Reason,
}

/// Why a batch is not written.
#[derive(Debug)]
pub enum RecordError {
    /// No cluster of this identifier lives now.
    UnknownCluster(Uuid),
    /// The author is not a service account member, directly and now, of the connector
    /// group the cluster designates.
    NotConnector,
    /// Lines of the batch refuse it whole, sorted.
    Invalid(Vec<Invalid>),
    /// The database.
    Store(sqlx::Error),
    /// The canonical form of the content.
    Canonical(kartenn_core::audit::jcs::CanonicalError),
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCluster(id) => write!(f, "no living cluster {id}"),
            Self::NotConnector => f.write_str("the author is not a connector of the cluster"),
            Self::Invalid(lines) => write!(f, "{} line(s) refuse the batch", lines.len()),
            Self::Store(e) => write!(f, "store: {e}"),
            Self::Canonical(e) => write!(f, "canonical content: {e}"),
        }
    }
}

impl std::error::Error for RecordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(e) => Some(e),
            Self::Canonical(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for RecordError {
    fn from(e: sqlx::Error) -> Self {
        Self::Store(e)
    }
}

impl From<kartenn_core::audit::jcs::CanonicalError> for RecordError {
    fn from(e: kartenn_core::audit::jcs::CanonicalError) -> Self {
        Self::Canonical(e)
    }
}

/// A batch written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// Its identifier.
    pub id: Uuid,
    /// When it was received: the instant of its transaction.
    pub received_at: OffsetDateTime,
    /// The observations it opened; an opening of an observation open with the same values
    /// writes nothing and is not counted.
    pub opened: u32,
    /// The observations it closed.
    pub closed: u32,
    /// SHA-256 of its canonical content ([`content`]).
    pub hash: [u8; 32],
    /// Its lines dated after the instant of the batch, brought back to it (Q-019-26).
    pub clamped: u32,
    /// The longest advance of those lines on the instant of the batch, in milliseconds;
    /// 0 without any.
    pub skew_max_ms: u64,
    /// Its earliest instant, sealed with it (task 019-6c, Q-019-27).
    pub earliest: OffsetDateTime,
}

/// An instant as PostgreSQL keeps it, to the microsecond, in UTC.
fn stored(at: OffsetDateTime) -> OffsetDateTime {
    let at = at.to_offset(UtcOffset::UTC);
    at.replace_nanosecond(at.nanosecond() / 1_000 * 1_000)
        .unwrap_or(at)
}

/// An instant in its canonical text: RFC 3339, UTC, six digits of fraction.
#[must_use]
pub fn rfc3339(at: OffsetDateTime) -> String {
    let at = stored(at);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
        at.microsecond()
    )
}

/// The canonical content of a batch of `cluster`, from what it wrote: its openings
/// sorted by pod and IP, then its closings sorted the same way (design « Données »,
/// « Audit »). Its JSON canonical form (RFC 8785) is what [`content_hash`] digests.
#[must_use]
pub fn content(cluster: Uuid, opened: &[Opening], closed: &[Closing]) -> Value {
    let mut opened: Vec<&Opening> = opened.iter().collect();
    opened.sort_by_key(|o| (o.pod_uid, o.ip));
    let mut closed: Vec<&Closing> = closed.iter().collect();
    closed.sort_by_key(|c| (c.pod_uid, c.ip));
    let ip = |ip: Option<IpAddr>| ip.map(|ip| ip.to_string());
    json!([
        opened
            .iter()
            .map(|o| json!({
                "cluster": cluster,
                "namespace": o.namespace,
                "pod": o.pod_name,
                "pod_uid": o.pod_uid,
                "workload": o.workload,
                "node": o.node,
                "ip": ip(o.ip),
                "host_network": o.host_network,
                "from": rfc3339(o.from),
            }))
            .collect::<Vec<_>>(),
        closed
            .iter()
            .map(|c| json!({"pod_uid": c.pod_uid, "ip": ip(c.ip), "at": rfc3339(c.at)}))
            .collect::<Vec<_>>(),
    ])
}

/// SHA-256 of the canonical form of [`content`].
///
/// # Errors
///
/// None in practice: the content holds no number.
pub fn content_hash(
    cluster: Uuid,
    opened: &[Opening],
    closed: &[Closing],
) -> Result<[u8; 32], kartenn_core::audit::jcs::CanonicalError> {
    kartenn_core::audit::jcs::event_hash(&content(cluster, opened, closed))
}

/// An observation of the cluster already in the journal, for a pod and IP of the batch.
struct Known {
    open: bool,
    from: OffsetDateTime,
    namespace: String,
    pod_name: String,
    workload: Option<Uuid>,
    node: Option<Uuid>,
    host_network: bool,
}

/// [`Known`] as read, with its pod and IP.
type KnownRow = (
    Uuid,
    Option<String>,
    bool,
    OffsetDateTime,
    String,
    String,
    Option<Uuid>,
    Option<Uuid>,
    bool,
);

/// An observation by its pod and IP, unique among the open ones of a cluster.
type Key = (Uuid, Option<IpAddr>);

/// Writes `batch` of `cluster` by `author` on `conn`, inside the caller's transaction,
/// or refuses it whole without writing anything (REQ-019-16; ADR 0043, decisions 3, 4,
/// 7): the batches of a cluster are serialised; the author is a service account member,
/// directly and now, of the connector group the cluster designates; each IP is in a pod
/// container of the cluster, in a pod range of one of its living nodes, or is an address
/// read of one of them; a workload and a node cited are of the cluster; a closing ends an
/// open observation of the cluster, after its start. An instant to come is the instant of
/// the batch. Nothing enters the model, the changesets or the outbox. On an error, the
/// caller rolls its transaction back: a day created or a write begun goes with it.
///
/// # Errors
///
/// [`RecordError`]: the cluster unknown, the author not its connector, lines invalid, or
/// the database.
#[allow(clippy::too_many_lines)] // the checks of a batch in their order, then its writes
pub async fn record(
    conn: &mut PgConnection,
    cluster: Uuid,
    author: Uuid,
    batch: &Batch,
) -> Result<Recorded, RecordError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('kube.observation:' || $1, 0))")
        .bind(cluster.to_string())
        .execute(&mut *conn)
        .await?;
    let received_at: OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *conn)
        .await?;
    let living: Option<bool> = sqlx::query_scalar(
        "SELECT EXISTS (
                SELECT 1
                  FROM public.group_member g
                  JOIN public.person p ON p.id = g.identity_id AND p.kind = 'service_account'
                  JOIN public.rbac_group r ON r.id = g.group_id AND r.valid_period @> now()
                 WHERE g.group_id = c.connector_group_id AND g.member_kind = 'identity'
                   AND g.identity_id = $2 AND g.valid_period @> now())
           FROM kube.cluster c
          WHERE c.id = $1 AND c.valid_period @> now()",
    )
    .bind(cluster)
    .bind(author)
    .fetch_optional(&mut *conn)
    .await?;
    match living {
        None => return Err(RecordError::UnknownCluster(cluster)),
        Some(false) => return Err(RecordError::NotConnector),
        Some(true) => {}
    }
    let now = stored(received_at);
    // The instants to come, brought back to the instant of the batch: counted, and the
    // longest advance kept, for its audit event (Q-019-26).
    let ahead: Vec<i128> = batch
        .opened
        .iter()
        .map(|o| o.from)
        .chain(batch.closed.iter().map(|c| c.at))
        .map(stored)
        .filter(|at| *at > now)
        .map(|at| (at - now).whole_milliseconds())
        .collect();
    let opened: Vec<Opening> = batch
        .opened
        .iter()
        .map(|o| Opening {
            from: stored(o.from).min(now),
            ..o.clone()
        })
        .collect();
    let closed: Vec<Closing> = batch
        .closed
        .iter()
        .map(|c| Closing {
            at: stored(c.at).min(now),
            ..c.clone()
        })
        .collect();
    // The days the closings end in, created by the owner of the schema before anything
    // reads the journal: the creation of a missing day holds a lock common to every
    // cluster, then waits for the readers of the closed observations; a batch that read
    // them before asking for that lock would close a cycle with another one.
    if !closed.is_empty() {
        sqlx::query(
            "SELECT kube.observation_partition(d)
               FROM (SELECT DISTINCT (t AT TIME ZONE 'UTC')::date AS d
                       FROM unnest($1::timestamptz[]) AS t) AS days",
        )
        .bind(closed.iter().map(|c| c.at).collect::<Vec<_>>())
        .execute(&mut *conn)
        .await?;
    }

    let mut invalid = Vec::new();
    let mut seen = BTreeSet::new();
    for (i, o) in opened.iter().enumerate() {
        if o.host_network == o.ip.is_some() {
            invalid.push(Invalid {
                line: Line::Opened(i),
                reason: Reason::HostNetwork,
            });
        }
        if !seen.insert((o.pod_uid, o.ip)) {
            invalid.push(Invalid {
                line: Line::Opened(i),
                reason: Reason::Repeated,
            });
        }
    }
    let mut seen = BTreeSet::new();
    for (i, c) in closed.iter().enumerate() {
        if !seen.insert((c.pod_uid, c.ip)) {
            invalid.push(Invalid {
                line: Line::Closed(i),
                reason: Reason::Repeated,
            });
        }
    }

    // The IPs: in a pod container of the cluster, a pod range of one of its living
    // nodes, or among the addresses read of them.
    let (lines, ips): (Vec<i32>, Vec<String>) = opened
        .iter()
        .enumerate()
        .filter_map(|(i, o)| Some((position(i), o.ip?.to_string())))
        .unzip();
    if !lines.is_empty() {
        let read: Vec<String> = sqlx::query_scalar(
            "SELECT a->>'value'
               FROM kube.node n, jsonb_array_elements(n.addresses) a
              WHERE n.cluster_id = $1 AND n.valid_period @> now()",
        )
        .bind(cluster)
        .fetch_all(&mut *conn)
        .await?;
        let addresses: Vec<String> = read
            .iter()
            .filter_map(|a| a.parse::<IpAddr>().ok())
            .map(|a| a.to_string())
            .collect();
        let outside: Vec<i32> = sqlx::query_scalar(
            "WITH line (i, ip) AS (SELECT * FROM unnest($2::int[], $3::text[]::inet[])),
                  allowed (net) AS (
                      SELECT p.prefix
                        FROM kube.cluster c
                        JOIN public.prefix p
                          ON p.id = ANY(c.pod_container_ids) AND p.valid_period @> now()
                       WHERE c.id = $1 AND c.valid_period @> now()
                      UNION ALL
                      SELECT p.prefix
                        FROM kube.node n
                        JOIN public.prefix p
                          ON p.id = ANY(n.range_ids) AND p.valid_period @> now()
                       WHERE n.cluster_id = $1 AND n.valid_period @> now()
                      UNION ALL
                      SELECT unnest($4::text[]::inet[]))
             SELECT i FROM line
              WHERE NOT EXISTS (SELECT 1 FROM allowed WHERE allowed.net >>= line.ip)
              ORDER BY i",
        )
        .bind(cluster)
        .bind(&lines)
        .bind(&ips)
        .bind(&addresses)
        .fetch_all(&mut *conn)
        .await?;
        invalid.extend(outside.into_iter().map(|i| Invalid {
            line: Line::Opened(index(i)),
            reason: Reason::OutsideCluster,
        }));
    }

    // The workloads and the nodes: of the cluster, in any of their versions.
    for (reason, sql, cited) in [
        (
            Reason::ForeignWorkload,
            "SELECT l.i FROM unnest($2::int[], $3::uuid[]) AS l (i, id)
              WHERE NOT EXISTS (SELECT 1 FROM kube.workload o
                                  JOIN kube.namespace n ON n.id = o.namespace_id
                                 WHERE o.id = l.id AND n.cluster_id = $1)
              ORDER BY l.i",
            opened.iter().map(|o| o.workload).collect::<Vec<_>>(),
        ),
        (
            Reason::ForeignNode,
            "SELECT l.i FROM unnest($2::int[], $3::uuid[]) AS l (i, id)
              WHERE NOT EXISTS (SELECT 1 FROM kube.node n WHERE n.id = l.id AND n.cluster_id = $1)
              ORDER BY l.i",
            opened.iter().map(|o| o.node).collect::<Vec<_>>(),
        ),
    ] {
        let (lines, ids): (Vec<i32>, Vec<Uuid>) = cited
            .iter()
            .enumerate()
            .filter_map(|(i, id)| Some((position(i), (*id)?)))
            .unzip();
        if lines.is_empty() {
            continue;
        }
        let foreign: Vec<i32> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(cluster)
            .bind(&lines)
            .bind(&ids)
            .fetch_all(&mut *conn)
            .await?;
        invalid.extend(foreign.into_iter().map(|i| Invalid {
            line: Line::Opened(index(i)),
            reason,
        }));
    }

    // What the journal of the cluster already holds for the pods of the batch.
    let pods: Vec<Uuid> = opened
        .iter()
        .map(|o| o.pod_uid)
        .chain(closed.iter().map(|c| c.pod_uid))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let rows: Vec<KnownRow> = sqlx::query_as(
        "SELECT pod_uid, host(ip), upper_inf(observed), lower(observed), namespace, pod_name,
                workload_id, node_id, host_network
           FROM kube.pod_observation
          WHERE cluster_id = $1 AND pod_uid = ANY($2)",
    )
    .bind(cluster)
    .bind(&pods)
    .fetch_all(&mut *conn)
    .await?;
    let mut known: HashMap<Key, Known> = HashMap::new();
    for (pod_uid, ip, open, from, namespace, pod_name, workload, node, host_network) in rows {
        let ip = ip.and_then(|ip| ip.parse().ok());
        let entry = Known {
            open,
            from,
            namespace,
            pod_name,
            workload,
            node,
            host_network,
        };
        // An open observation wins over a closed one of the same pod and IP.
        if known.get(&(pod_uid, ip)).is_none_or(|k| !k.open) {
            known.insert((pod_uid, ip), entry);
        }
    }

    // The openings: an observation open with the same values is not opened again; open
    // otherwise or closed, the line refuses the batch.
    let mut written: Vec<&Opening> = Vec::new();
    // The start of every observation open after the openings, for the closings.
    let mut starts: BTreeMap<Key, OffsetDateTime> = known
        .iter()
        .filter(|(_, k)| k.open)
        .map(|(key, k)| (*key, k.from))
        .collect();
    for (i, o) in opened.iter().enumerate() {
        match known.get(&(o.pod_uid, o.ip)) {
            Some(k) if k.open => {
                let same = (
                    &k.namespace,
                    &k.pod_name,
                    k.workload,
                    k.node,
                    k.host_network,
                ) == (
                    &o.namespace,
                    &o.pod_name,
                    o.workload,
                    o.node,
                    o.host_network,
                );
                if !same {
                    invalid.push(Invalid {
                        line: Line::Opened(i),
                        reason: Reason::OpenOtherwise,
                    });
                }
            }
            Some(_) => invalid.push(Invalid {
                line: Line::Opened(i),
                reason: Reason::Closed,
            }),
            None => {
                written.push(o);
                starts.entry((o.pod_uid, o.ip)).or_insert(o.from);
            }
        }
    }
    for (i, c) in closed.iter().enumerate() {
        match starts.get(&(c.pod_uid, c.ip)) {
            None => invalid.push(Invalid {
                line: Line::Closed(i),
                reason: Reason::NotOpen,
            }),
            Some(from) if c.at <= *from => invalid.push(Invalid {
                line: Line::Closed(i),
                reason: Reason::EndNotAfterStart,
            }),
            Some(_) => {}
        }
    }
    if !invalid.is_empty() {
        invalid.sort_by_key(|i| i.line);
        invalid.dedup();
        return Err(RecordError::Invalid(invalid));
    }

    // The writes: the batch, its openings, then its closings.
    let written: Vec<Opening> = written.into_iter().cloned().collect();
    let hash = content_hash(cluster, &written, &closed)?;
    // Its earliest instant: every row it writes ends at or after it, so that the
    // verification knows it whole while that instant is not before the purge bound of
    // the cluster (task 019-6c, Q-019-24).
    let earliest = written
        .iter()
        .map(|o| o.from)
        .chain(closed.iter().map(|c| c.at))
        .fold(received_at, OffsetDateTime::min);
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO kube.observation_batch
                (id, cluster_id, author, received_at, opened, closed, content_hash, earliest)
         VALUES (uuidv7(), $1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(cluster)
    .bind(author)
    .bind(received_at)
    .bind(count(written.len()))
    .bind(count(closed.len()))
    .bind(hash.as_slice())
    .bind(earliest)
    .fetch_one(&mut *conn)
    .await?;
    if !written.is_empty() {
        sqlx::query(
            "INSERT INTO kube.pod_observation
                    (cluster_id, namespace, pod_name, pod_uid, workload_id, node_id, ip,
                     host_network, observed, opened_batch)
             SELECT $1, l.namespace, l.pod_name, l.pod_uid, l.workload_id, l.node_id, l.ip::inet,
                    l.host_network, tstzrange(l.seen, NULL, '[)'), $2
               FROM unnest($3::text[], $4::text[], $5::uuid[], $6::uuid[], $7::uuid[], $8::text[],
                           $9::bool[], $10::timestamptz[])
                 AS l (namespace, pod_name, pod_uid, workload_id, node_id, ip, host_network, seen)",
        )
        .bind(cluster)
        .bind(id)
        .bind(
            written
                .iter()
                .map(|o| o.namespace.clone())
                .collect::<Vec<_>>(),
        )
        .bind(
            written
                .iter()
                .map(|o| o.pod_name.clone())
                .collect::<Vec<_>>(),
        )
        .bind(written.iter().map(|o| o.pod_uid).collect::<Vec<_>>())
        .bind(written.iter().map(|o| o.workload).collect::<Vec<_>>())
        .bind(written.iter().map(|o| o.node).collect::<Vec<_>>())
        .bind(
            written
                .iter()
                .map(|o| o.ip.map(|ip| ip.to_string()))
                .collect::<Vec<_>>(),
        )
        .bind(written.iter().map(|o| o.host_network).collect::<Vec<_>>())
        .bind(written.iter().map(|o| o.from).collect::<Vec<_>>())
        .execute(&mut *conn)
        .await?;
    }
    if !closed.is_empty() {
        let ends: Vec<OffsetDateTime> = closed.iter().map(|c| c.at).collect();
        let done: Vec<(Uuid, Option<String>)> = sqlx::query_as(
            "UPDATE kube.pod_observation o
                SET observed = tstzrange(lower(o.observed), l.at, '[)'), closed_batch = $2
               FROM unnest($3::uuid[], $4::text[], $5::timestamptz[]) AS l (pod_uid, ip, at)
              WHERE o.cluster_id = $1 AND upper_inf(o.observed) AND o.pod_uid = l.pod_uid
                AND o.ip IS NOT DISTINCT FROM l.ip::inet
          RETURNING o.pod_uid, host(o.ip)",
        )
        .bind(cluster)
        .bind(id)
        .bind(closed.iter().map(|c| c.pod_uid).collect::<Vec<_>>())
        .bind(
            closed
                .iter()
                .map(|c| c.ip.map(|ip| ip.to_string()))
                .collect::<Vec<_>>(),
        )
        .bind(&ends)
        .fetch_all(&mut *conn)
        .await?;
        // Each closing ended its observation, or the batch is refused and its transaction
        // rolled back: a closing that wrote nothing never stands in a fingerprint.
        let done: BTreeSet<Key> = done
            .into_iter()
            .map(|(pod_uid, ip)| (pod_uid, ip.and_then(|ip| ip.parse().ok())))
            .collect();
        let missed: Vec<Invalid> = closed
            .iter()
            .enumerate()
            .filter(|(_, c)| !done.contains(&(c.pod_uid, c.ip)))
            .map(|(i, _)| Invalid {
                line: Line::Closed(i),
                reason: Reason::NotOpen,
            })
            .collect();
        if !missed.is_empty() {
            return Err(RecordError::Invalid(missed));
        }
    }
    Ok(Recorded {
        id,
        received_at,
        opened: u32::try_from(written.len()).unwrap_or(u32::MAX),
        closed: u32::try_from(closed.len()).unwrap_or(u32::MAX),
        hash,
        clamped: u32::try_from(ahead.len()).unwrap_or(u32::MAX),
        skew_max_ms: ahead
            .iter()
            .max()
            .map_or(0, |ms| u64::try_from(*ms).unwrap_or(u64::MAX)),
        earliest,
    })
}

/// A position of a list as an `integer` of PostgreSQL.
fn position(i: usize) -> i32 {
    i32::try_from(i).unwrap_or(i32::MAX)
}

/// A position read back.
fn index(i: i32) -> usize {
    usize::try_from(i).unwrap_or(usize::MAX)
}

/// A count as an `integer` of PostgreSQL.
fn count(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64, nanos: u32) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds)
            .unwrap()
            .replace_nanosecond(nanos)
            .unwrap()
    }

    fn opening(pod: u128, ip: &str) -> Opening {
        Opening {
            namespace: "payments".into(),
            pod_name: format!("web-{pod}"),
            pod_uid: Uuid::from_u128(pod),
            workload: None,
            node: None,
            ip: Some(ip.parse().unwrap()),
            host_network: false,
            from: at(1_793_000_000, 123_456_789),
        }
    }

    /// The content does not depend on the order of the lines, and keeps the instants to
    /// the microsecond, as the database does.
    #[test]
    fn content_is_sorted_and_to_the_microsecond() {
        let cluster = Uuid::from_u128(7);
        let (a, b) = (opening(2, "198.51.100.17"), opening(1, "2001:db8::1"));
        let close = Closing {
            pod_uid: Uuid::from_u128(3),
            ip: None,
            at: at(1_793_000_100, 0),
        };
        let one = content(
            cluster,
            &[a.clone(), b.clone()],
            std::slice::from_ref(&close),
        );
        let other = content(cluster, &[b, a], &[close]);
        assert_eq!(one, other);
        assert_eq!(one[0][0]["pod_uid"], Uuid::from_u128(1).to_string());
        assert_eq!(one[0][1]["from"], "2026-10-26T07:33:20.123456Z");
        assert_eq!(one[1][0]["ip"], Value::Null);
        assert_eq!(
            content_hash(cluster, &[], &[]).unwrap(),
            kartenn_core::audit::jcs::event_hash(&json!([[], []])).unwrap()
        );
    }
}
