"""Humans by native OIDC (REQ-023-7, T10): the client is a public client of the issuer.

- Issuer: given, or found by its local identifier in `GET /v1/auth/native-clients`
  (`[{id, discovery_url, client_id, audience, scopes, device}]`, 023/A5).
- Login by loopback (RFC 8252 §7.3): a listener on `127.0.0.1:0` (`[::1]:0` when IPv4
  is missing), PKCE `S256` (RFC 7636 §4.2) from 32 random octets, random `state` and
  `nonce`, the system browser (RFC 8252 §4, §8.12), five minutes to answer. The `state`
  is checked before the code is exchanged.
- Only the access token is sent to kartenn, never the identity token.
- Refresh: 60 s before the expiry, or once on a 401; the issuer is expected to rotate
  the refresh token (RFC 9700 §2.2.2), the new one replaces the old. A refusal clears
  the tokens and asks for a new login (`client.login-required`).

See `docs/reference/oauth-clients-natifs.md`.
"""

from __future__ import annotations

import base64
import hashlib
import secrets
import socket
import threading
import time
import webbrowser
from dataclasses import dataclass
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import TYPE_CHECKING, Any, Self, cast
from urllib.parse import parse_qsl, urlencode, urlsplit

from kartenn._core.messages import Headers, Request
from kartenn._core.plans import checked
from kartenn._core.problems import ClientError

if TYPE_CHECKING:
    from collections.abc import Callable

    from kartenn._core.messages import Response
    from kartenn._core.plans import Plan
    from kartenn.auth.store import TokenStore

__all__ = ["Issuer", "OidcNative", "pkce_pair"]

REFRESH_MARGIN = 60.0
LOGIN_SECONDS = 300.0
_FORM = "application/x-www-form-urlencoded"
_PAGE = (
    b"<!doctype html><meta charset=utf-8><title>kartenn</title>"
    b"<p>kartenn: you may close this tab and go back to the terminal.</p>"
)


@dataclass(frozen=True)
class Issuer:
    """A public client at an issuer: what `GET /v1/auth/native-clients` publishes."""

    discovery_url: str
    client_id: str
    audience: str | None = None
    scopes: tuple[str, ...] = ("openid",)
    device: bool = False
    id: str | None = None

    @classmethod
    def parse(cls, data: dict[str, Any]) -> Issuer:
        scopes = data.get("scopes")
        return cls(
            discovery_url=str(data["discovery_url"]),
            client_id=str(data["client_id"]),
            audience=None if data.get("audience") is None else str(data["audience"]),
            scopes=tuple(str(s) for s in cast("list[object]", scopes))
            if isinstance(scopes, list)
            else ("openid",),
            device=bool(data.get("device", False)),
            id=None if data.get("id") is None else str(data["id"]),
        )


def pkce_pair() -> tuple[str, str]:
    """A `code_verifier` of 32 random octets and its `S256` challenge (RFC 7636 §4.1, 4.2)."""
    verifier = secrets.token_urlsafe(32)
    digest = hashlib.sha256(verifier.encode("ascii")).digest()
    return verifier, base64.urlsafe_b64encode(digest).decode("ascii").rstrip("=")


def form_request(url: str, fields: dict[str, str]) -> Request:
    headers = Headers.of({"Content-Type": _FORM, "Accept": "application/json"})
    return Request("POST", url, (), headers, urlencode(fields).encode())


def json_object(response: Response) -> dict[str, Any]:
    try:
        data = response.json()
    except ValueError:
        data = None
    return cast("dict[str, Any]", data) if isinstance(data, dict) else {}


def audience_fields(issuer: Issuer) -> dict[str, str]:
    """The audience asked for: `resource` (RFC 8707) for an absolute URI, the `audience`
    extension of some issuers otherwise."""
    if issuer.audience is None:
        return {}
    if urlsplit(issuer.audience).scheme in {"http", "https", "urn"}:
        return {"resource": issuer.audience}
    return {"audience": issuer.audience}


