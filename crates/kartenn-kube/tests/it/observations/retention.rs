//! The background tasks of the journal (task 019-6c; REQ-019-16; ADR 0043, decisions 4,
//! 5 and 8 as amended; Q-019-20, Q-019-24, Q-019-27): the verification checks the journal
//! against what the audit sealed of it (one event by batch, its instant, its earliest
//! instant, its fingerprint; the bound of each purge); the purge goes by cluster under a
//! bound it seals, under the largest recent retention, drops a day every cluster has
//! passed, never an open observation, and creates the days ahead; a row cites only a
//! batch of its own transaction.

use std::time::Duration;

use kartenn_core::audit::OBSERVATION_BATCH_RECORDED;
use kartenn_core::audit::seal::hex;
use kartenn_kube::jobs::{self, reasons};
use kartenn_kube::observations::{Batch, Opening, Recorded, record, rfc3339};
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

/// `cluster` with a retention of `n` days.
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

/// The event `observation.batch.recorded` of `recorded`, as the route writes it (019-6b).
fn recorded_event(cluster: Uuid, recorded: &Recorded) -> kartenn_core::audit::AuditEvent {
    RequestContext::system()
        .event(&OBSERVATION_BATCH_RECORDED, Outcome::Success)
        .map(|e| e.target("object", [cluster]))
        .and_then(|e| e.detail("journal", "kube.pod_observation"))
        .and_then(|e| e.detail("cluster", cluster.to_string()))
        .and_then(|e| e.detail("batch", recorded.id.to_string()))
        .and_then(|e| e.detail("opened", recorded.opened))
        .and_then(|e| e.detail("closed", recorded.closed))
        .and_then(|e| e.detail("hash", hex(&recorded.hash)))
        .and_then(|e| e.detail("clamped", recorded.clamped))
        .and_then(|e| e.detail("skew_max", recorded.skew_max_ms))
        .and_then(|e| e.detail("earliest", rfc3339(recorded.earliest)))
        .unwrap()
}

/// `batch` of `prod` by its connector, with its event in its transaction.
async fn sealed(env: &Env, batch: &Batch) -> Recorded {
    sealed_by(env, env.prod, env.connector, batch).await
}

/// `batch` of `cluster` by `author`, with its event in its transaction.
async fn sealed_by(env: &Env, cluster: Uuid, author: Uuid, batch: &Batch) -> Recorded {
    let mut tx = as_app(&env.pool).await;
    let recorded = record(&mut tx, cluster, author, batch).await.unwrap();
    PgAuditJournal::new(env.pool.clone(), Instance::new("test"))
        .record_in(&mut tx, &recorded_event(cluster, &recorded))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    recorded
}

/// One opening of `prod` sealed.
async fn one(env: &Env, pod: u128, ip: &str) -> Recorded {
    sealed(
        env,
        &Batch {
            opened: vec![env.pod(pod, ip)],
            closed: Vec::new(),
        },
    )
    .await
}

