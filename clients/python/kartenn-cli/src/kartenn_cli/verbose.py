"""`--verbose` (REQ-023-41): method, path, status, duration and `Request-Id` of each
request on the standard error. Never a header that may hold a secret: only these."""

from __future__ import annotations

import time
from contextlib import contextmanager
from typing import TYPE_CHECKING

import click

if TYPE_CHECKING:
    from collections.abc import Generator

    from kartenn._core.messages import Request, Response
    from kartenn.transport.port import EventStream, StreamResponse, Transport

__all__ = ["Verbose"]


def _path(request: Request) -> str:
    return request.url


class Verbose:
    """A transport that tells what it sends."""

    def __init__(self, inner: Transport) -> None:
        self._inner = inner

    def _say(self, request: Request, status: int, started: float, request_id: str | None) -> None:
        elapsed = (time.perf_counter() - started) * 1000
        suffix = f"  {request_id}" if request_id else ""
        click.echo(
            f"{request.method} {_path(request)} → {status} ({elapsed:.0f} ms){suffix}", err=True
        )

    def send(self, request: Request) -> Response:
        started = time.perf_counter()
        response = self._inner.send(request)
        self._say(request, response.status, started, response.headers.get("Request-Id"))
        return response

    @contextmanager
    def stream(self, request: Request) -> Generator[StreamResponse]:
        started = time.perf_counter()
        with self._inner.stream(request) as response:
            self._say(request, response.status, started, response.headers.get("Request-Id"))
            yield response

    @contextmanager
    def sse(self, request: Request) -> Generator[EventStream]:
        started = time.perf_counter()
        with self._inner.sse(request) as stream:
            self._say(request, stream.status, started, stream.headers.get("Request-Id"))
            yield stream

    def sleep(self, seconds: float) -> None:
        self._inner.sleep(seconds)

    def close(self) -> None:
        self._inner.close()
