//! Sealed cookies (spec 005, REQ-005-6; ADR 0020): a value encrypted and authenticated by
//! AES-256-GCM under a key of the ring, bound to the cookie's name; split over at most
//! three cookies when it exceeds what a browser keeps in one.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::keys::{KeyRing, Purpose};

/// Format version of a sealed value.
const VERSION: &str = "v1";
/// Most octets of one cookie value, under the 4 096 a browser keeps for name and value.
pub const CHUNK: usize = 3_800;
/// Most cookies a sealed value spans.
pub const CHUNKS: usize = 3;

/// Why a value cannot be sealed or opened.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SealError {
    /// Larger than [`CHUNKS`] cookies.
    #[error("the sealed value exceeds {CHUNKS} cookies")]
    TooLarge,
    /// Not a sealed value of this format.
    #[error("malformed sealed value")]
    Malformed,
    /// Sealed with a key the ring no longer holds.
    #[error("unknown key {0}")]
    UnknownKey(String),
    /// Altered, or sealed under another name or purpose.
    #[error("the sealed value does not authenticate")]
    Forged,
}

/// Seals `plaintext` for the cookie `name`: `v1.<key id>.<nonce and ciphertext>`,
/// base64url, split into at most [`CHUNKS`] values.
///
/// # Errors
///
/// [`SealError::TooLarge`].
///
/// # Panics
///
/// If the system has no source of randomness.
pub fn seal(
    ring: &KeyRing,
    purpose: Purpose,
    name: &str,
    plaintext: &[u8],
) -> Result<Vec<String>, SealError> {
    let (id, key) = ring.current(purpose);
    let cipher = Aes256Gcm::new(&key.into());
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).expect("the system provides randomness");
    let aad = format!("{name}|{id}");
    let ciphertext = cipher
        .encrypt(
            &Nonce::from(nonce),
            Payload {
                msg: plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| SealError::Malformed)?;
    let mut body = nonce.to_vec();
    body.extend(ciphertext);
    let value = format!("{VERSION}.{id}.{}", URL_SAFE_NO_PAD.encode(body));
    let chunks: Vec<String> = value
        .as_bytes()
        .chunks(CHUNK)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect();
    if chunks.len() > CHUNKS {
        return Err(SealError::TooLarge);
    }
    Ok(chunks)
}

/// Opens the value sealed for `name` from its chunks, in order.
///
/// # Errors
///
/// [`SealError`] for anything but an intact value sealed by a key of the ring, for this
/// name and purpose.
pub fn open(
    ring: &KeyRing,
    purpose: Purpose,
    name: &str,
    chunks: &[&str],
) -> Result<Vec<u8>, SealError> {
    if chunks.is_empty() || chunks.len() > CHUNKS {
        return Err(SealError::Malformed);
    }
    let value = chunks.concat();
    let mut parts = value.splitn(3, '.');
    let (Some(VERSION), Some(id), Some(body)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(SealError::Malformed);
    };
    let key = ring
        .by_id(id, purpose)
        .ok_or_else(|| SealError::UnknownKey(id.to_owned()))?;
    let body = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| SealError::Malformed)?;
    if body.len() < 12 + 16 {
        return Err(SealError::Malformed);
    }
    let (nonce, ciphertext) = body.split_at(12);
    let nonce: [u8; 12] = nonce.try_into().map_err(|_| SealError::Malformed)?;
    let aad = format!("{name}|{id}");
    Aes256Gcm::new(&key.into())
        .decrypt(
            &Nonce::from(nonce),
            Payload {
                msg: ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| SealError::Forged)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(ids: &[&str]) -> KeyRing {
        let text: Vec<String> = ids.iter().map(|id| KeyRing::generate_line(id)).collect();
        KeyRing::parse(&text.join("\n")).unwrap()
    }

    #[test]
    fn a_sealed_value_opens_on_another_instance_only_intact() {
        let ring = ring(&["k1"]);
        let sealed = seal(&ring, Purpose::Session, "__Host-kartenn", b"identity").unwrap();
        assert_eq!(sealed.len(), 1);
        let chunks: Vec<&str> = sealed.iter().map(String::as_str).collect();
        // Another instance: the same ring, read from the same file.
        let other = ring.clone();
        assert_eq!(
            open(&other, Purpose::Session, "__Host-kartenn", &chunks).unwrap(),
            b"identity"
        );
        // One octet changed.
        let mut forged = sealed[0].clone().into_bytes();
        let last = forged.len() - 2;
        forged[last] = if forged[last] == b'A' { b'B' } else { b'A' };
        let forged = String::from_utf8(forged).unwrap();
        assert_eq!(
            open(&ring, Purpose::Session, "__Host-kartenn", &[&forged]),
            Err(SealError::Forged)
        );
        // Another name, another purpose: refused.
        assert_eq!(
            open(&ring, Purpose::Session, "__Host-other", &chunks),
            Err(SealError::Forged)
        );
        assert_eq!(
            open(&ring, Purpose::Login, "__Host-kartenn", &chunks),
            Err(SealError::Forged)
        );
    }

    #[test]
    fn rotation_keeps_reading_until_the_key_is_removed() {
        let (k1, k2) = (KeyRing::generate_line("k1"), KeyRing::generate_line("k2"));
        let old = KeyRing::parse(&k1).unwrap();
        let sealed = seal(&old, Purpose::Session, "c", b"x").unwrap();
        let chunks: Vec<&str> = sealed.iter().map(String::as_str).collect();
        // The new key first, the old one kept: still read, and new values use k2.
        let rotated = KeyRing::parse(&format!("{k2}\n{k1}")).unwrap();
        assert_eq!(
            open(&rotated, Purpose::Session, "c", &chunks).unwrap(),
            b"x"
        );
        assert!(seal(&rotated, Purpose::Session, "c", b"x").unwrap()[0].starts_with("v1.k2."));
        // The old key removed: refused.
        let fresh = KeyRing::parse(&k2).unwrap();
        assert_eq!(
            open(&fresh, Purpose::Session, "c", &chunks),
            Err(SealError::UnknownKey("k1".into()))
        );
    }

    #[test]
    fn a_large_value_spans_up_to_three_cookies() {
        let ring = ring(&["k1"]);
        let large = vec![b'a'; 6_000];
        let sealed = seal(&ring, Purpose::Session, "c", &large).unwrap();
        assert_eq!(sealed.len(), 3);
        let chunks: Vec<&str> = sealed.iter().map(String::as_str).collect();
        assert_eq!(open(&ring, Purpose::Session, "c", &chunks).unwrap(), large);
        assert_eq!(
            seal(&ring, Purpose::Session, "c", &vec![b'a'; 12_000]),
            Err(SealError::TooLarge)
        );
    }
}
