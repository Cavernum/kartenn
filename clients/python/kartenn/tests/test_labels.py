"""The dictionary of labels (spec 018, REQ-018-21): kept by language and revision, in
memory and on disk, revalidated under `If-None-Match`, made stale by the events."""

from pathlib import Path

import httpx2

from fakes import URL, FakeServer, capabilities
from kartenn.catalog import CatalogCache
from kartenn.client import Client
from kartenn.transport.httpx2 import Httpx2Transport

STREAMING = capabilities(events={"heartbeat": 15, "ring_seconds": 900})


def dictionary(revision: str, prod: str) -> dict[str, object]:
    return {
        "lang": "fr",
        "default_lang": "fr",
        "revision": revision,
        "spaces": {
            "value": {"policy.environment": {"prod": prod}},
            "kind": {"core.prefix": "Préfixe"},
        },
        "fallback": {"lang": "fr", "keys": ["value/policy.environment/prod"]},
    }


class Labels:
    """`GET /v1/labels` at a revision, 304 under the `If-None-Match` of its `ETag`."""

    def __init__(self, server: FakeServer) -> None:
        self.revision = "1-aa"
        self.prod = "Production"
        self.asked: list[str | None] = []
        server.route("GET", "/v1/labels", self)

    def __call__(self, request: httpx2.Request) -> httpx2.Response:
        etag = f'"{self.revision}.0011"'
        self.asked.append(request.headers.get("If-None-Match"))
        if request.headers.get("If-None-Match") == etag:
            return httpx2.Response(304, headers={"ETag": etag})
        return httpx2.Response(
            200,
            json=dictionary(self.revision, self.prod),
            headers={"ETag": etag, "Content-Language": "fr"},
        )


def test_labels_cache_by_revision(tmp_path: Path) -> None:
    server = FakeServer()
    labels = Labels(server)
    with server.client(tmp_path) as kn:
        found = kn.labels("fr")
        assert found.resolve("value", "policy.environment", "prod") == "Production"
        assert found.resolve("value", "policy.environment", "dev") == "dev"
        assert found.resolve("kind", "core.prefix") == "Préfixe"
        assert found.resolve("rule", "core.addr.syntax") == "core.addr.syntax"
        assert found.fallback == ("value/policy.environment/prod",)
        assert kn.labels("fr") is found, "kept in memory"
    assert labels.asked == [None]
    # Another run: asked again under the ETag kept on disk, nothing downloaded.
    with server.client(tmp_path) as kn:
        assert kn.labels("fr").revision == "1-aa"
    assert labels.asked == [None, '"1-aa.0011"']
    # The revision moves: the new dictionary replaces the old one.
    labels.revision, labels.prod = "2-aa", "Prod."
    with server.client(tmp_path) as kn:
        found = kn.labels("fr")
        assert found.revision == "2-aa"
        assert found.resolve("value", "policy.environment", "prod") == "Prod."
    with server.client(tmp_path) as kn:
        assert kn.labels("fr").revision == "2-aa"
    assert labels.asked[-1] == '"2-aa.0011"'
    assert list((tmp_path / "labels").iterdir()), "beside the catalog"


def stream(*blocks: bytes) -> httpx2.Response:
    return httpx2.Response(200, headers={"Content-Type": "text/event-stream"}, content=blocks)


def test_labels_event_invalidates(tmp_path: Path) -> None:
    server = FakeServer(caps=STREAMING)
    labels = Labels(server)
    server.route(
        "GET",
        "/v1/events",
        stream(
            b"retry: 1000\nevent: ready\nid: k1.A\ndata: {}\n\n",
            b'event: changes\nid: k1.B\ndata: {"changeset":"C1","objects":[]}\n\n',
            b'event: labels\nid: k1.C\ndata: {"revision":"2-aa"}\n\n',
        ),
    )
    transport = Httpx2Transport(URL, mock=httpx2.MockTransport(server))
    with Client(URL, transport=transport, cache=CatalogCache(tmp_path)) as kn:
        kn.labels()
        kn.labels()
        assert len(labels.asked) == 1
        seen: list[str] = []
        for event in kn.events(reconnect=False):
            seen.append(event.kind)
            if event.kind == "changes":
                kn.labels()
                assert len(labels.asked) == 2, "ready made it stale"
                kn.labels()
                assert len(labels.asked) == 2, "fresh again"
        assert seen == ["ready", "changes", "labels"]
        labels.revision = "2-aa"
        assert kn.labels().revision == "2-aa", "labels made it stale"
        assert labels.asked[-1] == '"1-aa.0011"'
