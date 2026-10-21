//! Exporting the audit journal: at least once, in order, without gap after an outage or a
//! late commit; file and syslog outputs (spec 012, REQ-012-15, 26; task 012-11).

use std::future::Future;
use std::sync::Mutex;

use kartenn_core::audit::{OBJECTS_READ, Outcome};
use kartenn_service::audit::{AuditJournal, AuditSink, Instance, RequestContext, SinkError};
use kartenn_service::{Store, WriteTx};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::audit_relay::{RelayError, export_lag, relay_once};
use kartenn_store_pg::write::PgStore;
use kartenn_telemetry::sinks::{FileSink, SyslogSink};
use serde_json::Value;
use sqlx::PgPool;
use syslog_sender::Target;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

async fn record(pool: &PgPool, n: usize) {
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("test"));
    for _ in 0..n {
        let event = RequestContext::system()
            .event(&OBJECTS_READ, Outcome::Success)
            .unwrap();
        journal.record(&event).await.unwrap();
    }
}

/// An output that can be cut off.
#[derive(Default)]
struct Flaky {
    down: Mutex<bool>,
    received: Mutex<Vec<i64>>,
}

impl AuditSink for Flaky {
    fn name(&self) -> &'static str {
        "flaky"
    }

    fn deliver(&self, events: &[Value]) -> impl Future<Output = Result<(), SinkError>> + Send {
        let result = if *self.down.lock().unwrap() {
            Err(SinkError("connection refused".into()))
        } else {
            self.received
                .lock()
                .unwrap()
                .extend(events.iter().map(|e| e["seq"].as_i64().unwrap()));
            Ok(())
        };
        std::future::ready(result)
    }
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn an_outage_delays_without_loss_or_gap(pool: PgPool) {
    let sink = Flaky::default();
    record(&pool, 3).await;
    assert_eq!(relay_once(&pool, &sink, 500).await.unwrap(), 3);
    assert_eq!(relay_once(&pool, &sink, 500).await.unwrap(), 0, "once each");
    *sink.down.lock().unwrap() = true;
    record(&pool, 2).await;
    assert!(matches!(
        relay_once(&pool, &sink, 500).await,
        Err(RelayError::Sink { .. })
    ));
    assert!(
        export_lag(&pool, "flaky").await.unwrap().is_some(),
        "behind"
    );
    // A transaction committed late, with a smaller sequence.
    let store = PgStore::new(pool.clone());
    let mut long = store.begin().await.unwrap();
    long.audit(
        &RequestContext::system()
            .event(&OBJECTS_READ, Outcome::Success)
            .unwrap(),
    )
    .await
    .unwrap();
    record(&pool, 1).await;
    *sink.down.lock().unwrap() = false;
    assert_eq!(relay_once(&pool, &sink, 500).await.unwrap(), 3);
    long.commit().await.unwrap();
    assert_eq!(relay_once(&pool, &sink, 500).await.unwrap(), 1, "no gap");
    assert_eq!(export_lag(&pool, "flaky").await.unwrap(), None);
    let mut received = sink.received.lock().unwrap().clone();
    let all = received.len();
    received.sort_unstable();
    received.dedup();
    assert_eq!(
        (all, received.len()),
        (7, 7),
        "each event once, all of them"
    );
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_file_output_writes_json_lines(pool: PgPool) {
    let path = std::env::temp_dir().join(format!("kartenn-audit-{}.jsonl", uuid_like()));
    let sink = FileSink::new("file", &path);
    record(&pool, 3).await;
    relay_once(&pool, &sink, 500).await.unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|l| l["name"] == "data.objects.read"));
}

fn uuid_like() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

#[sqlx::test(migrator = "kartenn_store_pg::MIGRATOR")]
async fn the_syslog_output_speaks_rfc_5424(pool: PgPool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        bytes
    });
    let sink = SyslogSink::new("siem", Target::Tcp(address), Some("kartenn-1".into()));
    record(&pool, 2).await;
    relay_once(&pool, &sink, 500).await.unwrap();
    drop(sink);
    let bytes = server.await.unwrap();
    // Octet-counted frames.
    let mut rest = bytes.as_slice();
    let mut messages = Vec::new();
    while !rest.is_empty() {
        let space = rest.iter().position(|&b| b == b' ').unwrap();
        let len: usize = std::str::from_utf8(&rest[..space])
            .unwrap()
            .parse()
            .unwrap();
        messages.push(String::from_utf8(rest[space + 1..space + 1 + len].to_vec()).unwrap());
        rest = &rest[space + 1 + len..];
    }
    assert_eq!(messages.len(), 2);
    for (i, text) in messages.iter().enumerate() {
        let parsed = syslog_rfc5424::parse_message(text).unwrap();
        assert_eq!(parsed.facility as u8, 13, "log audit");
        assert_eq!(parsed.appname.as_deref(), Some("kartenn"));
        assert_eq!(parsed.hostname.as_deref(), Some("kartenn-1"));
        assert_eq!(parsed.msgid.as_deref(), Some("data.objects.read"));
        assert_eq!(
            parsed.sd.find_tuple("meta", "sequenceId"),
            Some(&(i + 1).to_string())
        );
        let event: Value = serde_json::from_str(parsed.msg.trim_start_matches('\u{feff}')).unwrap();
        assert_eq!(event["name"], "data.objects.read");
        assert!(event["seq"].is_i64());
    }
}
