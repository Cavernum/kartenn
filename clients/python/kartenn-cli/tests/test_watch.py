"""`kartenn watch` and `--follow` (REQ-023-38; maquette § 15)."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING

import httpx2

from fakes import FakeServer, capabilities, kartenn
from kartenn.transport.httpx2 import Httpx2Transport

if TYPE_CHECKING:
    from collections.abc import Iterator

    import pytest

STREAMING = capabilities(events={"heartbeat": 15, "ring_seconds": 900})
CHANGESET = "0c410000-0000-7000-8000-000000000001"
BLOCKS = (
    b'retry: 10\nevent: ready\nid: k1.A\ndata: {"heartbeat": 15}\n\n',
    (
        b"event: changes\nid: k1.B\n"
        b'data: {"changeset":"9d27","objects":[{"id":"p1","key":"interne/203.0.113.0/26"}]}\n\n'
    ),
    b'event: changeset\nid: k1.C\ndata: {"changeset":"'
    + CHANGESET.encode()
    + b'","state":"applied"}\n\n',
    b'event: reset\nid: k1.D\ndata: {"reason":"overflow"}\n\n',
)


def events(server: FakeServer, *, interrupt: bool = True) -> list[httpx2.Request]:
    """The stream: the blocks, then Ctrl-C at the reconnection."""
    opened: list[httpx2.Request] = []

    def stream(request: httpx2.Request) -> httpx2.Response:
        opened.append(request)
        if len(opened) > 1 and interrupt:
            raise KeyboardInterrupt

        def body() -> Iterator[bytes]:
            yield from BLOCKS

        return httpx2.Response(200, headers={"Content-Type": "text/event-stream"}, content=body())

    server.route("GET", "/v1/events", stream)
    return opened


def test_watch_in_a_table(server: FakeServer) -> None:
    server.capabilities = STREAMING
    opened = events(server)
    result = kartenn("watch", "--types", "core.prefix,core.vlan", "--domains", "interne")
    assert result.exit_code == 0, result.output
    lines = [line[10:] for line in result.stdout.splitlines()]
    assert lines == [
        "prêt",
        "changes    changeset 9d27  interne/203.0.113.0/26",
        f"changeset  {CHANGESET}  applied",
        "reset      débordement du tampon : relisez ce que vous suivez",
    ]
    said = result.stderr.splitlines()
    assert "Flux d'événements ouvert (profil prod) — Ctrl-C pour arrêter." in said
    assert said[-1] == (
        "Flux fermé ; reprendre : kartenn watch --since k1.D --types core.prefix,core.vlan "
        "--domains interne"
    )
    assert opened[0].url.params["types"] == "core.prefix,core.vlan"
    assert opened[1].headers["Last-Event-ID"] == "k1.D"


def test_watch_in_ndjson(server: FakeServer) -> None:
    server.capabilities = STREAMING
    events(server)
    result = kartenn("watch", "--objects", CHANGESET, "-o", "ndjson", "--since", "k1.0")
    first = json.loads(result.stdout.splitlines()[0])
    assert first == {"kind": "ready", "id": "k1.A", "data": {"heartbeat": 15}}
    assert len(result.stdout.splitlines()) == 4


def test_watch_absent(server: FakeServer) -> None:
    del server
    result = kartenn("watch")
    assert result.exit_code == 2
    assert result.stderr.endswith(
        "erreur [client.operation-unavailable] Ce serveur ne sert pas le flux d'événements "
        "(capabilities.events = null).\n"
    )


def test_follow_a_changeset_on_the_stream(server: FakeServer) -> None:
    server.capabilities = STREAMING
    events(server, interrupt=False)
    server.route(
        "GET",
        "/v1/changesets/{id}",
        httpx2.Response(
            200, json={"id": CHANGESET, "state": "pending", "kind": "approval", "motive": "m"}
        ),
    )
    result = kartenn("changeset", "show", CHANGESET, "--follow")
    assert result.exit_code == 0, result.output
    assert result.stdout.splitlines()[-1] == f"Changeset {CHANGESET} : applied."


def test_follow_a_changeset_by_reading_it(
    server: FakeServer, monkeypatch: pytest.MonkeyPatch
) -> None:
    states = iter(["pending", "pending", "applied"])
    server.route(
        "GET",
        "/v1/changesets/{id}",
        lambda _r: httpx2.Response(
            200, json={"id": CHANGESET, "state": next(states), "motive": "m"}
        ),
    )

    def no_wait(self: Httpx2Transport, seconds: float) -> None:
        del self, seconds

    monkeypatch.setattr(Httpx2Transport, "sleep", no_wait)
    result = kartenn("changeset", "show", CHANGESET, "--follow")
    assert result.exit_code == 0, result.output
    assert result.stdout.splitlines()[-1] == f"Changeset {CHANGESET} : applied."
