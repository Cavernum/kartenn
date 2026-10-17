// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A syslog message and its RFC 5424 and RFC 3164 forms.

use std::time::SystemTime;

use crate::field::{APP_NAME_MAX, HOSTNAME_MAX, MSGID_MAX, PROCID_MAX, header};
use crate::priority::priority;
use crate::time::Utc;
use crate::{Error, Facility, SdElement, Severity};

/// UTF-8 byte order mark that opens a UTF-8 MSG (RFC 5424 §6.4).
const BOM: &[u8] = b"\xEF\xBB\xBF";
/// Written for an absent field (RFC 5424 §6).
const NIL: &[u8] = b"-";

/// The MSG part.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Body {
    /// No MSG.
    #[default]
    None,
    /// UTF-8 text, written after the BOM in RFC 5424.
    Text(String),
    /// Octets of an unknown encoding, written as they are.
    Bytes(Vec<u8>),
}

/// A syslog message: priority, header fields, structured data and body. The timestamp
/// is the one given, normally the time of the event, never the time of sending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    facility: Facility,
    severity: Severity,
    timestamp: Option<SystemTime>,
    hostname: Option<String>,
    app_name: Option<String>,
    procid: Option<String>,
    msgid: Option<String>,
    structured: Vec<SdElement>,
    body: Body,
}

impl Message {
    /// A message with only its priority; every other field is absent.
    #[must_use]
    pub const fn new(facility: Facility, severity: Severity) -> Self {
        Self {
            facility,
            severity,
            timestamp: None,
            hostname: None,
            app_name: None,
            procid: None,
            msgid: None,
            structured: Vec::new(),
            body: Body::None,
        }
    }

    /// Time of the event (written in UTC, to the microsecond).
    #[must_use]
    pub const fn timestamp(mut self, at: SystemTime) -> Self {
        self.timestamp = Some(at);
        self
    }

    /// HOSTNAME: FQDN, static IP address, hostname or other identifier, at most 255
    /// octets (RFC 5424 §6.2.4).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidField`] or [`Error::FieldTooLong`].
    pub fn hostname(mut self, value: &str) -> Result<Self, Error> {
        self.hostname = Some(header("HOSTNAME", value, HOSTNAME_MAX)?);
        Ok(self)
    }

    /// APP-NAME, at most 48 octets (§6.2.5).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidField`] or [`Error::FieldTooLong`].
    pub fn app_name(mut self, value: &str) -> Result<Self, Error> {
        self.app_name = Some(header("APP-NAME", value, APP_NAME_MAX)?);
        Ok(self)
    }

    /// PROCID, at most 128 octets (§6.2.6), often the process identifier.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidField`] or [`Error::FieldTooLong`].
    pub fn procid(mut self, value: &str) -> Result<Self, Error> {
        self.procid = Some(header("PROCID", value, PROCID_MAX)?);
        Ok(self)
    }

    /// MSGID, the type of message, at most 32 octets (§6.2.7).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidField`] or [`Error::FieldTooLong`].
    pub fn msgid(mut self, value: &str) -> Result<Self, Error> {
        self.msgid = Some(header("MSGID", value, MSGID_MAX)?);
        Ok(self)
    }

    /// Appends a structured-data element.
    ///
    /// # Errors
    ///
    /// [`Error::DuplicateSdId`] when the message already has an element of this SD-ID
    /// (§6.3.2).
    pub fn structured(mut self, element: SdElement) -> Result<Self, Error> {
        if self.structured.iter().any(|e| e.id() == element.id()) {
            return Err(Error::DuplicateSdId(element.id().to_owned()));
        }
        self.structured.push(element);
        Ok(self)
    }

