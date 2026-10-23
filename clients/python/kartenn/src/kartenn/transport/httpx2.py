"""The default adapter of the transport port, on httpx2 (T5). The only module of the
library that imports httpx2.

- Delays: 10 s to connect, 60 s to read; an event stream has a read delay of 45 s, three
  missed `: ping` of the server (every 15 s), after which the caller resumes.
- Trust: without `ca_bundle`, the store of the system (httpx2 uses truststore). With
  `ca_bundle`, an explicit `ssl.SSLContext` loaded from that file alone, host name
  checked, TLS 1.2 at least: no fallback to the store of the system (REQ-023-4).
- HTTP/2 when the `h2` package is installed, unless `http2=False`.
- The proxy variables of the environment (`HTTPS_PROXY`, `NO_PROXY`) are honoured.
"""

from __future__ import annotations

import importlib.util
import ssl
from contextlib import asynccontextmanager, contextmanager
from typing import TYPE_CHECKING

import anyio
import httpx2

from kartenn._core.messages import Headers, Response
from kartenn._core.problems import ClientError, NetworkError
from kartenn._core.sse import RawEvent
from kartenn.transport.port import (
    AsyncEventStream,
    AsyncStreamResponse,
    EventStream,
    StreamResponse,
)

if TYPE_CHECKING:
    import os
    from collections.abc import AsyncGenerator, AsyncIterator, Generator, Iterator

    from kartenn._core.messages import Request

__all__ = ["AsyncHttpx2Transport", "Httpx2Transport", "ssl_context", "use_http2"]

CONNECT_SECONDS = 10.0
READ_SECONDS = 60.0
EVENT_SILENCE_SECONDS = 45.0
_EVENT_STREAM = "text/event-stream"


def ssl_context(ca_bundle: str | os.PathLike[str]) -> ssl.SSLContext:
    """The context of a profile's own authorities: this file and nothing else."""
    context = ssl.create_default_context(ssl.Purpose.SERVER_AUTH, cafile=str(ca_bundle))
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.check_hostname = True
    context.verify_mode = ssl.CERT_REQUIRED
    return context


def use_http2(wanted: bool | None) -> bool:  # noqa: FBT001 - the option of the profile
    """Whether to speak HTTP/2: when asked, or by default when `h2` is installed."""
    available = importlib.util.find_spec("h2") is not None
    if wanted and not available:
        msg = "HTTP/2 needs the h2 package (pip install 'httpx2[http2]')"
        raise ClientError("client.http2-unavailable", msg)
    return available if wanted is None else wanted


def _settings(
    ca_bundle: str | os.PathLike[str] | None,
    *,
    http2: bool | None,
) -> tuple[ssl.SSLContext | bool, bool, httpx2.Timeout]:
    verify: ssl.SSLContext | bool = True if ca_bundle is None else ssl_context(ca_bundle)
    return verify, use_http2(http2), httpx2.Timeout(READ_SECONDS, connect=CONNECT_SECONDS)


def _failure(error: httpx2.TransportError) -> NetworkError:
    cause: BaseException | None = error
    while cause is not None:
        if isinstance(cause, ssl.SSLError):
            return NetworkError("client.tls", f"TLS failed: {cause}")
        cause = cause.__cause__ or cause.__context__
    if isinstance(error, httpx2.TimeoutException):
        return NetworkError("client.timeout", f"no answer in time: {error}")
    return NetworkError("client.network", f"the server could not be reached: {error}")


@contextmanager
def _mapped() -> Generator[None]:
    try:
        yield
    except httpx2.TransportError as error:
        raise _failure(error) from error


def _response(response: httpx2.Response, content: bytes) -> Response:
    return Response(response.status_code, _headers(response), content)


def _headers(response: httpx2.Response) -> Headers:
    return Headers(tuple(response.headers.multi_items()))


def _params(request: Request) -> tuple[tuple[str, str], ...] | None:
    """`None` without parameters: a link of the server keeps its own query string."""
    return request.params or None


def _event(raw: httpx2.ServerSentEvent) -> RawEvent:
    return RawEvent(raw.event, raw.data, raw.id, raw.retry)


def _not_events(response: httpx2.Response) -> bool:
    kind = response.headers.get("content-type", "").partition(";")[0].strip().lower()
    return kind != _EVENT_STREAM


