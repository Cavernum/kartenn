//! The configuration of the connector (REQ-019-29, Q-019-12).

use kartenn_kube_sync::config::{self, Credentials};
use kartenn_kube_sync::{ConfigError, ConnectError};

fn text(cluster: &str) -> String {
    format!(
        r#"
[kartenn]
url = "https://kartenn.example.org"
secret_file = "/run/secrets/kartenn"

[[clusters]]
slug = "lab"
api_server = "https://192.0.2.10:6443"
{cluster}
"#
    )
}

fn refusal(cluster: &str) -> String {
    match config::from_toml(&text(cluster)) {
        Err(ConfigError::Invalid(message)) => message,
        Err(ConfigError::Load(error)) => error.to_string(),
        Ok(config) => panic!("accepted: {config:?}"),
    }
}

#[test]
fn config_accepts_token_or_certificate() {
    let config = config::from_toml(&text(
        r#"ca_file = "/etc/kartenn/lab-ca.crt"
token_file = "/run/secrets/lab-token""#,
    ))
    .unwrap();
    let cluster = &config.clusters[0];
    assert!(matches!(cluster.credentials(), Some(Credentials::Token(_))));
    assert_eq!(cluster.fallback_period_s, 300);
    assert!(!cluster.allow_excess_rights);

    let config = config::from_toml(&text(
        r#"ca_file = "/etc/kartenn/lab-ca.crt"
cert_file = "/run/secrets/lab.crt"
key_file = "/run/secrets/lab.key"
fallback_period_s = 60
allow_excess_rights = true"#,
    ))
    .unwrap();
    let cluster = &config.clusters[0];
    assert!(matches!(
        cluster.credentials(),
        Some(Credentials::Certificate { .. })
    ));
    assert!(cluster.allow_excess_rights);
}

#[test]
fn config_refuses_missing_ca() {
    let message = refusal(r#"token_file = "/run/secrets/lab-token""#);
    assert!(message.contains("clusters[0] (lab).ca_file"), "{message}");
}

#[test]
fn config_refuses_accept_invalid_certs() {
    for switch in ["accept_invalid_certs", "insecure_skip_tls_verify"] {
        let message = refusal(&format!(
            "ca_file = \"/etc/kartenn/lab-ca.crt\"\ntoken_file = \"/run/secrets/t\"\n{switch} = true"
        ));
        assert!(message.contains(switch), "{message}");
    }
}

#[test]
fn config_refuses_bad_values() {
    let ca = "ca_file = \"/etc/kartenn/lab-ca.crt\"\n";
    let cases = [
        (ca.to_owned(), "exactly one of token_file"),
        (
            format!("{ca}token_file = \"/t\"\ncert_file = \"/c\"\nkey_file = \"/k\""),
            "exactly one of token_file",
        ),
        (
            format!("{ca}cert_file = \"/c\""),
            "exactly one of token_file",
        ),
        (
            format!("{ca}token_file = \"/t\"\nfallback_period_s = 1"),
            "fallback_period_s",
        ),
    ];
    for (cluster, expected) in cases {
        let message = refusal(&cluster);
        assert!(message.contains(expected), "{cluster}: {message}");
    }
    let plain = text(&format!("{ca}token_file = \"/t\"")).replace("https://192", "http://192");
    let Err(ConfigError::Invalid(message)) = config::from_toml(&plain) else {
        panic!("http API server accepted");
    };
    assert!(message.contains("api_server"), "{message}");
    let twice = format!(
        "{}\n[[clusters]]\nslug = \"lab\"\napi_server = \"https://192.0.2.11:6443\"\n{ca}token_file = \"/t\"",
        text(&format!("{ca}token_file = \"/t\""))
    );
    let Err(ConfigError::Invalid(message)) = config::from_toml(&twice) else {
        panic!("slug given twice accepted");
    };
    assert!(message.contains("twice"), "{message}");
}

fn with_urls(kartenn: &str, api_server: &str) -> Result<config::Config, ConfigError> {
    let text = text("ca_file = \"/etc/kartenn/lab-ca.crt\"\ntoken_file = \"/t\"")
        .replace("https://kartenn.example.org", kartenn)
        .replace("https://192.0.2.10:6443", api_server);
    config::from_toml(&text)
}

#[test]
fn config_checks_the_kartenn_url() {
    let api = "https://192.0.2.10:6443";
    for url in [
        "https://kartenn.example.org",
        "https://kartenn.example.org:8443/base",
        "http://127.0.0.1:8080",
        "http://127.0.0.2:8080/",
        "http://[::1]:8080",
        "http://localhost",
        "http://LOCALHOST:8080",
    ] {
        assert!(with_urls(url, api).is_ok(), "{url} refused");
    }
    for url in [
        "http://kartenn.example.org",
        "http://192.0.2.20:8080",
        "http://localhost.example.org",
        "http://127.0.0.1.example.org",
        // User information that hides a remote host behind a loopback prefix.
        "http://127.0.0.1:80@kartenn.example.org/",
        "http://localhost:x@192.0.2.20",
        "https://user:password@kartenn.example.org",
        "https://user@kartenn.example.org",
        "https://kartenn.example.org/#fragment",
        "ftp://127.0.0.1",
        "kartenn.example.org",
        "https://",
    ] {
        let Err(ConfigError::Invalid(message)) = with_urls(url, api) else {
            panic!("{url} accepted");
        };
        assert!(message.contains("kartenn.url"), "{url}: {message}");
    }
}

#[test]
fn config_checks_the_api_server_url() {
    let kartenn = "https://kartenn.example.org";
    for url in [
        "https://192.0.2.10:6443",
        "https://[2001:db8::10]:6443",
        "https://api.lab.example.org",
    ] {
        assert!(with_urls(kartenn, url).is_ok(), "{url} refused");
    }
    for url in [
        "http://192.0.2.10:6443",
        "http://127.0.0.1:6443",
        "https://user:password@192.0.2.10:6443",
        "https://token@192.0.2.10:6443",
        "https://127.0.0.1:80@192.0.2.10",
        "https://192.0.2.10:6443/#fragment",
        "192.0.2.10:6443",
    ] {
        let Err(ConfigError::Invalid(message)) = with_urls(kartenn, url) else {
            panic!("{url} accepted");
        };
        assert!(message.contains("api_server"), "{url}: {message}");
    }
}

#[tokio::test]
async fn connect_refuses_unusable_ca() {
    let dir = tempfile::tempdir().unwrap();
    let mut cluster = crate::support::cluster(dir.path(), "https://192.0.2.10:6443", "t0ken");
    for (content, reason) in [
        (None, "unreadable"),
        (Some(""), "no certificate"),
        (Some("not a certificate\n"), "no certificate"),
    ] {
        let ca = dir.path().join("ca.crt");
        let _ = std::fs::remove_file(&ca);
        if let Some(content) = content {
            std::fs::write(&ca, content).unwrap();
        }
        cluster.ca_file = Some(ca);
        let Err(error) = kartenn_kube_sync::connect(&cluster) else {
            panic!("CA {content:?} accepted");
        };
        assert!(matches!(error, ConnectError::Ca { .. }), "{error}");
        assert!(error.to_string().contains(reason), "{error}");
    }
    cluster.ca_file = None;
    assert!(matches!(
        kartenn_kube_sync::connect(&cluster),
        Err(ConnectError::Unchecked(_))
    ));
}
