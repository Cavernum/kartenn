"""Named commands of the modules (REQ-023-20; maquette § 6, 7)."""

from __future__ import annotations

import copy
import json
from typing import TYPE_CHECKING, Any

import httpx2

from fakes import CONTRACT, FakeServer, body, capabilities, kartenn, without
from kartenn.catalog import derive
from kartenn_cli.named import named_commands, replaced

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

MACHINE = "aa000000-0000-7000-8000-000000000001"
FLOW = "4f9a0000-0000-7000-8000-000000000001"
CHANGESET = "0c410000-0000-7000-8000-000000000001"
VERDICT: dict[str, Any] = {
    "access": "full",
    "revision": 12,
    "sources": [{"axes": []}],
    "destinations": [{"axes": []}],
    "hidden": 0,
    "synthesis": {"verdict": "allowed", "outcome": "automatic", "allowed": 1, "denied": 0},
    "evaluations": [
        {
            "source": 0,
            "destination": 0,
            "service": "ssh",
            "verdict": "allowed",
            "outcome": "automatic",
            "decisive": {"kind": "rule", "rule": "admin-vers-tout"},
            "combination": [],
            "discarded": [],
            "conditions": [{"condition": "flux-trace", "state": "met"}],
            "signals": [],
            "score": 0,
        }
    ],
}
APPLIED: dict[str, Any] = {
    "changeset": {"id": CHANGESET, "href": "/x"},
    "state": "applied",
    "warnings": [],
    "diff": [],
    "cascaded": [],
}


def test_policy_check(server: FakeServer, snapshot: SnapshotAssertion) -> None:
    server.route("GET", "/v1/verdicts", VERDICT)
    result = kartenn(
        "policy",
        "check",
        "machine:bastion-1",
        "prefix:interne/198.51.100.128/25",
        "--service",
        "ssh",
    )
    assert result.exit_code == 0, result.output
    assert result.stdout == snapshot
    params = server.seen[-1].url.params
    assert (params["source"], params["service"]) == ("machine:bastion-1", "ssh")
    denied = copy.deepcopy(VERDICT)
    denied["synthesis"]["verdict"] = "denied"
    denied["access"] = "reduced"
    denied["hidden"] = 2
    denied["evaluations"][0]["conditions"] = [{"condition": "c", "state": "unmet"}]
    server.route("GET", "/v1/verdicts", denied)
    refused = kartenn("policy", "check", "a", "b", "--service", "ssh", "--exit-status")
    assert refused.exit_code == 9
    assert "✗ refusé" in refused.stdout
    assert "Explication réduite" in refused.stdout
    assert kartenn("policy", "check", "a", "b", "--service", "ssh").exit_code == 0
    as_json = kartenn("policy", "check", "a", "b", "-o", "json")
    assert json.loads(as_json.stdout)["synthesis"]["verdict"] == "denied"


def test_policy_matrix(server: FakeServer) -> None:
    matrix: dict[str, Any] = {
        "flow_type": "admin",
        "outside": 1,
        "cells": [
            {"source": "prod", "destination": "dev", "state": "allow", "patterns": [], "rules": []},
            {"source": "dev", "destination": "prod", "state": "deny", "patterns": [], "rules": []},
            {
                "source": "dev",
                "destination": "dev",
                "state": "conditional",
                "patterns": [],
                "rules": [],
            },
        ],
    }
    server.route(
        "GET", "/v1/policy-matrix", httpx2.Response(200, json=matrix, headers={"ETag": '"m7"'})
    )
    shown = kartenn("policy", "matrix", "show", "--flow-type", "admin")
    assert shown.stdout.splitlines() == [
        "Type de flux admin (lignes : source ; colonnes : destination)",
        "SOURCE  DEV  PROD",
        "dev     ◐    ✗",
        "prod    ✓    ✗",
        "1 règle(s) hors de la vue.",
    ]
    assert json.loads(kartenn("policy", "matrix", "show", "-o", "json").stdout)["outside"] == 1
    server.route(
        "PUT",
        "/v1/policy-matrix/{source}/{flow_type}/{destination}",
        httpx2.Response(200, json=APPLIED),
    )
    done = kartenn("policy", "matrix", "set", "prod", "admin", "dev", "deny", "--motive", "m")
    assert done.exit_code == 0, done.output
    assert server.seen[-1].headers["If-Match"] == '"m7"'
    assert body(server.seen[-1]) == {"state": "deny", "motive": "m"}


