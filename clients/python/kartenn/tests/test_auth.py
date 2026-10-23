import base64
import hashlib
import logging
import threading
import urllib.request
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, parse_qsl, urlencode, urlsplit

import httpx2
import pytest

from fakes import FakeServer, problem
from kartenn import Client, ClientError, KartennError, MemoryStore
from kartenn.auth.basic import BasicAuth, Secret, is_loopback
from kartenn.auth.breakglass import Breakglass
from kartenn.auth.device import DeviceCode, device_login
from kartenn.auth.oidc_native import Issuer, OidcNative, pkce_pair
from kartenn.catalog import CatalogCache
from kartenn.client import auth_of_profile, store_key
from kartenn.profiles import Profile

IDP = "https://idp.test"
DISCOVERY = f"{IDP}/.well-known/openid-configuration"
METADATA = {
    "issuer": IDP,
    "authorization_endpoint": f"{IDP}/authorize",
    "token_endpoint": f"{IDP}/token",
    "device_authorization_endpoint": f"{IDP}/device",
    "code_challenge_methods_supported": ["S256"],
}
NATIVE = [
    {
        "id": "interne",
        "discovery_url": DISCOVERY,
        "client_id": "kartenn-cli",
        "audience": "https://kartenn.test",
        "scopes": ["openid", "offline_access"],
        "device": True,
    },
]


class Clock:
    def __init__(self) -> None:
        self.now = 1_000.0

    def __call__(self) -> float:
        return self.now


def form(request: httpx2.Request) -> dict[str, str]:
    return dict(parse_qsl(request.content.decode()))


def idp(server: FakeServer, tokens: list[httpx2.Response]) -> list[dict[str, str]]:
    """The routes of an issuer; its token endpoint answers `tokens` in order."""
    seen: list[dict[str, str]] = []
    server.route("GET", "/v1/auth/native-clients", httpx2.Response(200, json=NATIVE))
    server.route("GET", "/.well-known/openid-configuration", httpx2.Response(200, json=METADATA))

    def token(request: httpx2.Request) -> httpx2.Response:
        seen.append(form(request))
        return tokens.pop(0) if len(tokens) > 1 else tokens[0]

    server.route("POST", "/token", token)
    return seen


def granted(access: str, refresh: str | None = "r2", expires: int | None = 3600) -> httpx2.Response:
    body: dict[str, Any] = {
        "access_token": access,
        "token_type": "Bearer",
        "id_token": "never-sent",
    }
    if refresh is not None:
        body["refresh_token"] = refresh
    if expires is not None:
        body["expires_in"] = expires
    return httpx2.Response(200, json=body)


def test_basic(tmp_path: Path) -> None:
    secret = Secret("hunter2")
    assert "hunter2" not in repr(secret)
    assert "hunter2" not in str(secret)
    auth = BasicAuth("sync-dns", secret)
    assert "hunter2" not in repr(auth)
    server = FakeServer()
    server.route("GET", "/v1/me", httpx2.Response(200, json={"name": "sync-dns"}))
    with server.client(tmp_path, auth=auth) as kn:
        assert kn.call("me") == {"name": "sync-dns"}
    expected = base64.b64encode(b"sync-dns:hunter2").decode()
    assert server.seen[-1].headers["authorization"] == f"Basic {expected}"


@pytest.mark.parametrize(
    ("url", "allowed"),
    [
        ("http://kartenn.test", False),
        ("http://127.0.0.1:8080", True),
        ("http://[::1]", True),
        ("http://localhost:1", True),
        ("ftp://kartenn.test", False),
    ],
)
def test_basic_refused_outside_https(url: str, *, allowed: bool) -> None:
    with pytest.raises(ClientError) as caught:
        BasicAuth("a", Secret("s")).check_server(url)
    assert caught.value.code == "client.insecure-transport"
    loopback = BasicAuth("a", Secret("s"), allow_insecure_loopback=True)
    if allowed:
        loopback.check_server(url)
    else:
        with pytest.raises(ClientError):
            loopback.check_server(url)
    with pytest.raises(ClientError):
        Client(url, auth=BasicAuth("a", Secret("s")))