class OidcNative:
    """The `Auth` of a human: Bearer tokens of the issuer, kept in a `TokenStore`."""

    def __init__(
        self,
        issuer: Issuer | str,
        store: TokenStore,
        *,
        key: str = "kartenn:default",
        clock: Callable[[], float] = time.time,
    ) -> None:
        self._issuer = issuer
        self._store = store
        self._key = key
        self._clock = clock
        self._metadata: dict[str, Any] | None = None

    def __repr__(self) -> str:
        return f"OidcNative({self._key!r})"

    def check_server(self, url: str) -> None:
        pass

    # The issuer ------------------------------------------------------------------

    def issuer(self) -> Plan[Issuer]:
        if isinstance(self._issuer, Issuer):
            return self._issuer
        answer = checked((yield Request("GET", "/v1/auth/native-clients")))
        listed = answer.json()
        for raw in cast("list[object]", listed) if isinstance(listed, list) else []:
            if isinstance(raw, dict) and cast("dict[str, Any]", raw).get("id") == self._issuer:
                self._issuer = Issuer.parse(cast("dict[str, Any]", raw))
                return self._issuer
        msg = f"the server publishes no native client for the issuer {self._issuer!r}"
        raise ClientError("client.unknown-issuer", msg)

    def metadata(self) -> Plan[dict[str, Any]]:
        """The discovery document of the issuer (OIDC Discovery §4)."""
        if self._metadata is None:
            issuer = yield from self.issuer()
            found = json_object(checked((yield Request("GET", issuer.discovery_url))))
            if "token_endpoint" not in found:
                msg = "the discovery document of the issuer has no token_endpoint"
                raise ClientError("client.oidc-error", msg)
            methods = found.get("code_challenge_methods_supported")
            if isinstance(methods, list) and "S256" not in cast("list[object]", methods):
                msg = "the issuer does not offer PKCE S256"
                raise ClientError("client.oidc-error", msg)
            self._metadata = found
        return self._metadata

    # Tokens ----------------------------------------------------------------------

    def keep(self, tokens: dict[str, Any]) -> str:
        access = tokens.get("access_token")
        if not isinstance(access, str) or not access:
            msg = "the issuer answered no access token"
            raise ClientError("client.oidc-error", msg)
        self._store.set(f"{self._key}:access_token", access)
        refresh = tokens.get("refresh_token")
        if isinstance(refresh, str) and refresh:
            self._store.set(f"{self._key}:refresh_token", refresh)
        expires = tokens.get("expires_in")
        if isinstance(expires, int | float):
            self._store.set(f"{self._key}:expires_at", str(self._clock() + float(expires)))
        else:
            self._store.delete(f"{self._key}:expires_at")
        return access

    def logout(self) -> None:
        """Forget the tokens of this profile."""
        for name in ("access_token", "refresh_token", "expires_at"):
            self._store.delete(f"{self._key}:{name}")

    def exchange(self, fields: dict[str, str]) -> Plan[dict[str, Any]]:
        """A request to the token endpoint; its error, `error` of RFC 6749 §5.2, kept."""
        metadata = yield from self.metadata()
        issuer = yield from self.issuer()
        answer = yield form_request(
            str(metadata["token_endpoint"]), {**fields, "client_id": issuer.client_id}
        )
        data = json_object(answer)
        if answer.status >= HTTPStatus.BAD_REQUEST and "error" not in data:
            data["error"] = f"http-{answer.status}"
        return data

    def refresh(self) -> Plan[str]:
        token = self._store.get(f"{self._key}:refresh_token")
        if token is None:
            self.logout()
            msg = "the session has expired: log in again (kartenn login)"
            raise ClientError("client.login-required", msg)
        tokens = yield from self.exchange({"grant_type": "refresh_token", "refresh_token": token})
        if "error" in tokens:
            self.logout()
            msg = f"the issuer refused to renew the session ({tokens['error']}): log in again"
            raise ClientError("client.login-required", msg)
        return self.keep(tokens)

    def authorize(self, request: Request) -> Plan[Request]:
        access = self._store.get(f"{self._key}:access_token")
        expires = self._store.get(f"{self._key}:expires_at")
        if access is None:
            msg = "not logged in: kartenn login"
            raise ClientError("client.login-required", msg)
        if expires is not None and float(expires) - self._clock() < REFRESH_MARGIN:
            access = yield from self.refresh()
        return request.with_header("Authorization", f"Bearer {access}")

    def unauthorized(self, response: Response) -> Plan[bool]:  # noqa: ARG002 - the port
        """A 401: renew once, then the client sends the request again."""
        yield from self.refresh()
        return True

    # Logins ----------------------------------------------------------------------

    def authorization_url(self, redirect_uri: str) -> Plan[tuple[str, dict[str, str]]]:
        """The URL to open and the secrets of this login (`state`, `nonce`, verifier)."""
        metadata = yield from self.metadata()
        issuer = yield from self.issuer()
        endpoint = metadata.get("authorization_endpoint")
        if not isinstance(endpoint, str):
            msg = "the issuer has no authorization_endpoint"
            raise ClientError("client.oidc-error", msg)
        verifier, challenge = pkce_pair()
        login = {
            "state": secrets.token_urlsafe(32),
            "nonce": secrets.token_urlsafe(32),
            "verifier": verifier,
            "redirect_uri": redirect_uri,
        }
        query = {
            "response_type": "code",
            "client_id": issuer.client_id,
            "redirect_uri": redirect_uri,
            "scope": " ".join(issuer.scopes),
            "state": login["state"],
            "nonce": login["nonce"],
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            **audience_fields(issuer),
        }
        separator = "&" if "?" in endpoint else "?"
        return f"{endpoint}{separator}{urlencode(query)}", login

    def complete(self, login: dict[str, str], callback: dict[str, str]) -> Plan[None]:
        """Check the answer of the issuer at the loopback, then exchange the code."""
        if not secrets.compare_digest(callback.get("state", ""), login["state"]):
            msg = "the answer of the issuer does not carry the state of this login"
            raise ClientError("client.oidc-state-mismatch", msg)
        if "error" in callback:
            msg = f"the issuer refused the login ({callback['error']})"
            raise ClientError("client.oidc-denied", msg)
        code = callback.get("code")
        if not code:
            msg = "the issuer answered no code"
            raise ClientError("client.oidc-error", msg)
        tokens = yield from self.exchange(
            {
                "grant_type": "authorization_code",
                "code": code,
                "redirect_uri": login["redirect_uri"],
                "code_verifier": login["verifier"],
            }
        )
        if "error" in tokens:
            msg = f"the issuer refused the code ({tokens['error']})"
            raise ClientError("client.oidc-denied", msg)
        self.keep(tokens)

    def login(
        self,
        run: Callable[[Plan[Any]], Any],
        *,
        open_browser: Callable[[str], object] = webbrowser.open,
        timeout: float = LOGIN_SECONDS,
    ) -> None:
        """Log in through the system browser and a loopback listener. `run` runs a plan
        without authentication (`Client.run_unauthenticated`)."""
        with _Listener() as listener:
            url, login = cast(
                "tuple[str, dict[str, str]]", run(self.authorization_url(listener.redirect_uri))
            )
            open_browser(url)
            callback = listener.wait(timeout)
        run(self.complete(login, callback))


