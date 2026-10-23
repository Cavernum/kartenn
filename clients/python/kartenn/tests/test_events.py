from collections.abc import AsyncIterator, Iterator
from itertools import islice
from pathlib import Path

import httpx2
import pytest

from fakes import URL, FakeServer, capabilities, problem
from kartenn import BadRequest, Client, ClientError, NetworkError, Unauthenticated
from kartenn._core.messages import Request, Response
from kartenn._core.plans import Plan
from kartenn.catalog import CatalogCache
from kartenn.client import AsyncClient
from kartenn.transport.httpx2 import AsyncHttpx2Transport, Httpx2Transport

STREAMING = capabilities(events={"heartbeat": 15, "ring_seconds": 900})


def stream(*blocks: bytes, cut: bool = False) -> httpx2.Response:
    def body() -> Iterator[bytes]:
        yield from blocks
        if cut:
            raise httpx2.ReadError("cut")

    return httpx2.Response(200, headers={"Content-Type": "text/event-stream"}, content=body())


FIRST = (
    b"retry: 1000\nevent: ready\nid: k1.A\ndata: {}\n\n",
    b'event: changes\nid: k1.B\ndata: {"changeset":"C1","objects":[]}\n\n',
)
SECOND = (
    b'event: changes\nid: k1.B2\ndata: {"changeset":"C1","objects":[]}\n\n',
    b": ping\n\nid: k1.B3\n\n",
    b'event: reset\nid: k1.C\ndata: {"reason":"overflow"}\n\n',
    b'event: changes\nid: k1.D\ndata: {"changeset":"C2","objects":[]}\n\n',
)


def astream(*blocks: bytes, cut: bool = False) -> httpx2.Response:
    async def body() -> AsyncIterator[bytes]:
        for block in blocks:
            yield block
        if cut:
            raise httpx2.ReadError("cut")

    return httpx2.Response(200, headers={"Content-Type": "text/event-stream"}, content=body())


class Sleepless(Httpx2Transport):
    def __init__(self, server: FakeServer) -> None:
        super().__init__(URL, mock=httpx2.MockTransport(server))
        self.slept: list[float] = []

    def sleep(self, seconds: float) -> None:
        self.slept.append(seconds)


class AsyncSleepless(AsyncHttpx2Transport):
    def __init__(self, server: FakeServer) -> None:
        super().__init__(URL, mock=httpx2.MockTransport(server))
        self.slept: list[float] = []

    async def sleep(self, seconds: float) -> None:
        self.slept.append(seconds)


def sequence(server: FakeServer, *answers: httpx2.Response) -> None:
    queue = list(answers)
    server.route("GET", "/v1/events", lambda _request: queue.pop(0))


def test_resume_after_a_cut_without_duplicates(tmp_path: Path) -> None:
    server = FakeServer(caps=STREAMING)
    sequence(
        server,
        httpx2.Response(429, headers={"Retry-After": "2"}, json={"status": 429}),
        stream(*FIRST, cut=True),
        stream(*SECOND),
    )
    transport = Sleepless(server)
    with Client(URL, transport=transport, cache=CatalogCache(tmp_path)) as kn:
        found = list(islice(kn.events(types=["core.prefix"], kinds=["changes", "reset"]), 4))
    assert [(e.kind, e.id) for e in found] == [
        ("ready", "k1.A"),
        ("changes", "k1.B"),
        ("reset", "k1.C"),
        ("changes", "k1.D"),
    ]
    assert found[2].data == {"reason": "overflow"}
    first, opened, resumed = server.seen
    assert first.url.params.get("types") == "core.prefix"
    assert first.url.params.get("kinds") == "changes,reset"
    assert first.headers["accept"].startswith("text/event-stream")
    assert "last-event-id" not in opened.headers
    assert resumed.headers["last-event-id"] == "k1.B"
    assert transport.slept[0] == 2.0
    assert 0.8 <= transport.slept[1] <= 1.2


def test_last_event_id_given_and_end_without_reconnect(tmp_path: Path) -> None:
    server = FakeServer(caps=STREAMING)
    sequence(server, stream(*SECOND))
    with Client(URL, transport=Sleepless(server), cache=CatalogCache(tmp_path)) as kn:
        found = list(kn.events(last_event_id="k1.Z", reconnect=False))
    assert [e.id for e in found] == ["k1.B2", "k1.C", "k1.D"]
    assert server.seen[0].headers["last-event-id"] == "k1.Z"


