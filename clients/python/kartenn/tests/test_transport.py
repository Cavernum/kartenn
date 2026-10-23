import logging
from collections.abc import Callable, Iterator
from datetime import UTC, datetime
from email.utils import format_datetime

import httpx2
import pytest

from kartenn._core.messages import Headers, Request
from kartenn._core.problems import ClientError, NetworkError
from kartenn._core.sse import RawEvent  # noqa: TC001 - a test needs no type-checking block
from kartenn.retry import AsyncSender, RetryPolicy, Sender, idempotent, retry_after
from kartenn.transport.httpx2 import AsyncHttpx2Transport, Httpx2Transport, use_http2

NOW = datetime(2026, 10, 23, 12, 0, tzinfo=UTC)
Handler = Callable[[httpx2.Request], httpx2.Response]


class Recorder:
    """A server simulated by a list of answers, and the requests it saw."""

    def __init__(self, *answers: httpx2.Response | Exception) -> None:
        self.answers = list(answers)
        self.seen: list[httpx2.Request] = []

    def __call__(self, request: httpx2.Request) -> httpx2.Response:
        self.seen.append(request)
        answer = self.answers.pop(0) if len(self.answers) > 1 else self.answers[0]
        if isinstance(answer, Exception):
            raise answer
        return answer


class Sleepless(Httpx2Transport):
    def __init__(self, handler: Handler) -> None:
        super().__init__("https://kartenn.test", mock=httpx2.MockTransport(handler))
        self.slept: list[float] = []

    def sleep(self, seconds: float) -> None:
        self.slept.append(seconds)


class AsyncSleepless(AsyncHttpx2Transport):
    def __init__(self, handler: Handler) -> None:
        super().__init__("https://kartenn.test", mock=httpx2.MockTransport(handler))
        self.slept: list[float] = []

    async def sleep(self, seconds: float) -> None:
        self.slept.append(seconds)


def sender(handler: Handler) -> tuple[Sender, Sleepless]:
    transport = Sleepless(handler)
    return Sender(
        transport, user_agent="kartenn-python/t", policy=RetryPolicy(), clock=lambda: NOW
    ), transport


def unavailable(after: str | None = "1") -> httpx2.Response:
    headers = {} if after is None else {"Retry-After": after}
    return httpx2.Response(503, headers=headers, json={"type": "urn:kartenn:problem:unavailable"})


def test_503_then_200_waits_retry_after() -> None:
    server = Recorder(unavailable("1"), httpx2.Response(200, json={"ok": True}))
    send, transport = sender(server)
    response = send.send(Request("GET", "/v1/capabilities"))
    assert response.status == 200
    assert response.json() == {"ok": True}
    assert transport.slept == [1.0]
    assert len(server.seen) == 2
    first, second = server.seen
    assert first.headers["user-agent"] == "kartenn-python/t"
    assert first.headers["traceparent"] == second.headers["traceparent"]


def test_retry_after_as_an_http_date() -> None:
    later = format_datetime(datetime(2026, 10, 23, 12, 0, 7, tzinfo=UTC), usegmt=True)
    server = Recorder(httpx2.Response(429, headers={"Retry-After": later}), httpx2.Response(200))
    send, transport = sender(server)
    assert send.send(Request("GET", "/v1/x")).status == 200
    assert transport.slept == [7.0]


def test_retry_after_parsing() -> None:
    assert retry_after(Headers(), now=NOW) is None
    assert retry_after(Headers.of({"Retry-After": "soon"}), now=NOW) is None
    past = format_datetime(datetime(2026, 1, 1, tzinfo=UTC), usegmt=True)
    assert retry_after(Headers.of({"Retry-After": past}), now=NOW) == 0.0


def test_post_without_key_is_never_replayed() -> None:
    server = Recorder(unavailable())
    send, transport = sender(server)
    response = send.send(Request("POST", "/v1/changesets", content=b"{}"))
    assert response.status == 503
    assert len(server.seen) == 1
    assert transport.slept == []


def test_post_with_key_is_replayed_with_the_same_key() -> None:
    server = Recorder(unavailable(None), httpx2.Response(201))
    send, transport = sender(server)
    response = send.send(idempotent(Request("POST", "/v1/changesets", content=b"{}")))
    assert response.status == 201
    keys = {request.headers["idempotency-key"] for request in server.seen}
    assert len(keys) == 1
    assert transport.slept == [1.0]
    assert idempotent(Request("POST", "/x"), "k").headers.get("Idempotency-Key") == "k"


