"""Writes: create, update, close, edit, --dry-run, changesets (REQ-023-28 to 31;
maquette § 8, 9, 13)."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import TYPE_CHECKING, Any

import httpx2
import pytest

from fakes import FakeServer, body, kartenn, problem
from kartenn_cli.runtime import Runtime

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

DOMAIN = "3f020000-0000-7000-8000-000000000001"
PREFIX = "7c1e0000-0000-7000-8000-000000000001"
VLAN = "9a000000-0000-7000-8000-000000000140"
GROUP = "6b000000-0000-7000-8000-000000000001"
CHANGESET = "9d270000-0000-7000-8000-000000000001"
APPLIED = {
    "changeset": {"id": CHANGESET, "href": f"/v1/changesets/{CHANGESET}"},
    "state": "applied",
    "warnings": [{"rule": "core.prefix.unattached", "severity": "warning", "detail": "Sans VLAN"}],
    "diff": [],
    "cascaded": [],
}
PENDING: dict[str, Any] = {
    "changeset": {"id": CHANGESET, "href": f"/v1/changesets/{CHANGESET}"},
    "state": "pending",
    "kind": "approval",
    "diff": [],
}
SIMULATED: dict[str, Any] = {
    "valid": True,
    "violations": [
        {
            "rule": "core.prefix.unattached",
            "severity": "warning",
            "operation": 0,
            "detail": "203.0.113.0/26 n'est rattaché à aucun VLAN",
            "subjects": [],
        }
    ],
    "diff": [{"property": "policy.environment", "count": 1, "before": None, "after": '"prod"'}],
    "sample": [],
    "cascaded": [],
    "sections": [
        {
            "name": "policy.combinations",
            "count": 2,
            "rows": [
                {"source": "Administration", "destination": "203.0.113.0/26", "after": "allowed"},
                {"source": "Recette", "destination": "203.0.113.0/26", "after": "denied"},
            ],
        }
    ],
}
VLAN_RECORD = {"id": VLAN, "group": {"id": GROUP, "slug": "site-nord"}, "vid": 140, "name": "lot3"}


@pytest.fixture
def served(server: FakeServer) -> FakeServer:
    server.route("GET", "/v1/domains", {"items": [{"id": DOMAIN, "slug": "interne"}], "links": {}})
    server.route("GET", "/v1/prefixes", {"items": [{"id": PREFIX}], "links": {}})
    server.route(
        "GET",
        "/v1/prefixes/{id}",
        httpx2.Response(
            200, json={"id": PREFIX, "value": "192.0.2.0/27"}, headers={"ETag": '"v1"'}
        ),
    )
    server.route("GET", "/v1/vlans", {"items": [VLAN_RECORD], "links": {}})
    server.route(
        "GET", "/v1/vlans/{id}", httpx2.Response(200, json=VLAN_RECORD, headers={"ETag": '"9d27"'})
    )
    kartenn("--help")
    return server


def test_create_with_typed_members(served: FakeServer) -> None:
    served.route(
        "POST",
        "/v1/prefixes",
        httpx2.Response(201, json=APPLIED, headers={"Location": f"/v1/prefixes/{PREFIX}"}),
    )
    result = kartenn(
        "prefixes", "create", "--domain", "interne", "--value", "203.0.113.0/26",
        "--nature", "network", "--set", "anycast=false", "--motive", "lot 3",
        "--valid-from", "2026-11-01",
    )  # fmt: skip
    assert result.exit_code == 0, result.output
    sent = served.seen[-1]
    assert body(sent) == {
        "domain": {"id": DOMAIN},
        "value": "203.0.113.0/26",
        "nature": "network",
        "anycast": False,
        "motive": "lot 3",
        "valid_from": "2026-11-01T00:00:00+01:00",
    }
    assert "Idempotency-Key" in sent.headers
    assert result.stdout == (
        f"Changeset {CHANGESET} appliqué.\n"
        "  ! [core.prefix.unattached] —  Sans VLAN (avertissement)\n"
        f"Créé : /v1/prefixes/{PREFIX}\n"
    )


def test_create_from_a_document(served: FakeServer, tmp_path: Path) -> None:
    served.route("POST", "/v1/prefixes", httpx2.Response(201, json=APPLIED))
    document = tmp_path / "prefix.yaml"
    document.write_text("domain: {id: x}\nvalue: 203.0.113.0/26\nnature: network\n")
    result = kartenn(
        "prefixes", "create", "--from-file", str(document), "--motive", "m", "-o", "json",
        "--on-behalf-of", "alice",
    )  # fmt: skip
    assert result.exit_code == 0, result.output
    assert json.loads(result.stdout)["state"] == "applied"
    assert body(served.seen[-1])["domain"] == {"id": "x"}
    assert served.seen[-1].headers["Kartenn-On-Behalf-Of"] == "alice"
    stdin = kartenn("prefixes", "create", "--from-file", "-", "--motive", "m", input="[1]")
    assert stdin.exit_code == 2
    assert "client.invalid-document" in stdin.stderr
    bad = kartenn("prefixes", "create", "--set", "noequal", "--motive", "m")
    assert "client.invalid-assignment" in bad.stderr


def test_a_write_without_motive_is_refused_without_a_terminal(served: FakeServer) -> None:
    result = kartenn("--no-input", "prefixes", "close", "interne/192.0.2.0/27")
    assert result.exit_code == 2
    assert result.stderr == (
        "erreur [client.motive-required] Cette écriture exige --motive (le serveur répondrait "
        "motive-required).\n"
    )
    assert all(r.method == "GET" for r in served.seen)


def test_close_and_update_send_if_match(served: FakeServer) -> None:
    served.route("DELETE", "/v1/prefixes/{id}", httpx2.Response(200, json=APPLIED))
    closed = kartenn("prefixes", "close", "interne/192.0.2.0/27", "--motive", "fin")
    assert closed.exit_code == 0, closed.output
    assert served.seen[-1].headers["If-Match"] == '"v1"'
    assert body(served.seen[-1]) == {"motive": "fin"}
    served.route("PATCH", "/v1/vlans/{id}", httpx2.Response(202, json=PENDING))
    pending = kartenn("vlan", "update", "site-nord/140", "--set", "name=lot3-bis", "--motive", "t")
    assert pending.exit_code == 0, pending.output
    assert pending.stdout == (
        f"Écriture en attente (approval) : changeset {CHANGESET}.\n"
        f"  Suivre : kartenn changeset show {CHANGESET}\n"
    )
    patch = served.seen[-1]
    assert patch.headers["Content-Type"] == "application/merge-patch+json"
    assert patch.headers["If-Match"] == '"9d27"'
    assert body(patch) == {"name": "lot3-bis", "motive": "t"}
    required = kartenn(
        "vlan", "update", "site-nord/140", "--name", "x", "--motive", "t", "--require-applied"
    )
    assert required.exit_code == 10


def test_a_stale_version_is_a_conflict(served: FakeServer, snapshot: SnapshotAssertion) -> None:
    served.route(
        "PATCH",
        "/v1/vlans/{id}",
        problem(
            412,
            "precondition-failed",
            title="L'objet a changé depuis sa lecture",
            instance="urn:uuid:c3d1",
            current_etag='"1aa2"',
        ),
    )
    result = kartenn(
        "--no-input",
        "vlan",
        "update",
        "site-nord/140",
        "--set",
        "name=lot3-bis",
        "--motive",
        "test",
    )
    assert result.exit_code == 5
    assert result.stderr == snapshot


def test_dry_run_simulates_the_same_changeset(
    served: FakeServer, snapshot: SnapshotAssertion
) -> None:
    served.route("POST", "/v1/changesets", httpx2.Response(200, json=SIMULATED))
    result = kartenn(
        "prefixes", "create", "--domain", DOMAIN, "--value", "203.0.113.0/26",
        "--nature", "network", "--motive", "lot 3", "--dry-run", "-o", "table",
    )  # fmt: skip
    assert result.exit_code == 0, result.output
    assert result.stdout == snapshot
    sent = served.seen[-1]
    assert sent.url.params["simulate"] == "true"
    assert body(sent)["operations"] == [
        {
            "op": "create_prefix",
            "domain": {"id": DOMAIN},
            "value": "203.0.113.0/26",
            "nature": "network",
        }
    ]


def test_dry_run_refused_ends_with_8(served: FakeServer) -> None:
    served.route(
        "POST",
        "/v1/changesets",
        problem(
            422,
            "changeset-refused",
            title="Changeset refusé",
            instance="urn:uuid:5b0e",
            violations=[
                {
                    "rule": "core.prefix.overlap",
                    "severity": "error",
                    "pointer": "/value",
                    "detail": "Chevauche",
                }
            ],
        ),
    )
    result = kartenn(
        "prefixes", "create", "--domain", DOMAIN, "--value", "203.0.113.32/27",
        "--nature", "network", "--motive", "m", "--dry-run",
    )  # fmt: skip
    assert result.exit_code == 8
    assert result.stdout == "Simulation — rien n'est écrit\n"
    assert result.stderr.startswith("erreur [changeset-refused] Changeset refusé (422)")
    invalid: dict[str, Any] = dict(
        SIMULATED,
        valid=False,
        violations=[{"rule": "r", "severity": "error", "detail": "d", "subjects": []}],
    )
    served.route("POST", "/v1/changesets", httpx2.Response(200, json=invalid))
    blocked = kartenn("prefixes", "close", PREFIX, "--motive", "m", "--dry-run", "-o", "json")
    assert blocked.exit_code == 8
    assert body(served.seen[-1])["operations"] == [
        {"op": "close", "kind": "prefix", "id": {"id": PREFIX}}
    ]


def test_a_write_without_changeset_operation_cannot_be_simulated(served: FakeServer) -> None:
    result = kartenn("anomalies", "acknowledge", PREFIX, "--motive", "vu", "--dry-run")
    assert result.exit_code == 2
    assert "client.dry-run-unavailable" in result.stderr
    served.route(
        "POST", "/v1/anomalies/{id}/acknowledgement", httpx2.Response(200, json={"ok": True})
    )
    done = kartenn("anomalies", "acknowledge", PREFIX, "--motive", "vu", "-o", "json")
    assert done.exit_code == 0, done.output
    assert body(served.seen[-1]) == {"motive": "vu"}


def test_local_changeset(
    served: FakeServer, tmp_path: Path, monkeypatch: pytest.MonkeyPatch, snapshot: SnapshotAssertion
) -> None:
    monkeypatch.chdir(tmp_path)
    assert kartenn("changeset", "simulate").exit_code == 2
    created = kartenn(
        "changeset", "new", "--motive", "Ouverture du lot 3", "--valid-from", "2026-11-01"
    )
    assert created.stdout == "Changeset local : ./.kartenn-changeset.yaml (0 opération)\n"
    assert kartenn("changeset", "new", "--motive", "x").exit_code == 2
    added = kartenn(
        "prefixes", "create", "--domain", DOMAIN, "--value", "203.0.113.0/26",
        "--nature", "network", "--into-changeset",
    )  # fmt: skip
    assert added.stdout == f"Ajouté (opération 0) : create_prefix {DOMAIN}/203.0.113.0/26\n"
    vlan = kartenn("vlan", "create", "--set", "vid=140", "--set", "name=lot3", "--into-changeset")
    assert vlan.stdout == "Ajouté (opération 1) : vlan.create_vlan 140\n"
    raw = kartenn("changeset", "add", '{"op": "create_domain", "slug": "lot3"}')
    assert raw.stdout == "Ajouté (opération 2) : create_domain\n"
    assert kartenn("changeset", "add").exit_code == 2
    assert kartenn("changeset", "add", "[1]").exit_code == 2
    served.route("POST", "/v1/changesets", httpx2.Response(200, json=SIMULATED))
    simulated = kartenn("changeset", "simulate", "-o", "table")
    assert simulated.exit_code == 0, simulated.output
    assert simulated.stdout == snapshot
    sent = body(served.seen[-1])
    assert sent["valid_from"] == "2026-11-01T00:00:00+01:00"
    assert [o["op"] for o in sent["operations"]] == [
        "create_prefix",
        "vlan.create_vlan",
        "create_domain",
    ]
    served.route("POST", "/v1/changesets", httpx2.Response(201, json=APPLIED))
    applied = kartenn("changeset", "apply")
    assert applied.exit_code == 0, applied.output
    key = served.seen[-1].headers["Idempotency-Key"]
    assert len(key) == 36
    assert not Path(".kartenn-changeset.yaml").exists()


def test_actions_on_a_changeset_of_the_server(served: FakeServer) -> None:
    record = {"id": CHANGESET, "state": "pending", "kind": "approval", "motive": "m", "diff": [
        {"property": "dns.ttl", "count": 2, "before": "60", "after": "300"},
        {"property": "secret.x", "count": 1, "masked": True},
    ]}  # fmt: skip
    served.route(
        "GET", "/v1/changesets/{id}", httpx2.Response(200, json=record, headers={"ETag": '"p1"'})
    )
    shown = kartenn("changeset", "show", CHANGESET, "-o", "table")
    assert shown.stdout == (
        f"Changeset {CHANGESET} — pending (approval)\n"
        "  Motif  m\n"
        "  dns.ttl   2 objets  60 → 300\n"
        "  secret.x  1 objet  \u2039masquée\u203a\n"
    )
    for action, path in (
        ("approve", "validation"),
        ("withdraw", "withdrawal"),
        ("undo", "undo"),
    ):
        served.route("POST", f"/v1/changesets/{{id}}/{path}", httpx2.Response(200, json=APPLIED))
        result = kartenn("changeset", action, CHANGESET)
        assert result.exit_code == 0, result.output
        assert served.seen[-1].headers.get("If-Match") == '"p1"'
    served.route(
        "POST", "/v1/changesets/{id}/rejection", httpx2.Response(200, json={"state": "rejected"})
    )
    assert kartenn("--no-input", "changeset", "reject", CHANGESET).exit_code == 2
    rejected = kartenn("changeset", "reject", CHANGESET, "--motive", "non", "-o", "json")
    assert json.loads(rejected.stdout) == {"state": "rejected"}
    served.route(
        "GET",
        "/v1/changesets/{id}/diff",
        {
            "items": [{"property": "dns.ttl", "object": {"id": "o"}, "before": 60, "after": 300}],
            "links": {},
        },
    )
    diff = kartenn(
        "changeset", "diff", CHANGESET, "--section", "policy.combinations", "-o", "ndjson"
    )
    assert json.loads(diff.stdout)["after"] == 300
    assert served.seen[-1].url.params["section"] == "policy.combinations"
    served.route("GET", "/v1/changesets", {"items": [record], "links": {}})
    listed = kartenn("changeset", "list", "-o", "ndjson", "--fields", "id")
    assert listed.stdout == json.dumps({"id": CHANGESET}) + "\n"
    served.route("GET", "/v1/proposals", {"items": [], "links": {}})
    assert kartenn("changeset", "proposals").stdout == "[]\n"


# Editor -------------------------------------------------------------------------------


def editor(tmp_path: Path, *replacements: tuple[str, str]) -> str:
    """An editor that applies one replacement per call and notes the file it was given."""
    script = tmp_path / "editor.py"
    calls = tmp_path / "calls"
    script.write_text(
        "import json, sys, pathlib\n"
        f"calls = pathlib.Path({str(calls)!r})\n"
        "seen = json.loads(calls.read_text()) if calls.exists() else []\n"
        "path = pathlib.Path(sys.argv[1])\n"
        f"old, new = {list(replacements)!r}[min(len(seen), {len(replacements) - 1})]\n"
        "seen.append([str(path), oct(path.stat().st_mode & 0o777), path.read_text()])\n"
        "calls.write_text(json.dumps(seen))\n"
        "path.write_text(path.read_text().replace(old, new))\n"
    )
    return f"{sys.executable} {script}"


def calls(tmp_path: Path) -> list[list[str]]:
    return json.loads((tmp_path / "calls").read_text())


@pytest.fixture
def terminal(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(Runtime, "interactive", property(lambda _self: True))


@pytest.mark.usefixtures("terminal")
def test_edit_sends_the_difference(
    served: FakeServer, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("EDITOR", editor(tmp_path, ("name: lot3", "name: lot3-prod")))
    monkeypatch.delenv("VISUAL", raising=False)
    served.route("PATCH", "/v1/vlans/{id}", httpx2.Response(200, json=APPLIED))
    result = kartenn("vlan", "edit", "site-nord/140", input="Renommage après mise en service\n")
    assert result.exit_code == 0, result.output
    assert "Modifications : name: lot3 → lot3-prod" in result.stdout
    assert f"Changeset {CHANGESET} appliqué." in result.stdout
    assert body(served.seen[-1]) == {
        "name": "lot3-prod",
        "motive": "Renommage après mise en service",
    }
    assert served.seen[-1].headers["If-Match"] == '"9d27"'
    path, mode, text = calls(tmp_path)[0]
    assert mode == "0o600"
    assert "# id: " in text
    assert not Path(path).exists()
    assert not Path(path).parent.exists()


@pytest.mark.usefixtures("terminal")
def test_edit_reopens_on_a_stale_version(
    served: FakeServer, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("EDITOR", editor(tmp_path, ("name: lot3", "name: lot3-prod")))
    answers = iter([problem(412, "precondition-failed"), httpx2.Response(200, json=APPLIED)])
    served.route("PATCH", "/v1/vlans/{id}", lambda _request: next(answers))
    result = kartenn("vlan", "edit", "site-nord/140", "--motive", "m", input="y\n")
    assert result.exit_code == 0, result.output
    second = calls(tmp_path)[1][2]
    assert "# name: lot3-prod" in second


@pytest.mark.usefixtures("terminal")
def test_edit_removes_its_file_on_an_error(
    served: FakeServer, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv("EDITOR", editor(tmp_path, ("name: lot3", "name: lot3-prod")))
    served.route("PATCH", "/v1/vlans/{id}", problem(412, "precondition-failed"))
    result = kartenn("vlan", "edit", "site-nord/140", "--motive", "m", input="n\n")
    assert result.exit_code == 5
    path = calls(tmp_path)[0][0]
    assert not Path(path).exists()


@pytest.mark.usefixtures("terminal")
def test_edit_without_change_or_with_a_broken_document(
    served: FakeServer, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    del served
    monkeypatch.setenv("EDITOR", editor(tmp_path, ("nothing", "nothing")))
    assert kartenn("vlan", "edit", "site-nord/140").stdout == "Aucune modification.\n"
    monkeypatch.setenv("EDITOR", editor(tmp_path, ("name: lot3", "name: [")))
    broken = kartenn("vlan", "edit", "site-nord/140", "--motive", "m")
    assert broken.exit_code == 2
    assert "client.invalid-document" in broken.stderr


def test_edit_needs_a_terminal(served: FakeServer) -> None:
    del served
    result = kartenn("vlan", "edit", "site-nord/140")
    assert result.exit_code == 2
    assert "client.input-required" in result.stderr
    assert os.environ.get("NO_COLOR") == "1"
