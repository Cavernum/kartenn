//! Credentials read from files and read again when they are renewed (REQ-019-29): a bound
//! service account token rotates within the hour, a client certificate before it
//! expires, the secret of the kartenn account when an operator renews it. A file is read
//! again as soon as its metadata changes (modification time, length, inode: an atomic
//! replacement by rename, as the kubelet does for projected tokens, changes the inode).
//! Neither a secret nor the content of a file ever reaches an error, a log or a trace.

use std::fmt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use rustls::client::ResolvesClientCert;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::sign::CertifiedKey;
use rustls::{SignatureScheme, crypto};

/// Why a credential file cannot be used; the message names the file, never its content.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialError {
    /// The file cannot be read.
    #[error("credential file {}: {source}", file.display())]
    Read {
        /// The file.
        file: PathBuf,
        /// The reason given by the system.
        #[source]
        source: std::io::Error,
    },
    /// The file holds nothing but blanks.
    #[error("credential file {} is empty", file.display())]
    Empty {
        /// The file.
        file: PathBuf,
    },
    /// The token holds a character an HTTP header refuses.
    #[error("token file {} holds a character not allowed in an HTTP header", file.display())]
    NotAHeader {
        /// The file.
        file: PathBuf,
    },
    /// A PEM file holds no certificate or no usable private key.
    #[error("{} : {reason}", file.display())]
    Pem {
        /// The file.
        file: PathBuf,
        /// What is wrong, without the content.
        reason: String,
    },
}

/// A secret read from a file; its `Debug` is redacted and it has no `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// The value itself, for the one place that sends it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

/// What tells that a file changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
    inode: u64,
}

impl From<&std::fs::Metadata> for Stamp {
    fn from(metadata: &std::fs::Metadata) -> Self {
        Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
            inode: metadata.ino(),
        }
    }
}

fn read_error(file: &Path) -> impl FnOnce(std::io::Error) -> CredentialError + '_ {
    |source| CredentialError::Read {
        file: file.to_owned(),
        source,
    }
}

/// A secret in a file (a token, the secret of the kartenn account): one line, blanks
/// around it ignored.
#[derive(Debug)]
pub struct CredentialFile {
    path: PathBuf,
    cached: Mutex<Option<(Stamp, Secret)>>,
}

impl CredentialFile {
    /// A file not read yet.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            cached: Mutex::new(None),
        }
    }

    /// The file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The current secret: the one read before if the file did not change, else the new
    /// content.
    ///
    /// # Errors
    ///
    /// [`CredentialError::Read`] or [`CredentialError::Empty`].
    pub async fn read(&self) -> Result<Secret, CredentialError> {
        let metadata = tokio::fs::metadata(&self.path)
            .await
            .map_err(read_error(&self.path))?;
        let stamp = Stamp::from(&metadata);
        if let Some(secret) = self.cached_if(stamp) {
            return Ok(secret);
        }
        let text = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(read_error(&self.path))?;
        let value = text.trim();
        if value.is_empty() {
            return Err(CredentialError::Empty {
                file: self.path.clone(),
            });
        }
        let secret = Secret(value.to_owned());
        let first = self.lock().replace((stamp, secret.clone())).is_none();
        if !first {
            tracing::info!(file = %self.path.display(), "credential file read again");
        }
        Ok(secret)
    }

    fn cached_if(&self, stamp: Stamp) -> Option<Secret> {
        self.lock()
            .as_ref()
            .filter(|(seen, _)| *seen == stamp)
            .map(|(_, secret)| secret.clone())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<(Stamp, Secret)>> {
        // The guarded value is replaced whole, a panic cannot leave it half written.
        self.cached.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A client certificate and its key in two files, read again by the TLS handshake that
/// follows a change of either.
pub(crate) struct ClientCertFiles {
    cert_file: PathBuf,
    key_file: PathBuf,
    provider: Arc<CryptoProvider>,
    cached: Mutex<Option<(Stamp, Stamp, Arc<CertifiedKey>)>>,
}

impl fmt::Debug for ClientCertFiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientCertFiles")
            .field("cert_file", &self.cert_file)
            .field("key_file", &self.key_file)
            .finish_non_exhaustive()
    }
}

