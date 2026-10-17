//! The optional signature of the seals (spec 012, REQ-012-13; 012/L23): Ed25519 over the
//! fingerprint of a batch.

use std::fmt;

use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

/// Why a key cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the signing key of the seals cannot be read: {0}")]
pub struct SealKeyError(String);

/// The key that signs the seals; it never prints.
pub struct SealKey {
    key: SigningKey,
    id: String,
}

impl fmt::Debug for SealKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealKey")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Identifier of a public key: the first 8 octets of its SHA-256, in hexadecimal.
#[must_use]
pub fn key_id(public: &[u8; 32]) -> String {
    hex(&Sha256::digest(public)[..8])
}

/// Lowercase hexadecimal.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

impl SealKey {
    /// A key from its 32 secret octets.
    #[must_use]
    pub fn from_bytes(secret: &[u8; 32]) -> Self {
        let key = SigningKey::from_bytes(secret);
        let id = key_id(&key.verifying_key().to_bytes());
        Self { key, id }
    }

    /// A key from a PKCS#8 document in PEM (`-----BEGIN PRIVATE KEY-----`).
    ///
    /// # Errors
    ///
    /// [`SealKeyError`] when the document is not an Ed25519 private key.
    pub fn from_pkcs8_pem(pem: &str) -> Result<Self, SealKeyError> {
        let key = SigningKey::from_pkcs8_pem(pem).map_err(|e| SealKeyError(e.to_string()))?;
        let id = key_id(&key.verifying_key().to_bytes());
        Ok(Self { key, id })
    }

    /// Identifier of the key, stored with each seal.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The public key, exported in `audit.seal.created`.
    #[must_use]
    pub fn public(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    /// The signature of a batch fingerprint.
    #[must_use]
    pub fn sign(&self, hash: &[u8; 32]) -> [u8; 64] {
        self.key.sign(hash).to_bytes()
    }
}

/// Whether `signature` is the signature of `hash` by `public`.
#[must_use]
pub fn verify(public: &[u8; 32], hash: &[u8; 32], signature: &[u8]) -> bool {
    let (Ok(key), Ok(signature)) = (
        VerifyingKey::from_bytes(public),
        Signature::from_slice(signature),
    ) else {
        return false;
    };
    key.verify(hash, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seal_signature_verifies_with_its_public_key_only() {
        let key = SealKey::from_bytes(&[7; 32]);
        let other = SealKey::from_bytes(&[8; 32]);
        let hash = [42; 32];
        let signature = key.sign(&hash);
        assert!(verify(&key.public(), &hash, &signature));
        assert!(!verify(&other.public(), &hash, &signature));
        assert!(!verify(&key.public(), &[43; 32], &signature));
        assert_eq!(key.id().len(), 16);
        assert!(
            !format!("{key:?}").contains("[7"),
            "the secret never prints"
        );
        assert!(SealKey::from_pkcs8_pem("not a key").is_err());
    }
}
