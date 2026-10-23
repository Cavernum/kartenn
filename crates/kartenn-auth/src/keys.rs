//! The key ring of the instances (spec 005, design § Authentification): one or more keys
//! of 256 bits, the first one writing, all of them reading; a key per use derived by
//! HKDF-SHA-256 (RFC 5869), so that a cookie key never serves as an HMAC key.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use hkdf::Hkdf;
use sha2::Sha256;

/// What a derived key is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// The session cookie.
    Session,
    /// The cookie of a login in progress (state, nonce, PKCE verifier).
    Login,
    /// The digests of the secrets of service accounts.
    ServiceSecret,
    /// The TOTP secrets of break-glass accounts.
    Totp,
    /// The pagination cursors of the API (spec 006, REQ-006-8).
    Cursor,
    /// The cursors of the event stream (spec 022, REQ-022-6), encrypted.
    EventCursor,
}

impl Purpose {
    const fn info(self) -> &'static [u8] {
        match self {
            Self::Session => b"kartenn/session/v1",
            Self::Login => b"kartenn/login/v1",
            Self::ServiceSecret => b"kartenn/service-secret/v1",
            Self::Totp => b"kartenn/totp/v1",
            Self::Cursor => b"kartenn/cursor/v1",
            Self::EventCursor => b"kartenn/event-cursor/v1",
        }
    }
}

/// Why a key ring cannot be read.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyError {
    /// A line is not `<id> <base64 of 32 octets>`.
    #[error("line {0}: expected `<id> <base64 of 32 octets>`")]
    Malformed(usize),
    /// Two keys share an identifier.
    #[error("two keys are named {0}")]
    Duplicate(String),
    /// No key at all.
    #[error("the key ring holds no key")]
    Empty,
}

/// A key of the ring.
#[derive(Clone)]
struct Key {
    id: String,
    material: [u8; 32],
}

/// The keys of the instances; never printed.
#[derive(Clone)]
pub struct KeyRing {
    keys: Vec<Key>,
}

impl fmt::Debug for KeyRing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyRing")
            .field("ids", &self.keys.iter().map(|k| &k.id).collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl KeyRing {
    /// Reads a ring: one key per line, `<id> <base64 of 32 octets>`, the first writing;
    /// blank lines and `#` comments ignored.
    ///
    /// # Errors
    ///
    /// [`KeyError`] for a malformed line, a duplicate identifier or no key.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let mut keys: Vec<Key> = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (id, encoded) = line
                .split_once(char::is_whitespace)
                .ok_or(KeyError::Malformed(n + 1))?;
            let material: [u8; 32] = STANDARD
                .decode(encoded.trim())
                .ok()
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(KeyError::Malformed(n + 1))?;
            if keys.iter().any(|k| k.id == id) {
                return Err(KeyError::Duplicate(id.to_owned()));
            }
            keys.push(Key {
                id: id.to_owned(),
                material,
            });
        }
        if keys.is_empty() {
            return Err(KeyError::Empty);
        }
        Ok(Self { keys })
    }

    /// A ring of one fresh random key, as a line to store.
    ///
    /// # Panics
    ///
    /// If the system has no source of randomness.
    #[must_use]
    pub fn generate_line(id: &str) -> String {
        let mut material = [0u8; 32];
        getrandom::fill(&mut material).expect("the system provides randomness");
        format!("{id} {}", STANDARD.encode(material))
    }

    /// The identifier of the writing key.
    #[must_use]
    pub fn current_id(&self) -> &str {
        &self.keys[0].id
    }

    /// The key for `purpose` derived from the writing key, with its identifier.
    #[must_use]
    pub fn current(&self, purpose: Purpose) -> (&str, [u8; 32]) {
        let key = &self.keys[0];
        (&key.id, derive(&key.material, purpose))
    }

    /// The key for `purpose` derived from the key `id`, if the ring holds it.
    #[must_use]
    pub fn by_id(&self, id: &str, purpose: Purpose) -> Option<[u8; 32]> {
        self.keys
            .iter()
            .find(|k| k.id == id)
            .map(|k| derive(&k.material, purpose))
    }
}

fn derive(material: &[u8; 32], purpose: Purpose) -> [u8; 32] {
    let mut out = [0u8; 32];
    Hkdf::<Sha256>::new(None, material)
        .expand(purpose.info(), &mut out)
        .expect("32 octets are a valid HKDF-SHA-256 output length");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_reads_and_derives_one_key_per_purpose() {
        let text = format!(
            "# rotated in 2026\n{}\n\n{}\n",
            KeyRing::generate_line("k2"),
            KeyRing::generate_line("k1")
        );
        let ring = KeyRing::parse(&text).unwrap();
        assert_eq!(ring.current_id(), "k2");
        let (id, session) = ring.current(Purpose::Session);
        assert_eq!(id, "k2");
        assert_ne!(
            session,
            ring.current(Purpose::Login).1,
            "one key per purpose"
        );
        assert_eq!(ring.by_id("k2", Purpose::Session), Some(session));
        assert!(ring.by_id("k1", Purpose::Session).is_some());
        assert!(ring.by_id("k0", Purpose::Session).is_none());
        assert!(
            !format!("{ring:?}").contains(&text[20..40]),
            "never printed"
        );
    }

    #[test]
    fn a_bad_ring_is_refused() {
        assert_eq!(KeyRing::parse("").err(), Some(KeyError::Empty));
        assert_eq!(
            KeyRing::parse("k1 c2hvcnQ=").err(),
            Some(KeyError::Malformed(1))
        );
        let line = KeyRing::generate_line("k1");
        assert_eq!(
            KeyRing::parse(&format!("{line}\n{line}")).err(),
            Some(KeyError::Duplicate("k1".into()))
        );
    }
}
