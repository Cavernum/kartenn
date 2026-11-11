//! The background tasks of the journal (task 019-6c; REQ-019-16; ADR 0043, decisions 5
//! and 8 as amended; Q-019-20, Q-019-24): the verification recomputes the fingerprint of
//! the batches the purge has not touched against their audit event; the purge goes by
//! cluster under a bound it writes, drops a day every cluster has passed, never an open
//! observation, and creates the days ahead.

use std::time::Duration;

use kartenn_core::audit::OBSERVATION_BATCH_RECORDED;
use kartenn_core::audit::seal::hex;
use kartenn_kube::jobs::{self, Purged};
use kartenn_kube::observations::{Batch, Opening, Recorded, record};
use kartenn_service::audit::{Instance, Outcome, RequestContext};
use kartenn_service::module::run_job;
use kartenn_store_pg::audit_journal::PgAuditJournal;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use super::{Env, as_app, closing, hours, op, rehash};

const VERIFY: &str = "kube.observations_verify";
const PURGE: &str = "kube.observations_purge";

/// `n` days.
const fn days(n: u64) -> Duration {
    hours(24 * n)
}

/// The partition of the day of `at` (UTC).
fn day_of(at: OffsetDateTime) -> String {
    let at = at.to_offset(sqlx::types::time::UtcOffset::UTC);
    format!(
        "pod_observation_{:04}{:02}{:02}",
        at.year(),
        u8::from(at.month()),
        at.day()
    )
}