def test_secrets_of_files(tmp_path: Path) -> None:
    path = tmp_path / "secret"
    path.write_text("s3cret\nignored\n")
    assert Secret.from_file(path).reveal() == "s3cret"
    for bad in (tmp_path / "absent", tmp_path / "empty"):
        if bad.name == "empty":
            bad.write_text("")
        with pytest.raises(ClientError) as caught:
            Secret.from_file(bad)
        assert caught.value.code == "client.secret-required"
    assert not is_loopback("https://kartenn.test")


def test_profiles_of_every_mode(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    store = MemoryStore()
    basic = Profile("ci", "https://kartenn.test", auth="basic", account="sync")
    with pytest.raises(ClientError) as missing:
        auth_of_profile(basic, store)
    assert missing.value.code == "client.secret-required"
    secret = tmp_path / "secret"
    secret.write_text("from-file")
    monkeypatch.setenv("KARTENN_SECRET_FILE", str(secret))
    assert isinstance(auth_of_profile(basic, store), BasicAuth)
    store.set(f"{store_key('ci')}:secret", "from-store")
    assert isinstance(auth_of_profile(basic, store), BasicAuth)
    for profile in (
        Profile("x", "https://kartenn.test", auth="basic"),
        Profile("x", "https://kartenn.test", auth="oidc"),
        Profile("x", "https://kartenn.test", auth="breakglass"),
    ):
        with pytest.raises(ClientError):
            auth_of_profile(profile, store)
    named = Profile("prod", "https://kartenn.test", auth="oidc", issuer="interne")
    assert isinstance(auth_of_profile(named, store), OidcNative)
    full = Profile(
        "prod",
        "https://kartenn.test",
        auth="oidc",
        extra={"discovery_url": DISCOVERY, "client_id": "c"},
    )
    assert isinstance(auth_of_profile(full, store), OidcNative)


def logged_in(
    store: MemoryStore, clock: Clock, *, expires_at: float, refresh: str | None = "r1"
) -> None:
    store.set("kartenn:prod:access_token", "a1")
    if refresh is not None:
        store.set("kartenn:prod:refresh_token", refresh)
    store.set("kartenn:prod:expires_at", str(expires_at))
    clock.now = 1_000.0


def test_bearer_and_refresh_with_rotation(tmp_path: Path, caplog: pytest.LogCaptureFixture) -> None:
    server = FakeServer()
    seen = idp(server, [granted("a2", "r2")])
    server.route("GET", "/v1/me", httpx2.Response(200, json={"name": "alice"}))
    store, clock = MemoryStore(), Clock()
    logged_in(store, clock, expires_at=1_030.0)  # 30 s left: renewed first
    auth = OidcNative("interne", store, key="kartenn:prod", clock=clock)
    with caplog.at_level(logging.DEBUG, logger="kartenn"), server.client(tmp_path, auth=auth) as kn:
        assert kn.call("me") == {"name": "alice"}
        assert kn.call("me") == {"name": "alice"}
    assert seen == [
        {"grant_type": "refresh_token", "refresh_token": "r1", "client_id": "kartenn-cli"}
    ]
    assert store.get("kartenn:prod:refresh_token") == "r2"
    assert store.get("kartenn:prod:access_token") == "a2"
    me = [r for r in server.seen if r.url.path == "/v1/me"]
    assert {r.headers["authorization"] for r in me} == {"Bearer a2"}
    for secret in ("a1", "a2", "r1", "r2", "never-sent"):
        assert secret not in caplog.text
    assert "never-sent" not in str([r.headers for r in server.seen])


def test_401_renews_once(tmp_path: Path) -> None:
    server = FakeServer()
    idp(server, [granted("a2", None, None)])
    answers = [problem(401, "unauthenticated"), httpx2.Response(200, json={"ok": 1})]
    server.route("GET", "/v1/me", lambda _r: answers.pop(0))
    store, clock = MemoryStore(), Clock()
    logged_in(store, clock, expires_at=9_999.0)
    auth = OidcNative(Issuer.parse(NATIVE[0]), store, key="kartenn:prod", clock=clock)
    with server.client(tmp_path, auth=auth) as kn:
        assert kn.call("me") == {"ok": 1}
    assert store.get("kartenn:prod:expires_at") is None
    assert store.get("kartenn:prod:refresh_token") == "r1"


def test_refused_refresh_clears_the_tokens(tmp_path: Path) -> None:
    server = FakeServer()
    idp(server, [httpx2.Response(400, json={"error": "invalid_grant"})])
    store, clock = MemoryStore(), Clock()
    logged_in(store, clock, expires_at=1_010.0)
    auth = OidcNative("interne", store, key="kartenn:prod", clock=clock)
    with server.client(tmp_path, auth=auth) as kn, pytest.raises(ClientError) as caught:
        kn.call("me")
    assert caught.value.code == "client.login-required"
    assert store.get("kartenn:prod:access_token") is None
    logged_in(store, clock, expires_at=1_010.0, refresh=None)
    with server.client(tmp_path, auth=auth) as kn, pytest.raises(ClientError):
        kn.call("me")
    with server.client(tmp_path, auth=auth) as kn, pytest.raises(ClientError) as absent:
        kn.call("me")
    assert absent.value.code == "client.login-required"


def test_issuer_errors(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("GET", "/v1/auth/native-clients", httpx2.Response(200, json=NATIVE))
    kn = server.client(tmp_path)
    for metadata, code in [
        ({}, "client.oidc-error"),
        ({**METADATA, "code_challenge_methods_supported": ["plain"]}, "client.oidc-error"),
    ]:
        server.route(
            "GET", "/.well-known/openid-configuration", httpx2.Response(200, json=metadata)
        )
        with pytest.raises(ClientError) as caught:
            kn.run_unauthenticated(OidcNative("interne", MemoryStore()).metadata())
        assert caught.value.code == code
    with pytest.raises(ClientError) as unknown:
        kn.run_unauthenticated(OidcNative("ailleurs", MemoryStore()).issuer())
    assert unknown.value.code == "client.unknown-issuer"
    server.route(
        "GET",
        "/.well-known/openid-configuration",
        httpx2.Response(200, json={"token_endpoint": "t"}),
    )
    with pytest.raises(ClientError):
        kn.run_unauthenticated(OidcNative("interne", MemoryStore()).authorization_url("http://x"))
    kn.close()


def browser(answer: dict[str, str] | None = None, *, forge: bool = False) -> Any:  # noqa: ANN401
    """The system browser and the issuer: the user approves, the issuer redirects."""
    opened: list[str] = []

    def open_url(url: str) -> bool:
        opened.append(url)
        query = {k: v[0] for k, v in parse_qs(urlsplit(url).query).items()}
        back = {"state": query["state"], **(answer if answer is not None else {"code": "c0de"})}
        if forge:
            back = {**back, "state": "forged"}
        target = f"{query['redirect_uri']}?{urlencode(back)}"
        threading.Thread(
            target=lambda: urllib.request.urlopen(target, timeout=5).read(),  # noqa: S310 - http://127.0.0.1
            daemon=True,
        ).start()
        return True

    open_url.opened = opened  # type: ignore[attr-defined]
    return open_url


def test_loopback_login_with_pkce(tmp_path: Path) -> None:
    server = FakeServer()
    seen = idp(server, [granted("a1", "r1")])
    store = MemoryStore()
    auth = OidcNative("interne", store, key="kartenn:prod")
    opener = browser()
    with server.client(tmp_path, auth=auth) as kn:
        auth.login(kn.run_unauthenticated, open_browser=opener, timeout=10)
    url: str = opener.opened[0]
    query = {k: v[0] for k, v in parse_qs(urlsplit(url).query).items()}
    assert url.startswith(f"{IDP}/authorize?")
    assert query["code_challenge_method"] == "S256"
    assert query["redirect_uri"].startswith("http://127.0.0.1:")
    assert query["redirect_uri"].endswith("/callback")
    assert query["scope"] == "openid offline_access"
    assert query["resource"] == "https://kartenn.test"
    assert len(query["state"]) >= 43
    assert query["nonce"] != query["state"]
    exchange = seen[0]
    assert exchange["grant_type"] == "authorization_code"
    assert exchange["code"] == "c0de"
    assert exchange["redirect_uri"] == query["redirect_uri"]
    digest = hashlib.sha256(exchange["code_verifier"].encode()).digest()
    assert base64.urlsafe_b64encode(digest).decode().rstrip("=") == query["code_challenge"]
    assert store.get("kartenn:prod:access_token") == "a1"


@pytest.mark.parametrize(
    ("answer", "forge", "code"),
    [
        (None, True, "client.oidc-state-mismatch"),
        ({"error": "access_denied"}, False, "client.oidc-denied"),
    ],
)
def test_loopback_refusals(
    tmp_path: Path, answer: dict[str, str] | None, *, forge: bool, code: str
) -> None:
    server = FakeServer()
    idp(server, [granted("a1")])
    auth = OidcNative("interne", MemoryStore(), key="kartenn:prod")
    with server.client(tmp_path, auth=auth) as kn, pytest.raises(ClientError) as caught:
        auth.login(kn.run_unauthenticated, open_browser=browser(answer, forge=forge), timeout=10)
    assert caught.value.code == code


def test_completion_checks(tmp_path: Path) -> None:
    server = FakeServer()
    idp(server, [httpx2.Response(400, json={"error": "invalid_grant"})])
    auth = OidcNative("interne", MemoryStore(), key="k")
    login = {
        "state": "s",
        "nonce": "n",
        "verifier": "v",
        "redirect_uri": "http://127.0.0.1:1/callback",
    }
    with server.client(tmp_path) as kn:
        for callback, code in [
            ({"state": "s", "error": "access_denied"}, "client.oidc-denied"),
            ({"state": "s"}, "client.oidc-error"),
            ({"state": "s", "code": "c"}, "client.oidc-denied"),
        ]:
            with pytest.raises(ClientError) as caught:
                kn.run_unauthenticated(auth.complete(login, callback))
            assert caught.value.code == code
        with pytest.raises(ClientError) as late:
            auth.login(kn.run_unauthenticated, open_browser=lambda _url: None, timeout=0.05)
        assert late.value.code == "client.login-timeout"
    server.route("POST", "/token", httpx2.Response(500))
    with server.client(tmp_path) as kn, pytest.raises(ClientError):
        kn.run_unauthenticated(auth.complete(login, {"state": "s", "code": "c"}))
    server.route("POST", "/token", httpx2.Response(200, json={"id_token": "x"}))
    with server.client(tmp_path) as kn, pytest.raises(ClientError) as no_access:
        kn.run_unauthenticated(auth.complete(login, {"state": "s", "code": "c"}))
    assert no_access.value.code == "client.oidc-error"
    verifier, challenge = pkce_pair()
    assert len(verifier) == 43
    assert len(challenge) == 43


def device_server(tokens: list[httpx2.Response]) -> FakeServer:
    server = FakeServer()
    idp(server, tokens)
    server.route(
        "POST",
        "/device",
        httpx2.Response(
            200,
            json={
                "device_code": "dc",
                "user_code": "WDJB-MJHT",
                "verification_uri": f"{IDP}/device",
                "verification_uri_complete": f"{IDP}/device?user_code=WDJB-MJHT",
                "expires_in": 600,
                "interval": 5,
            },
        ),
    )
    return server


def test_device_flow_slows_down(tmp_path: Path) -> None:
    pending = httpx2.Response(400, json={"error": "authorization_pending"})
    slow = httpx2.Response(400, json={"error": "slow_down"})
    server = device_server([pending, slow, pending, granted("a1")])
    store, clock = MemoryStore(), Clock()
    shown: list[DeviceCode] = []
    waits: list[float] = []

    def sleep(seconds: float) -> None:
        waits.append(seconds)
        clock.now += seconds

    auth = OidcNative("interne", store, key="kartenn:prod", clock=clock)
    with server.client(tmp_path) as kn:
        device_login(auth, kn.run_unauthenticated, display=shown.append, sleep=sleep, clock=clock)
    assert waits == [5, 5, 10, 10]
    assert shown[0].user_code == "WDJB-MJHT"
    assert shown[0].verification_uri_complete is not None
    assert "authorising" in shown[0].warning
    assert store.get("kartenn:prod:access_token") == "a1"
    device = next(r for r in server.seen if r.url.path == "/device")
    assert form(device)["client_id"] == "kartenn-cli"
    polls = [form(r) for r in server.seen if r.url.path == "/token"]
    assert {p["grant_type"] for p in polls} == {"urn:ietf:params:oauth:grant-type:device_code"}


@pytest.mark.parametrize(
    ("error", "code"),
    [
        ("access_denied", "client.oidc-denied"),
        ("expired_token", "client.oidc-expired"),
        ("invalid_client", "client.oidc-error"),
    ],
)
def test_device_flow_stops(tmp_path: Path, error: str, code: str) -> None:
    server = device_server([httpx2.Response(400, json={"error": error})])
    clock = Clock()
    auth = OidcNative("interne", MemoryStore(), key="k", clock=clock)
    with server.client(tmp_path) as kn, pytest.raises(ClientError) as caught:
        device_login(
            auth, kn.run_unauthenticated, display=print, sleep=lambda _s: None, clock=clock
        )
    assert caught.value.code == code


def test_device_flow_deadline_and_absence(tmp_path: Path) -> None:
    server = device_server([httpx2.Response(400, json={"error": "authorization_pending"})])
    clock = Clock()

    def sleep(seconds: float) -> None:
        clock.now += seconds * 100

    auth = OidcNative("interne", MemoryStore(), key="k", clock=clock)
    with server.client(tmp_path) as kn, pytest.raises(ClientError) as caught:
        device_login(auth, kn.run_unauthenticated, display=print, sleep=sleep, clock=clock)
    assert caught.value.code == "client.oidc-expired"
    plain = FakeServer()
    plain.route("GET", "/v1/auth/native-clients", httpx2.Response(200, json=NATIVE))
    plain.route(
        "GET",
        "/.well-known/openid-configuration",
        httpx2.Response(200, json={**METADATA, "device_authorization_endpoint": None}),
    )
    with plain.client(tmp_path / "p") as kn, pytest.raises(ClientError) as absent:
        device_login(
            OidcNative("interne", MemoryStore()),
            kn.run_unauthenticated,
            display=print,
            sleep=print,
            clock=clock,
        )
    assert absent.value.code == "client.device-unavailable"
    refused = FakeServer()
    refused.route("GET", "/v1/auth/native-clients", httpx2.Response(200, json=NATIVE))
    refused.route("GET", "/.well-known/openid-configuration", httpx2.Response(200, json=METADATA))
    refused.route("POST", "/device", httpx2.Response(400, json={"error": "invalid_scope"}))
    with refused.client(tmp_path / "r") as kn, pytest.raises(ClientError) as denied:
        device_login(
            OidcNative("interne", MemoryStore()),
            kn.run_unauthenticated,
            display=print,
            sleep=print,
            clock=clock,
        )
    assert denied.value.code == "client.oidc-denied"


def test_breakglass_session(tmp_path: Path) -> None:
    server = FakeServer()
    server.route(
        "POST",
        "/v1/auth/breakglass",
        httpx2.Response(204, headers={"Set-Cookie": "__Host-kartenn=s1; Secure; HttpOnly"}),
    )
    server.route("GET", "/v1/me", httpx2.Response(200, json={"name": "root"}))
    server.route("POST", "/v1/anomalies/{id}/acknowledgement", httpx2.Response(204))
    auth = Breakglass("root", Secret("correct horse"), "123456")
    assert "correct horse" not in repr(auth)
    with server.client(tmp_path, auth=auth) as kn:
        assert kn.call("me") == {"name": "root"}
        kn.call("acknowledge_anomaly", id="a1", body={"motive": "m"})
        login, me, write = server.seen
        assert login.headers["origin"] == "https://kartenn.test"
        assert login.content == b'{"name": "root", "passphrase": "correct horse", "code": "123456"}'
        assert me.headers["cookie"] == "__Host-kartenn=s1"
        assert "origin" not in me.headers
        assert write.headers["origin"] == "https://kartenn.test"
        server.route("GET", "/v1/me", problem(401, "unauthenticated"))
        with pytest.raises(KartennError):
            kn.call("me")
        assert auth.cookie is None
        with pytest.raises(ClientError) as again:
            kn.call("me")
        assert again.value.code == "client.login-required"
    silent = FakeServer()
    silent.route("POST", "/v1/auth/breakglass", httpx2.Response(204))
    with (
        silent.client(tmp_path, auth=Breakglass("root", Secret("p"), "1")) as kn,
        pytest.raises(ClientError),
    ):
        kn.call("me")


def test_client_helpers(tmp_path: Path) -> None:
    server = FakeServer()
    kn = Client("https://kartenn.test", transport=server.transport(), cache=CatalogCache(tmp_path))
    kn.sleep(0)
    kn.close()