/// The gaps of a verification under the role of the application, by batch and reason.
async fn gaps(pool: &PgPool) -> (u64, Vec<(Option<Uuid>, &'static str)>) {
    let mut tx = as_app(pool).await;
    let verified = jobs::verify(&mut tx).await.unwrap();
    tx.rollback().await.unwrap();
    let mut found: Vec<(Option<Uuid>, &'static str)> = verified
        .altered
        .iter()
        .map(|a| (a.batch, a.reason))
        .collect();
    found.sort();
    (verified.checked, found)
}

/// The events `observation.batch.altered`: severity, criticality, target, reason, details.
async fn altered(pool: &PgPool) -> Vec<(String, String, Vec<Uuid>, Option<String>, Value)> {
    sqlx::query_as(
        "SELECT severity, criticality, target_ids, reason, details FROM audit.event
          WHERE name = 'observation.batch.altered' ORDER BY details->>'batch'",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// The details of the events `observation.journal.purged`, by cluster then bound.
async fn purges(pool: &PgPool) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT details FROM audit.event WHERE name = 'observation.journal.purged'
          ORDER BY details->>'cluster', details->>'after'",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// `sql` run by the owner on `id`.
async fn owner(pool: &PgPool, sql: &'static str, id: Uuid) {
    sqlx::query(sql).bind(id).execute(pool).await.unwrap();
}

/// ADR 0043, decision 5, and Q-019-27: the daily verification finds each batch's one
/// sealed event by its identifier and reports, under its `reason`, a row deleted outside
/// the application (`content`, against the sealed fingerprint, whatever the copy says), a
/// missing event (`event_missing`; the copy stands in only for a month the audit purged),
/// a second event (`event_duplicated`), an instant moved (`received_at`), an earliest
/// instant lowered (`earliest`); one event ERROR, high, by gap, targeting the cluster.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // seven batches, each altered its own way
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
    let c = one(&env, 4, "198.51.100.20").await;
    sealed(
        &env,
        &Batch {
            opened: Vec::new(),
            closed: Vec::new(),
        },
    )
    .await;
    // No event in the audit.
    let d = env
        .recorded(&Batch {
            opened: vec![env.pod(5, "198.51.100.21")],
            closed: Vec::new(),
        })
        .await;
    let e = one(&env, 6, "198.51.100.22").await;
    let f = one(&env, 7, "198.51.100.23").await;
    let g = one(&env, 8, "198.51.100.24").await;
    assert_eq!(
        gaps(&pool).await,
        (7, vec![(Some(d.id), reasons::EVENT_MISSING)]),
        "an event missing in a month the audit keeps"
    );
    // The audit purges the month by its retention: the copy stands in.
    sqlx::query(
        "INSERT INTO audit.purge (criticality, month)
         VALUES ('normal', date_trunc('month', now())::date)",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(gaps(&pool).await, (8, vec![]));

    // Outside the application: a row of `a` deleted; a row of `b` deleted and its copy
    // rewritten to fit; the copy of `c` rewritten, its rows intact; the copy of `d`; the
    // instant of `e` moved; a second event for `f`; the earliest instant of `g` lowered.
    owner(
        &pool,
        "DELETE FROM kube.pod_observation WHERE pod_uid = $1",
        Uuid::from_u128(2),
    )
    .await;
    owner(
        &pool,
        "DELETE FROM kube.pod_observation WHERE pod_uid = $1",
        Uuid::from_u128(3),
    )
    .await;
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
    owner(
        &pool,
        "UPDATE kube.observation_batch SET received_at = received_at - interval '1 minute'
          WHERE id = $1",
        e.id,
    )
    .await;
    let mut tx = as_app(&pool).await;
    PgAuditJournal::new(pool.clone(), Instance::new("test"))
        .record_in(&mut tx, &recorded_event(env.prod, &f))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    owner(
        &pool,
        "UPDATE kube.observation_batch SET earliest = earliest - interval '1 day' WHERE id = $1",
        g.id,
    )
    .await;

    let mut tx = as_app(&pool).await;
    let verified = jobs::verify(&mut tx).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(
        verified.checked, 5,
        "a, b, c, d and the empty one recomputed"
    );
    let mut found: Vec<(Uuid, &str, String, String)> = verified
        .altered
        .iter()
        .map(|x| {
            (
                x.batch.unwrap(),
                x.reason,
                x.expected.clone(),
                x.found.clone(),
            )
        })
        .collect();
    found.sort();
    let shifted: OffsetDateTime =
        sqlx::query_scalar("SELECT received_at FROM kube.observation_batch WHERE id = $1")
            .bind(e.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut wanted = vec![
        (
            a.id,
            reasons::CONTENT,
            hex(&a.hash),
            hex(&rehash(&pool, env.prod, a.id).await),
        ),
        (b.id, reasons::CONTENT, hex(&b.hash), hex(&rewritten)),
        (d.id, reasons::CONTENT, hex(&[9; 32]), hex(&d.hash)),
        (
            e.id,
            reasons::RECEIVED_AT,
            rfc3339(e.received_at),
            rfc3339(shifted),
        ),
        (f.id, reasons::EVENT_DUPLICATED, "1".into(), "2".into()),
        (
            g.id,
            reasons::EARLIEST,
            rfc3339(g.earliest),
            rfc3339(g.earliest - days(1)),
        ),
    ];
    wanted.sort();
    assert_eq!(found, wanted, "not `c`: its sealed event holds");
    assert!(verified.altered.iter().all(|x| x.cluster == env.prod));

    assert!(run_job(&env.store, VERIFY).await.unwrap());
    let events = altered(&pool).await;
    assert_eq!(events.len(), 6, "one event by gap");
    for (severity, criticality, target, reason, details) in &events {
        assert_eq!((severity.as_str(), criticality.as_str()), ("ERROR", "high"));
        assert_eq!(target, &[env.prod]);
        assert_eq!(details["journal"], "kube.pod_observation");
        let batch: Uuid = details["batch"].as_str().unwrap().parse().unwrap();
        let wanted = wanted.iter().find(|w| w.0 == batch).unwrap();
        assert_eq!(reason.as_deref(), Some(wanted.1));
        assert_eq!(details["expected"], wanted.2.as_str());
        assert_eq!(details["found"], wanted.3.as_str());
    }
}

/// Q-019-20, Q-019-27: the purge goes by cluster, each under its own retention and bound,
/// which never moves back; each move is sealed (`observation.journal.purged`); a day is
/// dropped once every cluster has passed it, a day one cluster still keeps stays for it;
/// the days of the coming week are created. A batch the sealed bound has reached is no
/// longer recomputed (Q-019-24); a bound moved without its sealed purge is reported.
#[kartenn_testing::pg_test]
#[allow(clippy::too_many_lines)] // two clusters, three purges, their effects in order
async fn purge_drops_old_partitions(pool: PgPool) {
    let env = Env::new(&pool).await;
    let (other, group) = crate::support::connector(&pool, "other").await;
    let stage = env.cluster("stage", group).await;
    let (stage_node, stage_web) = env.populated(stage, other, "198.51.100.64/26").await;
    let now = OffsetDateTime::now_utc();
    let seen = |pod: u128, ip: &str, from: OffsetDateTime| Opening {
        from,
        ..env.pod(pod, ip)
    };
    // `prod` (30 days): ended 70, 45 and 35 days ago, one open since 80 days.
    sealed(
        &env,
        &Batch {
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
        },
    )
    .await;
    // `stage` (60 days): ended 70 and 45 days ago.
    let staged = |pod: u128, ip: &str, from: OffsetDateTime| Opening {
        workload: Some(stage_web),
        node: Some(stage_node),
        ..seen(pod, ip, from)
    };
    sealed_by(
        &env,
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
    .await;
    retention(&env, stage, 60).await;
    // A batch of today, whole after every purge.
    one(&env, 5, "198.51.100.21").await;
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
    // Each move sealed, with what it deleted and the day dropped.
    let d70_date = (now - days(70)).date().to_string();
    let sealed: Vec<(String, Value, String, u64, Value)> = purges(&pool)
        .await
        .into_iter()
        .map(|d| {
            (
                d["cluster"].as_str().unwrap().to_owned(),
                d["before"].clone(),
                d["after"].as_str().unwrap().to_owned(),
                d["deleted"].as_u64().unwrap(),
                d["dropped"].clone(),
            )
        })
        .collect();
    let mut wanted = vec![
        (
            env.prod.to_string(),
            Value::Null,
            rfc3339(prod_bound),
            3,
            json!([d70_date]),
        ),
        (
            stage.to_string(),
            Value::Null,
            rfc3339(stage_bound),
            1,
            json!([d70_date]),
        ),
    ];
    wanted.sort_by(|x, y| x.0.cmp(&y.0));
    assert_eq!(sealed, wanted);

    // The batches the sealed bound reached are no longer read, the other still is.
    assert_eq!(gaps(&pool).await, (1, vec![]));
    // The function called without its task: the bounds move, nothing seals them.
    let mut tx = as_app(&pool).await;
    let again = jobs::purge(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        (again.deleted(), again.dropped.len(), again.created),
        (0, 0, 0),
        "nothing more to purge"
    );
    assert_eq!(
        gaps(&pool).await,
        (
            1,
            vec![(None, reasons::PURGE_BOUND), (None, reasons::PURGE_BOUND)]
        ),
        "a bound moved without its sealed purge"
    );
    // Purged by the task, sealed again.
    assert!(run_job(&env.store, PURGE).await.unwrap());
    assert_eq!(gaps(&pool).await, (1, vec![]));

    // `prod` up to 400 days: its bound does not move back.
    let prod_bound = bound(&pool, env.prod).await.unwrap();
    retention(&env, env.prod, 400).await;
    assert!(run_job(&env.store, PURGE).await.unwrap());
    assert_eq!(bound(&pool, env.prod).await, Some(prod_bound), "never back");
}

/// Q-019-27: a retention lowered applies once the former one has run out: the purge right
/// after deletes nothing the former retention still kept.
#[kartenn_testing::pg_test]
async fn lowered_retention_waits_for_the_former(pool: PgPool) {
    let env = Env::new(&pool).await;
    let now = OffsetDateTime::now_utc();
    env.recorded(&Batch {
        opened: vec![Opening {
            from: now - days(12),
            ..env.pod(1, "198.51.100.17")
        }],
        closed: vec![closing(1, "198.51.100.17", now - days(10))],
    })
    .await;
    retention(&env, env.prod, 1).await;
    assert!(run_job(&env.store, PURGE).await.unwrap());
    assert_eq!(
        rows(&pool, env.prod).await,
        [(Uuid::from_u128(1), false)],
        "kept for the 30 days of the former retention"
    );
    let kept = bound(&pool, env.prod).await.unwrap();
    assert!(((now - days(30)) - kept).abs() < hours(1), "{kept}");
}

/// ADR 0043, decision 8: an open observation is never purged, whatever its age and the
/// retention; the retention runs from the end of an observation. A batch that closes in
/// a day created ahead creates none.
#[kartenn_testing::pg_test]
async fn open_observation_never_purged(pool: PgPool) {
    let env = Env::new(&pool).await;
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

/// Q-019-27: under the role of the application, a row opens and closes only under a batch
/// of its own transaction, and a batch is received at the instant of its transaction.
#[kartenn_testing::pg_test]
async fn row_cites_batch_of_its_transaction(pool: PgPool) {
    let env = Env::new(&pool).await;
    let old = one(&env, 1, "198.51.100.17").await;
    let refused = |e: sqlx::Error| match e {
        sqlx::Error::Database(e) => e.code().as_deref() == Some("42501"),
        _ => false,
    };
    let batch = "INSERT INTO kube.observation_batch
                     (id, cluster_id, author, received_at, opened, closed, content_hash, earliest)
                 VALUES ($1, $2, $3, now() - make_interval(days => $4), 0, 0, $5,
                         now() - make_interval(days => $4))";
    // A batch received `ago` days before the instant of its transaction.
    let insert_batch = |id: Uuid, ago: i32| {
        sqlx::query(batch)
            .bind(id)
            .bind(env.prod)
            .bind(env.connector)
            .bind(ago)
            .bind(vec![0u8; 32])
    };
    // An opening ascribed to the old batch.
    let mut tx = as_app(&pool).await;
    let opened = sqlx::query(
        "INSERT INTO kube.pod_observation
                (cluster_id, namespace, pod_name, pod_uid, workload_id, node_id, ip,
                 host_network, observed, opened_batch)
         VALUES ($1, 'payments', 'web-2', $2, NULL, NULL, '198.51.100.18', false,
                 tstzrange(now() - interval '1 hour', NULL, '[)'), $3)",
    )
    .bind(env.prod)
    .bind(Uuid::from_u128(2))
    .bind(old.id)
    .execute(&mut *tx)
    .await;
    assert!(opened.is_err_and(refused), "an opening under an old batch");
    drop(tx);
    // A closing under the old batch.
    let close = "UPDATE kube.pod_observation
                    SET observed = tstzrange(lower(observed), now(), '[)'), closed_batch = $1
                  WHERE pod_uid = $2";
    let mut tx = as_app(&pool).await;
    let closed = sqlx::query(close)
        .bind(old.id)
        .bind(Uuid::from_u128(1))
        .execute(&mut *tx)
        .await;
    assert!(closed.is_err_and(refused), "a closing under an old batch");
    drop(tx);
    // A batch received at another instant than its transaction's.
    let mut tx = as_app(&pool).await;
    let shifted = insert_batch(Uuid::now_v7(), 1).execute(&mut *tx).await;
    assert!(shifted.is_err_and(refused), "a batch received a day ago");
    drop(tx);
    // Under a batch of its own transaction, the closing passes.
    let mut tx = as_app(&pool).await;
    let own = Uuid::now_v7();
    insert_batch(own, 0).execute(&mut *tx).await.unwrap();
    sqlx::query("SELECT kube.observation_partition((now() AT TIME ZONE 'UTC')::date)")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(close)
        .bind(own)
        .bind(Uuid::from_u128(1))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
}
