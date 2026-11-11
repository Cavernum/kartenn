//! The background tasks of the journal of the pods observed (task 019-6c; REQ-019-16;
//! ADR 0043, decisions 5 and 8; 019/Q-019-20, Q-019-24), run under the lease of the
//! core's tasks (ADR 0023): `kube.observations_verify` recomputes the fingerprint of every
//! batch the purge has not touched and audits `observation.batch.altered` for one that no
//! longer matches; `kube.observations_purge` deletes, cluster by cluster, the closed
//! observations past the retention of their cluster, moves its purge bound, drops the
//! days every cluster has passed and creates the days ahead.

use std::net::IpAddr;
use std::time::Duration;

use kartenn_core::audit::seal::hex;
use kartenn_core::audit::{OBSERVATION_BATCH_ALTERED, Outcome};
use kartenn_service::audit::RequestContext;
use kartenn_service::module::{JobSpec, ModuleTx};
use kartenn_service::port::StoreError;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::observations::{Closing, JOURNAL, Opening, content_hash};

/// The verification of the fingerprints, within the module.
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

/// The batches a statement of the verification reads back.
const CHUNK: i64 = 500;

/// A batch whose rows no longer give its fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Altered {
    /// Its cluster.
    pub cluster: Uuid,
    /// The batch.
    pub batch: Uuid,
    /// Its fingerprint, in hexadecimal: the one of its audit event
    /// (`observation.batch.recorded`, which is authoritative), else, that event purged
    /// from the audit, the copy of `observation_batch`.
    pub expected: String,
    /// The fingerprint recomputed from its rows, in hexadecimal.
    pub found: String,
}

/// What a verification read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verified {
    /// The batches recomputed: those whose earliest instant is not before the purge bound
    /// of their cluster.
    pub checked: u64,
    /// Those that no longer match.
    pub altered: Vec<Altered>,
}

/// What a purge did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Purged {
    /// The closed observations deleted.
    pub deleted: u64,
    /// The days dropped, passed by every cluster.
    pub dropped: u32,
    /// The days created ahead.
    pub created: u32,
}

