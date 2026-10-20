//! The configuration of the issuer, in TOML: where it listens, its client, its users and
//! their groups, and the users whose login fails on purpose.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Deserialize;

/// The issuer.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Its identifier (`iss`), in `https`.
    pub issuer: String,
    /// Where it listens: the loopback only.
    pub listen: SocketAddr,
    /// The client of kartenn.
    pub client_id: String,
    /// The file of its secret (`client_secret_basic`).
    pub client_secret_file: PathBuf,
    /// Where it may send the browser back, compared exactly.
    pub redirect_uris: Vec<String>,
    /// The certificate it serves, PEM.
    pub cert_file: PathBuf,
    /// Its key, PEM.
    pub key_file: PathBuf,
    /// The users offered on the login page.
    pub users: Vec<User>,
}

/// A user.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct User {
    /// Subject (`sub`).
    pub sub: String,
    /// Name shown (`name`).
    pub name: String,
    /// Email (`email`), if any.
    #[serde(default)]
    pub email: Option<String>,
    /// Groups (`groups`).
    #[serde(default)]
    pub groups: Vec<String>,
    /// An error code answered instead of a code (RFC 6749 §4.1.2.1, OpenID Connect Core
    /// §3.1.2.6).
    #[serde(default)]
    pub error: Option<String>,
}

/// The error codes a user may answer.
const ERRORS: [&str; 11] = [
    "access_denied",
    "invalid_request",
    "unauthorized_client",
    "unsupported_response_type",
    "invalid_scope",
    "server_error",
    "temporarily_unavailable",
    "interaction_required",
    "login_required",
    "account_selection_required",
    "consent_required",
];

impl Config {
    /// Reads and checks `path`.
    ///
    /// # Errors
    ///
    /// An unreadable or malformed file, or a refused value ([`Config::check`]).
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let config: Self = toml::from_str(&text).with_context(|| format!("{}", path.display()))?;
        config.check()?;
        Ok(config)
    }

    /// Refuses a listening address off the loopback, an issuer not in `https`, a user
    /// without subject or twice, an unknown error code.
    ///
    /// # Errors
    ///
    /// The first refused value.
    pub fn check(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.listen.ip().is_loopback(),
            "listen: {} is not on the loopback; this issuer is for development only",
            self.listen
        );
        anyhow::ensure!(
            self.issuer.starts_with("https://")
                && !self.issuer.contains(['?', '#'])
                && !self.issuer.ends_with('/'),
            "issuer: https, without query, fragment or final slash"
        );
        anyhow::ensure!(!self.client_id.is_empty(), "client_id: required");
        anyhow::ensure!(
            !self.redirect_uris.is_empty(),
            "redirect_uris: at least one"
        );
        anyhow::ensure!(!self.users.is_empty(), "users: at least one");
        let mut subjects = HashSet::new();
        for user in &self.users {
            anyhow::ensure!(!user.sub.is_empty(), "users: a subject is empty");
            anyhow::ensure!(subjects.insert(&user.sub), "users: {} twice", user.sub);
            if let Some(error) = &user.error {
                anyhow::ensure!(
                    ERRORS.contains(&error.as_str()),
                    "users.{}.error: {error} is not an error code",
                    user.sub
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(listen: &str) -> anyhow::Result<Config> {
        let config: Config = toml::from_str(&format!(
            r#"
            issuer = "https://127.0.0.1:8444"
            listen = "{listen}"
            client_id = "kartenn"
            client_secret_file = "dev/idp-secret"
            redirect_uris = ["http://127.0.0.1:5173/v1/auth/callback"]
            cert_file = "cert.pem"
            key_file = "key.pem"

            [[users]]
            sub = "alice"
            name = "Alice Exemple"
            groups = ["kartenn-admins"]

            [[users]]
            sub = "refuse"
            name = "Refus"
            error = "access_denied"
            "#
        ))?;
        config.check()?;
        Ok(config)
    }

    #[test]
    fn the_loopback_only() {
        assert!(config("127.0.0.1:8444").is_ok());
        assert!(config("[::1]:8444").is_ok());
        for outside in ["0.0.0.0:8444", "192.0.2.1:8444", "[::]:8444"] {
            let error = config(outside).unwrap_err().to_string();
            assert!(error.contains("loopback"), "{outside}: {error}");
        }
    }

    #[test]
    fn refuses_unknown_errors_and_fields() {
        let mut c = config("127.0.0.1:8444").unwrap();
        c.users[1].error = Some("nope".into());
        assert!(c.check().is_err());
        c.users[1].error = None;
        c.users[1].sub = "alice".into();
        assert!(c.check().is_err());
        assert!(toml::from_str::<User>("sub = \"a\"\nname = \"A\"\nrole = \"x\"").is_err());
    }

    #[test]
    fn the_development_configuration_is_valid() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev/idp.toml");
        let config = Config::load(&file).unwrap();
        assert!(config.users.iter().any(|u| u.error.is_some()));
    }
}
