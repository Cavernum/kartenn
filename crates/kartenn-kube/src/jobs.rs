//! The background tasks of the journal of the pods observed (task 019-6c; REQ-019-16;
//! ADR 0043, decisions 5 and 8; 019/Q-019-20, Q-019-24, Q-019-27), run under the lease of
//! the core's tasks (ADR 0023). The journal holds against a compromised role of the
//! application (Q-019-27): what the verification trusts is sealed in the audit.
//!
//! - `kube.observations_purge` deletes, cluster by cluster, the closed observations past
//!   the retention of their cluster, moves its purge bound, drops the days every cluster
//!   has passed, creates the days ahead, and seals each move of a bound
//!   (`observation.journal.purged`) in its transaction.
//! - `kube.observations_verify` checks each purge bound against its last sealed purge,
//!   each batch against its one sealed event (`observation.batch.recorded`, found by the
//!   identifier of the batch: its instant, its earliest instant, its fingerprint
//!   recomputed from its rows while the sealed bound has not reached it), and audits
//!   `observation.batch.altered` for each gap.

use std::net::IpAddr;
use std::time::Duration;

use kartenn_core::audit::seal::hex;
use kartenn_core::audit::{OBSERVATION_BATCH_ALTERED, OBSERVATION_JOURNAL_PURGED, Outcome};
use kartenn_service::audit::RequestContext;
use kartenn_service::module::{JobSpec, ModuleTx};
use kartenn_service::port::StoreError;
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::types::Uuid;
use sqlx::types::time::{Date, OffsetDateTime};
use sqlx::{PgConnection, Row};

use crate::observations::{Closing, JOURNAL, Opening, content_hash, rfc3339};

/// The verification, within the module.
pub const VERIFY: &str = "observations_verify";
/// The purge past the retention, within the module.
pub const PURGE: &str = "observations_purge";

/// The tasks of the journal, once a day each.
pub static JOBS: &[JobSpec] = &[
    JobSpec {
        name: VERIFY,
        period: Duration::from_hours(24),
    },
    JobSpec {
        name: PURGE,
        period: Duration::from_hours(24),
    },
];

/// The batches the verification fetches at once from its cursor.
const FETCH: i64 = 500;

/// The gaps the verification reports, as the `reason` of `observation.batch.altered`.
pub mod reasons {
    /// The rows of a whole batch no longer give its sealed fingerprint.
    pub const CONTENT: &str = "content";
    /// No sealed event for a batch whose month the audit has not purged.
    pub const EVENT_MISSING: &str = "event_missing";
    /// More than one sealed event for a batch.
    pub const EVENT_DUPLICATED: &str = "event_duplicated";
    /// The instant of a batch is not that of its sealed event.
    pub const RECEIVED_AT: &str = "received_at";
    /// The earliest instant of a batch is not the sealed one.
    pub const EARLIEST: &str = "earliest";
    /// A purge bound is not the last sealed purge of its cluster.
    pub const PURGE_BOUND: &str = "purge_bound";
}

/// A gap between the journal and what the audit sealed of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Altered {
    /// Its cluster.
    pub cluster: Uuid,
    /// The batch; `None` for a purge bound.
    pub batch: Option<Uuid>,
    /// The gap, one of [`reasons`].
    pub reason: &'static str,
    /// What the audit sealed: a fingerprint in hexadecimal, an instant in RFC 3339, a
    /// number of events; empty for nothing sealed.
    pub expected: String,
    /// What the journal holds or gives back, in the same form.
    pub found: String,
}

/// What a verification read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verified {
    /// The batches whose fingerprint was recomputed: those the sealed bound of their
    /// cluster has not reached, their sealed event otherwise in order.
    pub checked: u64,
    /// The gaps, purge bounds first, then by batch.
    pub altered: Vec<Altered>,
}

