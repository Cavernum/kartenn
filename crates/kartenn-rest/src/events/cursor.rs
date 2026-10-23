//! The cursor of the event stream (spec 022, REQ-022-6, 8; ADR 0034): the floor of the
//! shared reader and the last number delivered, bound to an identity, encrypted and
//! authenticated by AES-256-GCM under a key of the ring derived for this purpose, so that
//! the client reads no number (R6) and a cursor stolen serves no one else.

use kartenn_auth::keys::{KeyRing, Purpose};
use kartenn_auth::seal;
use uuid::Uuid;

/// Prefix and format version of a cursor.
const PREFIX: &str = "k1.";
/// What the encryption binds the cursor to, beside its key.
const NAME: &str = "kartenn-events";
/// The version of the sealed value that the prefix replaces.
const SEALED: &str = "v1.";

/// A position in the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Every number up to it is committed and was read by the reader, or never will be.
    pub floor: i64,
    /// The greatest number delivered to the connection (a hint).
    pub last: i64,
}

/// What a cursor holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// The position.
    pub position: Position,
    /// The identity it was issued to.
    pub identity: Uuid,
    /// When it was issued, Unix seconds.
    pub issued: i64,
}

/// Why a cursor is refused: always treated as too old (`reset` `invalid_cursor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CursorError {
    /// Not a cursor, altered, or sealed by a key the ring no longer holds.
    #[error("the cursor does not open")]
    Invalid,
    /// Issued to another identity.
    #[error("the cursor belongs to another identity")]
    Foreign,
}

impl Cursor {
    /// The cursor as the `id:` field writes it: `k1.<key>.<base64url>`, without NUL, LF
    /// nor CR.
    ///
    /// # Panics
    ///
    /// If the system has no source of randomness.
    #[must_use]
    pub fn encode(&self, ring: &KeyRing) -> String {
        let mut plain = Vec::with_capacity(40);
        plain.extend(self.position.floor.to_be_bytes());
        plain.extend(self.position.last.to_be_bytes());
        plain.extend(self.identity.as_bytes());
        plain.extend(self.issued.to_be_bytes());
        let sealed = seal::seal(ring, Purpose::EventCursor, NAME, &plain)
            .expect("40 octets fit in one sealed value")
            .concat();
        format!("{PREFIX}{}", sealed.strip_prefix(SEALED).unwrap_or(&sealed))
    }

    /// The cursor `text`, issued to `identity`.
    ///
    /// # Errors
    ///
    /// [`CursorError`] for anything but an intact cursor of this ring issued to
    /// `identity`.
    pub fn decode(ring: &KeyRing, text: &str, identity: Uuid) -> Result<Self, CursorError> {
        let body = text.strip_prefix(PREFIX).ok_or(CursorError::Invalid)?;
        let sealed = format!("{SEALED}{body}");
        let plain = seal::open(ring, Purpose::EventCursor, NAME, &[&sealed])
            .map_err(|_| CursorError::Invalid)?;
        let plain: [u8; 40] = plain.try_into().map_err(|_| CursorError::Invalid)?;
        let i64_at = |at: usize| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&plain[at..at + 8]);
            i64::from_be_bytes(b)
        };
        let mut id = [0u8; 16];
        id.copy_from_slice(&plain[16..32]);
        let cursor = Self {
            position: Position {
                floor: i64_at(0),
                last: i64_at(8),
            },
            identity: Uuid::from_bytes(id),
            issued: i64_at(32),
        };
        if cursor.identity != identity {
            return Err(CursorError::Foreign);
        }
        Ok(cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor() -> Cursor {
        Cursor {
            position: Position {
                floor: 41,
                last: 57,
            },
            identity: Uuid::from_u128(9),
            issued: 1_792_756_800,
        }
    }

    fn ring() -> KeyRing {
        KeyRing::parse(&KeyRing::generate_line("k1")).unwrap()
    }

    #[test]
    fn a_cursor_reads_back() {
        let ring = ring();
        let text = cursor().encode(&ring);
        assert!(text.starts_with("k1.k1."));
        assert!(!text.contains(['\n', '\r', '\0']));
        assert_eq!(
            Cursor::decode(&ring, &text, Uuid::from_u128(9)),
            Ok(cursor())
        );
        // Two cursors of the same position differ (random nonce): nothing to compare.
        assert_ne!(text, cursor().encode(&ring));
    }

    #[test]
    fn an_altered_or_foreign_cursor_is_refused() {
        let ring = ring();
        let text = cursor().encode(&ring);
        for i in 0..text.len() {
            let mut altered = text.clone().into_bytes();
            altered[i] = if altered[i] == b'A' { b'B' } else { b'A' };
            let altered = String::from_utf8(altered).unwrap();
            assert!(
                Cursor::decode(&ring, &altered, Uuid::from_u128(9)).is_err(),
                "octet {i}"
            );
        }
        assert_eq!(
            Cursor::decode(&ring, &text, Uuid::from_u128(8)),
            Err(CursorError::Foreign)
        );
        assert_eq!(
            Cursor::decode(&ring, "garbage", Uuid::from_u128(9)),
            Err(CursorError::Invalid)
        );
        // Another ring with a key of the same name: invalid.
        assert_eq!(
            Cursor::decode(&self::ring(), &text, Uuid::from_u128(9)),
            Err(CursorError::Invalid)
        );
    }

    #[test]
    fn after_a_rotation_the_old_key_still_reads() {
        let old_line = KeyRing::generate_line("k1");
        let old = KeyRing::parse(&old_line).unwrap();
        let text = cursor().encode(&old);
        // The new key first, the old one kept for reading (005, « Trousseau »).
        let rotated =
            KeyRing::parse(&format!("{}\n{old_line}", KeyRing::generate_line("k2"))).unwrap();
        assert_eq!(rotated.current(Purpose::EventCursor).0, "k2");
        assert_eq!(
            Cursor::decode(&rotated, &text, Uuid::from_u128(9)),
            Ok(cursor())
        );
        assert!(cursor().encode(&rotated).starts_with("k1.k2."));
    }
}
