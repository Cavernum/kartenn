//! The client of an API server: token read again, CA alone trusted, no credential in a
//! log or a trace (REQ-019-29, Q-019-12).

use std::fmt::Write as _;

use k8s_openapi::api::core::v1::Namespace;
use kube::Api;
use kube::api::ListParams;
use tracing_subscriber::layer::SubscriberExt;

use crate::fake_api::FakeApi;
use crate::support::{Capture, certificate, cluster, tls_server};

const NAMESPACES: &str = "/api/v1/namespaces";

fn fake() -> FakeApi {
    let fake = FakeApi::new();
    fake.collection(NAMESPACES, "v1", "Namespace");
    fake
}

fn last_authorization(fake: &FakeApi) -> Option<String> {
    fake.requests().last().and_then(|r| r.authorization.clone())
}

#[tokio::test]
async fn token_file_reloaded() {
    let dir = tempfile::tempdir().unwrap();
    let cluster = cluster(dir.path(), "https://192.0.2.10:6443", "alpha-0001");
    let fake = fake();
    let client = kartenn_kube_sync::connect_with(&cluster, fake.clone()).unwrap();
    let namespaces: Api<Namespace> = Api::all(client);
    let token = cluster.token_file.clone().unwrap();

    namespaces.list(&ListParams::default()).await.unwrap();
    assert_eq!(
        last_authorization(&fake).as_deref(),
        Some("Bearer alpha-0001")
    );

    // Rewritten in place, as an operator would.
    std::fs::write(&token, "bravo-000002\n").unwrap();
    namespaces.list(&ListParams::default()).await.unwrap();
    assert_eq!(
        last_authorization(&fake).as_deref(),
        Some("Bearer bravo-000002")
    );

    // Replaced by a rename of the same length, as the kubelet rotates a projected token.
    let next = dir.path().join("token.next");
    std::fs::write(&next, "charl-000002\n").unwrap();
    std::fs::rename(&next, &token).unwrap();
    namespaces.list(&ListParams::default()).await.unwrap();
    assert_eq!(
        last_authorization(&fake).as_deref(),
        Some("Bearer charl-000002")
    );

    // Unchanged: the token read before is sent again.
    namespaces.list(&ListParams::default()).await.unwrap();
    assert_eq!(
        last_authorization(&fake).as_deref(),
        Some("Bearer charl-000002")
    );

    // Gone: the request fails, and the message names the file.
    std::fs::remove_file(&token).unwrap();
    let error = namespaces.list(&ListParams::default()).await.unwrap_err();
    let message = format!("{error} {error:?}");
    assert!(message.contains("token"), "{message}");
    assert_eq!(fake.requests().len(), 4);
}

#[tokio::test]
async fn tls_trusts_configured_ca_only() {
    let dir = tempfile::tempdir().unwrap();
    let (server_certificate, server_ca) = certificate(dir.path(), "server");
    let (_, other_ca) = certificate(dir.path(), "other");
    let server = tls_server(&server_certificate).await;
    let url = format!("https://127.0.0.1:{}", server.address.port());
    let mut cluster = cluster(dir.path(), &url, "delta-0004");

    cluster.ca_file = Some(server_ca);
    let client = kartenn_kube_sync::connect(&cluster).unwrap();
    let list = Api::<Namespace>::all(client)
        .list(&ListParams::default())
        .await
        .unwrap();
    assert_eq!(list.metadata.resource_version.as_deref(), Some("7"));
    assert_eq!(
        server.authorizations.lock().unwrap().as_slice(),
        ["Bearer delta-0004"]
    );

    cluster.ca_file = Some(other_ca);
    let client = kartenn_kube_sync::connect(&cluster).unwrap();
    let error = Api::<Namespace>::all(client)
        .list(&ListParams::default())
        .await
        .unwrap_err();
    // Same subject as the server certificate, another key: refused by its signature.
    assert!(
        format!("{error:?}").contains("InvalidCertificate"),
        "{error:?}"
    );
    assert_eq!(server.authorizations.lock().unwrap().len(), 1);
}

#[test]
fn no_credential_in_logs() {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone());
    let token = "echo-secret-0005";
    let rotated = "foxtrot-secret-0006";
    let dir = tempfile::tempdir().unwrap();
    let mut shown = String::new();
    tracing::subscriber::with_default(subscriber, || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // Over TLS, through every layer of the real client.
            let (server_certificate, server_ca) = certificate(dir.path(), "server");
            let server = tls_server(&server_certificate).await;
            let url = format!("https://127.0.0.1:{}", server.address.port());
            let mut cluster = cluster(dir.path(), &url, token);
            cluster.ca_file = Some(server_ca);
            write!(shown, "{cluster:?}").unwrap();
            let client = kartenn_kube_sync::connect(&cluster).unwrap();
            let namespaces: Api<Namespace> = Api::all(client);
            namespaces.list(&ListParams::default()).await.unwrap();
            let token_file = cluster.token_file.clone().unwrap();
            std::fs::write(&token_file, rotated).unwrap();
            namespaces.list(&ListParams::default()).await.unwrap();
            assert_eq!(server.authorizations.lock().unwrap().len(), 2);

            // The errors of the token file and of the secret of the kartenn account.
            std::fs::write(&token_file, "a\nb\u{7f}").unwrap();
            let error = namespaces.list(&ListParams::default()).await.unwrap_err();
            write!(shown, "{error} {error:?}").unwrap();
            let secret = kartenn_kube_sync::CredentialFile::new(&token_file);
            std::fs::write(&token_file, rotated).unwrap();
            let value = secret.read().await.unwrap();
            assert_eq!(value.expose(), rotated);
            write!(shown, "{value:?} {secret:?}").unwrap();
        });
    });
    let logs = capture.text();
    assert!(
        logs.contains("credential file read again"),
        "the capture sees the logs of the connector: {logs}"
    );
    for credential in [token, rotated] {
        assert!(
            !logs.contains(credential),
            "{credential} in the logs: {logs}"
        );
        assert!(!shown.contains(credential), "{credential} shown: {shown}");
    }
}
