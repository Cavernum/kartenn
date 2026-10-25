"""login, logout, me (REQ-023-6 to 8; maquette § 1)."""

from __future__ import annotations

import stat
import threading
import urllib.request
from typing import TYPE_CHECKING, Any
from urllib.parse import parse_qs, parse_qsl, urlencode, urlsplit

import httpx2
import pytest

from fakes import FakeServer, kartenn, profile
from kartenn_cli import session
from kartenn_cli.config import Settings
from kartenn_cli.runtime import Runtime

if TYPE_CHECKING:
    from conftest import MemoryKeyring

IDP = "https://idp.example.org"
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
        "discovery_url": f"{IDP}/.well-known/openid-configuration",
        "client_id": "kartenn-cli",
        "scopes": ["openid", "offline_access"],
        "device": True,
    }
]
ME = {
    "identity": "i1",
    "name": "Alice Exemple",
    "issuer": "interne",
    "kind": "person",
    "method": "bearer",
    "idp_groups": ["reseau-admins"],
    "groups": [{"key": "reseau", "via": "reseau-admins"}],
    "roles": ["network-admin"],
    "bindings": [],
}


@pytest.fixture
def oidc(server: FakeServer, monkeypatch: pytest.MonkeyPatch) -> list[dict[str, str]]:
    profile(auth="oidc", issuer="interne")
    monkeypatch.setattr(Runtime, "interactive", property(lambda _self: True))
    server.route("GET", "/v1/auth/native-clients", NATIVE)
    server.route("GET", "/.well-known/openid-configuration", METADATA)
    server.route("GET", "/v1/me", ME)
    seen: list[dict[str, str]] = []
    answers: list[dict[str, Any]] = [
        {"access_token": "a1", "refresh_token": "r1", "expires_in": 3600}
    ]

    def token(request: httpx2.Request) -> httpx2.Response:
        seen.append(dict(parse_qsl(request.content.decode())))
        answer = answers.pop(0) if len(answers) > 1 else answers[0]
        return httpx2.Response(400 if "error" in answer else 200, json=answer)

    server.route("POST", "/token", token)
    server.answers = answers  # type: ignore[attr-defined]
    return seen


def browser(url: str) -> bool:
    query = {k: v[0] for k, v in parse_qs(urlsplit(url).query).items()}
    target = f"{query['redirect_uri']}?{urlencode({'state': query['state'], 'code': 'c0de'})}"
    threading.Thread(
        target=lambda: urllib.request.urlopen(target, timeout=5).read(),  # noqa: S310 - loopback
        daemon=True,
    ).start()
    return True


def test_login_in_the_browser(
    oidc: list[dict[str, str]], isolated: MemoryKeyring, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setitem(session.OPEN_BROWSER, "open", browser)
    result = kartenn("login")
    assert result.exit_code == 0, result.output
    assert result.stdout == "Connecté : Alice Exemple (interne).\n"
    assert "Ouverture du navigateur pour vous connecter chez « interne »…" in result.stderr
    assert f"Si rien ne s'ouvre : {IDP}/authorize?" in result.stderr
    assert oidc[0]["grant_type"] == "authorization_code"
    assert isolated.values["kartenn", "kartenn:prod:access_token"] == "a1"
    me = kartenn("me")
    assert me.stdout.splitlines()[0].split(maxsplit=1) == [
        "Identité",
        "Alice Exemple (interne)  (person, bearer)",
    ]
    out = kartenn("logout")
    assert out.stdout == "Déconnecté du profil prod.\n"
    assert ("kartenn", "kartenn:prod:access_token") not in isolated.values


def test_login_with_a_device(
    server: FakeServer, oidc: list[dict[str, str]], monkeypatch: pytest.MonkeyPatch
) -> None:
    server.route(
        "POST",
        "/device",
        {
            "device_code": "d1",
            "user_code": "WDJB-MJHT",
            "verification_uri": f"{IDP}/device",
            "expires_in": 600,
            "interval": 1,
        },
    )
    server.answers[:] = [{"error": "authorization_pending"}, {"access_token": "a2"}]  # type: ignore[attr-defined]
    monkeypatch.setitem(session.SLEEP, "sleep", lambda _s: None)
    result = kartenn("--no-input", "login", "--device")
    assert result.exit_code == 0, result.output
    assert "WDJB-MJHT" in result.stderr
    assert "APPAREIL" in result.stderr
    assert "expire dans 10 min" in result.stderr
    assert [s["grant_type"] for s in oidc] == ["urn:ietf:params:oauth:grant-type:device_code"] * 2


def test_login_needs_a_terminal_for_the_browser(server: FakeServer) -> None:
    del server
    profile(auth="oidc", issuer="interne")
    result = kartenn("login")
    assert result.exit_code == 2
    assert "client.input-required" in result.stderr


def test_basic_secret_and_token(
    server: FakeServer, isolated: MemoryKeyring, monkeypatch: pytest.MonkeyPatch
) -> None:
    server.route("GET", "/v1/me", ME)
    profile(auth="basic", account="sync-dns")
    monkeypatch.setattr(Runtime, "interactive", property(lambda _self: True))
    result = kartenn("login", input="s3cret\n")
    assert result.exit_code == 0, result.output
    assert isolated.values["kartenn", "kartenn:prod:secret"] == "s3cret"
    assert server.seen[-1].headers["Authorization"].startswith("Basic ")
    kartenn("logout")
    assert ("kartenn", "kartenn:prod:secret") not in isolated.values
    profile(auth="token")
    assert kartenn("login").stdout.startswith("Rien à faire")


def test_breakglass_kept(server: FakeServer, monkeypatch: pytest.MonkeyPatch) -> None:
    server.route(
        "POST",
        "/v1/auth/breakglass",
        httpx2.Response(
            200, json={}, headers={"Set-Cookie": "kartenn_session=s1; HttpOnly; Secure"}
        ),
    )
    server.route("GET", "/v1/me", dict(ME, kind="breakglass", name="secours"))
    server.route("POST", "/v1/auth/logout", httpx2.Response(303, headers={"Location": "/"}))
    monkeypatch.setattr(Runtime, "interactive", property(lambda _self: True))
    result = kartenn("login", "--breakglass", "--keep", input="secours\nune phrase\n123456\n")
    assert result.exit_code == 0, result.output
    assert result.stdout.endswith("Connecté : secours (interne).\n")
    kept = session.session_path(Runtime(Settings()).require_profile())
    assert stat.S_IMODE(kept.stat().st_mode) == 0o600
    assert "une phrase" not in kept.read_text()
    me = kartenn("me", "-o", "json")
    assert '"kind": "breakglass"' in me.stdout
    assert server.seen[-1].headers["Cookie"] == "kartenn_session=s1"
    assert kartenn("logout").stdout == "Session de secours fermée.\n"
    assert not kept.exists()
