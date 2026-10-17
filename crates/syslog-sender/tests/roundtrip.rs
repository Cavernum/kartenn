//! What the crate writes, an independent RFC 5424 parser reads back unchanged.

use std::time::{Duration, UNIX_EPOCH};

use proptest::prelude::*;
use syslog_rfc5424::parse_message;
use syslog_sender::{Facility, Message, SdElement, Severity};

const FACILITIES: [Facility; 4] = [
    Facility::Kernel,
    Facility::Auth,
    Facility::Audit,
    Facility::Local7,
];
const SEVERITIES: [Severity; 3] = [Severity::Emergency, Severity::Warning, Severity::Debug];

/// A header field: printable US-ASCII without space, as RFC 5424 allows, other than the
/// NILVALUE `-` (refused by the crate).
fn field(max: usize) -> impl Strategy<Value = String> {
    proptest::string::string_regex(&format!("[!-~]{{1,{max}}}"))
        .expect("valid regex")
        .prop_filter("not the NILVALUE", |f| f != "-")
}

/// The MSG as the parser returns it, without the BOM.
fn msg(parsed: &syslog_rfc5424::SyslogMessage) -> &str {
    parsed.msg.trim_start_matches('\u{feff}')
}

proptest! {
    #[test]
    fn written_messages_parse_back(
        f in 0..FACILITIES.len(),
        s in 0..SEVERITIES.len(),
        micros in 0u64..253_402_300_799_999_999,
        hostname in field(64),
        app_name in field(48),
        // the parser reads a numeric PROCID as a number
        procid in field(16).prop_filter("not numeric", |p| p.parse::<i32>().is_err()),
        msgid in field(32),
        value in any::<String>(),
        body in any::<String>(),
    ) {
        let message = Message::new(FACILITIES[f], SEVERITIES[s])
            .timestamp(UNIX_EPOCH + Duration::from_micros(micros))
            .hostname(&hostname).unwrap()
            .app_name(&app_name).unwrap()
            .procid(&procid).unwrap()
            .msgid(&msgid).unwrap()
            .structured(SdElement::new("x@32473").unwrap().param("v", value.clone()).unwrap()).unwrap()
            .text(body.clone());
        let bytes = message.to_rfc5424(1 << 20).unwrap();
        let parsed = parse_message(String::from_utf8(bytes).unwrap()).unwrap();
        prop_assert_eq!(parsed.facility as u8, FACILITIES[f].code());
        prop_assert_eq!(parsed.severity as u8, SEVERITIES[s].code());
        prop_assert_eq!(parsed.timestamp, Some(i64::try_from(micros / 1_000_000).unwrap()));
        prop_assert_eq!(parsed.timestamp_nanos, Some(u32::try_from(micros % 1_000_000).unwrap() * 1000));
        prop_assert_eq!(parsed.hostname.as_deref(), Some(hostname.as_str()));
        prop_assert_eq!(parsed.appname.as_deref(), Some(app_name.as_str()));
        prop_assert_eq!(parsed.msgid.as_deref(), Some(msgid.as_str()));
        prop_assert_eq!(parsed.sd.find_tuple("x@32473", "v"), Some(&value));
        prop_assert_eq!(msg(&parsed), body.as_str());
    }

    #[test]
    fn truncation_keeps_valid_utf8_within_the_limit(body in any::<String>(), max in 64usize..600) {
        let bytes = Message::new(Facility::User, Severity::Notice)
            .text(body.clone())
            .to_rfc5424(max)
            .unwrap();
        prop_assert!(bytes.len() <= max);
        let text = String::from_utf8(bytes).unwrap();
        let parsed = parse_message(text).unwrap();
        prop_assert!(body.starts_with(msg(&parsed)));
    }
}
