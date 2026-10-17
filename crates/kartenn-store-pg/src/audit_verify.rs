//! Verifying the audit journal (spec 012, REQ-012-14; task 012-10): each batch is
//! recomputed from the fingerprints of its members, chained to the previous seal, its
//! signature checked; each event still present is checked against its fingerprint; an
//! event missing outside a recorded purge is a deletion.

use std::collections::BTreeSet;

use kartenn_core::audit::jcs::{batch_hash, event_hash};
use kartenn_core::audit::seal::verify as verify_signature;
use kartenn_core::audit::{AUDIT_CHAIN_BROKEN, Outcome};
use kartenn_service::StoreError;
use kartenn_service::audit::{Instance, RequestContext};
use sqlx::PgPool;
use sqlx::types::time::Date;

fn store_err(e: impl std::fmt::Display) -> StoreError {
    StoreError(e.to_string())
}

/// Unsealed events older than this are reported: a legitimate one is sealed within a
/// minute (REQ-012-13).
const LATE_SECONDS: f64 = 600.0;

/// What a verification found wrong, at the first faulty seal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    /// Sequence of the seal.
    pub seal: i64,
    /// What is wrong.
    pub reason: String,
}

/// Result of a verification.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    /// Seals checked.
    pub seals: usize,
    /// Events checked against their fingerprint.
    pub events: usize,
    /// Members whose event was purged by retention.
    pub purged: usize,
    /// Events not sealed though older than ten minutes.
    pub late_unsealed: i64,
    /// The first faulty seal, if any.
    pub broken: Option<Broken>,
}

/// A seal: sequence, previous fingerprint, fingerprint, count, signature.
type SealRow = (i64, Vec<u8>, Vec<u8>, i32, Option<Vec<u8>>);
/// A member: event sequence, criticality, month of recording, stored fingerprint, row.
type MemberRow = (i64, String, Date, Vec<u8>, Option<serde_json::Value>);

fn bytes32(v: &[u8]) -> Result<[u8; 32], StoreError> {
    <[u8; 32]>::try_from(v).map_err(store_err)
}

/// Verifies the whole chain; with `public`, the signatures too (a seal without signature
/// then breaks the chain).
///
/// # Errors
///
/// Database errors (a faulty journal is a [`Report::broken`], not an error).
pub async fn verify(pool: &PgPool, public: Option<&[u8; 32]>) -> Result<Report, StoreError> {
    let mut report = Report::default();
    let purges: BTreeSet<(String, Date)> =
        sqlx::query_as::<_, (String, Date)>("SELECT criticality, month FROM audit.purge")
            .fetch_all(pool)
            .await
            .map_err(store_err)?
            .into_iter()
            .collect();
    let seals: Vec<SealRow> = sqlx::query_as(
        "SELECT seq, prev_hash, hash, count, signature FROM audit.seal ORDER BY seq",
    )
    .fetch_all(pool)
    .await
    .map_err(store_err)?;
    let mut previous: Option<[u8; 32]> = None;
    for (seq, prev_hash, hash, count, signature) in seals {
        report.seals += 1;
        let broken = |reason: String| Broken { seal: seq, reason };
        let prev_hash = bytes32(&prev_hash)?;
        let hash = bytes32(&hash)?;
        if previous.is_some_and(|p| p != prev_hash) {
            report.broken = Some(broken("does not name the previous seal".into()));
            break;
        }
        let members: Vec<MemberRow> = sqlx::query_as(
            "SELECT s.event_seq, s.criticality, date_trunc('month', s.recorded_at)::date,
                    s.event_hash, (SELECT to_jsonb(e) FROM audit.event e
                                   WHERE e.seq = s.event_seq AND e.criticality = s.criticality
                                     AND e.recorded_at = s.recorded_at)
             FROM audit.sealed s WHERE s.seal_seq = $1 ORDER BY s.event_seq",
        )
        .bind(seq)
        .fetch_all(pool)
        .await
        .map_err(store_err)?;
        if members.len() != usize::try_from(count).unwrap_or(0) {
            report.broken = Some(broken(format!(
                "{} members for a seal of {count}",
                members.len()
            )));
            break;
        }
        let mut hashes = Vec::with_capacity(members.len());
        let mut fault = None;
        for (event_seq, criticality, month, stored, row) in members {
            let stored = bytes32(&stored)?;
            match row {
                Some(json) => {
                    report.events += 1;
                    if event_hash(&json).map_err(store_err)? != stored {
                        fault = Some(format!("event {event_seq} was modified"));
                    }
                }
                None if purges.contains(&(criticality, month)) => report.purged += 1,
                None => fault = Some(format!("event {event_seq} was deleted")),
            }
            if fault.is_some() {
                break;
            }
            hashes.push(stored);
        }
        if let Some(reason) = fault {
            report.broken = Some(broken(reason));
            break;
        }
        if batch_hash(&prev_hash, &hashes) != hash {
            report.broken = Some(broken("the fingerprint of the batch differs".into()));
            break;
        }
        if let Some(public) = public {
            let signed = signature
                .as_deref()
                .is_some_and(|s| verify_signature(public, &hash, s));
            if !signed {
                report.broken = Some(broken("missing or invalid signature".into()));
                break;
            }
        }
        previous = Some(hash);
    }
    report.late_unsealed = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event e
         WHERE e.recorded_at < now() - make_interval(secs => $1)
           AND NOT EXISTS (SELECT 1 FROM audit.sealed s WHERE s.event_seq = e.seq)",
    )
    .bind(LATE_SECONDS)
    .fetch_one(pool)
    .await
    .map_err(store_err)?;
    Ok(report)
}

/// [`verify`], recording `audit.chain.broken` when the chain is broken (the planned
/// verification).
///
/// # Errors
///
/// As [`verify`], and if the event cannot be recorded.
pub async fn verify_and_record(
    pool: &PgPool,
    public: Option<&[u8; 32]>,
    instance: &Instance,
) -> Result<Report, StoreError> {
    let report = verify(pool, public).await?;
    if let Some(broken) = &report.broken {
        let event = RequestContext::system()
            .event(&AUDIT_CHAIN_BROKEN, Outcome::Failure)
            .map_err(store_err)?
            .reason(broken.reason.clone())
            .detail("seal", broken.seal)
            .map_err(store_err)?;
        crate::audit_journal::insert(pool, &event, instance).await?;
    }
    Ok(report)
}
