//! Secrets of service accounts (spec 005, REQ-005-11 to 13; K8): generated here, 256
//! random bits shown once; stored as an HMAC-SHA-256 under a key of the ring; presented
//! in a Basic header (RFC 7617) over TLS only; failures limited per source.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use kartenn_service::authn::SecretHasher;
use kartenn_service::operations::SecretRecord;
use sha2::Sha256;
use uuid::Uuid;

use crate::keys::{KeyRing, Purpose};

/// A new secret: what the caller shows once, and what the changeset stores.
#[derive(Debug)]
pub struct NewSecret {
    /// The secret, 43 base64url characters; shown once, never stored.
    pub plaintext: String,
    /// Its record (digest and key), for `CreateServiceAccount` or `AddServiceSecret`.
    pub record: SecretRecord,
}

/// The HMAC of `secret` under `key`.
fn mac(key: &[u8; 32], secret: &str) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes a key of any length");
    mac.update(secret.as_bytes());
    mac.finalize().into_bytes().into()
}

/// Generates a secret and its record under the writing key of `ring`, valid until
/// `expires_at` (microseconds since the epoch) if given.
///
/// # Panics
///
/// If the system has no source of randomness.
#[must_use]
pub fn generate(ring: &KeyRing, expires_at: Option<i64>) -> NewSecret {
    let mut bits = [0u8; 32];
    getrandom::fill(&mut bits).expect("the system provides randomness");
    let plaintext = URL_SAFE_NO_PAD.encode(bits);
    let (key_id, key) = ring.current(Purpose::ServiceSecret);
    let digest = mac(&key, &plaintext);
    NewSecret {
        record: SecretRecord {
            id: Uuid::now_v7(),
            digest: kartenn_core::audit::seal::hex(&digest),
            key_id: key_id.to_owned(),
            expires_at,
        },
        plaintext,
    }
}

impl SecretHasher for KeyRing {
    fn digest(&self, key_id: &str, secret: &str) -> Option<[u8; 32]> {
        self.by_id(key_id, Purpose::ServiceSecret)
            .map(|key| mac(&key, secret))
    }
}

/// The user and password of a `Basic` header value (RFC 7617 §2): `None` if malformed.
/// The user cannot hold a `:`.
#[must_use]
pub fn parse_basic(header: &str) -> Option<(String, String)> {
    let (scheme, encoded) = header.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let decoded = String::from_utf8(STANDARD.decode(encoded.trim()).ok()?).ok()?;
    let (user, password) = decoded.split_once(':')?;
    Some((user.to_owned(), password.to_owned()))
}

/// Limits the failed authentications of a source (REQ-005-13): past `max` failures in a
/// window, the source is refused until the window ends; no account is ever locked (a
/// generated secret cannot be guessed). Per instance (D9).
#[derive(Debug)]
pub struct FailureLimiter {
    max: u32,
    window: Duration,
    sources: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl FailureLimiter {
    /// At most `max` failures per `window` and source.
    #[must_use]
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            max,
            window,
            sources: Mutex::new(HashMap::new()),
        }
    }

    /// Whether `source` may try at `now`.
    pub fn allows(&self, source: IpAddr, now: Instant) -> bool {
        let mut sources = self.sources.lock().unwrap_or_else(PoisonError::into_inner);
        match sources.get(&source) {
            Some((start, _)) if now.duration_since(*start) >= self.window => {
                sources.remove(&source);
                true
            }
            Some((_, failures)) => *failures < self.max,
            None => true,
        }
    }

    /// Forgets the sources whose window is over at `now` (the local timer of REQ-006-38).
    pub fn purge(&self, now: Instant) {
        self.sources
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, (start, _)| now.duration_since(*start) < self.window);
    }

    /// Counts a failure of `source` at `now`.
    pub fn failed(&self, source: IpAddr, now: Instant) {
        let mut sources = self.sources.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = sources.entry(source).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring() -> KeyRing {
        KeyRing::parse(&KeyRing::generate_line("k1")).unwrap()
    }

    #[test]
    fn a_generated_secret_matches_its_digest_only() {
        let ring = ring();
        let new = generate(&ring, None);
        assert_eq!(new.plaintext.len(), 43, "256 bits in base64url");
        assert_eq!(new.record.digest.len(), 64);
        let digest = ring.digest("k1", &new.plaintext).unwrap();
        assert_eq!(kartenn_core::audit::seal::hex(&digest), new.record.digest);
        assert_ne!(ring.digest("k1", "another"), Some(digest));
        assert_eq!(ring.digest("k9", &new.plaintext), None, "unknown key");
        assert_ne!(generate(&ring, None).plaintext, new.plaintext);
    }

    #[test]
    fn the_basic_header_is_read_per_rfc_7617() {
        let header = format!("Basic {}", STANDARD.encode("svc-backup:s3cr:et"));
        assert_eq!(
            parse_basic(&header),
            Some(("svc-backup".into(), "s3cr:et".into())),
            "the password may hold a colon"
        );
        assert_eq!(
            parse_basic(&format!("basic {}", STANDARD.encode("a:b"))),
            Some(("a".into(), "b".into()))
        );
        for bad in [
            "Bearer abc",
            "Basic !!!",
            "Basic",
            &format!("Basic {}", STANDARD.encode("nocolon")),
        ] {
            assert_eq!(parse_basic(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_source_is_limited_then_released() {
        let limiter = FailureLimiter::new(3, Duration::from_mins(1));
        let ip: IpAddr = "192.0.2.7".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..3 {
            assert!(limiter.allows(ip, t0));
            limiter.failed(ip, t0);
        }
        assert!(!limiter.allows(ip, t0 + Duration::from_secs(30)));
        assert!(
            limiter.allows("192.0.2.8".parse().unwrap(), t0),
            "another source"
        );
        assert!(
            limiter.allows(ip, t0 + Duration::from_secs(61)),
            "window over"
        );
    }

    #[test]
    fn the_purge_forgets_the_windows_over() {
        let limiter = FailureLimiter::new(3, Duration::from_mins(1));
        let t0 = Instant::now();
        limiter.failed("192.0.2.7".parse().unwrap(), t0);
        limiter.failed("192.0.2.8".parse().unwrap(), t0 + Duration::from_secs(30));
        limiter.purge(t0 + Duration::from_secs(61));
        let sources = limiter.sources.lock().unwrap();
        assert_eq!(
            sources.keys().copied().collect::<Vec<IpAddr>>(),
            vec!["192.0.2.8".parse::<IpAddr>().unwrap()]
        );
    }
}
