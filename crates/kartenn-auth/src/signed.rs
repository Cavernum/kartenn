//! Signed values (spec 006, REQ-006-8): readable by their holder, authenticated by
//! HMAC-SHA-256 (RFC 2104) under a key of the ring derived for their purpose, so that a
//! value altered, forged or signed by a key the ring no longer holds is refused.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::keys::{KeyRing, Purpose};

/// Format version of a signed value.
const VERSION: &str = "v1";

/// Why a signed value is refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SignedError {
    /// Not a signed value of this format.
    #[error("malformed signed value")]
    Malformed,
    /// Signed with a key the ring no longer holds.
    #[error("unknown key {0}")]
    UnknownKey(String),
    /// Altered, or signed for another purpose.
    #[error("the signed value does not authenticate")]
    Forged,
}

fn mac(key: &[u8; 32], signed: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes a key of any length");
    mac.update(signed.as_bytes());
    mac
}

/// Signs `payload` for `purpose`: `v1.<key id>.<payload>.<tag>`, both base64url; the tag
/// covers the version and the key identifier.
#[must_use]
pub fn sign(ring: &KeyRing, purpose: Purpose, payload: &[u8]) -> String {
    let (id, key) = ring.current(purpose);
    let signed = format!("{VERSION}.{id}.{}", URL_SAFE_NO_PAD.encode(payload));
    let tag = mac(&key, &signed).finalize().into_bytes();
    format!("{signed}.{}", URL_SAFE_NO_PAD.encode(tag))
}

/// The payload of `value` signed for `purpose`, checked before it is decoded.
///
/// # Errors
///
/// [`SignedError`] for another format, an unknown key or a tag that does not match.
pub fn verify(ring: &KeyRing, purpose: Purpose, value: &str) -> Result<Vec<u8>, SignedError> {
    let (signed, tag) = value.rsplit_once('.').ok_or(SignedError::Malformed)?;
    let rest = signed
        .strip_prefix(VERSION)
        .and_then(|rest| rest.strip_prefix('.'))
        .ok_or(SignedError::Malformed)?;
    // The payload is base64url, without dots; the key identifier may hold some.
    let (id, payload) = rest.rsplit_once('.').ok_or(SignedError::Malformed)?;
    let tag = URL_SAFE_NO_PAD
        .decode(tag)
        .map_err(|_| SignedError::Malformed)?;
    let key = ring
        .by_id(id, purpose)
        .ok_or_else(|| SignedError::UnknownKey(id.to_owned()))?;
    mac(&key, signed)
        .verify_slice(&tag)
        .map_err(|_| SignedError::Forged)?;
    URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| SignedError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(ids: &[&str]) -> KeyRing {
        let lines: Vec<String> = ids.iter().map(|id| KeyRing::generate_line(id)).collect();
        KeyRing::parse(&lines.join("\n")).unwrap()
    }

    #[test]
    fn a_signed_value_reads_back_and_resists_tampering() {
        let ring = ring(&["k.1"]);
        let signed = sign(&ring, Purpose::Cursor, b"{\"n\":100}");
        assert!(signed.starts_with("v1.k.1."));
        assert_eq!(
            verify(&ring, Purpose::Cursor, &signed).unwrap(),
            b"{\"n\":100}"
        );
        assert_eq!(
            verify(&ring, Purpose::Session, &signed),
            Err(SignedError::Forged)
        );
        // Every octet of the value counts.
        for i in 0..signed.len() {
            let mut altered = signed.clone().into_bytes();
            altered[i] = if altered[i] == b'A' { b'B' } else { b'A' };
            let altered = String::from_utf8(altered).unwrap();
            assert!(
                verify(&ring, Purpose::Cursor, &altered).is_err(),
                "octet {i}"
            );
        }
        assert_eq!(
            verify(&ring, Purpose::Cursor, "garbage"),
            Err(SignedError::Malformed)
        );
    }

    #[test]
    fn a_retired_key_no_longer_verifies() {
        let old = ring(&["k1"]);
        let signed = sign(&old, Purpose::Cursor, b"x");
        let fresh = ring(&["k2"]);
        assert_eq!(
            verify(&fresh, Purpose::Cursor, &signed),
            Err(SignedError::UnknownKey("k1".into()))
        );
    }
}
