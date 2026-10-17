// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Structured data (RFC 5424 §6.3) and the registered SD-IDs of §7.

use crate::Error;
use crate::field::printable;

/// Longest SD-NAME (§6.3.3).
const SD_NAME_MAX: usize = 32;
/// Largest `sequenceId` (§7.3.1).
const SEQUENCE_MAX: u32 = 2_147_483_647;

/// One SD-ELEMENT: an SD-ID and its parameters, in order.
///
/// An SD-ID without `@` is reserved for names registered with IANA (`timeQuality`,
/// `origin`, `meta`…, §7); others are `name@<private enterprise number>` (§6.3.2). The
/// crate checks the syntax, not the registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdElement {
    id: String,
    params: Vec<(String, String)>,
}

/// An SD-NAME: 1 to 32 printable US-ASCII characters except `=`, space, `]` and `"`.
fn sd_name(name: &str) -> Result<String, Error> {
    let valid = !name.is_empty()
        && name.len() <= SD_NAME_MAX
        && name
            .bytes()
            .all(|b| printable(b) && !matches!(b, b'=' | b']' | b'"'));
    if valid {
        Ok(name.to_owned())
    } else {
        Err(Error::InvalidSdName(name.to_owned()))
    }
}

impl SdElement {
    /// An element with no parameter.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSdName`] when `id` is not an SD-NAME, or has an `@` not followed
    /// by an enterprise number (digits and dots).
    pub fn new(id: &str) -> Result<Self, Error> {
        let id = sd_name(id)?;
        if let Some((name, number)) = id.split_once('@') {
            let number_ok = !number.is_empty()
                && number.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                && !number.starts_with('.')
                && !number.ends_with('.');
            if name.is_empty() || !number_ok {
                return Err(Error::InvalidSdName(id));
            }
        }
        Ok(Self {
            id,
            params: Vec::new(),
        })
    }

    /// Adds a parameter; the value is any UTF-8 text, escaped when written (§6.3.3).
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSdName`] when `name` is not an SD-NAME.
    pub fn param(mut self, name: &str, value: impl Into<String>) -> Result<Self, Error> {
        self.params.push((sd_name(name)?, value.into()));
        Ok(self)
    }

    /// The SD-ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// `[meta sequenceId="n"]` (§7.3.1): starts at 1 and lets a receiver see losses.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSdValue`] when `n` is 0 or above 2147483647.
    pub fn meta_sequence(n: u32) -> Result<Self, Error> {
        if n == 0 || n > SEQUENCE_MAX {
            return Err(Error::InvalidSdValue {
                param: "sequenceId",
                reason: "must be between 1 and 2147483647",
            });
        }
        Self::new("meta")?.param("sequenceId", n.to_string())
    }

    /// `[timeQuality …]` (§7.1). `sync_accuracy` is in microseconds and MUST NOT be given
    /// when the clock is not synchronised.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSdValue`] when `sync_accuracy` comes with `is_synced = false`.
    pub fn time_quality(
        tz_known: bool,
        is_synced: bool,
        sync_accuracy: Option<u64>,
    ) -> Result<Self, Error> {
        if !is_synced && sync_accuracy.is_some() {
            return Err(Error::InvalidSdValue {
                param: "syncAccuracy",
                reason: "must not be given when isSynced is 0",
            });
        }
        let flag = |b: bool| if b { "1" } else { "0" };
        let mut element = Self::new("timeQuality")?
            .param("tzKnown", flag(tz_known))?
            .param("isSynced", flag(is_synced))?;
        if let Some(accuracy) = sync_accuracy {
            element = element.param("syncAccuracy", accuracy.to_string())?;
        }
        Ok(element)
    }

    /// Appends the element as written on the wire.
    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        out.push(b'[');
        out.extend_from_slice(self.id.as_bytes());
        for (name, value) in &self.params {
            out.push(b' ');
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(b"=\"");
            for c in value.chars() {
                if matches!(c, '"' | '\\' | ']') {
                    out.push(b'\\');
                }
                let mut buf = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
            out.push(b'"');
        }
        out.push(b']');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(e: &SdElement) -> String {
        let mut out = Vec::new();
        e.write(&mut out);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn names_follow_the_sd_name_grammar() {
        assert!(SdElement::new("exampleSDID@32473").is_ok());
        assert!(SdElement::new("origin").is_ok());
        assert!(SdElement::new("x@1.3.6").is_ok());
        for bad in [
            "", "a b", "a=b", "a]b", "a\"b", "é", "x@", "@32473", "x@1a", "x@.1", "x@1.",
        ] {
            assert!(SdElement::new(bad).is_err(), "{bad:?}");
        }
        assert!(SdElement::new(&"a".repeat(33)).is_err());
        assert!(SdElement::new("meta").unwrap().param("a b", "x").is_err());
    }

    #[test]
    fn values_escape_quote_backslash_and_bracket() {
        // RFC 5424 §6.3.3.
        let e = SdElement::new("exampleSDID@32473")
            .unwrap()
            .param("iut", "3")
            .unwrap()
            .param("v", r#"a"b\c]d é"#)
            .unwrap();
        assert_eq!(
            written(&e),
            r#"[exampleSDID@32473 iut="3" v="a\"b\\c\]d é"]"#
        );
    }

    #[test]
    fn registered_elements_check_their_values() {
        assert_eq!(
            written(&SdElement::meta_sequence(1).unwrap()),
            r#"[meta sequenceId="1"]"#
        );
        assert!(SdElement::meta_sequence(0).is_err());
        assert!(SdElement::meta_sequence(2_147_483_648).is_err());
        assert_eq!(
            written(&SdElement::time_quality(true, true, Some(1000)).unwrap()),
            r#"[timeQuality tzKnown="1" isSynced="1" syncAccuracy="1000"]"#
        );
        assert!(SdElement::time_quality(true, false, Some(1)).is_err());
    }
}