def test_idempotency_in_progress_is_awaited() -> None:
    busy = httpx2.Response(
        409, json={"type": "urn:kartenn:problem:idempotency-in-progress", "status": 409}
    )
    server = Recorder(busy, httpx2.Response(201))
    send, _ = sender(server)
    assert send.send(idempotent(Request("POST", "/v1/prefixes"))).status == 201
    plain = Recorder(httpx2.Response(409, json={"type": "urn:kartenn:problem:conflict"}))
    send, _ = sender(plain)
    assert send.send(idempotent(Request("POST", "/v1/prefixes"))).status == 409
    assert len(plain.seen) == 1


def test_at_most_three_replays_within_the_budget() -> None:
    server = Recorder(unavailable(None))
    send, transport = sender(server)
    assert send.send(Request("GET", "/v1/x")).status == 503
    assert transport.slept == [1.0, 2.0, 4.0]
    assert len(server.seen) == 4
    late = Recorder(unavailable("61"))
    send, transport = sender(late)
    assert send.send(Request("GET", "/v1/x")).status == 503
    assert transport.slept == []


def test_network_failures() -> None:
    down = httpx2.ConnectError("refused")
    server = Recorder(down, httpx2.Response(200))
    send, transport = sender(server)
    assert send.send(Request("GET", "/v1/x")).status == 200
    assert transport.slept == [1.0]
    send, _ = sender(Recorder(down))
    with pytest.raises(NetworkError) as caught:
        send.send(Request("POST", "/v1/x"))
    assert caught.value.code == "client.network"
    send, _ = sender(Recorder(httpx2.ReadTimeout("slow")))
    with pytest.raises(NetworkError) as caught:
        send.send(Request("GET", "/v1/x"))
    assert caught.value.code == "client.timeout"


def test_requests_as_sent() -> None:
    server = Recorder(httpx2.Response(200, json={}))
    send, _ = sender(server)
    send.send(Request("QUERY", "/v1/prefixes", content=b'{"filters":[]}'))
    send.send(Request("GET", "/v1/prefixes?cursor=abc"))
    send.send(Request("GET", "/v1/prefixes", (("domain", "a"), ("domain", "b"))))
    query, link, listing = server.seen
    assert query.method == "QUERY"
    assert query.content == b'{"filters":[]}'
    assert str(link.url) == "https://kartenn.test/v1/prefixes?cursor=abc"
    assert str(listing.url) == "https://kartenn.test/v1/prefixes?domain=a&domain=b"


def test_no_header_nor_query_in_the_log(caplog: pytest.LogCaptureFixture) -> None:
    server = Recorder(httpx2.Response(200))
    send, _ = sender(server)
    with caplog.at_level(logging.DEBUG, logger="kartenn"):
        send.send(
            Request(
                "GET", "/v1/me?token=s3cret", headers=Headers.of({"Authorization": "Bearer t0k"})
            )
        )
    assert "GET /v1/me 200" in caplog.text
    assert "s3cret" not in caplog.text
    assert "t0k" not in caplog.text


def event_stream(
    body: bytes, status: int = 200, kind: str = "text/event-stream"
) -> httpx2.Response:
    return httpx2.Response(status, headers={"Content-Type": kind}, content=body)


EVENTS = (
    b"retry: 2000\nevent: ready\nid: k1.A\ndata: {}\n\n: ping\n\nid: k1.B\n\n"
    b'event: changes\nid: k1.C\ndata: {"changeset":"c"}\n\n'
)


def test_stream_and_sse() -> None:
    server = Recorder(httpx2.Response(200, content=b"line1\nline2\n"))
    transport = Httpx2Transport("https://kartenn.test", mock=httpx2.MockTransport(server))
    with transport.stream(Request("GET", "/v1/x")) as answer:
        assert answer.status == 200
        assert b"".join(answer.chunks) == b"line1\nline2\n"
    failing = Httpx2Transport(
        "https://kartenn.test",
        mock=httpx2.MockTransport(Recorder(httpx2.Response(404, content=b"gone"))),
    )
    with failing.stream(Request("GET", "/v1/x")) as answer:
        assert (answer.status, answer.content, list(answer.chunks)) == (404, b"gone", [])
    events = Httpx2Transport(
        "https://kartenn.test", mock=httpx2.MockTransport(Recorder(event_stream(EVENTS)))
    )
    with events.sse(Request("GET", "/v1/events")) as stream:
        raw = list(stream.events)
    assert [(e.event, e.id) for e in raw] == [
        ("ready", "k1.A"),
        ("message", "k1.B"),
        ("changes", "k1.C"),
    ]
    assert raw[0].retry == 2000
    transport.close()
    failing.close()
    events.close()


