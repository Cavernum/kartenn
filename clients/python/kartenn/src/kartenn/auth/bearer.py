"""A Bearer token given by the caller (`KARTENN_TOKEN`, CI): sent as is, never refreshed."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from kartenn._core.messages import Request, Response
    from kartenn._core.plans import Plan

__all__ = ["BearerToken"]


class BearerToken:
    def __init__(self, token: str) -> None:
        self._token = token

    def __repr__(self) -> str:
        return "BearerToken(***)"

    def authorize(self, request: Request) -> Plan[Request]:
        return request.with_header("Authorization", f"Bearer {self._token}")
        yield  # pragma: no cover - a plan without a request

    def unauthorized(self, response: Response) -> Plan[bool]:  # noqa: ARG002 - the port
        return False
        yield  # pragma: no cover - a plan without a request

    def check_server(self, url: str) -> None:
        pass