class Httpx2Transport:
    """The synchronous adapter; `mock` replaces the network (`httpx2.MockTransport`)."""

    def __init__(
        self,
        base_url: str,
        *,
        ca_bundle: str | os.PathLike[str] | None = None,
        http2: bool | None = None,
        mock: httpx2.BaseTransport | None = None,
    ) -> None:
        verify, use_http2, timeout = _settings(ca_bundle, http2=http2)
        self._client = httpx2.Client(
            base_url=base_url,
            verify=verify,
            http2=use_http2,
            timeout=timeout,
            transport=mock,
            follow_redirects=False,
        )

    def send(self, request: Request) -> Response:
        with _mapped():
            answer = self._client.request(
                request.method,
                request.url,
                params=_params(request),
                headers=list(request.headers),
                content=request.content,
            )
        return _response(answer, answer.content)

    @contextmanager
    def stream(self, request: Request) -> Generator[StreamResponse]:
        with (
            _mapped(),
            self._client.stream(
                request.method,
                request.url,
                params=_params(request),
                headers=list(request.headers),
                content=request.content,
            ) as answer,
        ):
            if answer.is_error:
                yield StreamResponse(answer.status_code, _headers(answer), content=answer.read())
            else:
                yield StreamResponse(answer.status_code, _headers(answer), self._chunks(answer))

    @staticmethod
    def _chunks(answer: httpx2.Response) -> Iterator[bytes]:
        with _mapped():
            yield from answer.iter_bytes()

    @contextmanager
    def sse(self, request: Request) -> Generator[EventStream]:
        timeout = httpx2.Timeout(EVENT_SILENCE_SECONDS, connect=CONNECT_SECONDS)
        with (
            _mapped(),
            self._client.sse(
                request.url,
                method=request.method,
                timeout=timeout,
                params=_params(request),
                headers=list(request.headers),
                content=request.content,
            ) as source,
        ):
            answer = source.response
            if answer.is_error or _not_events(answer):
                yield EventStream(answer.status_code, _headers(answer), content=answer.read())
            else:
                yield EventStream(answer.status_code, _headers(answer), self._events(source))

    @staticmethod
    def _events(source: httpx2.EventSource) -> Iterator[RawEvent]:
        with _mapped():
            for raw in source:
                yield _event(raw)

    def sleep(self, seconds: float) -> None:
        import time  # noqa: PLC0415 - only when a retry waits

        time.sleep(seconds)

    def close(self) -> None:
        self._client.close()


class AsyncHttpx2Transport:
    """The asynchronous adapter, for asyncio and trio alike (through anyio, a dependency
    of httpx2)."""

    def __init__(
        self,
        base_url: str,
        *,
        ca_bundle: str | os.PathLike[str] | None = None,
        http2: bool | None = None,
        mock: httpx2.AsyncBaseTransport | None = None,
    ) -> None:
        verify, use_http2, timeout = _settings(ca_bundle, http2=http2)
        self._client = httpx2.AsyncClient(
            base_url=base_url,
            verify=verify,
            http2=use_http2,
            timeout=timeout,
            transport=mock,
            follow_redirects=False,
        )

    async def send(self, request: Request) -> Response:
        with _mapped():
            answer = await self._client.request(
                request.method,
                request.url,
                params=_params(request),
                headers=list(request.headers),
                content=request.content,
            )
        return _response(answer, answer.content)

    @asynccontextmanager
    async def stream(self, request: Request) -> AsyncGenerator[AsyncStreamResponse]:
        with _mapped():
            async with self._client.stream(
                request.method,
                request.url,
                params=_params(request),
                headers=list(request.headers),
                content=request.content,
            ) as answer:
                if answer.is_error:
                    content = await answer.aread()
                    yield AsyncStreamResponse(answer.status_code, _headers(answer), content=content)
                else:
                    yield AsyncStreamResponse(
                        answer.status_code, _headers(answer), self._chunks(answer)
                    )

    @staticmethod
    async def _chunks(answer: httpx2.Response) -> AsyncIterator[bytes]:
        with _mapped():
            async for chunk in answer.aiter_bytes():
                yield chunk

    @asynccontextmanager
    async def sse(self, request: Request) -> AsyncGenerator[AsyncEventStream]:
        timeout = httpx2.Timeout(EVENT_SILENCE_SECONDS, connect=CONNECT_SECONDS)
        with _mapped():
            async with self._client.sse(
                request.url,
                method=request.method,
                timeout=timeout,
                params=_params(request),
                headers=list(request.headers),
                content=request.content,
            ) as source:
                answer = source.response
                if answer.is_error or _not_events(answer):
                    content = await answer.aread()
                    yield AsyncEventStream(answer.status_code, _headers(answer), content=content)
                else:
                    yield AsyncEventStream(
                        answer.status_code, _headers(answer), self._events(source)
                    )

    @staticmethod
    async def _events(source: httpx2.EventSource) -> AsyncIterator[RawEvent]:
        with _mapped():
            async for raw in source:
                yield _event(raw)

    async def sleep(self, seconds: float) -> None:
        await anyio.sleep(seconds)

    async def aclose(self) -> None:
        await self._client.aclose()