/// The purge of a cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterPurged {
    /// The cluster.
    pub cluster: Uuid,
    /// Its bound before the purge; `None` at the first.
    pub before: Option<OffsetDateTime>,
    /// Its bound after.
    pub after: OffsetDateTime,
    /// Its closed observations deleted.
    pub deleted: u64,
}

/// What a purge did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Purged {
    /// Cluster by cluster.
    pub clusters: Vec<ClusterPurged>,
    /// The days dropped, passed by every cluster.
    pub dropped: Vec<Date>,
    /// The days created ahead.
    pub created: u32,
}

impl Purged {
    /// The closed observations deleted, every cluster together.
    #[must_use]
    pub fn deleted(&self) -> u64 {
        self.clusters.iter().map(|c| c.deleted).sum()
    }
}

fn store(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// The last sealed bound of each cluster: the largest `after` of its purges.
const SEALED_BOUNDS: &str = "
SELECT e.details->>'cluster' AS cluster,
       max(CASE WHEN pg_input_is_valid(e.details->>'after', 'timestamptz')
                THEN (e.details->>'after')::timestamptz END) AS bound
  FROM audit.event e
 WHERE e.name = 'observation.journal.purged'
 GROUP BY 1";

/// The batches to check and their rows, in one statement read through a cursor, so that
/// the sealed events, the bounds and the rows come from one snapshot: a purge committed
/// meanwhile never makes a batch look altered. A batch is read when its sealed event is
/// not the only one, or when its earliest instant is not the sealed one, or when the
/// sealed bound of its cluster has not reached it.
const BATCHES: &str = "
DECLARE kube_observations_verify NO SCROLL CURSOR FOR
WITH sealed_bound AS (
    SELECT e.details->>'cluster' AS cluster,
           max(CASE WHEN pg_input_is_valid(e.details->>'after', 'timestamptz')
                    THEN (e.details->>'after')::timestamptz END) AS bound
      FROM audit.event e
     WHERE e.name = 'observation.journal.purged'
     GROUP BY 1),
recorded AS (
    SELECT e.details->>'batch' AS batch, count(*) AS n, min(e.details->>'hash') AS hash,
           min(e.details->>'earliest') AS earliest, min(e.recorded_at) AS recorded_at
      FROM audit.event e
     WHERE e.name = 'observation.batch.recorded'
     GROUP BY 1),
batches AS (
    SELECT b.id, b.cluster_id, b.received_at, b.earliest, b.content_hash,
           coalesce(r.n, 0) AS n, r.hash, r.earliest AS sealed_earliest, r.recorded_at,
           EXISTS (SELECT 1 FROM audit.purge a
                    WHERE a.criticality = 'normal'
                      AND a.month = date_trunc('month', b.received_at)::date) AS audit_purged,
           s.bound
      FROM kube.observation_batch b
      LEFT JOIN sealed_bound s ON s.cluster = b.cluster_id::text
      LEFT JOIN recorded r ON r.batch = b.id::text
     WHERE coalesce(r.n, 0) <> 1
        OR r.earliest IS DISTINCT FROM
           to_char(b.earliest AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')
        OR b.earliest >= coalesce(s.bound, '-infinity')),
opened AS (
    SELECT o.opened_batch AS id, array_agg(o.namespace) AS namespaces,
           array_agg(o.pod_name) AS pods, array_agg(o.pod_uid) AS uids,
           array_agg(o.workload_id) AS workloads, array_agg(o.node_id) AS nodes,
           array_agg(host(o.ip)) AS ips, array_agg(o.host_network) AS host_networks,
           array_agg(lower(o.observed)) AS froms
      FROM kube.pod_observation o
     WHERE o.opened_batch IN (SELECT id FROM batches)
     GROUP BY o.opened_batch),
closed AS (
    SELECT o.closed_batch AS id, array_agg(o.pod_uid) AS uids, array_agg(host(o.ip)) AS ips,
           array_agg(upper(o.observed)) AS ats
      FROM kube.pod_observation o
     WHERE o.closed_batch IN (SELECT id FROM batches)
     GROUP BY o.closed_batch)
SELECT w.id, w.cluster_id, w.received_at, w.earliest, w.content_hash, w.n, w.hash,
       w.sealed_earliest, w.recorded_at, w.audit_purged, w.bound,
       o.namespaces, o.pods, o.uids, o.workloads, o.nodes, o.ips, o.host_networks, o.froms,
       c.uids, c.ips, c.ats
  FROM batches w
  LEFT JOIN opened o ON o.id = w.id
  LEFT JOIN closed c ON c.id = w.id
 ORDER BY w.id";

fn address(ip: Option<String>) -> Result<Option<IpAddr>, StoreError> {
    ip.map(|ip| ip.parse().map_err(store)).transpose()
}

/// The `i`-th value of an aggregated column, which every row of the batch fills.
fn nth<T: Clone>(column: Option<&Vec<T>>, i: usize) -> Result<T, StoreError> {
    column
        .and_then(|values| values.get(i).cloned())
        .ok_or_else(|| store("a row of the journal read back without one of its columns"))
}

/// The fingerprint the rows of a batch of `cluster` give back, in hexadecimal: columns 11
/// to 21 of [`BATCHES`].
fn recomputed(row: &PgRow, cluster: Uuid) -> Result<String, StoreError> {
    let get = |i: usize| row.try_get::<Option<Vec<String>>, _>(i).map_err(store);
    let (namespaces, pods) = (get(11)?, get(12)?);
    let uids: Option<Vec<Uuid>> = row.try_get(13).map_err(store)?;
    let workloads: Option<Vec<Option<Uuid>>> = row.try_get(14).map_err(store)?;
    let nodes: Option<Vec<Option<Uuid>>> = row.try_get(15).map_err(store)?;
    let ips: Option<Vec<Option<String>>> = row.try_get(16).map_err(store)?;
    let hosts: Option<Vec<bool>> = row.try_get(17).map_err(store)?;
    let froms: Option<Vec<OffsetDateTime>> = row.try_get(18).map_err(store)?;
    let mut opened = Vec::new();
    for (i, namespace) in namespaces.unwrap_or_default().into_iter().enumerate() {
        opened.push(Opening {
            namespace,
            pod_name: nth(pods.as_ref(), i)?,
            pod_uid: nth(uids.as_ref(), i)?,
            workload: nth(workloads.as_ref(), i)?,
            node: nth(nodes.as_ref(), i)?,
            ip: address(nth(ips.as_ref(), i)?)?,
            host_network: nth(hosts.as_ref(), i)?,
            from: nth(froms.as_ref(), i)?,
        });
    }
    let closed_uids: Option<Vec<Uuid>> = row.try_get(19).map_err(store)?;
    let closed_ips: Option<Vec<Option<String>>> = row.try_get(20).map_err(store)?;
    let ats: Option<Vec<OffsetDateTime>> = row.try_get(21).map_err(store)?;
    let mut closed = Vec::new();
    for (i, pod_uid) in closed_uids.unwrap_or_default().into_iter().enumerate() {
        closed.push(Closing {
            pod_uid,
            ip: address(nth(closed_ips.as_ref(), i)?)?,
            at: nth(ats.as_ref(), i)?,
        });
    }
    Ok(hex(&content_hash(cluster, &opened, &closed).map_err(store)?))
}

/// The gap of the batch of `row`, if any; `checked` counts it when its fingerprint is
/// recomputed. One gap by batch, the first found: its events, its instant, its earliest
/// instant, then its content.
fn checked_batch(row: &PgRow, checked: &mut u64) -> Result<Option<Altered>, StoreError> {
    let batch: Uuid = row.try_get(0).map_err(store)?;
    let cluster: Uuid = row.try_get(1).map_err(store)?;
    let received_at: OffsetDateTime = row.try_get(2).map_err(store)?;
    let earliest: OffsetDateTime = row.try_get(3).map_err(store)?;
    let copy: Vec<u8> = row.try_get(4).map_err(store)?;
    let events: i64 = row.try_get(5).map_err(store)?;
    let sealed_hash: Option<String> = row.try_get(6).map_err(store)?;
    let sealed_earliest: Option<String> = row.try_get(7).map_err(store)?;
    let recorded_at: Option<OffsetDateTime> = row.try_get(8).map_err(store)?;
    let audit_purged: bool = row.try_get(9).map_err(store)?;
    let bound: Option<OffsetDateTime> = row.try_get(10).map_err(store)?;
    let gap = |reason, expected: String, found: String| {
        Ok(Some(Altered {
            cluster,
            batch: Some(batch),
            reason,
            expected,
            found,
        }))
    };
    let expected = match (events, audit_purged) {
        // The audit purged its month by its retention: the copy stands in (Q-019-27).
        (0, true) => hex(&copy),
        (0, false) => return gap(reasons::EVENT_MISSING, "1".into(), "0".into()),
        (1, _) => {
            if recorded_at != Some(received_at) {
                return gap(
                    reasons::RECEIVED_AT,
                    recorded_at.map(rfc3339).unwrap_or_default(),
                    rfc3339(received_at),
                );
            }
            if sealed_earliest.as_deref() != Some(rfc3339(earliest).as_str()) {
                return gap(
                    reasons::EARLIEST,
                    sealed_earliest.unwrap_or_default(),
                    rfc3339(earliest),
                );
            }
            sealed_hash.unwrap_or_default()
        }
        (n, _) => return gap(reasons::EVENT_DUPLICATED, "1".into(), n.to_string()),
    };
    if bound.is_some_and(|bound| earliest < bound) {
        // Rows of it may be purged: nothing to recompute.
        return Ok(None);
    }
    *checked += 1;
    let found = recomputed(row, cluster)?;
    if found == expected {
        Ok(None)
    } else {
        gap(reasons::CONTENT, expected, found)
    }
}

/// Checks the journal against what the audit sealed of it (ADR 0043, decision 5;
/// Q-019-24, Q-019-27): each purge bound against the last sealed purge of its cluster;
/// each batch against its one sealed event, found by its identifier (its instant, its
/// earliest instant), and, while the sealed bound has not reached it, its fingerprint
/// recomputed from its rows. A batch without event counts only in a month the audit has
/// purged, against the copy of `observation_batch`. Reads only.
///
/// # Errors
///
/// [`StoreError`] of the database; the caller rolls its transaction back.
pub async fn verify(conn: &mut PgConnection) -> Result<Verified, StoreError> {
    let mut verified = Verified::default();
    let bounds: Vec<(Uuid, OffsetDateTime, Option<OffsetDateTime>)> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "WITH sealed AS ({SEALED_BOUNDS})
             SELECT p.cluster_id, p.purged_before, s.bound
               FROM kube.observation_purge_bound p
               LEFT JOIN sealed s ON s.cluster = p.cluster_id::text
              WHERE s.bound IS DISTINCT FROM p.purged_before
              ORDER BY p.cluster_id"
        )))
        .fetch_all(&mut *conn)
        .await
        .map_err(store)?;
    verified
        .altered
        .extend(bounds.into_iter().map(|(cluster, stored, sealed)| Altered {
            cluster,
            batch: None,
            reason: reasons::PURGE_BOUND,
            expected: sealed.map(rfc3339).unwrap_or_default(),
            found: rfc3339(stored),
        }));
    sqlx::query(BATCHES)
        .execute(&mut *conn)
        .await
        .map_err(store)?;
    loop {
        let rows: Vec<PgRow> = sqlx::query(sqlx::AssertSqlSafe(format!(
            "FETCH {FETCH} FROM kube_observations_verify"
        )))
        .fetch_all(&mut *conn)
        .await
        .map_err(store)?;
        for row in &rows {
            if let Some(altered) = checked_batch(row, &mut verified.checked)? {
                verified.altered.push(altered);
            }
        }
        if rows.len() < usize::try_from(FETCH).unwrap_or(usize::MAX) {
            break;
        }
    }
    sqlx::query("CLOSE kube_observations_verify")
        .execute(&mut *conn)
        .await
        .map_err(store)?;
    Ok(verified)
}

