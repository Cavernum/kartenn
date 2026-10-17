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

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_instance_runs_and_stops_cleanly(pool: PgPool) {
    let dir = std::env::temp_dir().join(format!("kartenn-run-{}", std::process::id()));
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
