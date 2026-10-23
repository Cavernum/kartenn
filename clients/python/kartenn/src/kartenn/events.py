"""Following `GET /v1/events` (REQ-023-38; spec 022, § Contrat consommé par les clients).

- `Accept: text/event-stream`, the authentication of the client, and `Last-Event-ID`
  set by the client when it resumes; the cursor is opaque and kept as received.
- Before the stream opens: a 401 renews the authentication once and opens again; 429
  and 503 wait `Retry-After`; any other error raises its problem.
- A cut (end of the stream, network failure, 45 s of silence) resumes after a growing
  wait (the `retry:` received, doubled, capped at 30 s, ±20 %), with the last cursor.
- Duplicates are set aside by identity; `reset` is handed to the caller and the stream
  goes on.
- Without `capabilities.events`, the stream is absent: `client.operation-unavailable`.
"""

from __future__ import annotations

from datetime import UTC, datetime
from http import HTTPStatus
from typing import TYPE_CHECKING

from kartenn._core.messages import Headers, Request, Response
from kartenn._core.problems import ClientError, NetworkError, problem_error
from kartenn._core.sse import Event, EventTracker
from kartenn.retry import retry_after

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Awaitable, Callable, Iterable, Iterator
    from contextlib import AbstractAsyncContextManager, AbstractContextManager

    from kartenn.catalog import Catalog
    from kartenn.transport.port import AsyncEventStream, EventStream

__all__ = ["Event", "afollow", "event_request", "follow", "require_events"]

PATH = "/v1/events"
_WAIT = frozenset({HTTPStatus.TOO_MANY_REQUESTS, HTTPStatus.SERVICE_UNAVAILABLE})


def require_events(catalog: Catalog) -> None:
    if catalog.events is None:
        msg = f"the server (version {catalog.version}) serves no event stream"
        raise ClientError("client.operation-unavailable", msg)


def event_request(
    *,
    types: Iterable[str] | None = None,
    domains: Iterable[str] | None = None,
    objects: Iterable[str] | None = None,
    kinds: Iterable[str] | None = None,
) -> Request:
    """The request of the stream, its filters as comma-separated lists (022)."""
    params = tuple(
        (name, ",".join(values))
        for name, values in (
            ("types", types),
            ("domains", domains),
            ("objects", objects),
            ("kinds", kinds),
        )
        if values
    )
    return Request("GET", PATH, params, Headers.of({"Accept": "text/event-stream"}))


def _refused(stream: EventStream | AsyncEventStream) -> Response:
    return Response(stream.status, stream.headers, stream.content)


def _wait(stream: EventStream | AsyncEventStream, tracker: EventTracker) -> float:
    found = retry_after(stream.headers, now=datetime.now(UTC))
    return tracker.backoff() if found is None else found


def _check(stream: EventStream | AsyncEventStream, *, renewed: bool) -> str:
    """What to do with a stream as it opens: `renew`, `wait`, or `read`; a refusal raises
    its problem."""
    if stream.status == HTTPStatus.UNAUTHORIZED and not renewed:
        return "renew"
    if stream.status in _WAIT:
        return "wait"
    if stream.status != HTTPStatus.OK:
        raise problem_error(_refused(stream))
    return "read"


def follow(  # noqa: PLR0913 - the parts of a client a stream needs
    request: Request,
    tracker: EventTracker,
    *,
    open_stream: Callable[[Request], AbstractContextManager[EventStream]],
    authorize: Callable[[Request], Request],
    renew: Callable[[Response], bool],
    sleep: Callable[[float], None],
    reconnect: bool = True,
) -> Iterator[Event]:
    """The events of the stream, resumed after each cut while `reconnect`."""
    renewed = False
    while True:
        try:
            with open_stream(authorize(_resume(request, tracker))) as stream:
                check = _check(stream, renewed=renewed)
                if check == "renew":
                    renewed = True
                    if not renew(_refused(stream)):
                        raise problem_error(_refused(stream))
                    continue
                if check == "wait":
                    sleep(_wait(stream, tracker))
                    continue
                renewed = False
                yield from filter(None, map(tracker.feed, stream.events))
        except NetworkError as failure:
            if failure.code == "client.tls":
                raise
        if not reconnect:
            return
        sleep(tracker.backoff())


async def afollow(  # noqa: PLR0913 - the parts of a client a stream needs
    request: Request,
    tracker: EventTracker,
    *,
    open_stream: Callable[[Request], AbstractAsyncContextManager[AsyncEventStream]],
    authorize: Callable[[Request], Awaitable[Request]],
    renew: Callable[[Response], Awaitable[bool]],
    sleep: Callable[[float], Awaitable[None]],
    reconnect: bool = True,
) -> AsyncIterator[Event]:
    """The asynchronous `follow`."""
    renewed = False
    while True:
        try:
            async with open_stream(await authorize(_resume(request, tracker))) as stream:
                check = _check(stream, renewed=renewed)
                if check == "renew":
                    renewed = True
                    if not await renew(_refused(stream)):
                        raise problem_error(_refused(stream))
                    continue
                if check == "wait":
                    await sleep(_wait(stream, tracker))
                    continue
                renewed = False
                async for raw in stream.events:
                    if (event := tracker.feed(raw)) is not None:
                        yield event
        except NetworkError as failure:
            if failure.code == "client.tls":
                raise
        if not reconnect:
            return
        await sleep(tracker.backoff())


def _resume(request: Request, tracker: EventTracker) -> Request:
    headers = request.headers
    for name, value in tracker.headers().items():
        headers = headers.set(name, value)
    return Request(request.method, request.url, request.params, headers, request.content)