class _Handler(BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        server = cast("_Server", self.server)
        parts = urlsplit(self.path)
        if parts.path != "/callback":
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        server.answer = dict(parse_qsl(parts.query))
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(_PAGE)))
        self.end_headers()
        self.wfile.write(_PAGE)
        server.done.set()

    def log_message(self, format: str, *args: object) -> None:  # noqa: A002 - the base
        """Nothing logged: the query string holds the code."""


class _Server(HTTPServer):
    answer: dict[str, str]

    def __init__(self, address: tuple[str, int], family: socket.AddressFamily) -> None:
        self.address_family = family
        self.done = threading.Event()
        self.answer = {}
        super().__init__(address, _Handler)


class _Listener:
    """A listener of one answer on a free port of the loopback (RFC 8252 §7.3)."""

    def __init__(self) -> None:
        try:
            self._server = _Server(("127.0.0.1", 0), socket.AF_INET)
            host = "127.0.0.1"
        except OSError:  # pragma: no cover - a host without IPv4
            self._server = _Server(("::1", 0), socket.AF_INET6)
            host = "[::1]"
        self.redirect_uri = f"http://{host}:{self._server.server_address[1]}/callback"
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)

    def __enter__(self) -> Self:
        self._thread.start()
        return self

    def __exit__(self, *_: object) -> None:
        self._server.shutdown()
        self._server.server_close()

    def wait(self, timeout: float) -> dict[str, str]:
        if not self._server.done.wait(timeout):
            msg = f"no answer of the issuer in {timeout:.0f} s"
            raise ClientError("client.login-timeout", msg)
        return self._server.answer
