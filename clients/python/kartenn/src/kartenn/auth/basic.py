"""Service accounts in Basic (RFC 7617; REQ-023-6, 005/K8).

The secret is a `Secret`, never an argument of the command line: it comes from a
`TokenStore`, from a file (`KARTENN_SECRET_FILE`) or from the standard input of the CLI.
Basic is refused outside `https`, except towards a loopback address when the caller asks
for it explicitly (tests).
"""

from __future__ import annotations

import base64
import ipaddress
from pathlib import Path
from typing import TYPE_CHECKING
from urllib.parse import urlsplit

from kartenn._core.problems import ClientError

if TYPE_CHECKING:
    from kartenn._core.messages import Request, Response
    from kartenn._core.plans import Plan

__all__ = ["BasicAuth", "Secret", "is_loopback"]


class Secret:
    """A secret value that never shows in a representation or a log."""

    def __init__(self, value: str) -> None:
        if not value:
            msg = "an empty secret"
            raise ClientError("client.secret-required", msg)
        self._value = value

    @classmethod
    def from_file(cls, path: str | Path) -> Secret:
        """The first line of a file (`KARTENN_SECRET_FILE`), its end of line dropped."""
        try:
            text = Path(path).read_text(encoding="utf-8")
        except OSError as error:
            msg = f"the secret file cannot be read: {error.strerror}"
            raise ClientError("client.secret-required", msg) from error
        return cls(text.splitlines()[0] if text else "")

    def reveal(self) -> str:
        return self._value

    def __repr__(self) -> str:
        return "Secret(***)"

    __str__ = __repr__


def is_loopback(url: str) -> bool:
    host = urlsplit(url).hostname or ""
    if host == "localhost":
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


class BasicAuth:
    def __init__(
        self, account: str, secret: Secret, *, allow_insecure_loopback: bool = False
    ) -> None:
        self.account = account
        self._secret = secret
        self._allow_loopback = allow_insecure_loopback

    def __repr__(self) -> str:
        return f"BasicAuth({self.account!r}, ***)"

    def check_server(self, url: str) -> None:
        scheme = urlsplit(url).scheme
        if scheme == "https" or (self._allow_loopback and is_loopback(url)):
            return
        msg = f"Basic authentication is refused over {scheme or 'an unknown scheme'}: use https"
        raise ClientError("client.insecure-transport", msg)

    def authorize(self, request: Request) -> Plan[Request]:
        pair = f"{self.account}:{self._secret.reveal()}".encode()
        token = base64.b64encode(pair).decode("ascii")
        return request.with_header("Authorization", f"Basic {token}")
        yield  # pragma: no cover - a plan without a request

    def unauthorized(self, response: Response) -> Plan[bool]:  # noqa: ARG002 - the port
        return False
        yield  # pragma: no cover - a plan without a request