/// The days of the closed observations, by name.
async fn days_of(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid
          WHERE i.inhparent = 'kube.pod_observation_closed'::regclass ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Whether the days from today to a week ahead (UTC, as PostgreSQL has it) all exist.
async fn week_ahead(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT bool_and(to_regclass(format('kube.pod_observation_%s',
                    to_char((now() AT TIME ZONE 'UTC')::date + d, 'YYYYMMDD'))) IS NOT NULL)
           FROM generate_series(0, 7) AS d",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The purge bound of `cluster`.
async fn bound(pool: &PgPool, cluster: Uuid) -> Option<OffsetDateTime> {
    sqlx::query_scalar(
        "SELECT purged_before FROM kube.observation_purge_bound WHERE cluster_id = $1",
    )
    .bind(cluster)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// The pods of the observations of `cluster`, open or not, by pod.
async fn rows(pool: &PgPool, cluster: Uuid) -> Vec<(Uuid, bool)> {
    sqlx::query_as(
        "SELECT pod_uid, upper_inf(observed) FROM kube.pod_observation
          WHERE cluster_id = $1 ORDER BY pod_uid",
    )
    .bind(cluster)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// `cluster` with a retention of `days` days.
async fn retention(env: &Env, cluster: Uuid, n: u16) {
    env.applied(
        env.admin,
        vec![op(
            "update_cluster",
            json!({"id": {"id": cluster}, "retention_days": n}),
        )],
    )
    .await;
}

/// `batch` of `prod` by its connector, with its event `observation.batch.recorded` in its
/// transaction, as the route writes it (019-6b).
async fn sealed(env: &Env, batch: &Batch) -> Recorded {
    let mut tx = as_app(&env.pool).await;
    let recorded = record(&mut tx, env.prod, env.connector, batch)
        .await
        .unwrap();
    let event = RequestContext::system()
        .event(&OBSERVATION_BATCH_RECORDED, Outcome::Success)
        .map(|e| e.target("object", [env.prod]))
        .and_then(|e| e.detail("journal", "kube.pod_observation"))
        .and_then(|e| e.detail("cluster", env.prod.to_string()))
        .and_then(|e| e.detail("batch", recorded.id.to_string()))
        .and_then(|e| e.detail("opened", recorded.opened))
        .and_then(|e| e.detail("closed", recorded.closed))
        .and_then(|e| e.detail("hash", hex(&recorded.hash)))
        .and_then(|e| e.detail("clamped", recorded.clamped))
        .and_then(|e| e.detail("skew_max", recorded.skew_max_ms))
        .unwrap();
    PgAuditJournal::new(env.pool.clone(), Instance::new("test"))
        .record_in(&mut tx, &event)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    recorded
}

/// The events `observation.batch.altered`: severity, criticality, target, details.
async fn altered(pool: &PgPool) -> Vec<(String, String, Vec<Uuid>, Value)> {
    sqlx::query_as(
        "SELECT severity, criticality, target_ids, details FROM audit.event
          WHERE name = 'observation.batch.altered' ORDER BY details->>'batch'",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// ADR 0043, decision 5: the daily verification recomputes each whole batch from its rows
/// and audits `observation.batch.altered` (ERROR, high, `expected` and `found` in
/// hexadecimal) for a row deleted outside the application; the fingerprint of the audit
/// event is authoritative, the copy of `observation_batch` stands in only for an event the
/// audit no longer holds.
#[kartenn_testing::pg_test]
async fn altered_batch_detected(pool: PgPool) {
    let env = Env::new(&pool).await;
    let now = OffsetDateTime::now_utc();
    let a = sealed(
        &env,
        &Batch {
            opened: vec![env.pod(1, "198.51.100.17"), env.pod(2, "198.51.100.18")],
            closed: Vec::new(),
        },
    )
    .await;
    let b = sealed(
        &env,
        &Batch {
            opened: vec![env.pod(3, "198.51.100.19")],
            closed: vec![closing(1, "198.51.100.17", now - Duration::from_secs(60))],
        },
    )
    .await;
    let c = sealed(
        &env,
        &Batch {
            opened: vec![env.pod(4, "198.51.100.20")],
            closed: Vec::new(),
        },
    )
    .await;
    sealed(
        &env,
        &Batch {
            opened: Vec::new(),
            closed: Vec::new(),
        },
    )
    .await;
    // No event in the audit: the copy stands in.
    let d = env
        .recorded(&Batch {
            opened: vec![env.pod(5, "198.51.100.21")],
            closed: Vec::new(),
        })
        .await;
    assert!(run_job(&env.store, VERIFY).await.unwrap());
    assert!(altered(&pool).await.is_empty(), "nothing altered yet");

    // Outside the application: a row of `a` deleted; a row of `b` deleted and its copy
    // rewritten to fit; the copy of `c` rewritten, its rows intact; the copy of `d`.
    let owner = |sql: &'static str, id: Uuid| sqlx::query(sql).bind(id);
    owner(
        "DELETE FROM kube.pod_observation WHERE pod_uid = $1",
        Uuid::from_u128(2),
    )
    .execute(&pool)
    .await
    .unwrap();
    owner(
        "DELETE FROM kube.pod_observation WHERE pod_uid = $1",
        Uuid::from_u128(3),
    )
    .execute(&pool)
    .await
    .unwrap();
    let rewritten = rehash(&pool, env.prod, b.id).await;
    let copy = |id: Uuid, hash: [u8; 32]| {
        sqlx::query("UPDATE kube.observation_batch SET content_hash = $2 WHERE id = $1")
            .bind(id)
            .bind(hash.to_vec())
            .execute(&pool)
    };
    copy(b.id, rewritten).await.unwrap();
    copy(c.id, [7; 32]).await.unwrap();
    copy(d.id, [9; 32]).await.unwrap();

    // Under the role of the application, as the task reads.
    let mut tx = as_app(&pool).await;
    let verified = jobs::verify(&mut tx).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(verified.checked, 5, "every batch is whole");
    let found: Vec<(Uuid, String, String)> = verified
        .altered
        .iter()
        .map(|a| (a.batch, a.expected.clone(), a.found.clone()))
        .collect();
    let mut expected = vec![
        (
            a.id,
            hex(&a.hash),
            hex(&rehash(&pool, env.prod, a.id).await),
        ),
        (b.id, hex(&b.hash), hex(&rewritten)),
        (d.id, hex(&[9; 32]), hex(&d.hash)),
    ];
    expected.sort();
    assert_eq!(found, expected, "not `c`: its audit event holds");
    assert!(!found.iter().any(|(id, ..)| *id == c.id));
    assert!(verified.altered.iter().all(|a| a.cluster == env.prod));

    assert!(run_job(&env.store, VERIFY).await.unwrap());
    let events = altered(&pool).await;
    let mut seen: Vec<(String, String, String)> = events
        .iter()
        .map(|(severity, criticality, target, details)| {
            assert_eq!((severity.as_str(), criticality.as_str()), ("ERROR", "high"));
            assert_eq!(target, &[env.prod]);
            assert_eq!(details["journal"], "kube.pod_observation");
            let text = |key: &str| details[key].as_str().unwrap().to_owned();
            (text("batch"), text("expected"), text("found"))
        })
        .collect();
    seen.sort();
    let mut wanted: Vec<(String, String, String)> = expected
        .into_iter()
        .map(|(id, e, f)| (id.to_string(), e, f))
        .collect();
    wanted.sort();
    assert_eq!(seen, wanted, "one event by batch altered");
}

/// Q-019-20: the purge goes by cluster, each under its own retention and bound, which
/// never moves back; a day is dropped once every cluster has passed it, a day one cluster
/// still keeps stays for it; the days of the coming week are created. A batch the purge
/// has touched is no longer verified (Q-019-24). The role of the application runs it.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // two clusters, three purges, their effects in order
async fn purge_drops_old_partitions(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    let stage = env.cluster("stage", group).await;
    let (stage_node, stage_web) = env.populated(stage, other, "198.51.100.64/26").await;
    retention(&env, stage, 60).await;
    let now = OffsetDateTime::now_utc();
    let seen = |pod: u128, ip: &str, from: OffsetDateTime| Opening {
        from,
        ..env.pod(pod, ip)
    };
    // `prod` (30 days): ended 70, 45 and 35 days ago, one open since 80 days.
    env.recorded(&Batch {
        opened: vec![
            seen(1, "198.51.100.17", now - days(75)),
            seen(2, "198.51.100.18", now - days(50)),
            seen(3, "198.51.100.19", now - days(40)),
            seen(4, "198.51.100.20", now - days(80)),
        ],
        closed: vec![
            closing(1, "198.51.100.17", now - days(70)),
            closing(2, "198.51.100.18", now - days(45)),
            closing(3, "198.51.100.19", now - days(35)),
        ],
    })
    .await;
    // `stage` (60 days): ended 70 and 45 days ago.
    let staged = |pod: u128, ip: &str, from: OffsetDateTime| Opening {
        workload: Some(stage_web),
        node: Some(stage_node),
        ..seen(pod, ip, from)
    };
    env.record_by(
        stage,
        other,
        &Batch {
            opened: vec![
                staged(11, "198.51.100.71", now - days(75)),
                staged(12, "198.51.100.72", now - days(50)),
            ],
            closed: vec![
                closing(11, "198.51.100.71", now - days(70)),
                closing(12, "198.51.100.72", now - days(45)),
            ],
        },
    )
    .await
    .unwrap();
    // A batch of today, whole after every purge.
    sealed(
        &env,
        &Batch {
            opened: vec![env.pod(5, "198.51.100.21")],
            closed: Vec::new(),
        },
    )
    .await;
    let (d70, d45, d35) = (
        day_of(now - days(70)),
        day_of(now - days(45)),
        day_of(now - days(35)),
    );
    assert!(days_of(&pool).await.contains(&d70));

    assert!(run_job(&env.store, PURGE).await.unwrap());
    let u = Uuid::from_u128;
    assert_eq!(rows(&pool, env.prod).await, [(u(4), true), (u(5), true)]);
    assert_eq!(
        rows(&pool, stage).await,
        [(u(12), false)],
        "stage keeps 60 days"
    );
    let names = days_of(&pool).await;
    assert!(!names.contains(&d70), "passed by both clusters: {names:?}");
    assert!(
        names.contains(&d45) && names.contains(&d35),
        "stage has not passed them"
    );
    assert!(week_ahead(&pool).await, "the coming week created");
    let prod_bound = bound(&pool, env.prod).await.unwrap();
    let stage_bound = bound(&pool, stage).await.unwrap();
    let near = |at: OffsetDateTime, ago: Duration| ((now - ago) - at).abs() < hours(1);
    assert!(near(prod_bound, days(30)), "{prod_bound}");
    assert!(near(stage_bound, days(60)), "{stage_bound}");

    // The batches the purge touched are no longer read, the others still are; the role of
    // the application verifies and purges.
    let mut tx = as_app(&pool).await;
    let verified = jobs::verify(&mut tx).await.unwrap();
    assert_eq!(verified.checked, 1, "only the batch of today is whole");
    assert!(verified.altered.is_empty(), "{:?}", verified.altered);
    let again = jobs::purge(&mut tx).await.unwrap();
    assert_eq!(again, Purged::default(), "nothing more to purge");
    tx.commit().await.unwrap();
    assert!(run_job(&env.store, VERIFY).await.unwrap());
    assert!(altered(&pool).await.is_empty());

    // `stage` down to 30 days: its last row goes, and with it the days both have passed.
    retention(&env, stage, 30).await;
    // `prod` up to 400 days: its bound, moved by the purge just above, does not move back.
    let prod_bound = bound(&pool, env.prod).await.unwrap();
    retention(&env, env.prod, 400).await;
    let mut tx = as_app(&pool).await;
    let purged = jobs::purge(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!((purged.deleted, purged.dropped), (1, 2), "{purged:?}");
    assert!(rows(&pool, stage).await.is_empty());
    let names = days_of(&pool).await;
    assert!(!names.contains(&d45) && !names.contains(&d35), "{names:?}");
    assert_eq!(bound(&pool, env.prod).await, Some(prod_bound), "never back");
    assert!(near(bound(&pool, stage).await.unwrap(), days(30)));
}

/// ADR 0043, decision 8: an open observation is never purged, whatever its age and the
/// retention; the retention runs from the end of an observation. A batch that closes in
/// a day created ahead creates none.
#[kartenn_testing::pg_test]
async fn open_observation_never_purged(pool: PgPool) {
    let env = Env::new(&pool).await;
    retention(&env, env.prod, 1).await;
    let now = OffsetDateTime::now_utc();
    let seen = |pod: u128, ip: &str| Opening {
        from: now - days(100),
        ..env.pod(pod, ip)
    };
    env.recorded(&Batch {
        opened: vec![seen(1, "198.51.100.17"), seen(2, "198.51.100.18")],
        closed: vec![closing(2, "198.51.100.18", now - days(90))],
    })
    .await;
    assert!(run_job(&env.store, PURGE).await.unwrap());
    let u = Uuid::from_u128;
    assert_eq!(rows(&env.pool, env.prod).await, [(u(1), true)]);

    // Closed now, in a day the purge created: no day created by the batch, and its end
    // is within the retention.
    let before = days_of(&pool).await;
    env.recorded(&Batch {
        opened: Vec::new(),
        closed: vec![closing(1, "198.51.100.17", OffsetDateTime::now_utc())],
    })
    .await;
    assert_eq!(days_of(&pool).await, before);
    assert!(run_job(&env.store, PURGE).await.unwrap());
    assert_eq!(rows(&env.pool, env.prod).await, [(u(1), false)]);
}
