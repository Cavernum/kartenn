//! Break-glass accounts (spec 005, REQ-005-14, 15; K9): a passphrase of 20 characters
//! hashed by Argon2id (RFC 9106 §4, second recommended option: 64 MiB, t = 3, p = 4, salt
//! of 128 bits) and a TOTP second factor (RFC 6238: SHA-1, 30 s, 6 digits, one step of
//! delay, never the same code twice) whose secret is sealed under the key ring.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use crate::keys::{KeyRing, Purpose};
use crate::seal::{open, seal};

/// The checks of a break-glass login, under a key ring.
#[derive(Debug, Clone)]
pub struct Verifier(pub KeyRing);

impl kartenn_service::authn::BreakglassVerifier for Verifier {
    fn passphrase(&self, hash: &str, passphrase: &str) -> bool {
        verify_passphrase(hash, passphrase)
    }

    fn burn(&self, passphrase: &str) {
        let salt = SaltString::generate(&mut OsRng);
        let _ = argon2().hash_password(passphrase.as_bytes(), &salt);
    }

    fn totp_step(&self, sealed: &[u8], code: &str, now: i64, last_step: i64) -> Option<i64> {
        accepted_step(&self.0, sealed, code, now, last_step)
    }
}

/// Time step of the codes (RFC 6238 §5.2).
pub const STEP: i64 = 30;
/// The name a TOTP secret is sealed under.
const SEALED: &str = "totp";

/// What creating a break-glass account gives: shown once to the operator, then stored.
#[derive(Debug)]
pub struct NewBreakglass {
    /// The passphrase, 20 characters, shown once.
    pub passphrase: String,
    /// The `otpauth://` URI of the TOTP secret, shown once (for an authenticator app).
    pub otpauth: String,
    /// The Argon2id hash to store.
    pub password_hash: String,
    /// The TOTP secret sealed under the ring, to store.
    pub totp_sealed: Vec<u8>,
    /// The key of the ring that sealed it.
    pub totp_key_id: String,
}

fn argon2() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(64 * 1024, 3, 4, Some(32)).expect("RFC 9106 parameters are valid"),
    )
}

/// RFC 4648 base32 without padding, for the `otpauth://` URI.
fn base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(ALPHABET[((buffer >> bits) & 31) as usize]));
        }
    }
    if bits > 0 {
        out.push(char::from(ALPHABET[((buffer << (5 - bits)) & 31) as usize]));
    }
    out
}

/// A new account `name`: its passphrase, TOTP secret (160 bits, the size of the SHA-1
/// output, RFC 6238 §5.1) and what to store.
///
/// # Panics
///
/// If the system has no randomness, or the ring cannot seal 20 octets (never).
#[must_use]
pub fn create(ring: &KeyRing, name: &str) -> NewBreakglass {
    let mut bits = [0u8; 15];
    getrandom::fill(&mut bits).expect("the system provides randomness");
    let passphrase = URL_SAFE_NO_PAD.encode(bits);
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2()
        .hash_password(passphrase.as_bytes(), &salt)
        .expect("Argon2id hashes any passphrase")
        .to_string();
    let mut secret = [0u8; 20];
    getrandom::fill(&mut secret).expect("the system provides randomness");
    let sealed = seal(ring, Purpose::Totp, SEALED, &secret).expect("20 octets fit in a cookie");
    NewBreakglass {
        otpauth: format!(
            "otpauth://totp/kartenn:{name}?secret={}&issuer=kartenn&algorithm=SHA1&digits=6&period=30",
            base32(&secret)
        ),
        passphrase,
        password_hash,
        totp_sealed: sealed.concat().into_bytes(),
        totp_key_id: ring.current_id().to_owned(),
    }
}

/// Whether `passphrase` matches the Argon2id `hash`.
#[must_use]
pub fn verify_passphrase(hash: &str, passphrase: &str) -> bool {
    PasswordHash::new(hash)
        .is_ok_and(|h| argon2().verify_password(passphrase.as_bytes(), &h).is_ok())
}

