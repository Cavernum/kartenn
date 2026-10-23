"""Sending with retries (REQ-023-5, 14, 41; design § Clients, « Reprise »).

- 429 and 503 are replayed after `Retry-After` (seconds or an HTTP date), 3 times at
  most and within 60 s in all; without `Retry-After`, after 1 s, then 2 s, then 4 s.
- A 409 `idempotency-in-progress` is waited for, then replayed.
- A failure to reach the server is replayed the same way, never a TLS failure.
- Only a request that may be sent twice is replayed: a safe or idempotent method, or one
  that carries an `Idempotency-Key` (a `POST` without one never is).

Every request carries `User-Agent` and a `traceparent` of its own (W3C Trace Context).
The log (`kartenn`, DEBUG) names the method, the path, the status and the duration:
never a header, a body or a query string, which may hold a secret (REQ-023-40).
"""

from __future__ import annotations

import logging
import time
from dataclasses import dataclass
from datetime import UTC, datetime
from email.utils import parsedate_to_datetime
from http import HTTPStatus
from typing import TYPE_CHECKING
from urllib.parse import urlsplit

from kartenn._core.problems import PROBLEM_URN, NetworkError
from kartenn._core.requests import idempotency_key, traceparent

if TYPE_CHECKING:
    from collections.abc import Callable

    from kartenn._core.messages import Headers, Request, Response
    from kartenn.transport.port import AsyncTransport, Transport

__all__ = ["AsyncSender", "RetryPolicy", "Sender", "idempotent", "retry_after"]

LOG = logging.getLogger("kartenn")
_WAIT = frozenset({HTTPStatus.TOO_MANY_REQUESTS, HTTPStatus.SERVICE_UNAVAILABLE})
_IN_PROGRESS = f"{PROBLEM_URN}idempotency-in-progress".encode()


@dataclass(frozen=True)
class RetryPolicy:
    replays: int = 3
    budget: float = 60.0
    base: float = 1.0


def retry_after(headers: Headers, *, now: datetime) -> float | None:
    """`Retry-After` in seconds (RFC 9110 §10.2.3): a number, or an HTTP date."""
    value = headers.get("Retry-After")
    if value is None:
        return None
    value = value.strip()
    if value.isdigit():
        return float(value)
    try:
        moment = parsedate_to_datetime(value)
    except (TypeError, ValueError):
        return None
    return max(0.0, (moment - now).total_seconds())


def _delay(  # noqa: PLR0913 - the whole state of one decision
    policy: RetryPolicy,
    request: Request,
    *,
    response: Response | None,
    error: NetworkError | None,
    attempt: int,
    waited: float,
    now: datetime,
) -> float | None:
    """How long to wait before replaying, or `None` to stop here."""
    if attempt >= policy.replays or not request.replayable:
        return None
    fallback = policy.base * 2**attempt
    if error is not None:
        delay = None if error.code == "client.tls" else fallback
    elif response is None:
        delay = None
    elif response.status in _WAIT or (
        response.status == HTTPStatus.CONFLICT
        and "Idempotency-Key" in request.headers
        and _IN_PROGRESS in response.content
    ):
        found = retry_after(response.headers, now=now)
        delay = fallback if found is None else found
    else:
        delay = None
    if delay is None or waited + delay > policy.budget:
        return None
    return delay


def idempotent(request: Request, key: str | None = None) -> Request:
    """The request with an `Idempotency-Key` (REQ-023-14): the one given, else the one it
    has, else a new one. Every replay of the request then carries the same key."""
    if key is not None:
        return request.with_header("Idempotency-Key", key)
    return request.with_default_header("Idempotency-Key", idempotency_key())


def _stamp(request: Request, agent: str) -> Request:
    return request.with_default_header("User-Agent", agent).with_default_header(
        "traceparent", traceparent()
    )


def _path(request: Request) -> str:
    return urlsplit(request.url).path or request.url


def _log(request: Request, outcome: str, started: float) -> None:
    elapsed = (time.monotonic() - started) * 1000
    LOG.debug("%s %s %s %.0f ms", request.method, _path(request), outcome, elapsed)


class Sender:
    """A transport with the headers of every request and the retries."""

    def __init__(
        self,
        transport: Transport,
        *,
        user_agent: str,
        policy: RetryPolicy | None = None,
        clock: Callable[[], datetime] = lambda: datetime.now(UTC),
    ) -> None:
        self.transport = transport
        self._agent = user_agent
        self._policy = policy or RetryPolicy()
        self._clock = clock

    def prepare(self, request: Request) -> Request:
        return _stamp(request, self._agent)

    def send(self, request: Request) -> Response:
        request = self.prepare(request)
        attempt, waited = 0, 0.0
        while True:
            started = time.monotonic()
            response, error = None, None
            try:
                response = self.transport.send(request)
            except NetworkError as failure:
                error = failure
            _log(
                request, error.code if error else str(response.status if response else ""), started
            )
            delay = _delay(
                self._policy,
                request,
                response=response,
                error=error,
                attempt=attempt,
                waited=waited,
                now=self._clock(),
            )
            if delay is None:
                if error is not None:
                    raise error
                assert response is not None  # noqa: S101 - one of the two is set
                return response
            self.transport.sleep(delay)
            attempt, waited = attempt + 1, waited + delay


class AsyncSender:
    """The asynchronous `Sender`."""

    def __init__(
        self,
        transport: AsyncTransport,
        *,
        user_agent: str,
        policy: RetryPolicy | None = None,
        clock: Callable[[], datetime] = lambda: datetime.now(UTC),
    ) -> None:
        self.transport = transport
        self._agent = user_agent
        self._policy = policy or RetryPolicy()
        self._clock = clock

    def prepare(self, request: Request) -> Request:
        return _stamp(request, self._agent)

    async def send(self, request: Request) -> Response:
        request = self.prepare(request)
        attempt, waited = 0, 0.0
        while True:
            started = time.monotonic()
            response, error = None, None
            try:
                response = await self.transport.send(request)
            except NetworkError as failure:
                error = failure
            _log(
                request, error.code if error else str(response.status if response else ""), started
            )
            delay = _delay(
                self._policy,
                request,
                response=response,
                error=error,
                attempt=attempt,
                waited=waited,
                now=self._clock(),
            )
            if delay is None:
                if error is not None:
                    raise error
                assert response is not None  # noqa: S101 - one of the two is set
                return response
            await self.transport.sleep(delay)
            attempt, waited = attempt + 1, waited + delay
