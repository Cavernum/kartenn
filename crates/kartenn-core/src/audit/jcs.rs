//! Canonical JSON (RFC 8785, JCS) for the fingerprints of audit events, and the
//! fingerprint of a sealed batch (spec 012, REQ-012-13, 14).
//!
//! Audit events carry no floating-point number: only integers are accepted, which keeps
//! the canonical form to the simple cases of RFC 8785 §3.2.2.3 (an integer is written as
//! its decimal digits).

use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Why a value has no canonical form here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CanonicalError {
    /// A floating-point number, which audit events never carry.
    #[error("{0} is not an integer: audit events carry no floating-point number")]
    NotInteger(String),
}

/// The canonical form of `value` (RFC 8785): objects sorted by the UTF-16 code units of
/// their keys, no whitespace, strings escaped as ECMAScript `JSON.stringify` does.
///
/// # Errors
///
/// [`CanonicalError::NotInteger`] for a floating-point number.
pub fn canonical(value: &Value) -> Result<String, CanonicalError> {
    let mut out = String::new();
    write(value, &mut out)?;
    Ok(out)
}

fn write(value: &Value, out: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                out.push_str(&n.to_string());
            } else {
                return Err(CanonicalError::NotInteger(n.to_string()));
            }
        }
        Value::String(s) => string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            // RFC 8785 §3.2.3: by UTF-16 code units, not by code points.
            entries.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, (key, item)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                string(key, out);
                out.push(':');
                write(item, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// RFC 8785 §3.2.2.2: `"` and `\` escaped, controls as `\b \t \n \f \r` or `\u00xx`
/// (lowercase), everything else as it is.
fn string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// SHA-256 of the canonical form of an event.
///
/// # Errors
///
/// As [`canonical`].
pub fn event_hash(event: &Value) -> Result<[u8; 32], CanonicalError> {
    Ok(Sha256::digest(canonical(event)?.as_bytes()).into())
}

/// Fingerprint of a batch: `SHA-256(previous ‖ count (8 octets, big endian) ‖ h₁ ‖ … ‖ hₙ)`,
/// where `hᵢ` are the [`event_hash`] of the events in sequence order and `previous` the
/// fingerprint of the batch before (32 zero octets for the first).
#[must_use]
pub fn batch_hash(previous: &[u8; 32], events: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(previous);
    hasher.update((events.len() as u64).to_be_bytes());
    for h in events {
        hasher.update(h);
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn keys_sort_by_utf16_code_units() {
        // RFC 8785 §3.2.3: U+1F600 (surrogates D83D DE00) sorts before U+FB33.
        let value = json!({
            "\u{20ac}": "Euro Sign",
            "\r": "Carriage Return",
            "\u{fb33}": "Hebrew Letter Dalet With Dagesh",
            "1": "One",
            "\u{1f600}": "Emoji: Grinning Face",
            "\u{80}": "Control",
            "\u{f6}": "Latin Small Letter O With Diaeresis"
        });
        let keys: Vec<String> = canonical(&value)
            .unwrap()
            .split(',')
            .map(|entry| {
                entry
                    .trim_start_matches('{')
                    .split(':')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            keys,
            [
                "\"\\r\"",
                "\"1\"",
                "\"\u{80}\"",
                "\"\u{f6}\"",
                "\"\u{20ac}\"",
                "\"\u{1f600}\"",
                "\"\u{fb33}\""
            ]
        );
    }

    #[test]
    fn strings_escape_as_json_stringify() {
        // RFC 8785 §3.2.2.2 example: "\u20ac$\u000f\u000aA'\u0042\u0022\u005c\\\"\/".
        let value = Value::String("\u{20ac}$\u{f}\nA'B\"\\\\\"/".into());
        assert_eq!(
            canonical(&value).unwrap(),
            "\"\u{20ac}$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\""
        );
    }

    #[test]
    fn integers_only_and_no_whitespace() {
        let value = json!({"b": [1, -2, true, null], "a": {"z": 18_446_744_073_709_551_615_u64}});
        assert_eq!(
            canonical(&value).unwrap(),
            r#"{"a":{"z":18446744073709551615},"b":[1,-2,true,null]}"#
        );
        assert!(matches!(
            canonical(&json!({"x": 1.5})),
            Err(CanonicalError::NotInteger(_))
        ));
    }

    #[test]
    fn a_batch_depends_on_its_order_its_members_and_its_predecessor() {
        let a = event_hash(&json!({"seq": 1})).unwrap();
        let b = event_hash(&json!({"seq": 2})).unwrap();
        let zero = [0; 32];
        let ab = batch_hash(&zero, &[a, b]);
        assert_ne!(ab, batch_hash(&zero, &[b, a]));
        assert_ne!(ab, batch_hash(&zero, &[a]));
        assert_ne!(ab, batch_hash(&ab, &[a, b]));
        assert_eq!(ab, batch_hash(&zero, &[a, b]));
    }
}