    /// UTF-8 text as MSG.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.body = Body::Text(text.into());
        self
    }

    /// Octets of an unknown encoding as MSG.
    #[must_use]
    pub fn bytes(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.body = Body::Bytes(bytes.into());
        self
    }

    /// The RFC 5424 form, at most `max_len` octets: a longer MSG is truncated at its end
    /// (§6.1), on a character boundary for text.
    ///
    /// # Errors
    ///
    /// [`Error::HeaderTooLong`] when the header and structured data alone exceed
    /// `max_len`.
    pub fn to_rfc5424(&self, max_len: usize) -> Result<Vec<u8>, Error> {
        let mut out = format!("<{}>1 ", priority(self.facility, self.severity)).into_bytes();
        match self.timestamp.and_then(|t| Utc::of(t).rfc3339()) {
            Some(ts) => out.extend_from_slice(ts.as_bytes()),
            None => out.extend_from_slice(NIL),
        }
        for field in [&self.hostname, &self.app_name, &self.procid, &self.msgid] {
            out.push(b' ');
            out.extend_from_slice(field.as_deref().map_or(NIL, str::as_bytes));
        }
        out.push(b' ');
        if self.structured.is_empty() {
            out.extend_from_slice(NIL);
        } else {
            for element in &self.structured {
                element.write(&mut out);
            }
        }
        if out.len() > max_len {
            return Err(Error::HeaderTooLong {
                len: out.len(),
                max: max_len,
            });
        }
        // Room for " MSG".
        let room = max_len - out.len();
        match &self.body {
            Body::None => {}
            Body::Text(text) => {
                if room > 1 + BOM.len() {
                    out.push(b' ');
                    out.extend_from_slice(BOM);
                    out.extend_from_slice(truncate(text, room - 1 - BOM.len()).as_bytes());
                }
            }
            Body::Bytes(bytes) => {
                if room > 1 {
                    out.push(b' ');
                    out.extend_from_slice(&bytes[..bytes.len().min(room - 1)]);
                }
            }
        }
        Ok(out)
    }

    /// The RFC 3164 form used by local syslog sockets (`/dev/log`), as the C library
    /// writes it: `<PRI>Mmm dd hh:mm:ss APP-NAME[PROCID]: MSG`, at most `max_len` octets.
    /// The time is written in UTC (RFC 3164 has no time zone); without a timestamp, the
    /// current time is used, the format requiring one. HOSTNAME and structured data are
    /// left out: the local daemon adds the host name.
    ///
    /// # Errors
    ///
    /// [`Error::HeaderTooLong`] when the header alone exceeds `max_len`.
    pub fn to_rfc3164(&self, max_len: usize) -> Result<Vec<u8>, Error> {
        let at = Utc::of(self.timestamp.unwrap_or_else(SystemTime::now));
        let mut out = format!(
            "<{}>{} {}",
            priority(self.facility, self.severity),
            at.rfc3164(),
            self.app_name.as_deref().unwrap_or("-")
        )
        .into_bytes();
        if let Some(procid) = &self.procid {
            out.push(b'[');
            out.extend_from_slice(procid.as_bytes());
            out.push(b']');
        }
        out.push(b':');
        if out.len() > max_len {
            return Err(Error::HeaderTooLong {
                len: out.len(),
                max: max_len,
            });
        }
        let room = max_len - out.len();
        let body: &[u8] = match &self.body {
            Body::None => b"",
            Body::Text(text) => truncate(text, room.saturating_sub(1)).as_bytes(),
            Body::Bytes(bytes) => &bytes[..bytes.len().min(room.saturating_sub(1))],
        };
        if !body.is_empty() {
            out.push(b' ');
            out.extend_from_slice(body);
        }
        Ok(out)
    }
}

