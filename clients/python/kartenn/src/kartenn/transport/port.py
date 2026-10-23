"""The transport port (design § Transport, T5): three operations, synchronous and
asynchronous, over the HTTP messages of the core.

- `send`: a request, its response read in full;
- `stream`: a response read as it comes (imports, `ndjson`);
- `sse`: a `text/event-stream` read event by event (022).

An error status keeps the response: `stream` and `sse` then hold its body in
`content` and yield nothing. A failure to reach the server raises `NetworkError`
(`client.network`, `client.tls`, `client.timeout`). `sleep` belongs to the port so that
the library waits the way its event loop does, and tests do not wait at all.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Protocol

from kartenn._core.messages import Headers

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Iterator
    from contextlib import AbstractAsyncContextManager, AbstractContextManager

    from kartenn._core.messages import Request, Response
    from kartenn._core.sse import RawEvent

__all__ = [
    "AsyncEventStream",
    "AsyncStreamResponse",
    "AsyncTransport",
    "EventStream",
    "StreamResponse",
    "Transport",
]


def _no_chunks() -> Iterator[bytes]:
    return iter(())


def _no_events() -> Iterator[RawEvent]:
    return iter(())


async def _no_async() -> AsyncIterator[bytes]:
    for chunk in ():
        yield chunk


async def _no_async_events() -> AsyncIterator[RawEvent]:
    for event in ():
        yield event


@dataclass
class StreamResponse:
    status: int
    headers: Headers = field(default_factory=Headers)
    chunks: Iterator[bytes] = field(default_factory=_no_chunks)
    content: bytes = b""


@dataclass
class EventStream:
    status: int
    headers: Headers = field(default_factory=Headers)
    events: Iterator[RawEvent] = field(default_factory=_no_events)
    content: bytes = b""


@dataclass
class AsyncStreamResponse:
    status: int
    headers: Headers = field(default_factory=Headers)
    chunks: AsyncIterator[bytes] = field(default_factory=_no_async)
    content: bytes = b""


@dataclass
class AsyncEventStream:
    status: int
    headers: Headers = field(default_factory=Headers)
    events: AsyncIterator[RawEvent] = field(default_factory=_no_async_events)
    content: bytes = b""


class Transport(Protocol):
    def send(self, request: Request) -> Response: ...

    def stream(self, request: Request) -> AbstractContextManager[StreamResponse]: ...

    def sse(self, request: Request) -> AbstractContextManager[EventStream]: ...

    def sleep(self, seconds: float) -> None: ...

    def close(self) -> None: ...


class AsyncTransport(Protocol):
    async def send(self, request: Request) -> Response: ...

    def stream(self, request: Request) -> AbstractAsyncContextManager[AsyncStreamResponse]: ...

    def sse(self, request: Request) -> AbstractAsyncContextManager[AsyncEventStream]: ...

    async def sleep(self, seconds: float) -> None: ...

    async def aclose(self) -> None: ...
