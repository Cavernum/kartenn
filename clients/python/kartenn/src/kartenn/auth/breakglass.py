"""The break-glass session (REQ-023-8; 005, 006 § Authentification).

`POST /v1/auth/breakglass {name, passphrase, code}` answers a session cookie of at most
one hour, kept in memory only (the CLI may keep it in `$XDG_RUNTIME_DIR` on request).
The passphrase and the code are forgotten once used. Every write under this session
carries the `Origin` of the instance (CSRF control, 006).
"""

from __future__ import annotations

import json
from http import HTTPStatus
from typing import TYPE_CHECKING
from urllib.parse import urlsplit

from kartenn._core.messages import Headers, Request
from kartenn._core.plans import checked
from kartenn._core.problems import ClientError

if TYPE_CHECKING:
    from kartenn._core.messages import Response
    from kartenn._core.plans import Plan
    from kartenn.auth.basic import Secret

__all__ = ["Breakglass"]

_SAFE = frozenset({"GET", "HEAD", "OPTIONS", "QUERY"})


class Breakglass:
    def __init__(
        self, name: str, passphrase: Secret | None, code: str | None, *, cookie: str | None = None
    ) -> None:
        self.name = name
        self._passphrase = passphrase
        self._code = code
        self.cookie = cookie
        self._origin: str | None = None

    def __repr__(self) -> str:
        return f"Breakglass({self.name!r}, ***)"

    def check_server(self, url: str) -> None:
        parts = urlsplit(url)
        self._origin = f"{parts.scheme}://{parts.netloc}"

    def _login(self) -> Plan[str]:
        if self._passphrase is None or self._code is None:
            msg = "the break-glass session ended: log in again"
            raise ClientError("client.login-required", msg)
        proofs = {"name": self.name, "passphrase": self._passphrase.reveal(), "code": self._code}
        body = json.dumps(proofs)
        headers = Headers.of({"Content-Type": "application/json"})
        if self._origin is not None:
            headers = headers.set("Origin", self._origin)
        answer = checked((yield Request("POST", "/v1/auth/breakglass", (), headers, body.encode())))
        self._passphrase, self._code = None, None
        for header in answer.headers.get_all("Set-Cookie"):
            pair = header.split(";", 1)[0].strip()
            if "=" in pair:
                self.cookie = pair
                return pair
        msg = "the server opened no session"
        raise ClientError("client.unexpected-answer", msg)

    def authorize(self, request: Request) -> Plan[Request]:
        cookie = self.cookie or (yield from self._login())
        request = request.with_header("Cookie", cookie)
        if request.method not in _SAFE and self._origin is not None:
            request = request.with_header("Origin", self._origin)
        return request

    def unauthorized(self, response: Response) -> Plan[bool]:
        if response.status == HTTPStatus.UNAUTHORIZED:
            self.cookie = None
        return False
        yield  # pragma: no cover - a plan without a request
