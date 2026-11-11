//! Configuration of the connector (design 019 « Connecteur », REQ-019-29): a TOML file,
//! like `kartenn.toml`. It names files, never secrets: the secret of the kartenn account,
//! the token or the client key of each cluster live in files read again when they are
//! renewed. Without the CA of an API server the connector does not start (Q-019-12): it
//! never falls back on the roots of the system and has no switch to skip verification.

use std::path::{Path, PathBuf};

use figment::Figment;
use figment::providers::{Format, Toml};
use serde::Deserialize;

/// Why the configuration is refused.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Unreadable, missing or unknown field.
    #[error(transparent)]
    Load(#[from] Box<figment::Error>),
    /// A value that parses but cannot be used; the message names the setting.
    #[error("{0}")]
    Invalid(String),
}

/// The whole configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The kartenn server the connector writes to.
    pub kartenn: Kartenn,
    /// The clusters it synchronises, at least one.
    pub clusters: Vec<Cluster>,
}

/// The kartenn server and the account of the connector.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kartenn {
    /// Base URL, `https` (or `http` on the loopback, for development).
    pub url: String,
    /// The file holding the secret of the service account of the connector.
    pub secret_file: PathBuf,
}

/// One cluster to synchronise.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cluster {
    /// The slug of the `kube.cluster` declared in kartenn.
    pub slug: String,
    /// The URL of the API server, `https` only.
    pub api_server: String,
    /// The CA that signs the certificate of the API server (PEM), mandatory; read at
    /// start.
    pub ca_file: Option<PathBuf>,
    /// A bearer token (a bound service account token), read again when it changes.
    pub token_file: Option<PathBuf>,
    /// A client certificate chain (PEM), with `key_file`, read again when it changes.
    pub cert_file: Option<PathBuf>,
    /// The private key of `cert_file` (PEM).
    pub key_file: Option<PathBuf>,
    /// The period of the full lists where the watch is unavailable (REQ-019-25).
    #[serde(default = "fallback_period_s")]
    pub fallback_period_s: u64,
    /// Start even with rights beyond reading (REQ-019-28), logged and audited.
    #[serde(default)]
    pub allow_excess_rights: bool,
}

const fn fallback_period_s() -> u64 {
    300
}

/// The bounds of [`Cluster::fallback_period_s`], in seconds.
const FALLBACK_PERIOD_S: std::ops::RangeInclusive<u64> = 10..=86_400;

/// How the connector proves its identity to an API server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credentials<'a> {
    /// A bearer token in a file.
    Token(&'a Path),
    /// A client certificate and its key, in two files.
    Certificate {
        /// The chain, leaf first.
        cert_file: &'a Path,
        /// The private key.
        key_file: &'a Path,
    },
}

impl Cluster {
    /// The CA file; [`check`] has made it present.
    #[must_use]
    pub fn ca(&self) -> Option<&Path> {
        self.ca_file.as_deref()
    }

    /// The credentials; [`check`] has made exactly one kind present.
    #[must_use]
    pub fn credentials(&self) -> Option<Credentials<'_>> {
        match (&self.token_file, &self.cert_file, &self.key_file) {
            (Some(token), None, None) => Some(Credentials::Token(token)),
            (None, Some(cert_file), Some(key_file)) => Some(Credentials::Certificate {
                cert_file,
                key_file,
            }),
            _ => None,
        }
    }

    /// The fallback period as a duration.
    #[must_use]
    pub const fn fallback_period(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.fallback_period_s)
    }
}

/// Loads and checks `file`.
///
/// # Errors
///
/// [`ConfigError`], naming the faulty setting, never the content of a file it names.
pub fn load(file: &Path) -> Result<Config, ConfigError> {
    let config: Config = Figment::new()
        .merge(Toml::file_exact(file))
        .extract()
        .map_err(Box::new)?;
    check(&config)?;
    Ok(config)
}

/// Loads a configuration from TOML text (tests, examples).
///
/// # Errors
///
/// As [`load`].
pub fn from_toml(text: &str) -> Result<Config, ConfigError> {
    let config: Config = Figment::from(Toml::string(text))
        .extract()
        .map_err(Box::new)?;
    check(&config)?;
    Ok(config)
}

/// Whether `url` is `https`, or `http` on the loopback.
fn secure_url(url: &str) -> bool {
    let loopback = ["http://127.0.0.1", "http://[::1]", "http://localhost"]
        .iter()
        .any(|base| {
            url.strip_prefix(base)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([':', '/']))
        });
    (url.len() > "https://".len() && url.starts_with("https://")) || loopback
}

fn check(config: &Config) -> Result<(), ConfigError> {
    let invalid = |m: String| Err(ConfigError::Invalid(m));
    if !secure_url(&config.kartenn.url) {
        return invalid("kartenn.url: https, or http on the loopback".into());
    }
    if config.clusters.is_empty() {
        return invalid("clusters: at least one cluster".into());
    }
    for (index, cluster) in config.clusters.iter().enumerate() {
        let name = format!("clusters[{index}] ({})", cluster.slug);
        if config.clusters[..index]
            .iter()
            .any(|other| other.slug == cluster.slug)
        {
            return invalid(format!("{name}: the slug is given twice"));
        }
        match cluster.api_server.parse::<http::Uri>() {
            Ok(uri) if uri.scheme() == Some(&http::uri::Scheme::HTTPS) && uri.host().is_some() => {}
            _ => return invalid(format!("{name}.api_server: an https URL")),
        }
        if cluster.ca_file.is_none() {
            return invalid(format!(
                "{name}.ca_file: required, the connector trusts no other CA than the one of the \
                 API server"
            ));
        }
        if cluster.credentials().is_none() {
            return invalid(format!(
                "{name}: exactly one of token_file, or cert_file with key_file"
            ));
        }
        if !FALLBACK_PERIOD_S.contains(&cluster.fallback_period_s) {
            return invalid(format!(
                "{name}.fallback_period_s: from {} to {}",
                FALLBACK_PERIOD_S.start(),
                FALLBACK_PERIOD_S.end()
            ));
        }
    }
    Ok(())
}
