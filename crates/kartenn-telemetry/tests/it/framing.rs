//! Arbitrary values break the framing of no output (spec 012, REQ-012-29): the text line
//! stays one line, the JSON line one line that reads back the same, the journald datagram
//! the same fields, the syslog message one RFC 5424 message without a line feed.

use std::collections::{BTreeMap, HashMap};

use kartenn_core::audit::Severity;
use kartenn_telemetry::capture::Record;
use kartenn_telemetry::outputs::{Journald, Syslog, json, text};
use proptest::prelude::*;
use syslog_sender::{Facility, Target};

/// Any text, control characters and forged frames included.
fn value() -> impl Strategy<Value = String> {
    prop_oneof![
        proptest::collection::vec(any::<char>(), 0..200).prop_map(String::from_iter),
        "[\\x00-\\x1f\\x7f\"\\\\\\]\\[=<>]{0,40}",
        Just("line one\n<14>1 2026-10-17T00:00:00Z host kartenn - forged - forged".to_owned()),
        Just("x\r\n{\"event.name\":\"forged\"}\n".to_owned()),
    ]
}

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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn values_break_no_framing(message in value(), motive in value(), detail in value()) {
        let record = Record {
            time: std::time::SystemTime::now(),
            severity: Severity::Warn,
            name: "pipeline.changeset.refused",
            target: "kartenn_service::pipeline",
            message: message.clone(),
            fields: BTreeMap::from([
                ("motive".to_owned(), motive.clone()),
                ("detail".to_owned(), detail.clone()),
            ]),
            trace_id: None,
            span_id: None,
        };

        // Text: one line.
        let line = text(&record);
        prop_assert!(!line.contains('\n') && !line.contains('\r'), "{line:?}");

        // JSON: one line, read back unchanged.
        let encoded = json(&record).to_string();
        prop_assert!(!encoded.contains('\n') && !encoded.contains('\r'));
        let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        prop_assert_eq!(decoded["message"].as_str(), Some(message.as_str()));
        prop_assert_eq!(decoded["fields"]["motive"].as_str(), Some(motive.as_str()));

        // journald: the same fields, nothing forged.
        let fields = journald_fields(&Journald::datagram(&record));
        prop_assert_eq!(&fields["MESSAGE"], &message);
        prop_assert_eq!(&fields["KARTENN_MOTIVE"], &motive);
        prop_assert_eq!(&fields["KARTENN_DETAIL"], &detail);
        prop_assert_eq!(&fields["KARTENN_EVENT_NAME"], "pipeline.changeset.refused");

        // syslog: one message, no line feed, the event read back from MSG.
        let mut syslog = Syslog::new(
            Target::Tcp("127.0.0.1:9".into()),
            Facility::Daemon,
            Some("host-1".into()),
        );
        let bytes = syslog.message(&record).unwrap();
        prop_assert!(!bytes.contains(&b'\n'));
        let frame = String::from_utf8(bytes).unwrap();
        let parsed = syslog_rfc5424::parse_message(&frame).unwrap();
        prop_assert_eq!(parsed.msgid.as_deref(), Some("pipeline.changeset.refused"));
        let body: serde_json::Value =
            serde_json::from_str(parsed.msg.trim_start_matches('\u{feff}')).unwrap();
        prop_assert_eq!(body["message"].as_str(), Some(message.as_str()));
        prop_assert_eq!(body["fields"]["detail"].as_str(), Some(detail.as_str()));
    }
}