/// The longest prefix of `text` of at most `max` octets that ends on a character
/// boundary.
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    /// 2003-10-11T22:14:15.003Z, the time of RFC 5424 §6.5.
    fn at() -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(1_065_910_455_003)
    }

    fn text(bytes: &[u8]) -> String {
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[test]
    fn the_examples_of_rfc_5424() {
        // §6.5, example 1 (the timestamp gets its six digits).
        let one = Message::new(Facility::Auth, Severity::Critical)
            .timestamp(at())
            .hostname("mymachine.example.com")
            .unwrap()
            .app_name("su")
            .unwrap()
            .msgid("ID47")
            .unwrap()
            .text("'su root' failed for lonvick on /dev/pts/8");
        assert_eq!(
            text(&one.to_rfc5424(2048).unwrap()),
            "<34>1 2003-10-11T22:14:15.003000Z mymachine.example.com su - ID47 - \u{feff}'su root' failed for lonvick on /dev/pts/8"
        );
        // Example 4: two elements, no MSG.
        let four = Message::new(Facility::Local4, Severity::Notice)
            .timestamp(at())
            .hostname("mymachine.example.com")
            .unwrap()
            .app_name("evntslog")
            .unwrap()
            .msgid("ID47")
            .unwrap()
            .structured(
                SdElement::new("exampleSDID@32473")
                    .unwrap()
                    .param("iut", "3")
                    .unwrap()
                    .param("eventSource", "Application")
                    .unwrap()
                    .param("eventID", "1011")
                    .unwrap(),
            )
            .unwrap()
            .structured(
                SdElement::new("examplePriority@32473")
                    .unwrap()
                    .param("class", "high")
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            text(&four.to_rfc5424(2048).unwrap()),
            "<165>1 2003-10-11T22:14:15.003000Z mymachine.example.com evntslog - ID47 [exampleSDID@32473 iut=\"3\" eventSource=\"Application\" eventID=\"1011\"][examplePriority@32473 class=\"high\"]"
        );
    }

    #[test]
    fn absent_fields_are_nil() {
        let m = Message::new(Facility::Kernel, Severity::Alert);
        assert_eq!(text(&m.to_rfc5424(480).unwrap()), "<1>1 - - - - - -");
        let raw = Message::new(Facility::User, Severity::Debug).bytes(vec![0xff, b'x']);
        assert_eq!(
            raw.to_rfc5424(480).unwrap(),
            b"<15>1 - - - - - - \xffx".to_vec()
        );
    }

    #[test]
    fn a_long_msg_is_cut_at_its_end_on_a_character_boundary() {
        let m = Message::new(Facility::User, Severity::Notice)
            .timestamp(at())
            .text("é".repeat(1000));
        let out = m.to_rfc5424(480).unwrap();
        assert!(out.len() <= 480);
        assert!(std::str::from_utf8(&out).is_ok());
        assert!(out.ends_with("é".as_bytes()));
        // Header alone too long: refused rather than broken.
        let big = Message::new(Facility::User, Severity::Notice)
            .structured(
                SdElement::new("x@1")
                    .unwrap()
                    .param("v", "a".repeat(600))
                    .unwrap(),
            )
            .unwrap();
        assert!(matches!(
            big.to_rfc5424(480),
            Err(Error::HeaderTooLong { max: 480, .. })
        ));
    }

    #[test]
    fn an_sd_id_appears_once() {
        let m = Message::new(Facility::User, Severity::Notice)
            .structured(SdElement::meta_sequence(1).unwrap())
            .unwrap();
        assert_eq!(
            m.structured(SdElement::meta_sequence(2).unwrap()),
            Err(Error::DuplicateSdId("meta".into()))
        );
    }

    #[test]
    fn the_local_form_of_rfc_3164() {
        let m = Message::new(Facility::Auth, Severity::Critical)
            .timestamp(at())
            .app_name("su")
            .unwrap()
            .procid("4242")
            .unwrap()
            .structured(SdElement::meta_sequence(1).unwrap())
            .unwrap()
            .text("'su root' failed");
        assert_eq!(
            text(&m.to_rfc3164(1024).unwrap()),
            "<34>Oct 11 22:14:15 su[4242]: 'su root' failed"
        );
        // 29 octets of header, a space, 9 of MSG.
        let cut = m.to_rfc3164(39).unwrap();
        assert_eq!(text(&cut), "<34>Oct 11 22:14:15 su[4242]: 'su root'");
    }
}
