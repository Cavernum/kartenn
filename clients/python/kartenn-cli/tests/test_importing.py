"""`kartenn import` (REQ-023-32; maquette § 10)."""

from __future__ import annotations

import json
import sys
from typing import TYPE_CHECKING, Any

import httpx2

from fakes import FakeServer, body, kartenn

if TYPE_CHECKING:
    from pathlib import Path

    import pytest

IMPORT = "61b00000-0000-7000-8000-000000000001"
DOMAIN = "3f020000-0000-7000-8000-000000000001"


class Imports:
    """The routes of an import on the simulated server."""

    def __init__(self, server: FakeServer, *, final: dict[str, Any] | None = None) -> None:
        self.chunks: list[list[Any]] = []
        self.states = [
            {"state": "sorting"},
            final or {"state": "done", "elements": 3},
        ]
        server.route("POST", "/v1/imports", httpx2.Response(201, json={"id": IMPORT}))
        server.route("PUT", "/v1/imports/{id}/chunks/{n}", self.chunk)
        server.route("POST", "/v1/imports/{id}/start", httpx2.Response(202, json={"id": IMPORT}))
        server.route("GET", "/v1/imports/{id}", self.show)
        server.route(
            "GET", "/v1/domains", {"items": [{"id": DOMAIN, "slug": "interne"}], "links": {}}
        )

    def chunk(self, request: httpx2.Request) -> httpx2.Response:
        self.chunks.append(body(request)["elements"])
        return httpx2.Response(200, json={})

    def show(self, _request: httpx2.Request) -> httpx2.Response:
        state = self.states.pop(0) if len(self.states) > 1 else self.states[0]
        record = {"id": IMPORT, "source": "s", "chunks": 1, "elements": 3, **state}
        return httpx2.Response(200, json=record, headers={"Retry-After": "0"})


def ndjson(path: Path, count: int) -> Path:
    lines = [
        json.dumps({"operation": {"op": "create_domain", "slug": f"d{n}"}}) for n in range(count)
    ]
    path.write_text("\n".join(lines) + "\n")
    return path


def test_ndjson_by_chunks_with_a_line_per_step(server: FakeServer, tmp_path: Path) -> None:
    imports = Imports(server)
    result = kartenn("import", str(ndjson(tmp_path / "d.ndjson", 3)), "--chunk", "2")
    assert result.exit_code == 0, result.output
    assert [len(c) for c in imports.chunks] == [2, 1]
    assert result.stderr == f"import {IMPORT} envoyé\nimport {IMPORT} en tri\n"
    assert result.stdout.startswith(f"Import {IMPORT} : done, 3 éléments, en ")
    assert body(next(r for r in server.seen if r.method == "POST"))["source"] == "d.ndjson"


def test_csv_of_a_resource(server: FakeServer, tmp_path: Path) -> None:
    imports = Imports(server)
    source = tmp_path / "adresses.csv"
    source.write_text(
        "domain,value,usage,prop.dns.fqdn,external_id\n"
        "interne,198.51.100.21,host,web-1.example.org,src-0001\n"
        "interne,198.51.100.22,host,,src-0002\n"
    )
    result = kartenn(
        "import", str(source), "--resource", "addresses", "--id-column", "external_id",
        "--source", "cmdb-export", "--no-follow",
    )  # fmt: skip
    assert result.exit_code == 0, result.output
    assert imports.chunks[0] == [
        {
            "operation": {
                "op": "create_address",
                "domain": {"id": DOMAIN},
                "value": "198.51.100.21",
                "usage": "host",
                "properties": {"dns.fqdn": "web-1.example.org"},
            },
            "external_id": "src-0001",
        },
        {
            "operation": {
                "op": "create_address",
                "domain": {"id": DOMAIN},
                "value": "198.51.100.22",
                "usage": "host",
            },
            "external_id": "src-0002",
        },
    ]
    lookups = [r for r in server.seen if r.url.path == "/v1/domains"]
    assert len(lookups) == 1
    as_json = tmp_path / "vlans.json"
    as_json.write_text(json.dumps([{"vid": "140", "name": "lot3"}, 3]))
    vlans = kartenn("import", str(as_json), "--resource", "vlans", "--no-follow")
    assert vlans.exit_code == 0, vlans.output
    assert imports.chunks[-1] == [
        {"operation": {"op": "vlan.create_vlan", "vid": 140, "name": "lot3"}}
    ]


