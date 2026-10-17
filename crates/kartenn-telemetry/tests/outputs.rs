//! One technical event on every output: same name, severity and time; a line feed in a
//! value forges no line (spec 012, REQ-012-3, 4, 5, 20, 21, 29; task 012-14).

use std::collections::HashMap;

use kartenn_telemetry::capture::{CaptureLayer, Output, drive};
use kartenn_telemetry::outputs::{File, Journald, Syslog, neutralised, text};
use serde_json::Value;
use syslog_sender::{Facility, Target};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, UnixDatagram};
use tracing_subscriber::layer::SubscriberExt;

/// Fields of a journald datagram (text form, or binary form for multi-line values).
fn journald_fields(mut bytes: &[u8]) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    while !bytes.is_empty() {
        let end = bytes.iter().position(|&b| b == b'\n' || b == b'=').unwrap();
        let name = String::from_utf8(bytes[..end].to_vec()).unwrap();
        if bytes[end] == b'=' {
            let stop = end + 1 + bytes[end + 1..].iter().position(|&b| b == b'\n').unwrap();
            fields.insert(
                name,
                String::from_utf8(bytes[end + 1..stop].to_vec()).unwrap(),
            );
            bytes = &bytes[stop + 1..];
        } else {
            let len = usize::try_from(u64::from_le_bytes(
                bytes[end + 1..end + 9].try_into().unwrap(),
            ))
            .unwrap();
            let start = end + 9;
            fields.insert(
                name,
                String::from_utf8(bytes[start..start + len].to_vec()).unwrap(),
            );
            bytes = &bytes[start + len + 1..];
        }
    }
    fields
}

#[tokio::test]
async fn an_event_reads_the_same_on_every_output() {
    // Unique per run: a directory left by an earlier run would hold a bound socket.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("kartenn-outputs-{}-{unique}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("logs.jsonl");
    let journal_path = dir.join("journal.sock");
    let journal = UnixDatagram::bind(&journal_path).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let syslog_target = Target::Tcp(listener.local_addr().unwrap().to_string());
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        bytes
    });

    let (file, file_rx) = Output::new("file", "info", 16).unwrap();
    let (journald, journald_rx) = Output::new("journald", "info", 16).unwrap();
    let (syslog, syslog_rx) = Output::new("syslog", "info", 16).unwrap();
    let subscriber = tracing_subscriber::registry().with(CaptureLayer::new(vec![
        file.clone(),
        journald.clone(),
        syslog.clone(),
    ]));
    tracing::subscriber::with_default(subscriber, || {
        tracing::event!(
            name: "pipeline.changeset.refused",
            tracing::Level::WARN,
            motive = "line one\nline two",
            "a changeset was refused"
        );
    });
    drop((file, journald, syslog));
    drive(file_rx, File::new(&file_path)).await;
    drive(journald_rx, Journald::new(&journal_path).unwrap()).await;
    drive(
        syslog_rx,
        Syslog::new(syslog_target, Facility::Daemon, Some("host-1".into())),
    )
    .await;

    // File: one JSON line.
    let text_file = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(
        text_file.lines().count(),
        1,
        "the line feed of a value stays inside"
    );
    let line: Value = serde_json::from_str(text_file.trim()).unwrap();
    assert_eq!(line["event.name"], "pipeline.changeset.refused");
    assert_eq!(line["severity"], "WARN");
    assert_eq!(line["severity_number"], 13);
    let time = line["timestamp"].as_str().unwrap().to_owned();
    assert_eq!(time.len(), "2026-10-17T08:30:12.123456Z".len());
    assert!(time.ends_with('Z'));

    // journald: PRIORITY 4, same name, the multi-line value in binary form.
    let mut buf = vec![0; 65_536];
    let n = journal.recv(&mut buf).await.unwrap();
    let fields = journald_fields(&buf[..n]);
    assert_eq!(fields["PRIORITY"], "4");
    assert_eq!(fields["KARTENN_EVENT_NAME"], "pipeline.changeset.refused");
    assert_eq!(fields["KARTENN_MOTIVE"], "line one\nline two");
    assert_eq!(fields["MESSAGE_ID"].len(), 32);

    // syslog: severity 4, MSGID the name, timestamp of the event.
    let bytes = server.await.unwrap();
    let space = bytes.iter().position(|&b| b == b' ').unwrap();
    let frame = String::from_utf8(bytes[space + 1..].to_vec()).unwrap();
    let parsed = syslog_rfc5424::parse_message(&frame).unwrap();
    assert_eq!(parsed.severity as u8, 4);
    assert_eq!(parsed.msgid.as_deref(), Some("pipeline.changeset.refused"));
    let body: Value = serde_json::from_str(parsed.msg.trim_start_matches('\u{feff}')).unwrap();
    assert_eq!(body["timestamp"], time, "the same time on every output");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn text_neutralises_control_characters() {
    assert_eq!(neutralised("a\nb\r\u{1b}[31m"), "a\\u{a}b\\u{d}\\u{1b}[31m");
    let record = kartenn_telemetry::capture::Record {
        time: std::time::UNIX_EPOCH,
        severity: kartenn_core::audit::Severity::Info,
        name: "x.y.z".into(),
        target: "t".into(),
        message: "forged\n1970-01-01T00:00:00.000000Z ERROR fake: line".into(),
        fields: std::collections::BTreeMap::new(),
        trace_id: None,
        span_id: None,
    };
    assert_eq!(text(&record).lines().count(), 1);
}
