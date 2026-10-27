import json
import stat
from pathlib import Path
from typing import Any

import httpx2
import pytest

from fakes import URL, FakeServer, body, problem
from kartenn import (
    Applied,
    Client,
    ClientError,
    FileStore,
    MemoryStore,
    Pending,
    PreconditionFailed,
    Record,
    Refused,
    Simulation,
    Unauthenticated,
    UnknownResource,
    between,
)
from kartenn.auth.bearer import BearerToken
from kartenn.catalog import CatalogCache

PREFIX = {
    "id": "7c1e0000-0000-4000-8000-000000000001",
    "domain": {"id": "d1", "slug": "interne", "href": "/v1/domains/d1"},
    "value": "192.0.2.0/24",
    "nature": "network",
    "links": {"self": "/v1/prefixes/7c1e", "history": "/v1/prefixes/7c1e/history"},
}
APPLIED: dict[str, Any] = {
    "changeset": {"id": "c2", "href": "/v1/changesets/c2"},
    "state": "applied",
    "warnings": [],
    "diff": [],
    "cascaded": [],
}


def object_answer(etag: str = '"c1.00ff00ff00ff00ff"') -> httpx2.Response:
    return httpx2.Response(200, json=PREFIX, headers={"ETag": etag})


def test_get_keeps_the_etag_and_update_sends_it(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("GET", "/v1/prefixes/{id}", object_answer())
    server.route("PATCH", "/v1/prefixes/{id}", httpx2.Response(200, json=APPLIED))
    with server.client(tmp_path) as kn:
        prefix = kn.prefixes.get(PREFIX["id"])
        assert isinstance(prefix, Record)
        assert prefix["value"] == "192.0.2.0/24"
        assert prefix.etag == '"c1.00ff00ff00ff00ff"'
        assert prefix.id == PREFIX["id"]
        assert prefix.links["history"].endswith("/history")
        result = kn.prefixes.update(prefix, {"nature": "container"}, motive="regroupement")
    assert isinstance(result, Applied)
    assert result.changeset == "c2"
    patch = server.seen[-1]
    assert patch.headers["if-match"] == '"c1.00ff00ff00ff00ff"'
    assert patch.headers["content-type"] == "application/merge-patch+json"
    assert body(patch) == {"nature": "container", "motive": "regroupement"}
    assert patch.url.path == f"/v1/prefixes/{PREFIX['id']}"


def test_412_gives_the_current_etag(tmp_path: Path) -> None:
    server = FakeServer()
    stale = problem(412, "precondition-failed", violations=[])
    stale.headers["ETag"] = '"c9.1111111111111111"'
    server.route("PATCH", "/v1/prefixes/{id}", stale)
    stale_record = Record(PREFIX, etag='"c1.00ff00ff00ff00ff"')
    with server.client(tmp_path) as kn, pytest.raises(PreconditionFailed) as caught:
        kn.prefixes.update(stale_record, nature="container", motive="m")
    assert caught.value.current_etag == '"c9.1111111111111111"'
    assert caught.value.code == "precondition-failed"


def test_update_without_etag_is_refused_locally(tmp_path: Path) -> None:
    server = FakeServer()
    with server.client(tmp_path) as kn, pytest.raises(ClientError) as caught:
        kn.prefixes.update(PREFIX["id"], nature="container", motive="m")
    assert caught.value.code == "client.etag-required"
    assert server.seen == []


def test_close_without_motive_is_refused_locally(tmp_path: Path) -> None:
    server = FakeServer()
    with server.client(tmp_path) as kn:
        with pytest.raises(ClientError) as caught:
            kn.prefixes.close(Record(PREFIX, etag='"c1"'))
        assert caught.value.code == "client.motive-required"
        assert server.seen == []
        server.route("DELETE", "/v1/prefixes/{id}", httpx2.Response(200, json=APPLIED))
        kn.prefixes.close(
            Record(PREFIX, etag='"c1"'), motive="retrait", valid_from="2026-11-01T00:00:00Z"
        )
    sent = server.seen[-1]
    assert sent.headers["if-match"] == '"c1"'
    assert body(sent) == {"motive": "retrait", "valid_from": "2026-11-01T00:00:00Z"}


def test_a_write_simulated_writes_nothing(tmp_path: Path) -> None:
    """Spec 024, REQ-024-17: `simulate=True` sends the same write with `?simulate=true`
    and answers its simulation; a write that submits no changeset is refused locally."""
    server = FakeServer()
    simulated: dict[str, Any] = {
        "valid": False,
        "violations": [{"rule": "core.prefix.overlap", "severity": "error", "detail": "x"}],
        "diff": [],
        "sample": [],
        "cascaded": [],
    }
    server.route("POST", "/v1/prefixes", httpx2.Response(200, json=simulated))
    server.route("DELETE", "/v1/prefixes/{id}", httpx2.Response(200, json=simulated))
    with server.client(tmp_path) as kn:
        created = kn.prefixes.create(value="198.51.100.0/24", motive="m", simulate=True)
        closed = kn.prefixes.close(Record(PREFIX, etag='"c1"'), motive="m", simulate=True)
        with pytest.raises(ClientError) as caught:
            kn.call("acknowledge_anomaly", id=PREFIX["id"], body={"motive": "m"}, simulate=True)
    assert isinstance(created, Simulation)
    assert isinstance(closed, Simulation)
    assert not created.valid
    assert [v.rule for v in created.blocking] == ["core.prefix.overlap"]
    writes = [r for r in server.seen if r.method != "GET"]
    assert [(r.method, r.url.params["simulate"]) for r in writes] == [
        ("POST", "true"),
        ("DELETE", "true"),
    ]
    assert caught.value.code == "client.simulate-unsupported"


def test_create_carries_an_idempotency_key_and_answers_pending(tmp_path: Path) -> None:
    server = FakeServer()
    pending: dict[str, Any] = {
        "changeset": {"id": "c3", "href": "/v1/changesets/c3"},
        "state": "pending",
        "kind": "approval",
        "diff": [],
    }
    server.route(
        "POST",
        "/v1/prefixes",
        httpx2.Response(
            202,
            json=pending,
            headers={"Link": '</v1/changesets/c3>; rel="urn:kartenn:rel:changeset"'},
        ),
    )
    with server.client(tmp_path) as kn:
        result = kn.prefixes.create(
            {"domain": {"id": "d1"}}, value="198.51.100.0/24", nature="network", motive="lot 3"
        )
    assert isinstance(result, Pending)
    assert (result.changeset, result.kind, result.link) == ("c3", "approval", "/v1/changesets/c3")
    sent = server.seen[-1]
    assert len(sent.headers["idempotency-key"]) == 36
    assert body(sent) == {
        "domain": {"id": "d1"},
        "value": "198.51.100.0/24",
        "nature": "network",
        "motive": "lot 3",
    }


def test_refused_carries_violations(tmp_path: Path) -> None:
    server = FakeServer()
    server.route(
        "POST",
        "/v1/prefixes",
        problem(
            422,
            "changeset-refused",
            violations=[
                {"rule": "core.prefix.overlap", "severity": "error", "detail": "x", "pointer": ""}
            ],
        ),
    )
    with server.client(tmp_path) as kn, pytest.raises(Refused) as caught:
        kn.prefixes.create(value="198.51.100.0/24", motive="m", idempotency_key="k-1")
    assert caught.value.violations[0].rule == "core.prefix.overlap"
    assert server.seen[-1].headers["idempotency-key"] == "k-1"


def paged(total: int) -> Any:  # noqa: ANN401 - a handler
    def answer(request: httpx2.Request) -> httpx2.Response:
        size = int(request.url.params.get("page_size", "100"))
        cursor = request.url.params.get("cursor", "0")
        start = int(cursor) if cursor.isdigit() else 0
        items = [{"id": f"p{n}"} for n in range(start, min(start + size, total))]
        following = start + size
        link = f"/v1/prefixes?cursor={following}&page_size={size}" if following < total else None
        return httpx2.Response(200, json={"items": items, "links": {"next": link}})

    return answer


def test_list_follows_the_cursor(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("GET", "/v1/prefixes", paged(2500))
    with server.client(tmp_path) as kn:
        found = [record["id"] for record in kn.prefixes.list(page_size=1000)]
    assert len(found) == 2500
    assert len(set(found)) == 2500
    assert len(server.seen) == 3
    assert str(server.seen[1].url) == f"{URL}/v1/prefixes?cursor=1000&page_size=1000"


def test_list_filters_and_dates(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("GET", "/v1/prefixes", paged(1))
    server.route("QUERY", "/v1/prefixes", paged(1))
    with server.client(tmp_path) as kn:
        at = kn.at(valid_at="2026-11-01T00:00:00+01:00")
        listing = at.prefixes.list(domain="interne", prop={"dns.ttl": between(60, 3600)})
        first = listing.first_page()
        assert first.as_of is None
        with pytest.raises(ClientError) as caught:
            kn.prefixes.list(colour="red").first_page()
        assert caught.value.code == "client.unknown-filter"
        assert len(server.seen) == 1
        long = kn.prefixes.list(value=[f"10.{n // 256}.{n % 256}.0/24" for n in range(800)])
        assert len(long.first_page().items) == 1
        frozen = kn.prefixes.list(cursor="abc", page_size=5)
        frozen.first_page()
        with pytest.raises(ClientError):
            kn.prefixes.list(cursor="abc", domain="x").first_page()
        pinned = kn.prefixes.list(as_of="2026-01-01T00:00:00Z")
        assert pinned.first_page().as_of == "2026-01-01T00:00:00Z"
    sent = server.seen[0].url.params
    assert sent.get_list("domain") == ["interne"]
    assert sent.get("prop.dns.ttl:between") == "60..3600"
    assert sent.get("valid_at") == "2026-11-01T00:00:00+01:00"
    assert server.seen[1].method == "QUERY"
    assert len(body(server.seen[1])["filters"][0]["value"]) == 800
    assert str(server.seen[2].url) == f"{URL}/v1/prefixes?cursor=abc&page_size=5"


def test_verbs_sub_resources_and_call(tmp_path: Path) -> None:
    server = FakeServer()
    server.route(
        "GET",
        "/v1/prefixes/{id}/history",
        httpx2.Response(200, json={"items": [{"a": 1}], "links": {"next": None}}),
    )
    server.route(
        "GET",
        "/v1/prefixes/{id}/properties/{key}/explanation",
        httpx2.Response(200, json={"key": "dns.ttl"}),
    )
    server.route("POST", "/v1/anomalies/{id}/acknowledgement", httpx2.Response(204))
    server.route("GET", "/v1/me", httpx2.Response(200, json={"name": "alice"}))
    server.route("GET", "/v1/graph/edges", httpx2.Response(200, json=[1, 2]))
    with server.client(tmp_path) as kn:
        history = kn.prefixes.history(PREFIX["id"])
        assert history["items"] == [{"a": 1}]
        assert [r["a"] for r in kn.prefixes.pages_of("history", PREFIX["id"])] == [1]
        explained = kn.prefixes.properties_explain(PREFIX["id"], key="dns.ttl")
        assert explained["key"] == "dns.ttl"
        assert kn.anomalies.acknowledgement("a1", body={"motive": "vu"}) is None
        assert kn.call("me") == {"name": "alice"}
        assert kn.graph.edges() == [1, 2]
        with pytest.raises(ClientError) as missing:
            kn.prefixes.properties_explain(PREFIX["id"])
        assert missing.value.code == "client.missing-parameter"
        with pytest.raises(ClientError) as unknown:
            kn.prefixes.history(PREFIX["id"], colour="red")
        assert unknown.value.code == "client.unknown-parameter"
        with pytest.raises(ClientError) as absent:
            kn.prefixes.fly(PREFIX["id"])
        assert absent.value.code == "client.operation-unavailable"
        with pytest.raises(ClientError):
            kn.call("no_such_operation")
    explanation = server.seen[2]
    assert explanation.url.path.endswith("/properties/dns.ttl/explanation")


def test_modules_and_unknown_resources(tmp_path: Path) -> None:
    server = FakeServer()
    with server.client(tmp_path) as kn:
        assert kn.modules == ("policy", "flow", "machines")
        assert kn.has("machines")
        assert not kn.has("vlan")
        assert not kn.has("demo")
        assert kn.policy_rules.name == "policy-rules"
        assert kn.resource("vlan-groups").model.module == "core"
        assert kn.resource("interfaces").model.module == "machines"
        with pytest.raises(UnknownResource):
            kn.resource("widgets")
        assert not hasattr(kn, "widgets")
        with pytest.raises(AttributeError):
            kn._private  # noqa: B018, SLF001
        assert kn.refresh().version == "0.1.0"
        assert repr(kn.prefixes) == "Resource('prefixes')"


def test_view_does_not_close_the_transport(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("GET", "/v1/prefixes/{id}", object_answer())
    kn = server.client(tmp_path)
    view = kn.at(as_of="2026-01-01T00:00:00Z")
    view.close()
    record = view.prefixes.get(PREFIX["id"])
    assert record.as_of == "2026-01-01T00:00:00Z"
    assert server.seen[-1].url.params.get("as_of") == "2026-01-01T00:00:00Z"
    kn.close()


def test_bearer_token_and_401(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("GET", "/v1/me", problem(401, "unauthenticated"))
    with (
        server.client(tmp_path, auth=BearerToken("t0k")) as kn,
        pytest.raises(Unauthenticated) as caught,
    ):
        kn.call("me")
    assert caught.value.code == "unauthenticated"
    assert server.seen[-1].headers["authorization"] == "Bearer t0k"
    assert "t0k" not in repr(BearerToken("t0k"))


CONFIG = """
default = "prod"

[profiles.prod]
url = "https://kartenn.test/"
auth = "token"
timezone = "Europe/Paris"
http2 = false
colour = "blue"

[profiles.open]
url = "https://kartenn.test"

[profiles.leaky]
url = "https://kartenn.test"
auth = "basic"
account = "sync"
secret = "hunter2"

[profiles.empty]
auth = "none"

[profiles.odd]
url = "https://kartenn.test"
auth = "kerberos"
"""


def test_profiles(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    config = tmp_path / "config.toml"
    config.write_text(CONFIG)
    server = FakeServer()
    server.route("GET", "/v1/me", httpx2.Response(200, json={"name": "ci"}))
    monkeypatch.setenv("KARTENN_TOKEN", "ci-token")
    kn = Client.from_profile(
        config=config, transport=server.transport(), cache=CatalogCache(tmp_path / "cache")
    )
    assert kn.url == "https://kartenn.test"
    assert kn.call("me") == {"name": "ci"}
    assert server.seen[-1].headers["authorization"] == "Bearer ci-token"
    from kartenn.profiles import read_profile  # noqa: PLC0415

    prod = read_profile(path=config)
    assert (prod.timezone, prod.http2, prod.extra) == ("Europe/Paris", False, {"colour": "blue"})
    assert read_profile("open", path=config).auth == "none"
    monkeypatch.setenv("KARTENN_PROFILE", "open")
    assert read_profile(path=config).name == "open"
    for name, code in [
        ("leaky", "client.secret-in-profile"),
        ("empty", "client.invalid-configuration"),
        ("ghost", "client.unknown-profile"),
    ]:
        with pytest.raises(ClientError) as caught:
            read_profile(name, path=config)
        assert caught.value.code == code
        assert "hunter2" not in str(caught.value)
    with pytest.raises(ClientError) as odd:
        Client.from_profile("odd", config=config)
    assert odd.value.code == "client.invalid-configuration"
    monkeypatch.delenv("KARTENN_TOKEN")
    with pytest.raises(ClientError) as missing:
        Client.from_profile("prod", config=config)
    assert missing.value.code == "client.login-required"
    with pytest.raises(ClientError):
        read_profile(path=tmp_path / "absent.toml")
    (tmp_path / "bad.toml").write_text("=")
    with pytest.raises(ClientError):
        read_profile(path=tmp_path / "bad.toml")
    plain = Client.from_profile("open", config=config)
    plain.close()


def test_token_stores(tmp_path: Path) -> None:
    memory = MemoryStore()
    memory.set("k", "v")
    assert memory.get("k") == "v"
    memory.delete("k")
    memory.delete("k")
    assert memory.get("k") is None
    store = FileStore(tmp_path / "state" / "tokens.json")
    assert store.get("k") is None
    store.set("k", "v")
    store.set("j", "w")
    assert store.get("k") == "v"
    assert stat.S_IMODE(store.path.stat().st_mode) == 0o600
    assert stat.S_IMODE(store.path.parent.stat().st_mode) == 0o700
    store.delete("k")
    store.delete("k")
    assert json.loads(store.path.read_text()) == {"j": "w"}
    store.path.chmod(0o644)
    with pytest.raises(ClientError) as caught:
        store.get("j")
    assert caught.value.code == "client.store-permissions"
    store.path.chmod(0o600)
    for content in ("{", "[]"):
        store.path.write_text(content)
        with pytest.raises(ClientError):
            store.get("j")


@pytest.mark.anyio
async def test_async_client(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("GET", "/v1/prefixes", paged(250))
    server.route("GET", "/v1/prefixes/{id}", object_answer())
    server.route("PATCH", "/v1/prefixes/{id}", httpx2.Response(200, json=APPLIED))
    server.route("DELETE", "/v1/prefixes/{id}", httpx2.Response(200, json=APPLIED))
    server.route(
        "POST",
        "/v1/prefixes",
        httpx2.Response(201, json=APPLIED, headers={"Location": "/v1/prefixes/n"}),
    )
    server.route(
        "GET", "/v1/prefixes/{id}/history", httpx2.Response(200, json={"items": [{"a": 1}]})
    )
    server.route("GET", "/v1/me", problem(401, "unauthenticated"))
    async with server.async_client(tmp_path) as kn:
        assert await kn.modules() == ("policy", "flow", "machines")
        assert await kn.has("flow")
        ids = [record["id"] async for record in kn.prefixes.list(page_size=100)]
        assert len(ids) == 250
        first = await kn.at(as_of="2026-01-01T00:00:00Z").prefixes.list().first_page()
        assert first.as_of == "2026-01-01T00:00:00Z"
        prefix = await kn.prefixes.get(PREFIX["id"])
        updated = await kn.prefixes.update(prefix, nature="container", motive="m")
        assert isinstance(updated, Applied)
        created = await kn.prefixes.create(value="198.51.100.0/24", motive="m")
        assert isinstance(created, Applied)
        assert created.location == "/v1/prefixes/n"
        await kn.prefixes.close(prefix, motive="m")
        assert (await kn.prefixes.history(PREFIX["id"]))["items"] == [{"a": 1}]
        assert [r["a"] async for r in kn.prefixes.pages_of("history", PREFIX["id"])] == [1]
        assert (await kn.prefixes.model()).module == "core"
        assert repr(kn.prefixes) == "AsyncResource('prefixes')"
        with pytest.raises(Unauthenticated):
            await kn.call("me")
        with pytest.raises(AttributeError):
            kn._private  # noqa: B018, SLF001
        with pytest.raises(AttributeError):
            kn.prefixes._private  # noqa: B018, SLF001
