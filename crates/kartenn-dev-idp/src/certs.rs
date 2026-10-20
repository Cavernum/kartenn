//! `kartenn-dev-idp certs <dir>`: the material of a development setup, written once and
//! kept afterwards. A development authority (`ca.pem`, to trust through `SSL_CERT_FILE`),
//! a certificate for the loopback signed by it (`cert.pem`, `key.pem`), shared by the
//! issuer and kartenn-server; the client secret of kartenn at the issuer (`idp-secret`);
//! the key ring of kartenn-server (`kartenn-keys`).

use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

use anyhow::Context as _;
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use rand_core::{OsRng, RngCore as _};
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair, KeyUsagePurpose};

/// The authority.
pub const CA: &str = "ca.pem";
/// The certificate of the loopback.
pub const CERT: &str = "cert.pem";
/// Its key.
pub const KEY: &str = "key.pem";
/// The client secret of kartenn.
pub const SECRET: &str = "idp-secret";
/// The key ring of kartenn-server.
pub const KEYS: &str = "kartenn-keys";

/// Names the loopback answers to.
const LOOPBACK: [&str; 3] = ["127.0.0.1", "::1", "localhost"];

/// Writes in `dir` what is missing: the authority and the certificate together, the
/// secret and the key ring each on their own.
///
/// # Errors
///
/// An unwritable directory or file.
pub fn generate(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    if [CA, CERT, KEY].iter().any(|f| !dir.join(f).exists()) {
        let (ca, cert, key) = authority()?;
        write(&dir.join(CA), &ca, 0o644)?;
        write(&dir.join(CERT), &cert, 0o644)?;
        write(&dir.join(KEY), &key, 0o600)?;
    }
    if !dir.join(SECRET).exists() {
        write(
            &dir.join(SECRET),
            &format!("{}\n", URL_SAFE_NO_PAD.encode(random::<32>())),
            0o600,
        )?;
    }
    if !dir.join(KEYS).exists() {
        write(
            &dir.join(KEYS),
            &format!("dev {}\n", STANDARD.encode(random::<32>())),
            0o600,
        )?;
    }
    Ok(())
}

/// The authority, then the certificate of the loopback and its key, in PEM.
fn authority() -> anyhow::Result<(String, String, String)> {
    let ca_key = KeyPair::generate()?;
    let mut ca = CertificateParams::new(Vec::<String>::new())?;
    ca.distinguished_name
        .push(DnType::CommonName, "kartenn development authority");
    ca.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    ca.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let ca_pem = ca.self_signed(&ca_key)?.pem();
    let issuer = Issuer::new(ca, ca_key);

    let key = KeyPair::generate()?;
    let mut leaf = CertificateParams::new(LOOPBACK.map(String::from).to_vec())?;
    leaf.distinguished_name
        .push(DnType::CommonName, "kartenn development loopback");
    let cert = leaf.signed_by(&key, &issuer)?;
    Ok((ca_pem, cert.pem(), key.serialize_pem()))
}

/// `N` random bytes from the system.
pub(crate) fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn write(path: &Path, text: &str, mode: u32) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)
        .with_context(|| format!("writing {}", path.display()))?;
    file.write_all(text.as_bytes())
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_once_and_keeps() {
        let dir =
            std::env::temp_dir().join(format!("kartenn-dev-idp-certs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        generate(&dir).unwrap();
        let first: Vec<String> = [CA, CERT, KEY, SECRET, KEYS]
            .iter()
            .map(|f| std::fs::read_to_string(dir.join(f)).unwrap())
            .collect();
        assert!(first[0].starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(first[4].starts_with("dev "));
        generate(&dir).unwrap();
        let second: Vec<String> = [CA, CERT, KEY, SECRET, KEYS]
            .iter()
            .map(|f| std::fs::read_to_string(dir.join(f)).unwrap())
            .collect();
        assert_eq!(first, second, "kept once written");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
