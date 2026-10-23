"""REQ-023-4: a `ca_bundle` is the only trust; the store of the system holds otherwise.

A local TLS server presents a certificate of a test authority. The « store of the
system » is that authority set through `SSL_CERT_FILE`, which the default context of
httpx2 reads: the authority is then known to the system, and must still be refused when
the profile names another bundle.
"""

import datetime
import ipaddress
import ssl
import threading
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import pytest
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID

from kartenn._core.messages import Request
from kartenn._core.problems import NetworkError
from kartenn.transport.httpx2 import Httpx2Transport, ssl_context

NOW = datetime.datetime.now(datetime.UTC)


def authority(name: str) -> tuple[ec.EllipticCurvePrivateKey, x509.Certificate]:
    key = ec.generate_private_key(ec.SECP256R1())
    subject = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, name)])
    certificate = (
        x509.CertificateBuilder()
        .subject_name(subject)
        .issuer_name(subject)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(NOW - datetime.timedelta(days=1))
        .not_valid_after(NOW + datetime.timedelta(days=1))
        .add_extension(x509.BasicConstraints(ca=True, path_length=None), critical=True)
        .add_extension(
            x509.KeyUsage(
                digital_signature=True,
                content_commitment=False,
                key_encipherment=False,
                data_encipherment=False,
                key_agreement=False,
                key_cert_sign=True,
                crl_sign=True,
                encipher_only=False,
                decipher_only=False,
            ),
            critical=True,
        )
        .add_extension(x509.SubjectKeyIdentifier.from_public_key(key.public_key()), critical=False)
        .sign(key, hashes.SHA256())
    )
    return key, certificate


def leaf(
    issuer_key: ec.EllipticCurvePrivateKey, issuer: x509.Certificate
) -> tuple[ec.EllipticCurvePrivateKey, x509.Certificate]:
    key = ec.generate_private_key(ec.SECP256R1())
    certificate = (
        x509.CertificateBuilder()
        .subject_name(x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "127.0.0.1")]))
        .issuer_name(issuer.subject)
        .public_key(key.public_key())
        .serial_number(x509.random_serial_number())
        .not_valid_before(NOW - datetime.timedelta(days=1))
        .not_valid_after(NOW + datetime.timedelta(days=1))
        .add_extension(
            x509.SubjectAlternativeName([x509.IPAddress(ipaddress.ip_address("127.0.0.1"))]),
            critical=False,
        )
        .add_extension(
            x509.ExtendedKeyUsage([x509.oid.ExtendedKeyUsageOID.SERVER_AUTH]), critical=False
        )
        .add_extension(
            x509.AuthorityKeyIdentifier.from_issuer_public_key(issuer_key.public_key()),
            critical=False,
        )
        .sign(issuer_key, hashes.SHA256())
    )
    return key, certificate


def pem(certificate: x509.Certificate) -> bytes:
    return certificate.public_bytes(serialization.Encoding.PEM)


class Answer(BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        body = b'{"ok":true}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:  # noqa: A002 - the signature of the base
        pass


@pytest.fixture(scope="module")
def tls(tmp_path_factory: pytest.TempPathFactory) -> Iterator[tuple[str, Path, Path]]:
    """The URL of a server of authority A, and the bundles of A and of another one, B."""
    folder = tmp_path_factory.mktemp("tls")
    key_a, ca_a = authority("kartenn test A")
    _, ca_b = authority("kartenn test B")
    server_key, server_certificate = leaf(key_a, ca_a)
    (folder / "a.pem").write_bytes(pem(ca_a))
    (folder / "b.pem").write_bytes(pem(ca_b))
    (folder / "server.pem").write_bytes(pem(server_certificate))
    (folder / "server.key").write_bytes(
        server_key.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.PKCS8,
            serialization.NoEncryption(),
        )
    )
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(folder / "server.pem", folder / "server.key")
    server = ThreadingHTTPServer(("127.0.0.1", 0), Answer)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"https://127.0.0.1:{server.server_address[1]}", folder / "a.pem", folder / "b.pem"
    server.shutdown()
    server.server_close()


def get(url: str, ca_bundle: Path | None = None) -> int:
    transport = Httpx2Transport(url, ca_bundle=ca_bundle)
    try:
        return transport.send(Request("GET", "/v1/capabilities")).status
    finally:
        transport.close()


def test_bundle_of_the_profile_is_accepted(tls: tuple[str, Path, Path]) -> None:
    url, ca_a, _ = tls
    assert get(url, ca_a) == 200


def test_unknown_authority_is_refused(
    tls: tuple[str, Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    url, _, ca_b = tls
    monkeypatch.setenv("SSL_CERT_FILE", str(ca_b))
    with pytest.raises(NetworkError) as caught:
        get(url)
    assert caught.value.code == "client.tls"


def test_system_store_is_used_without_bundle(
    tls: tuple[str, Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    url, ca_a, _ = tls
    monkeypatch.setenv("SSL_CERT_FILE", str(ca_a))
    assert get(url) == 200


def test_bundle_never_falls_back_to_the_system_store(
    tls: tuple[str, Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    url, ca_a, ca_b = tls
    monkeypatch.setenv("SSL_CERT_FILE", str(ca_a))  # A is known to « the system »
    with pytest.raises(NetworkError) as caught:
        get(url, ca_b)
    assert caught.value.code == "client.tls"


def test_context_of_a_bundle(tls: tuple[str, Path, Path]) -> None:
    _, ca_a, _ = tls
    context = ssl_context(ca_a)
    assert context.minimum_version == ssl.TLSVersion.TLSv1_2
    assert context.check_hostname
    assert [entry["subject"] for entry in context.get_ca_certs()] == [
        ((("commonName", "kartenn test A"),),)
    ]
