// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Framing and transports against local test servers.

use std::sync::Arc;
use std::time::Duration;

use syslog_sender::rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use syslog_sender::rustls::{ClientConfig, RootCertStore, ServerConfig, crypto};
use syslog_sender::{Backoff, Framing, Sender, Target};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::{TcpListener, UdpSocket};

/// Everything a stream server receives until the client closes.
async fn read_all(mut stream: impl AsyncRead + Unpin) -> Vec<u8> {
    let mut out = Vec::new();
    stream.read_to_end(&mut out).await.unwrap();
    out
}

/// Splits octet-counted frames (RFC 6587 §3.4.1).
fn frames(mut bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        let space = bytes.iter().position(|&b| b == b' ').unwrap();
        let len: usize = std::str::from_utf8(&bytes[..space])
            .unwrap()
            .parse()
            .unwrap();
        out.push(bytes[space + 1..space + 1 + len].to_vec());
        bytes = &bytes[space + 1 + len..];
    }
    out
}

const MESSAGES: [&[u8]; 2] = [b"<14>1 - - - - - - first", b"<14>1 - - - - - - two\nlines"];

#[tokio::test]
async fn tcp_frames_by_octet_counting() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move { read_all(listener.accept().await.unwrap().0).await });
    let mut sender = Sender::new(Target::Tcp(address));
    sender.send(&MESSAGES).await.unwrap();
    sender.close().await.unwrap();
    let received = server.await.unwrap();
    assert_eq!(
        frames(&received),
        MESSAGES.iter().map(|m| m.to_vec()).collect::<Vec<_>>(),
        "a line feed inside a message does not break the framing"
    );
}

#[tokio::test]
async fn line_feed_framing_refuses_a_line_feed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move { read_all(listener.accept().await.unwrap().0).await });
    let mut sender = Sender::new(Target::Tcp(address)).framing(Framing::LineFeed);
    let refused = sender.send(&MESSAGES).await.unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::InvalidInput);
    sender.send(&[b"one", b"two"]).await.unwrap();
    sender.close().await.unwrap();
    assert_eq!(server.await.unwrap(), b"one\ntwo\n");
}

#[tokio::test]
async fn udp_sends_one_datagram_per_message() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap().to_string();
    let mut sender = Sender::new(Target::Udp(address));
    sender.send(&MESSAGES).await.unwrap();
    let mut buf = [0; 512];
    for expected in MESSAGES {
        let n = socket.recv(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], expected);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn local_datagram_socket() {
    let path = std::env::temp_dir().join(format!("syslog-sender-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let socket = tokio::net::UnixDatagram::bind(&path).unwrap();
    let mut sender = Sender::new(Target::UnixDatagram(path.clone()));
    sender
        .send(&[b"<14>Oct 11 22:14:15 app: hello"])
        .await
        .unwrap();
    let mut buf = [0; 512];
    let n = socket.recv(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"<14>Oct 11 22:14:15 app: hello");
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn tls_with_octet_counting() {
    let provider = Arc::new(crypto::ring::default_provider());
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert = certified.cert.der().clone();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        certified.signing_key.serialize_der(),
    ));
    let server_config = ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], key)
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        read_all(acceptor.accept(tcp).await.unwrap()).await
    });

    let mut roots = RootCertStore::empty();
    roots.add(cert).unwrap();
    let client_config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let mut sender = Sender::new(Target::Tls {
        address,
        server_name: ServerName::try_from("localhost").unwrap(),
        config: Arc::new(client_config),
    });
    sender.send(&MESSAGES).await.unwrap();
    sender.close().await.unwrap();
    assert_eq!(
        frames(&server.await.unwrap()),
        MESSAGES.iter().map(|m| m.to_vec()).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn reconnects_after_a_back_off() {
    // A port with nobody listening.
    let probe = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let backoff = Backoff {
        min: Duration::from_millis(50),
        max: Duration::from_millis(200),
    };
    let mut sender = Sender::new(Target::Tcp(address.to_string())).backoff(backoff);
    assert!(sender.send(&[b"lost"]).await.is_err());
    assert_eq!(
        sender.send(&[b"too soon"]).await.unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let listener = TcpListener::bind(address).await.unwrap();
    let server = tokio::spawn(async move { read_all(listener.accept().await.unwrap().0).await });
    tokio::time::sleep(Duration::from_millis(60)).await;
    sender.send(&[b"back"]).await.unwrap();
    assert!(sender.is_connected());
    sender.close().await.unwrap();
    assert_eq!(frames(&server.await.unwrap()), vec![b"back".to_vec()]);
}
