"""Authentication of the clients (design § Authentification, REQ-023-6 to 8).

An `Auth` works by plans, like the calls of the client: `authorize` turns a request into
an authenticated one (refreshing a token first when it must), `unauthorized` reacts to
a 401 and tells whether the request is worth sending again. The requests of an issuer
go through the same transport, unauthenticated.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

from kartenn.auth.basic import BasicAuth, Secret
from kartenn.auth.bearer import BearerToken
from kartenn.auth.breakglass import Breakglass
from kartenn.auth.device import DeviceCode, device_login
from kartenn.auth.oidc_native import Issuer, OidcNative
from kartenn.auth.store import FileStore, MemoryStore, TokenStore

if TYPE_CHECKING:
    from kartenn._core.messages import Request, Response
    from kartenn._core.plans import Plan

__all__ = [
    "Anonymous",
    "Auth",
    "BasicAuth",
    "BearerToken",
    "Breakglass",
    "DeviceCode",
    "FileStore",
    "Issuer",
    "MemoryStore",
    "OidcNative",
    "Secret",
    "TokenStore",
    "device_login",
]


class Auth(Protocol):
    def authorize(self, request: Request) -> Plan[Request]: ...

    def unauthorized(self, response: Response) -> Plan[bool]: ...

    def check_server(self, url: str) -> None:
        """Refuse a server this mode must not be used with (Basic over `http`)."""


class Anonymous:
    """No authentication: the public routes (`capabilities`, `openapi.json`)."""

    def authorize(self, request: Request) -> Plan[Request]:
        return request
        yield  # pragma: no cover - a plan without a request

    def unauthorized(self, response: Response) -> Plan[bool]:  # noqa: ARG002 - the port
        return False
        yield  # pragma: no cover - a plan without a request

    def check_server(self, url: str) -> None:
        pass