impl ClientCertFiles {
    pub(crate) fn new(cert_file: &Path, key_file: &Path, provider: Arc<CryptoProvider>) -> Self {
        Self {
            cert_file: cert_file.to_owned(),
            key_file: key_file.to_owned(),
            provider,
            cached: Mutex::new(None),
        }
    }

    /// The current certificate and key, read again if either file changed. Blocking,
    /// called by the handshake (a few small files).
    pub(crate) fn current(&self) -> Result<Arc<CertifiedKey>, CredentialError> {
        let stamp = |file: &Path| {
            std::fs::metadata(file)
                .map(|m| Stamp::from(&m))
                .map_err(read_error(file))
        };
        let (cert_stamp, key_stamp) = (stamp(&self.cert_file)?, stamp(&self.key_file)?);
        let mut cached = self.cached.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((cert_seen, key_seen, key)) = &*cached
            && *cert_seen == cert_stamp
            && *key_seen == key_stamp
        {
            return Ok(Arc::clone(key));
        }
        let key = Arc::new(self.load()?);
        if cached
            .replace((cert_stamp, key_stamp, Arc::clone(&key)))
            .is_some()
        {
            tracing::info!(file = %self.cert_file.display(), "client certificate read again");
        }
        Ok(key)
    }

    fn load(&self) -> Result<CertifiedKey, CredentialError> {
        let pem = |file: &Path, reason: &str| CredentialError::Pem {
            file: file.to_owned(),
            reason: reason.to_owned(),
        };
        let chain = CertificateDer::pem_file_iter(&self.cert_file)
            .map_err(|_| pem(&self.cert_file, "unreadable PEM file"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| pem(&self.cert_file, "malformed PEM certificate"))?;
        if chain.is_empty() {
            return Err(pem(&self.cert_file, "no certificate"));
        }
        let der = PrivateKeyDer::from_pem_file(&self.key_file)
            .map_err(|_| pem(&self.key_file, "no PEM private key"))?;
        let signing = self
            .provider
            .key_provider
            .load_private_key(der)
            .map_err(|_| pem(&self.key_file, "unsupported private key"))?;
        Ok(CertifiedKey::new(chain, signing))
    }
}

impl ResolvesClientCert for ClientCertFiles {
    fn resolve(
        &self,
        _root_hint_subjects: &[&[u8]],
        _sigschemes: &[SignatureScheme],
    ) -> Option<Arc<CertifiedKey>> {
        self.current()
            .inspect_err(|e| tracing::warn!(error = %e, "client certificate unavailable"))
            .ok()
    }

    fn has_certs(&self) -> bool {
        true
    }
}

/// The ring provider, the only one of kartenn (no aws-lc-rs, Q-019-12).
pub(crate) fn provider() -> Arc<CryptoProvider> {
    Arc::new(crypto::ring::default_provider())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_pair(dir: &Path) -> (PathBuf, PathBuf, Vec<u8>) {
        let certified = rcgen::generate_simple_self_signed(vec!["kube-sync".into()]).unwrap();
        let (cert, key) = (dir.join("client.crt"), dir.join("client.key"));
        std::fs::write(&cert, certified.cert.pem()).unwrap();
        std::fs::write(&key, certified.signing_key.serialize_pem()).unwrap();
        (cert, key, certified.cert.der().to_vec())
    }

    #[test]
    fn client_certificate_read_again() {
        let dir = tempfile::tempdir().unwrap();
        let (cert, key, first) = write_pair(dir.path());
        let files = ClientCertFiles::new(&cert, &key, provider());
        let current = files.current().unwrap();
        assert_eq!(current.cert[0].as_ref(), first.as_slice());
        assert!(Arc::ptr_eq(&current, &files.current().unwrap()));

        let renewed = dir.path().join("renewed");
        std::fs::create_dir(&renewed).unwrap();
        let (new_cert, new_key, second) = write_pair(&renewed);
        std::fs::rename(new_key, &key).unwrap();
        std::fs::rename(new_cert, &cert).unwrap();
        assert_eq!(files.current().unwrap().cert[0].as_ref(), second.as_slice());
        assert!(files.resolve(&[], &[]).is_some());

        std::fs::write(&key, "not a key").unwrap();
        let error = files.current().unwrap_err().to_string();
        assert!(
            error.contains("client.key") && !error.contains("not a key"),
            "{error}"
        );
        assert!(files.resolve(&[], &[]).is_none());
    }
}