/// The code of `secret` at the time step `counter` (RFC 4226 §5.3, 6 digits).
///
/// # Panics
///
/// Never: HMAC takes a key of any length.
#[must_use]
pub fn code(secret: &[u8], counter: i64) -> u32 {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("HMAC takes a key of any length");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[19] & 0x0f);
    let binary = (u32::from(digest[offset]) & 0x7f) << 24
        | u32::from(digest[offset + 1]) << 16
        | u32::from(digest[offset + 2]) << 8
        | u32::from(digest[offset + 3]);
    binary % 1_000_000
}

/// The step at which `given` is the code of the sealed `secret` at `now` (Unix seconds):
/// the current step or the one before (RFC 6238 §5.2), strictly after `last_step` so that
/// a code is never accepted twice; `None` otherwise.
#[must_use]
pub fn accepted_step(
    ring: &KeyRing,
    sealed: &[u8],
    given: &str,
    now: i64,
    last_step: i64,
) -> Option<i64> {
    let sealed = std::str::from_utf8(sealed).ok()?;
    let secret = open(ring, Purpose::Totp, SEALED, &[sealed]).ok()?;
    let given: u32 = given
        .trim()
        .parse()
        .ok()
        .filter(|_| given.trim().len() == 6)?;
    let current = now / STEP;
    [current, current - 1]
        .into_iter()
        .find(|&step| step > last_step && code(&secret, step) == given)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_follow_the_vectors_of_rfc_6238() {
        // Appendix B, SHA-1, secret "12345678901234567890", 8 digits: the last six.
        let secret = b"12345678901234567890";
        for (time, expected) in [
            (59, 287_082),
            (1_111_111_109, 81_804),
            (1_111_111_111, 50_471),
            (1_234_567_890, 5_924),
            (2_000_000_000, 279_037),
        ] {
            assert_eq!(code(secret, time / STEP), expected, "T = {time}");
        }
    }

    #[test]
    fn base32_follows_rfc_4648() {
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI");
    }

    #[test]
    fn a_new_account_verifies_once_per_code() {
        let ring = KeyRing::parse(&KeyRing::generate_line("k1")).unwrap();
        let account = create(&ring, "rescue");
        assert!(account.passphrase.len() >= 15, "NIST SP 800-63B §3.1.1.2");
        assert!(
            account
                .password_hash
                .starts_with("$argon2id$v=19$m=65536,t=3,p=4$")
        );
        assert!(verify_passphrase(
            &account.password_hash,
            &account.passphrase
        ));
        assert!(!verify_passphrase(&account.password_hash, "not it"));
        // The secret, as the authenticator app reads it from the URI.
        let sealed = std::str::from_utf8(&account.totp_sealed).unwrap();
        let secret = open(&ring, Purpose::Totp, SEALED, &[sealed]).unwrap();
        assert!(account.otpauth.contains(&base32(&secret)));
        let now = 1_800_000_000;
        let current = format!("{:06}", code(&secret, now / STEP));
        let step = accepted_step(&ring, &account.totp_sealed, &current, now, 0).unwrap();
        assert_eq!(step, now / STEP);
        // The same code again: refused (RFC 6238 §5.2).
        assert_eq!(
            accepted_step(&ring, &account.totp_sealed, &current, now, step),
            None
        );
        // One step late: accepted; two: refused.
        let previous = format!("{:06}", code(&secret, now / STEP - 1));
        assert!(accepted_step(&ring, &account.totp_sealed, &previous, now, 0).is_some());
        let older = format!("{:06}", code(&secret, now / STEP - 2));
        assert_eq!(
            accepted_step(&ring, &account.totp_sealed, &older, now, 0),
            None
        );
        assert_eq!(
            accepted_step(&ring, &account.totp_sealed, "12345", now, 0),
            None
        );
    }
}