/// Purges the journal at the instant of the transaction, by the function of the owner of
/// the schema `kube.observation_purge` (Q-019-20, Q-019-27): by cluster, under the largest
/// retention of its recent versions, the closed observations ended before its bound, an
/// open one never; its bound moved; the days of the coming week created; the days every
/// cluster has passed dropped.
///
/// # Errors
///
/// [`StoreError`] of the database.
pub async fn purge(conn: &mut PgConnection) -> Result<Purged, StoreError> {
    type Row = (
        Uuid,
        Option<OffsetDateTime>,
        OffsetDateTime,
        i64,
        Vec<Date>,
        i32,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT cluster_id, bound_before, bound_after, deleted, dropped, created
           FROM kube.observation_purge()",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store)?;
    let mut purged = Purged::default();
    for (cluster, before, after, deleted, dropped, created) in rows {
        purged.dropped = dropped;
        purged.created = u32::try_from(created).unwrap_or_default();
        purged.clusters.push(ClusterPurged {
            cluster,
            before,
            after,
            deleted: u64::try_from(deleted).unwrap_or_default(),
        });
    }
    Ok(purged)
}

/// Runs the task `name` of the journal in `tx`; `false` for another task. The purge seals
/// each bound it moves (`observation.journal.purged`, targeting the cluster); the
/// verification audits each gap (`observation.batch.altered`, ERROR, high, its `reason`),
/// both in that transaction.
///
/// # Errors
///
/// [`StoreError`] of the database or of the audit: the transaction is rolled back.
pub async fn run(name: &str, tx: &mut dyn ModuleTx) -> Result<bool, StoreError> {
    let conn: &mut PgConnection = tx
        .connection()
        .downcast_mut()
        .ok_or_else(|| store("not a PostgreSQL transaction"))?;
    match name {
        VERIFY => {
            let verified = verify(conn).await?;
            for altered in &verified.altered {
                let event = RequestContext::system()
                    .event(&OBSERVATION_BATCH_ALTERED, Outcome::Failure)
                    .map(|e| e.target("object", [altered.cluster]).reason(altered.reason))
                    .and_then(|e| e.detail("journal", JOURNAL))
                    .and_then(|e| match altered.batch {
                        Some(batch) => e.detail("batch", batch.to_string()),
                        None => Ok(e),
                    })
                    .and_then(|e| e.detail("expected", altered.expected.as_str()))
                    .and_then(|e| e.detail("found", altered.found.as_str()))
                    .map_err(store)?;
                tx.audit(&event).await?;
            }
            Ok(true)
        }
        PURGE => {
            let purged = purge(conn).await?;
            let dropped: Vec<Value> = purged
                .dropped
                .iter()
                .map(|d| Value::from(d.to_string()))
                .collect();
            for cluster in &purged.clusters {
                let event = RequestContext::system()
                    .event(&OBSERVATION_JOURNAL_PURGED, Outcome::Success)
                    .map(|e| e.target("object", [cluster.cluster]))
                    .and_then(|e| e.detail("journal", JOURNAL))
                    .and_then(|e| e.detail("cluster", cluster.cluster.to_string()))
                    .and_then(|e| e.detail("before", cluster.before.map(rfc3339)))
                    .and_then(|e| e.detail("after", rfc3339(cluster.after)))
                    .and_then(|e| e.detail("deleted", cluster.deleted))
                    .and_then(|e| e.detail("dropped", dropped.clone()))
                    .map_err(store)?;
                tx.audit(&event).await?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
