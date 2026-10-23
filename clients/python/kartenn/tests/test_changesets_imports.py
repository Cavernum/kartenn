import json
import tracemalloc
from collections.abc import Iterator
from contextlib import AbstractContextManager
from pathlib import Path
from typing import Any

import httpx2
import pytest

from fakes import CONTRACT, URL, FakeServer, body, capabilities, problem
from kartenn import (
    Applied,
    Client,
    ClientError,
    ImportProgress,
    ImportRefused,
    Pending,
    Refused,
    file_digest,
    ndjson_elements,
)
from kartenn._core.messages import Headers, Request, Response
from kartenn.catalog import CatalogCache
from kartenn.transport.port import EventStream, StreamResponse

APPLIED: dict[str, Any] = {
    "changeset": {"id": "c1", "href": "/v1/changesets/c1"},
    "state": "applied",
    "warnings": [],
    "diff": [],
    "cascaded": [],
}
PENDING: dict[str, Any] = {
    "changeset": {"id": "c2", "href": "/v1/changesets/c2"},
    "state": "pending",
    "kind": "proposal",
    "diff": [],
}
SIMULATED: dict[str, Any] = {
    "valid": False,
    "violations": [
        {"rule": "core.prefix.overlap", "severity": "error", "detail": "x"},
        {"rule": "core.prefix.unused", "severity": "warning", "detail": "y"},
    ],
    "diff": [{"kind": "prefix", "created": 1}],
    "sample": [],
    "cascaded": [],
    "sections": [{"section": "policy.combinations", "rows": []}],
}


def test_simulate_then_submit(tmp_path: Path) -> None:
    server = FakeServer()

    def submit(request: httpx2.Request) -> httpx2.Response:
        if request.url.params.get("simulate") == "true":
            return httpx2.Response(200, json=SIMULATED)
        return httpx2.Response(201, json=APPLIED, headers={"Location": "/v1/changesets/c1"})

    server.route("POST", "/v1/changesets", submit)
    with server.client(tmp_path) as kn:
        cs = kn.changeset(
            motive="lot 3", valid_from="2026-11-01T00:00:00+01:00", on_behalf_of="alice"
        )
        domain = cs.add({"op": "create_domain", "slug": "lot3"})
        prefix = cs.create("prefixes", domain={"local": domain}, value="198.51.100.0/24")
        assert (domain, prefix) == (0, 1)
        report = cs.simulate()
        assert not report.valid
        assert [v.rule for v in report.blocking] == ["core.prefix.overlap"]
        assert report.sections[0]["section"] == "policy.combinations"
        result = cs.submit()
        assert isinstance(result, Applied)
        assert result.location == "/v1/changesets/c1"
        cs.submit()
    simulated, first, again = server.seen
    assert "idempotency-key" not in simulated.headers
    assert (
        first.headers["idempotency-key"] == again.headers["idempotency-key"] == cs.idempotency_key
    )
    assert first.headers["kartenn-on-behalf-of"] == "alice"
    sent = body(first)
    assert sent["motive"] == "lot 3"
    assert sent["valid_from"] == "2026-11-01T00:00:00+01:00"
    assert sent["operations"][1] == {
        "op": "create_prefix",
        "domain": {"local": 0},
        "value": "198.51.100.0/24",
    }
    assert len(cs.operations) == 2