def test_errors_before_opening(tmp_path: Path) -> None:
    server = FakeServer(caps=STREAMING)
    sequence(server, problem(400, "bad-request"))
    with Client(URL, transport=Sleepless(server), cache=CatalogCache(tmp_path)) as kn:
        with pytest.raises(BadRequest):
            next(kn.events())
        sequence(server, problem(401, "unauthenticated"))
        with pytest.raises(Unauthenticated):
            next(kn.events())


def test_absent_stream(tmp_path: Path) -> None:
    server = FakeServer()
    with Client(URL, transport=Sleepless(server), cache=CatalogCache(tmp_path)) as kn:
        with pytest.raises(ClientError) as caught:
            kn.events()
        assert caught.value.code == "client.operation-unavailable"


class Renewing:
    """An authentication that renews its token once on a 401."""

    def __init__(self) -> None:
        self.bearer = "old"

    def authorize(self, request: Request) -> Plan[Request]:
        return request.with_header("Authorization", f"Bearer {self.bearer}")
        yield  # pragma: no cover

    def unauthorized(self, response: Response) -> Plan[bool]:  # noqa: ARG002
        self.bearer = "new"
        return True
        yield  # pragma: no cover

    def check_server(self, url: str) -> None:
        pass


def test_401_renews_then_opens(tmp_path: Path) -> None:
    server = FakeServer(caps=STREAMING)
    sequence(server, problem(401, "unauthenticated"), stream(*FIRST))
    with Client(
        URL, auth=Renewing(), transport=Sleepless(server), cache=CatalogCache(tmp_path)
    ) as kn:
        found = list(kn.events(reconnect=False))
    assert [e.kind for e in found] == ["ready", "changes"]
    assert [r.headers["authorization"] for r in server.seen] == ["Bearer old", "Bearer new"]


def test_tls_failure_is_not_resumed(tmp_path: Path) -> None:
    server = FakeServer(caps=STREAMING)

    def tls(_request: httpx2.Request) -> httpx2.Response:
        import ssl  # noqa: PLC0415

        raise httpx2.ConnectError("tls") from ssl.SSLError("certificate verify failed")

    server.route("GET", "/v1/events", tls)
    with Client(URL, transport=Sleepless(server), cache=CatalogCache(tmp_path)) as kn:
        with pytest.raises(NetworkError) as caught:
            next(kn.events())
        assert caught.value.code == "client.tls"


@pytest.mark.anyio
async def test_async_events(tmp_path: Path) -> None:
    server = FakeServer(caps=STREAMING)
    sequence(
        server,
        problem(401, "unauthenticated"),
        httpx2.Response(503, headers={"Retry-After": "1"}),
        astream(*FIRST, cut=True),
        astream(*SECOND),
        problem(400, "bad-request"),
    )
    transport = AsyncSleepless(server)
    kn = AsyncClient(URL, auth=Renewing(), transport=transport, cache=CatalogCache(tmp_path))
    found: list[str | None] = []
    async for event in kn.aevents(objects=["p1"]):
        found.append(event.id)
        if len(found) == 4:
            break
    assert found == ["k1.A", "k1.B", "k1.C", "k1.D"]
    assert transport.slept[0] == 1.0
    assert server.seen[3].headers["last-event-id"] == "k1.B"
    with pytest.raises(BadRequest):
        async for _ in kn.aevents(reconnect=False):
            pass
    sequence(server, problem(401, "unauthenticated"), problem(401, "unauthenticated"))
    with pytest.raises(Unauthenticated):
        async for _ in kn.aevents(reconnect=False):
            pass
    sequence(server, astream(*SECOND))
    assert len([e async for e in kn.aevents(reconnect=False)]) == 3
    absent = AsyncClient(
        URL, transport=AsyncSleepless(FakeServer()), cache=CatalogCache(tmp_path / "x")
    )
    with pytest.raises(ClientError):
        async for _ in absent.aevents():
            pass
    await kn.aclose()
    await absent.aclose()