@pytest.mark.parametrize(
    "answer",
    [
        event_stream(b'{"status":429}', 429, "application/problem+json"),
        event_stream(b"<html>", 200, "text/html"),
    ],
)
def test_sse_refusals_keep_their_body(answer: httpx2.Response) -> None:
    transport = Httpx2Transport("https://kartenn.test", mock=httpx2.MockTransport(Recorder(answer)))
    with transport.sse(Request("GET", "/v1/events")) as stream:
        assert stream.status == answer.status_code
        assert stream.content == answer.content
        assert list(stream.events) == []


def test_cut_stream_is_a_network_error() -> None:
    def cut(_: httpx2.Request) -> httpx2.Response:
        def body() -> Iterator[bytes]:
            yield b"event: ready\ndata: {}\n\n"
            raise httpx2.ReadError("cut")

        return httpx2.Response(200, headers={"Content-Type": "text/event-stream"}, content=body())

    transport = Httpx2Transport("https://kartenn.test", mock=httpx2.MockTransport(cut))
    seen: list[RawEvent] = []
    with pytest.raises(NetworkError), transport.sse(Request("GET", "/v1/events")) as stream:
        seen.extend(stream.events)
    assert [event.event for event in seen] == ["ready"]


def no_module(_name: str) -> None:
    return None


def some_module(_name: str) -> object:
    return object()


def test_http2_choice(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("importlib.util.find_spec", no_module)
    assert use_http2(None) is False
    assert use_http2(wanted=False) is False
    with pytest.raises(ClientError, match="h2"):
        use_http2(wanted=True)
    monkeypatch.setattr("importlib.util.find_spec", some_module)
    assert use_http2(None) is True


@pytest.mark.anyio
async def test_async_sender_and_streams() -> None:
    server = Recorder(unavailable("2"), httpx2.Response(200, json={"ok": 1}))
    transport = AsyncSleepless(server)
    send = AsyncSender(transport, user_agent="ua", clock=lambda: NOW)
    assert (await send.send(Request("GET", "/v1/x"))).json() == {"ok": 1}
    assert transport.slept == [2.0]
    post = AsyncSender(AsyncSleepless(Recorder(unavailable())), user_agent="ua")
    assert (await post.send(Request("POST", "/v1/x"))).status == 503
    down = AsyncSender(AsyncSleepless(Recorder(httpx2.ConnectError("no"))), user_agent="ua")
    with pytest.raises(NetworkError):
        await down.send(Request("POST", "/v1/x"))

    lines = AsyncHttpx2Transport(
        "https://kartenn.test",
        mock=httpx2.MockTransport(Recorder(httpx2.Response(200, content=b"abc"))),
    )
    async with lines.stream(Request("GET", "/v1/x")) as answer:
        assert b"".join([chunk async for chunk in answer.chunks]) == b"abc"
    gone = AsyncHttpx2Transport(
        "https://kartenn.test",
        mock=httpx2.MockTransport(Recorder(httpx2.Response(410, content=b"x"))),
    )
    async with gone.stream(Request("GET", "/v1/x")) as answer:
        assert (answer.status, answer.content) == (410, b"x")
        assert [chunk async for chunk in answer.chunks] == []
    events = AsyncHttpx2Transport(
        "https://kartenn.test", mock=httpx2.MockTransport(Recorder(event_stream(EVENTS)))
    )
    async with events.sse(Request("GET", "/v1/events")) as stream:
        kinds = [event.event async for event in stream.events]
    assert kinds == ["ready", "message", "changes"]
    refused = AsyncHttpx2Transport(
        "https://kartenn.test",
        mock=httpx2.MockTransport(Recorder(event_stream(b"{}", 503, "application/json"))),
    )
    async with refused.sse(Request("GET", "/v1/events")) as stream:
        assert (stream.status, stream.content) == (503, b"{}")
        assert [event async for event in stream.events] == []
    real_sleep = AsyncHttpx2Transport("https://kartenn.test")
    await real_sleep.sleep(0)
    for each in (lines, gone, events, refused, real_sleep):
        await each.aclose()