fn store(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// A whole batch and its rows, as the verification reads them: its identifier, cluster,
/// copy of the fingerprint and fingerprint of its audit event; the columns of its
/// openings; the columns of its closings.
type BatchRows = (
    Uuid,
    Uuid,
    Vec<u8>,
    Option<String>,
    Option<Vec<String>>,
    Option<Vec<String>>,
    Option<Vec<Uuid>>,
    Option<Vec<Option<Uuid>>>,
    Option<Vec<Option<Uuid>>>,
    Option<Vec<Option<String>>>,
    Option<Vec<bool>>,
    Option<Vec<OffsetDateTime>>,
    Option<Vec<Uuid>>,
    Option<Vec<Option<String>>>,
    Option<Vec<OffsetDateTime>>,
);

/// The whole batches among `$1` and their rows, in one statement: the purge bounds and
/// the rows are read in one snapshot, so that a purge committed meanwhile never makes a
/// batch look altered. The audit event of a batch is inserted in its transaction, at its
/// instant (019-6b).
const WHOLE: &str = "
WITH whole AS (
    SELECT b.id, b.cluster_id, b.received_at, b.content_hash
      FROM kube.observation_batch b
      LEFT JOIN kube.observation_purge_bound p ON p.cluster_id = b.cluster_id
     WHERE b.id = ANY($1) AND b.earliest >= coalesce(p.purged_before, '-infinity')),
opened AS (
    SELECT opened_batch AS id, array_agg(namespace) AS namespaces, array_agg(pod_name) AS pods,
           array_agg(pod_uid) AS uids, array_agg(workload_id) AS workloads,
           array_agg(node_id) AS nodes, array_agg(host(ip)) AS ips,
           array_agg(host_network) AS host_networks, array_agg(lower(observed)) AS froms
      FROM kube.pod_observation
     WHERE opened_batch = ANY($1)
     GROUP BY opened_batch),
closed AS (
    SELECT closed_batch AS id, array_agg(pod_uid) AS uids, array_agg(host(ip)) AS ips,
           array_agg(upper(observed)) AS ats
      FROM kube.pod_observation
     WHERE closed_batch = ANY($1)
     GROUP BY closed_batch)
SELECT w.id, w.cluster_id, w.content_hash,
       (SELECT e.details->>'hash' FROM audit.event e
         WHERE e.name = 'observation.batch.recorded' AND e.recorded_at = w.received_at
           AND e.details->>'batch' = w.id::text
         LIMIT 1),
       o.namespaces, o.pods, o.uids, o.workloads, o.nodes, o.ips, o.host_networks, o.froms,
       c.uids, c.ips, c.ats
  FROM whole w
  LEFT JOIN opened o ON o.id = w.id
  LEFT JOIN closed c ON c.id = w.id";

fn address(ip: Option<String>) -> Result<Option<IpAddr>, StoreError> {
    ip.map(|ip| ip.parse().map_err(store)).transpose()
}

/// The `i`-th value of an aggregated column, which every row of the batch fills.
fn nth<T: Clone>(column: Option<&Vec<T>>, i: usize) -> Result<T, StoreError> {
    column
        .and_then(|values| values.get(i).cloned())
        .ok_or_else(|| store("a row of the journal read back without one of its columns"))
}

/// The batch of `rows` as its rows give it back: `(batch, cluster, expected, found)`.
fn recomputed(rows: BatchRows) -> Result<(Uuid, Uuid, String, String), StoreError> {
    let (
        id,
        cluster,
        copy,
        sealed,
        namespaces,
        pods,
        uids,
        workloads,
        nodes,
        ips,
        hosts,
        froms,
        closed_uids,
        closed_ips,
        ats,
    ) = rows;
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
    let mut closed = Vec::new();
    for (i, pod_uid) in closed_uids.unwrap_or_default().into_iter().enumerate() {
        closed.push(Closing {
            pod_uid,
            ip: address(nth(closed_ips.as_ref(), i)?)?,
            at: nth(ats.as_ref(), i)?,
        });
    }
    let found = content_hash(cluster, &opened, &closed).map_err(store)?;
    Ok((
        id,
        cluster,
        sealed.unwrap_or_else(|| hex(&copy)),
        hex(&found),
    ))
}

/// Recomputes the fingerprint of every batch whose earliest instant is not before the
/// purge bound of its cluster, from the rows it opened and closed, against the one of its
/// audit event (ADR 0043, decision 5; Q-019-24): a batch the purge has touched is not
/// read, a row deleted outside the purge alters its batch. Reads only.
///
/// # Errors
///
/// [`StoreError`] of the database.
pub async fn verify(conn: &mut PgConnection) -> Result<Verified, StoreError> {
    let mut verified = Verified::default();
    let mut after = Uuid::nil();
    loop {
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM kube.observation_batch WHERE id > $1 ORDER BY id LIMIT $2",
        )
        .bind(after)
        .bind(CHUNK)
        .fetch_all(&mut *conn)
        .await
        .map_err(store)?;
        let Some(last) = ids.last() else {
            break;
        };
        after = *last;
        let rows: Vec<BatchRows> = sqlx::query_as(WHOLE)
            .bind(&ids)
            .fetch_all(&mut *conn)
            .await
            .map_err(store)?;
        for rows in rows {
            let (batch, cluster, expected, found) = recomputed(rows)?;
            verified.checked += 1;
            if expected != found {
                verified.altered.push(Altered {
                    cluster,
                    batch,
                    expected,
                    found,
                });
            }
        }
        if ids.len() < usize::try_from(CHUNK).unwrap_or(usize::MAX) {
            break;
        }
    }
    verified.altered.sort_by_key(|a| a.batch);
    Ok(verified)
}

/// Purges the journal at the instant of the transaction, by the function of the owner of
/// the schema `kube.observation_purge` (Q-019-20): by cluster, the closed observations
/// ended before its retention, an open one never; its bound moved; the days every cluster
/// has passed dropped; the days of the coming week created.
///
/// # Errors
///
/// [`StoreError`] of the database.
pub async fn purge(conn: &mut PgConnection) -> Result<Purged, StoreError> {
    let (deleted, dropped, created): (i64, i32, i32) =
        sqlx::query_as("SELECT deleted, dropped, created FROM kube.observation_purge()")
            .fetch_one(&mut *conn)
            .await
            .map_err(store)?;
    Ok(Purged {
        deleted: u64::try_from(deleted).unwrap_or_default(),
        dropped: u32::try_from(dropped).unwrap_or_default(),
        created: u32::try_from(created).unwrap_or_default(),
    })
}

/// Runs the task `name` of the journal in `tx`; `false` for another task. The
/// verification audits each batch it finds altered in that transaction
/// (`observation.batch.altered`, ERROR, high), targeting its cluster.
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
                    .map(|e| e.target("object", [altered.cluster]))
                    .and_then(|e| e.detail("journal", JOURNAL))
                    .and_then(|e| e.detail("batch", altered.batch.to_string()))
                    .and_then(|e| e.detail("expected", altered.expected.as_str()))
                    .and_then(|e| e.detail("found", altered.found.as_str()))
                    .map_err(store)?;
                tx.audit(&event).await?;
            }
            Ok(true)
        }
        PURGE => purge(conn).await.map(|_| true),
        _ => Ok(false),
    }
}