def test_policy_report(server: FakeServer) -> None:
    states = iter(["running", "done"])
    server.route(
        "POST", "/v1/policy-reports", httpx2.Response(201, json={"id": "r1", "state": "pending"})
    )
    server.route(
        "GET",
        "/v1/policy-reports/{id}",
        lambda _r: httpx2.Response(
            200, json={"id": "r1", "state": next(states), "counts": {"denied": 2, "x": None}}
        ),
    )
    result = kartenn("policy", "report", "--wait")
    assert result.exit_code == 0, result.output
    assert result.stdout == "Rapport r1 : done\n  denied  2\n  x  —\n"
    assert result.stderr.endswith("Rapport r1 demandé.\n")
    server.route("GET", "/v1/policy-reports/{id}/items", {"items": [{"a": 1}], "links": {}})
    server.route("GET", "/v1/policy-reports/{id}", {"id": "r1", "state": "done", "counts": {}})
    items = kartenn("policy", "report", "--id", "r1", "--rubric", "denied", "-o", "ndjson")
    assert items.stdout == '{"a": 1}\n'
    assert server.seen[-1].url.params["rubric"] == "denied"
    record = kartenn("policy", "report", "--id", "r1", "-o", "json")
    assert json.loads(record.stdout)["state"] == "done"


def test_flow_request_and_review(server: FakeServer) -> None:
    server.route("GET", "/v1/services", {"items": [{"id": "s1", "key": "https"}], "links": {}})
    server.route("GET", "/v1/entities", {"items": [{"id": "e1", "slug": "exemple"}], "links": {}})
    pending: dict[str, Any] = {
        "changeset": {"id": CHANGESET, "href": "/x"},
        "state": "pending",
        "kind": "proposal",
        "diff": [],
    }
    server.route("POST", "/v1/flows", httpx2.Response(202, json=pending))
    result = kartenn(
        "flow", "request", "--source", f"machine:{MACHINE}",
        "--destination", '{"kind": "prefix", "id": {"id": "p"}}',
        "--service", "https", "--entity", "exemple", "--ref", "ticketing:CHG-2048",
        "--justification", "Appel", "--end", "2027-04-30", "--motive", "nouvelle recette",
        "--set", "personal_data=false",
    )  # fmt: skip
    assert result.exit_code == 0, result.output
    assert "à instruire" not in result.stdout or CHANGESET in result.stdout
    assert body(server.seen[-1]) == {
        "source": {"kind": "machine", "id": {"id": MACHINE}},
        "destination": {"kind": "prefix", "id": {"id": "p"}},
        "service": {"id": "s1"},
        "entity": {"id": "e1"},
        "external_refs": [{"system": "ticketing", "id": "CHG-2048"}],
        "motive": "nouvelle recette",
        "justification": "Appel",
        "end": "2027-04-30T00:00:00+02:00",
        "personal_data": False,
    }
    bad = kartenn(
        "flow",
        "request",
        "--source",
        "nowhere",
        "--destination",
        "x:y",
        "--service",
        "https",
        "--entity",
        "exemple",
        "--motive",
        "m",
    )
    assert bad.exit_code == 2
    assert "client.invalid-endpoint" in bad.stderr
    server.route("GET", "/v1/flows/{id}", {"id": FLOW, "changeset": {"id": CHANGESET}})
    server.route(
        "GET",
        "/v1/changesets/{id}",
        httpx2.Response(200, json={"id": CHANGESET, "state": "pending"}, headers={"ETag": '"c"'}),
    )
    server.route("POST", "/v1/changesets/{id}/validation", httpx2.Response(200, json=APPLIED))
    approved = kartenn("flow", "review", FLOW, "--approve")
    assert approved.exit_code == 0, approved.output
    server.route(
        "POST", "/v1/changesets/{id}/rejection", httpx2.Response(200, json={"state": "rejected"})
    )
    rejected = kartenn("flow", "review", FLOW, "--reject", "--motive", "non", "-o", "json")
    assert json.loads(rejected.stdout) == {"state": "rejected"}
    assert kartenn("flow", "review", FLOW).exit_code == 2
    server.route("GET", "/v1/flows/{id}", {"id": FLOW})
    assert "client.nothing-to-review" in kartenn("flow", "review", FLOW, "--approve").stderr


def test_named_commands_need_their_operations() -> None:
    catalog = derive("u", capabilities(), CONTRACT)
    assert set(named_commands("policy", catalog)) == {"check", "matrix", "report"}
    assert set(named_commands("flow", catalog)) == {"request", "review"}
    assert replaced("flow", catalog) == {"create"}
    no_flow = derive(
        "u",
        capabilities(modules=[m for m in capabilities()["modules"] if m["name"] != "flow"]),
        without(CONTRACT, "flow"),
    )
    assert named_commands("flow", no_flow) == {}
    trimmed = copy.deepcopy(CONTRACT)
    del trimmed["paths"]["/v1/flows"]["post"]
    assert "request" not in named_commands("flow", derive("u", capabilities(), trimmed))