def test_a_refused_sort_names_the_line_of_the_source(server: FakeServer, tmp_path: Path) -> None:
    error = {
        "type": "urn:kartenn:problem:invalid-import",
        "title": "L'import n'a pas pu être trié",
        "status": 422,
        "violations": [
            {
                "rule": "import.unknown-reference",
                "severity": "error",
                "detail": "Le domaine « interne-old » est inconnu à la date visée.",
                "pointer": "/operation/domain",
                "element": 1,
            }
        ],
    }
    Imports(server, final={"state": "uploading", "error": error})
    source = tmp_path / "adresses.csv"
    source.write_text("domain,value\ninterne,198.51.100.21\ninterne,198.51.100.22\n")
    result = kartenn("import", str(source), "--resource", "addresses")
    assert result.exit_code == 6
    assert result.stderr.splitlines()[2:] == [
        "erreur [invalid-import] L'import n'a pas pu être trié (422)",
        f"  1 violation — import {IMPORT}",
        (
            "  ✗ [import.unknown-reference] adresses.csv:3  Le domaine « interne-old » est "
            "inconnu à la date visée."
        ),
        (
            "  L'import est revenu à l'état « envoi » : corrigez puis « kartenn import --resume "
            f"{IMPORT} adresses.csv »."
        ),
    ]


def test_resume_checks_the_file(server: FakeServer, tmp_path: Path) -> None:
    imports = Imports(server)
    failing = iter([httpx2.Response(200, json={}), httpx2.Response(500, json={})])
    server.route("PUT", "/v1/imports/{id}/chunks/{n}", lambda _r: next(failing))
    path = ndjson(tmp_path / "d.ndjson", 3)
    cut = kartenn("import", str(path), "--chunk", "1")
    assert cut.exit_code == 1
    server.route("PUT", "/v1/imports/{id}/chunks/{n}", imports.chunk)
    resumed = kartenn("import", "--resume", IMPORT, str(path), "--chunk", "1")
    assert resumed.exit_code == 0, resumed.output
    assert len(imports.chunks) == 2
    other = kartenn("import", "--resume", "autre", str(path))
    assert other.exit_code == 2
    assert "client.no-import" in other.stderr


def test_resume_of_a_changed_file_is_refused(server: FakeServer, tmp_path: Path) -> None:
    Imports(server)
    server.route("PUT", "/v1/imports/{id}/chunks/{n}", httpx2.Response(500, json={}))
    path = ndjson(tmp_path / "d.ndjson", 2)
    assert kartenn("import", str(path)).exit_code == 1
    ndjson(path, 4)
    changed = kartenn("import", "--resume", IMPORT, str(path))
    assert changed.exit_code == 2
    assert "client.import-source-changed" in changed.stderr


def test_a_file_of_objects_needs_its_resource(server: FakeServer, tmp_path: Path) -> None:
    del server
    path = tmp_path / "x.csv"
    path.write_text("a\n1\n")
    result = kartenn("import", str(path))
    assert result.exit_code == 2
    assert "client.resource-required" in result.stderr
    assert kartenn("import", str(path), "--resource", "audit-events").exit_code == 2


def test_imports_of_the_server(server: FakeServer) -> None:
    Imports(server)
    server.route("GET", "/v1/imports", {"items": [{"id": IMPORT, "state": "paused"}], "links": {}})
    server.route("POST", "/v1/imports/{id}/cancel", {"id": IMPORT, "state": "cancelled"})
    server.route("POST", "/v1/imports/{id}/resume", {"id": IMPORT, "state": "running"})
    assert json.loads(kartenn("import", "--list").stdout)[0]["state"] == "paused"
    assert json.loads(kartenn("import", "--status", IMPORT).stdout)["state"] == "sorting"
    assert json.loads(kartenn("import", "--cancel", IMPORT).stdout)["state"] == "cancelled"
    assert json.loads(kartenn("import", "--resume", IMPORT).stdout)["state"] == "running"
    assert "client.file-required" in kartenn("import").stderr


def test_progress_on_a_terminal(
    server: FakeServer, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    Imports(server)
    monkeypatch.setattr(sys.stderr, "isatty", lambda: True)
    result = kartenn("import", str(ndjson(tmp_path / "d.ndjson", 3)))
    assert result.exit_code == 0, result.output
    assert f"import {IMPORT} en tri" in result.stderr
