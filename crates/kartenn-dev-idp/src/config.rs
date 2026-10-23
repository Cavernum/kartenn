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
    /// The public client of the native applications (the CLI, spec 023, T10), if any.
    #[serde(default)]
    pub native: Option<Native>,
}

/// A public client of the native applications (RFC 8252): no secret, PKCE `S256`, a
/// loopback redirection on any port (§7.3), the device flow if enabled (RFC 8628); its
/// access tokens are JWT (`at+jwt`, RFC 9068) for the audience of kartenn.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Native {
    /// Its identifier, other than that of the confidential client.
    pub client_id: String,
    /// The audience of its access tokens: that kartenn expects of the issuer.
    pub audience: String,
    /// Its redirections, `http` on the loopback, without port: any port is accepted
    /// (RFC 8252 §7.3).
    #[serde(default = "loopback")]
    pub redirect_uris: Vec<String>,
    /// Whether it may use the device flow.
    #[serde(default)]
    pub device: bool,
    /// The interval of the device flow, in seconds (RFC 8628 §3.2; 5 by default).
    #[serde(default = "interval")]
    pub device_interval: u64,
    /// The lifetime of a device code, in seconds.
    #[serde(default = "device_lifetime")]
    pub device_expires_in: u64,
    /// The polls of each device code answered `slow_down` first, whatever their pace, to
    /// exercise the client (RFC 8628 §3.5); a poll before its interval gets it anyway.
    #[serde(default)]
    pub slow_down: u32,
}

fn loopback() -> Vec<String> {
    vec![
        "http://127.0.0.1/callback".into(),
        "http://[::1]/callback".into(),
    ]
}

const fn interval() -> u64 {
    5
}

const fn device_lifetime() -> u64 {
    600
}

impl Native {
    /// Whether `uri` is one of its redirections, on any port (RFC 8252 §7.3).
    #[must_use]
    pub fn redirects_to(&self, uri: &str) -> bool {
        let Some(rest) = uri.strip_prefix("http://") else {
            return false;
        };
        let host_end = if rest.starts_with('[') {
            rest.find(']').map(|i| i + 1)
        } else {
            rest.find([':', '/'])
        };
        let Some(host_end) = host_end else {
            return false;
        };
        let (host, after) = rest.split_at(host_end);
        let path = match after.strip_prefix(':') {
            Some(port) => {
                let digits = port.find('/').unwrap_or(port.len());
                if digits == 0 || !port[..digits].bytes().all(|b| b.is_ascii_digit()) {
                    return false;
                }
                &port[digits..]
            }
            None => after,
        };
        let bare = format!("http://{host}{path}");
        self.redirect_uris.contains(&bare)
    }
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
        if let Some(native) = &self.native {
            anyhow::ensure!(
                !native.client_id.is_empty() && native.client_id != self.client_id,
                "native.client_id: required, other than client_id"
            );
            anyhow::ensure!(!native.audience.is_empty(), "native.audience: required");
            for uri in &native.redirect_uris {
                let host = ["http://127.0.0.1/", "http://[::1]/"]
                    .iter()
                    .any(|base| uri.starts_with(base));
                anyhow::ensure!(
                    host && !uri.contains(['?', '#']),
                    "native.redirect_uris: {uri} is not http on the loopback IP, without port"
                );
            }
            anyhow::ensure!(
                native.device_interval >= 1 && native.device_expires_in >= 1,
                "native.device_interval and device_expires_in: at least 1 s"
            );
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
    fn a_native_client_redirects_to_the_loopback_on_any_port() {
        let native: Native = toml::from_str("client_id = \"cli\"\naudience = \"kartenn\"").unwrap();
        for uri in [
            "http://127.0.0.1:49152/callback",
            "http://127.0.0.1/callback",
            "http://[::1]:8080/callback",
        ] {
            assert!(native.redirects_to(uri), "{uri}");
        }
        for uri in [
            "http://127.0.0.1:49152/other",
            "http://localhost:49152/callback",
            "https://127.0.0.1:49152/callback",
            "http://127.0.0.1:/callback",
            "http://127.0.0.1:12ab/callback",
            "http://127.0.0.1.example:80/callback",
        ] {
            assert!(!native.redirects_to(uri), "{uri}");
        }
        let mut c = config("127.0.0.1:8444").unwrap();
        c.native = Some(native.clone());
        assert!(c.check().is_ok());
        c.native = Some(Native {
            client_id: "kartenn".into(),
            ..native.clone()
        });
        assert!(c.check().is_err(), "the confidential client is not public");
        c.native = Some(Native {
            redirect_uris: vec!["http://localhost/callback".into()],
            ..native
        });
        assert!(
            c.check().is_err(),
            "localhost is not recommended (RFC 8252 §8.3)"
        );
    }

    #[test]
    fn the_development_configuration_is_valid() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev/idp.toml");
        let config = Config::load(&file).unwrap();
        assert!(config.users.iter().any(|u| u.error.is_some()));
    }
}
