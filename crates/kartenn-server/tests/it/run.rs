//! An instance from start to stop: technical logs and audit written to their outputs, the
//! stop sealed and exported, the chain intact (spec 012, REQ-012-15, 20, 31, 34; task
//! 012-16).

use std::time::Duration;

use kartenn_core::audit::{OBJECTS_READ, Outcome};
use kartenn_service::audit::{AuditJournal, Instance, RequestContext};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use sqlx::PgPool;

fn lines(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[kartenn_testing::pg_test]
async fn an_instance_runs_and_stops_cleanly(pool: PgPool) {
    let dir = std::env::temp_dir().join(format!("kartenn-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let technical = dir.join("technical.jsonl");
    let audit = dir.join("audit.jsonl");
    let config = kartenn_server::config::from_toml(&format!(
        r#"
        [telemetry]
        outputs = [{{ kind = "file", path = "{}", filter = "info" }}]
        [audit]
        sinks = [{{ kind = "file", name = "archive", path = "{}" }}]
        seal_interval_ms = 100
        relay_interval_ms = 100
        "#,
        technical.display(),
        audit.display()
    ))
    .unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let run = tokio::spawn({
        let pool = pool.clone();
        async move {
            kartenn_server::app::run(pool, &config, async {
                let _ = stopped.await;
            })
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    let event = RequestContext::system()
        .event(&OBJECTS_READ, Outcome::Success)
        .unwrap();
    PgAuditJournal::new(pool.clone(), Instance::new("test"))
        .record(&event)
        .await
        .unwrap();
    tracing::info!(name: "test.run.event", "a technical event");
    tokio::time::sleep(Duration::from_millis(300)).await;
    stop.send(()).unwrap();
    let report = run.await.unwrap().unwrap();
    assert_eq!(report.lost_technical, 0);

    let names = |path| -> Vec<String> {
        lines(path)
            .iter()
            .map(|l| {
                l.get("event.name")
                    .or_else(|| l.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect()
    };
    let audited = names(&audit);
    for expected in [
        "sys.instance.started",
        "data.objects.read",
        "sys.instance.stopped",
    ] {
        assert!(
            audited.iter().any(|n| n == expected),
            "{expected}: {audited:?}"
        );
    }
    let logged = names(&technical);
    for expected in [
        "sys.instance.started",
        "test.run.event",
        "sys.instance.stopping",
    ] {
        assert!(
            logged.iter().any(|n| n == expected),
            "{expected}: {logged:?}"
        );
    }
    let report = kartenn_store_pg::audit_verify::verify(&pool, None)
        .await
        .unwrap();
    assert_eq!(report.broken, None);
    assert!(report.seals >= 1);
    let unsealed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.event e
         WHERE NOT EXISTS (SELECT 1 FROM audit.sealed s WHERE s.event_seq = e.seq)
           AND e.name <> 'audit.seal.created'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unsealed, 0, "the stop is sealed");
    std::fs::remove_dir_all(dir).unwrap();
}

#[kartenn_testing::pg_test]
async fn an_audit_output_removed_from_the_configuration_is_audited(pool: PgPool) {
    // `siem` received events before; the new configuration no longer has it.
    sqlx::query(
        "INSERT INTO pass_floor (pass) VALUES ('audit.relay/siem'), ('audit.relay/archive')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let dir = std::env::temp_dir().join(format!("kartenn-disabled-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = kartenn_server::config::from_toml(&format!(
        r#"
        [telemetry]
        outputs = [{{ kind = "stdout", filter = "off" }}]
        [audit]
        sinks = [{{ kind = "file", name = "archive", path = "{}" }}]
        "#,
        dir.join("audit.jsonl").display()
    ))
    .unwrap();
    kartenn_server::app::run(pool.clone(), &config, async {})
        .await
        .unwrap();
    let disabled: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT details ->> 'output' FROM audit.event WHERE name = 'sys.output.disabled'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(disabled, [Some("siem".to_owned())]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[kartenn_testing::pg_test]
async fn a_backlog_is_drained_in_one_tick(pool: PgPool) {
    let dir = std::env::temp_dir().join(format!("kartenn-backlog-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let audit = dir.join("audit.jsonl");
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    let event = RequestContext::system()
        .event(&OBJECTS_READ, Outcome::Success)
        .unwrap();
    for _ in 0..1_200 {
        journal.record(&event).await.unwrap();
    }
    // Ticks far apart: only the first one, at start, can drain the backlog.
    let config = kartenn_server::config::from_toml(&format!(
        r#"
        [telemetry]
        outputs = [{{ kind = "stdout", filter = "off" }}]
        [audit]
        sinks = [{{ kind = "file", name = "archive", path = "{}" }}]
        seal_interval_ms = 600000
        relay_interval_ms = 600000
        "#,
        audit.display()
    ))
    .unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let run = tokio::spawn({
        let pool = pool.clone();
        async move {
            kartenn_server::app::run(pool, &config, async {
                let _ = stopped.await;
            })
            .await
        }
    });
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    let reads = lines(&audit)
        .iter()
        .filter(|l| l["name"] == "data.objects.read")
        .count();
    let sealed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM audit.sealed s JOIN audit.event e ON e.seq = s.event_seq WHERE e.name = 'data.objects.read'")
            .fetch_one(&pool)
            .await
            .unwrap();
    stop.send(()).unwrap();
    run.await.unwrap().unwrap();
    assert_eq!(reads, 1_200, "more than one batch exported in one tick");
    assert_eq!(sealed, 1_200, "sealed in one tick");
    std::fs::remove_dir_all(dir).unwrap();
}
