//! Authentication put to work (spec 005): the files that `[auth]` names, read at the start
//! so that a missing or malformed one stops it (REQ-012-31).

use std::time::Duration;

use anyhow::Context as _;
use kartenn_auth::issuer::{IssuerConfig, accepted_algorithm};
use kartenn_auth::keys::KeyRing;

use crate::config::Auth;

/// What authentication needs at run time.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The key ring of the cookies, service-account secrets and TOTP secrets.
    pub ring: KeyRing,
    /// The issuers, with their secrets.
    pub issuers: Vec<IssuerConfig>,
    /// Where the issuers send the browser back.
    pub redirect_uri: Option<String>,
    /// Inactivity that closes a session.
    pub idle: Duration,
    /// Lifetime of a session.
    pub absolute: Duration,
}

/// Reads the files of `auth`, already checked by the configuration.
///
/// # Errors
///
/// An unreadable file, a malformed key ring, an empty client secret.
pub fn load(auth: &Auth) -> anyhow::Result<Loaded> {
    let path = &auth.key_file;
    let ring = KeyRing::parse(
        &std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
    )
    .with_context(|| format!("the key ring {}", path.display()))?;
    let issuers = auth
        .issuers
        .iter()
        .map(|issuer| {
            let path = &issuer.client_secret_file;
            let secret = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            let secret = secret.trim();
            anyhow::ensure!(
                !secret.is_empty(),
                "the client secret {} is empty",
                path.display()
            );
            Ok(IssuerConfig {
                id: issuer.id.clone(),
                issuer: issuer.issuer.clone(),
                client_id: issuer.client_id.clone(),
                client_secret: secret.to_owned(),
                audience: issuer.audience.clone(),
                groups_claim: issuer.groups_claim.clone(),
                algorithms: issuer
                    .algorithms
                    .iter()
                    .filter_map(|a| accepted_algorithm(a))
                    .collect(),
            })
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(Loaded {
        ring,
        issuers,
        redirect_uri: auth.redirect_uri.clone(),
        idle: Duration::from_mins(auth.session_idle_minutes),
        absolute: Duration::from_hours(auth.session_absolute_hours),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::config::from_toml;

    /// A fresh directory of the temporary area.
    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kartenn-auth-{}", uuid()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uuid() -> sqlx::types::Uuid {
        sqlx::types::Uuid::now_v7()
    }

    fn config(dir: &std::path::Path) -> Auth {
        from_toml(&format!(
            r#"
            [auth]
            key_file = "{0}/keys"
            session_idle_minutes = 15
            redirect_uri = "https://kartenn.example.invalid/auth/callback"
            [[auth.issuers]]
            id = "internal"
            issuer = "https://idp.example.invalid"
            client_id = "kartenn"
            client_secret_file = "{0}/secret"
            audience = "kartenn"
            algorithms = ["ES256"]
            "#,
            dir.display()
        ))
        .unwrap()
        .auth
        .unwrap()
    }

    #[test]
    fn the_files_of_the_configuration_are_read_at_the_start() {
        let dir = scratch();
        let auth = config(&dir);
        let missing = load(&auth).unwrap_err();
        assert!(format!("{missing:#}").contains("keys"), "{missing:#}");
        std::fs::write(dir.join("keys"), "k1 not-a-key\n").unwrap();
        let malformed = load(&auth).unwrap_err();
        assert!(format!("{malformed:#}").contains("line 1"), "{malformed:#}");
        std::fs::write(dir.join("keys"), KeyRing::generate_line("k1")).unwrap();
        std::fs::write(dir.join("secret"), "\n").unwrap();
        let empty = load(&auth).unwrap_err();
        assert!(format!("{empty:#}").contains("empty"), "{empty:#}");
        std::fs::write(dir.join("secret"), "s3cret\n").unwrap();
        let loaded = load(&auth).unwrap();
        assert_eq!(loaded.ring.current_id(), "k1");
        assert_eq!(loaded.issuers[0].client_secret, "s3cret");
        assert_eq!(
            loaded.issuers[0].algorithms,
            [accepted_algorithm("ES256").unwrap()]
        );
        assert_eq!(loaded.idle, Duration::from_mins(15));
        assert_eq!(loaded.absolute, Duration::from_hours(8));
        assert!(!format!("{loaded:?}").contains("s3cret"), "never printed");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
