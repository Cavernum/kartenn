"""`kartenn api` and the tools of the base (REQ-023-19; maquette § 11)."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING

import httpx2

from fakes import FakeServer, body, kartenn, problem

if TYPE_CHECKING:
    from pathlib import Path

PREFIX = "7c1e0000-0000-7000-8000-000000000001"


def test_get_with_fields(server: FakeServer) -> None:
    answer = {"value": "203.0.113.64/28", "domain": {"id": "3f02", "slug": "interne"}}
    server.route("GET", "/v1/prefixes/{id}/next-free", answer)
    result = kartenn(
        "api", "GET", f"/v1/prefixes/{PREFIX}/next-free", "-f", "kind=prefix", "-f", "length=28",
        "--at", "2026-11-01",
    )  # fmt: skip
    assert result.exit_code == 0, result.output
    assert result.stdout == json.dumps(answer, ensure_ascii=False) + "\n"
    params = server.seen[-1].url.params
    assert (params["kind"], params["length"]) == ("prefix", "28")
    assert params["valid_at"] == "2026-11-01T00:00:00+01:00"


def test_query_with_data_and_paginate(server: FakeServer, tmp_path: Path) -> None:
    pages: dict[str | None, object] = {
        None: {"items": [{"name": "a"}], "links": {"next": "/v1/audit-events?cursor=c2"}},
        "c2": {"items": [{"name": "b"}], "links": {}},
    }
    server.route("QUERY", "/v1/audit-events", httpx2.Response(200, json=pages[None]))
    server.route(
        "GET",
        "/v1/audit-events",
        lambda r: httpx2.Response(200, json=pages[r.url.params.get("cursor")]),
    )
    data = '{"filters":[{"field":"actor","op":"eq","value":"sync-dns"}]}'
    result = kartenn("api", "query", "/v1/audit-events", "--data", data, "--paginate")
    assert result.exit_code == 0, result.output
    assert result.stdout == '{"name": "a"}\n{"name": "b"}\n'
    query = next(r for r in server.seen if r.method == "QUERY")
    assert body(query)["filters"][0]["value"] == "sync-dns"
    path = tmp_path / "q.json"
    path.write_text(data)
    from_file = kartenn("api", "QUERY", "/v1/audit-events", "--data", f"@{path}", "-o", "json")
    assert json.loads(from_file.stdout)["items"] == [{"name": "a"}]
    stdin = kartenn("api", "QUERY", "/v1/audit-events", "--data", "-", input=data)
    assert stdin.exit_code == 0


def test_headers_post_and_problems(server: FakeServer) -> None:
    server.route("POST", "/v1/domains", httpx2.Response(201, json={"ok": True}))
    created = kartenn(
        "api", "POST", "/v1/domains", "--data", "{}", "-H", "Kartenn-On-Behalf-Of: alice"
    )
    assert created.exit_code == 0, created.output
    sent = server.seen[-1]
    assert sent.headers["Kartenn-On-Behalf-Of"] == "alice"
    assert "Idempotency-Key" in sent.headers
    server.route("DELETE", "/v1/domains/{id}", httpx2.Response(204))
    assert kartenn("api", "DELETE", "/v1/domains/x").stdout == ""
    server.route("GET", "/v1/text", httpx2.Response(200, text="plain"))
    assert kartenn("api", "GET", "/v1/text").stdout == "plain\n"
    server.route("GET", "/v1/gone", problem(404, "not-found", title="Introuvable"))
    gone = kartenn("api", "GET", "/v1/gone", "--paginate")
    assert gone.exit_code == 3
    assert gone.stderr.startswith("erreur [not-found] Introuvable (404)")
    server.route("GET", "/v1/one", {"a": 1})
    assert kartenn("api", "GET", "/v1/one", "--paginate").stdout == '{"a": 1}\n'


def test_usage_errors(server: FakeServer) -> None:
    del server
    assert "client.invalid-method" in kartenn("api", "FETCH", "/v1/x").stderr
    assert "client.invalid-path" in kartenn("api", "GET", "v1/x").stderr
    assert "client.invalid-field" in kartenn("api", "GET", "/v1/x", "-f", "nope").stderr
    assert "client.invalid-header" in kartenn("api", "GET", "/v1/x", "-H", "nope").stderr


def test_spec_capabilities_health_and_stubs(server: FakeServer, tmp_path: Path) -> None:
    spec = kartenn("api", "spec")
    assert json.loads(spec.stdout)["openapi"].startswith("3.")
    caps = kartenn("capabilities", "-o", "json")
    assert json.loads(caps.stdout)["version"] == "0.2.0"
    server.route("GET", "/health/ready", {"status": "ok"})
    health = kartenn("health")
    assert health.stdout.startswith("ready : 200\n")
    out = tmp_path / "typings"
    stubs = kartenn("stubs", "generate", "--out", str(out))
    assert stubs.exit_code == 0, stubs.output
    assert (out / "kartenn_server" / "__init__.pyi").exists()


def test_stats(server: FakeServer) -> None:
    counters = {
        "objects": {"core.prefix": 3},
        "pending": {"to_apply": 1, "mine": 0},
        "anomalies": {"open": 2},
        "vlans": {"reserved": 4, "deprecated": 0},
        "modules": {"machines": {"physical": 4}},
    }
    server.route("GET", "/v1/stats", counters)
    result = kartenn("stats", "-o", "json")
    assert result.exit_code == 0, result.output
    assert json.loads(result.stdout) == counters