def test_pending_and_refused(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("POST", "/v1/changesets", httpx2.Response(202, json=PENDING))
    with server.client(tmp_path) as kn:
        cs = kn.changeset(motive="m", valid_to="2027-01-01T00:00:00Z")
        cs.add({"op": "create_domain", "slug": "x"})
        result = cs.submit()
        assert isinstance(result, Pending)
        assert (result.changeset, result.kind) == ("c2", "proposal")
        assert body(server.seen[-1])["valid_to"] == "2027-01-01T00:00:00Z"
        server.route(
            "POST",
            "/v1/changesets",
            problem(
                422,
                "changeset-refused",
                violations=[
                    {"rule": "r", "severity": "error", "detail": "d", "pointer": "/operations/0"}
                ],
            ),
        )
        with pytest.raises(Refused) as caught:
            cs.submit()
        assert caught.value.violations[0].pointer == "/operations/0"
        server.route("POST", "/v1/changesets", httpx2.Response(200, json={"odd": True}))
        with pytest.raises(ClientError):
            cs.submit()


def test_local_refusals(tmp_path: Path) -> None:
    small = capabilities()
    small["limits"] = {**small["limits"], "changeset_operations": 2}
    server = FakeServer(caps=small)
    with server.client(tmp_path) as kn:
        with pytest.raises(ClientError) as unknown:
            kn.changeset(motive="m").add({"slug": "x"})
        assert unknown.value.code == "client.unknown-operation"
        cs = kn.changeset(motive="m")
        cs.add({"op": "fly_away"})
        with pytest.raises(ClientError) as kind:
            cs.submit()
        assert kind.value.code == "client.unknown-operation"
        absent = kn.changeset(motive="m")
        absent.add({"op": "demo.create_widget"})
        with pytest.raises(ClientError) as module:
            absent.submit()
        assert module.value.code == "client.unknown-operation"
        big = kn.changeset(motive="m")
        for n in range(3):
            big.add({"op": "create_domain", "slug": f"d{n}"})
        with pytest.raises(ClientError) as large:
            big.simulate()
        assert large.value.code == "client.changeset-too-large"
        assert "import" in str(large.value)
        nothing = kn.changeset(motive="m")
        nothing.create("anomalies", motive="x")
        with pytest.raises(ClientError):
            nothing.submit()
    assert server.seen == []


def test_an_operation_of_a_module_served_is_sent(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("POST", "/v1/changesets", httpx2.Response(200, json=SIMULATED))
    with server.client(tmp_path) as kn:
        cs = kn.changeset(motive="m")
        cs.add({"op": "vlan.create_vlan", "vid": 140})
        assert not cs.simulate().valid
    assert body(server.seen[-1])["operations"] == [{"op": "vlan.create_vlan", "vid": 140}]


def test_actions_on_an_existing_changeset(tmp_path: Path) -> None:
    server = FakeServer()
    shown = {"id": "c2", "state": "pending", "links": {"self": "/v1/changesets/c2"}}
    server.route(
        "GET", "/v1/changesets/{id}", httpx2.Response(200, json=shown, headers={"ETag": '"c2.aa"'})
    )
    decided = {"changeset": {"id": "c2", "href": "/v1/changesets/c2"}, "state": "applied"}
    for action in ("validation", "rejection", "withdrawal", "undo"):
        server.route("POST", f"/v1/changesets/{{id}}/{action}", httpx2.Response(200, json=decided))

    def diff(request: httpx2.Request) -> httpx2.Response:
        if "cursor" in request.url.params:
            return httpx2.Response(
                200,
                json={"items": [{"section": "flow.flows", "kind": "b"}], "links": {"next": None}},
            )
        return httpx2.Response(
            200,
            json={
                "items": [{"section": "flow.flows", "kind": "a"}],
                "links": {"next": "/v1/changesets/c2/diff?cursor=x"},
            },
        )

    server.route("GET", "/v1/changesets/{id}/diff", diff)
    with server.client(tmp_path) as kn:
        handle = kn.changeset_of("c2")
        assert handle.id == "c2"
        assert handle.read()["state"] == "pending"
        handle.approve([{"op": "create_domain", "slug": "x"}])
        handle.reject("non")
        handle.withdraw()
        handle.undo(simulate=True)
        rows = [row["kind"] for row in handle.diff(section="flow.flows")]
        assert rows == ["a", "b"]
        from_record = kn.changeset_of({"id": "c2"})
        assert from_record.id == "c2"
    actions = [r for r in server.seen if r.method == "POST"]
    assert all(r.headers["if-match"] == '"c2.aa"' for r in actions)
    assert body(actions[0]) == {"operations": [{"op": "create_domain", "slug": "x"}]}
    assert body(actions[1]) == {"motive": "non"}
    assert actions[3].url.params.get("simulate") == "true"
    diff_first = next(r for r in server.seen if r.url.path.endswith("/diff"))
    assert diff_first.url.params.get("section") == "flow.flows"


class ImportServer(FakeServer):
    """The import routes, a chunk refused once on demand."""

    def __init__(self) -> None:
        super().__init__()
        self.chunks: list[int] = []
        self.fail_at: int | None = None
        self.states = ["sorting", "done"]
        self.route(
            "POST", "/v1/imports", httpx2.Response(201, json={"id": "i1", "state": "uploading"})
        )
        self.route("PUT", "/v1/imports/{id}/chunks/{n}", self.chunk)
        self.route("POST", "/v1/imports/{id}/start", httpx2.Response(202))
        self.route("GET", "/v1/imports/{id}", self.show)
        self.error: dict[str, Any] | None = None

    def chunk(self, request: httpx2.Request) -> httpx2.Response:
        number = int(request.url.path.rsplit("/", 1)[-1])
        if number == self.fail_at:
            return problem(500, "internal")
        self.chunks.append(number)
        return httpx2.Response(201)

    def show(self, _request: httpx2.Request) -> httpx2.Response:
        if self.error is not None:
            return httpx2.Response(
                200, json={"id": "i1", "state": "uploading", "error": self.error}
            )
        state = self.states.pop(0) if len(self.states) > 1 else self.states[0]
        return httpx2.Response(200, json={"id": "i1", "state": state}, headers={"Retry-After": "0"})


def elements(count: int) -> Iterator[dict[str, Any]]:
    for n in range(count):
        yield {"external_id": f"e{n}", "operation": {"op": "create_domain", "slug": f"d{n}"}}


def test_import_by_chunks(tmp_path: Path) -> None:
    server = ImportServer()
    seen: list[ImportProgress] = []
    with server.client(tmp_path / "cache") as kn:
        session = kn.import_session(
            "dns.ndjson",
            digest="abc",
            chunk_elements=10,
            state_dir=tmp_path / "state",
            on_progress=seen.append,
        )
        assert session.run(elements(25)) == "i1"
        assert session.import_id == "i1"
        assert not session.state_path.exists()
        assert session.wait()["state"] == "done"
    assert server.chunks == [0, 1, 2]
    assert [(p.chunks, p.elements) for p in seen] == [(1, 10), (2, 20), (3, 25)]
    put = next(r for r in server.seen if r.method == "PUT")
    assert len(body(put)["elements"]) == 10


def test_resume_sends_only_the_chunks_left(tmp_path: Path) -> None:
    server = ImportServer()
    server.fail_at = 40
    state = tmp_path / "state"
    with server.client(tmp_path / "cache") as kn:
        first = kn.import_session("big.ndjson", digest="d1", chunk_elements=2, state_dir=state)
        with pytest.raises(Exception, match="internal"):
            first.run(elements(200))
        assert first.state_path.exists()
        assert server.chunks == list(range(40))
        server.fail_at = None
        changed = kn.import_session("big.ndjson", digest="d2", chunk_elements=2, state_dir=state)
        with pytest.raises(ClientError) as caught:
            changed.resume(elements(200))
        assert caught.value.code == "client.import-source-changed"
        again = kn.import_session("big.ndjson", digest="d1", state_dir=state)
        again.resume(elements(200))
        with pytest.raises(ClientError) as nothing:
            kn.import_session("other", digest="d1", state_dir=state).resume([])
        assert nothing.value.code == "client.no-import"
    assert server.chunks == list(range(100))
    starts = [r for r in server.seen if r.url.path.endswith("/start")]
    assert len(starts) == 1
    creations = [r for r in server.seen if r.method == "POST" and r.url.path == "/v1/imports"]
    assert len(creations) == 1


def test_refused_sort_names_the_source_line(tmp_path: Path) -> None:
    server = ImportServer()
    server.error = {
        "type": "urn:kartenn:problem:invalid-import",
        "title": "Invalid import",
        "status": 422,
        "violations": [
            {
                "rule": "import.unknown-reference",
                "severity": "error",
                "detail": "?",
                "pointer": "/elements/1",
            },
            {
                "rule": "import.cycle",
                "severity": "error",
                "detail": "?",
                "pointer": None,
                "element": 0,
            },
            {"rule": "import.other", "severity": "error", "detail": "?"},
            {"rule": "import.far", "severity": "error", "detail": "?", "pointer": "/9"},
        ],
    }
    source = tmp_path / "dns.ndjson"
    lines = [json.dumps(e) for e in elements(2)]
    source.write_text(f"{lines[0]}\n\n{lines[1]}\n")
    with server.client(tmp_path / "cache") as kn:
        session = kn.import_session(str(source), digest=file_digest(source), state_dir=tmp_path)
        session.run(ndjson_elements(source))
        with pytest.raises(ImportRefused) as caught:
            session.wait()
    assert caught.value.code == "invalid-import"
    assert caught.value.lines == (3, 1, None, 10)
    assert caught.value.violations[0].rule == "import.unknown-reference"


def test_sources(tmp_path: Path) -> None:
    source = tmp_path / "s.ndjson"
    source.write_text('{"a": 1}\n[1]\n')
    found = ndjson_elements(source)
    assert next(found) == (1, {"a": 1})
    with pytest.raises(ClientError, match=r"s\.ndjson:2"):
        next(found)
    assert len(file_digest(source)) == 64


class Silent:
    """A transport that answers without keeping anything: memory is the client's."""

    def __init__(self) -> None:
        self.chunks = 0

    def send(self, request: Request) -> Response:
        if request.url == "/v1/capabilities":
            return Response(200, content=json.dumps(capabilities()).encode())
        if request.url == "/v1/openapi.json":
            return Response(200, content=CONTRACT_BYTES)
        if request.method == "PUT":
            self.chunks += 1
            return Response(201)
        if request.url == "/v1/imports":
            return Response(201, Headers(), b'{"id":"i1"}')
        return Response(202)

    def stream(self, request: Request) -> AbstractContextManager[StreamResponse]:
        raise NotImplementedError

    def sse(self, request: Request) -> AbstractContextManager[EventStream]:
        raise NotImplementedError

    def sleep(self, seconds: float) -> None:
        pass

    def close(self) -> None:
        pass


CONTRACT_BYTES = json.dumps(CONTRACT).encode()


@pytest.mark.timeout(300)
def test_a_million_elements_in_bounded_memory(tmp_path: Path) -> None:
    transport = Silent()
    kn = Client(URL, transport=transport, cache=CatalogCache(tmp_path))
    kn.catalog  # noqa: B018 - the contract is not the import's memory
    session = kn.import_session("big", digest="x", state_dir=tmp_path)
    tracemalloc.start()
    try:
        session.run(elements(1_000_000))
        _, peak = tracemalloc.get_traced_memory()
    finally:
        tracemalloc.stop()
    assert transport.chunks == 100
    assert peak <= 100 * 1024 * 1024, peak


@pytest.mark.anyio
async def test_async_changesets_and_imports(tmp_path: Path) -> None:
    server = ImportServer()
    server.route(
        "GET",
        "/v1/changesets/{id}",
        httpx2.Response(200, json={"id": "c1"}, headers={"ETag": '"c1.bb"'}),
    )
    server.route("POST", "/v1/changesets/{id}/validation", httpx2.Response(200, json=APPLIED))
    server.route("POST", "/v1/changesets/{id}/rejection", httpx2.Response(200, json=APPLIED))
    server.route("POST", "/v1/changesets/{id}/withdrawal", httpx2.Response(200, json=APPLIED))
    server.route("POST", "/v1/changesets/{id}/undo", httpx2.Response(200, json=APPLIED))
    server.route(
        "GET",
        "/v1/changesets/{id}/diff",
        httpx2.Response(200, json={"items": [{"k": 1}], "links": {}}),
    )
    server.route(
        "POST",
        "/v1/changesets",
        lambda r: httpx2.Response(
            200 if r.url.params.get("simulate") else 201,
            json=SIMULATED if r.url.params.get("simulate") else APPLIED,
        ),
    )
    async with server.async_client(tmp_path / "cache") as kn:
        cs = kn.changeset(motive="m")
        cs.add({"op": "create_domain", "slug": "x"})
        cs.create("domains", slug="y")
        assert len(cs.operations) == 2
        assert cs.idempotency_key
        assert not (await cs.simulate()).valid
        assert isinstance(await cs.submit(), Applied)
        handle = kn.changeset_of("c1")
        assert handle.id == "c1"
        assert (await handle.read())["id"] == "c1"
        await handle.approve()
        await handle.reject("no")
        await handle.withdraw()
        await handle.undo()
        assert [row["k"] async for row in handle.diff()] == [1]
        session = kn.import_session("a", digest="b", chunk_elements=5, state_dir=tmp_path / "s")
        assert await session.run(elements(7)) == "i1"
        assert session.import_id == "i1"
        assert (await session.wait())["state"] == "done"
        server.error = {"type": "urn:kartenn:problem:invalid-import", "violations": []}
        with pytest.raises(ImportRefused):
            await session.wait()
        with pytest.raises(ClientError):
            await kn.import_session("z", digest="b", state_dir=tmp_path / "s").resume([])
    assert server.chunks == [0, 1]
