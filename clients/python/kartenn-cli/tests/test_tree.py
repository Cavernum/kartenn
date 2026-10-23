"""The tree derived from the contract (REQ-023-18, 21; T13; maquette § 2, 3, 12)."""

from __future__ import annotations

import copy
from typing import TYPE_CHECKING, Any

import pytest

from fakes import CONTRACT, MODULES, FakeServer, capabilities, kartenn, without
from kartenn import NetworkError
from kartenn.catalog import CatalogCache
from kartenn_cli import runtime
from kartenn_cli.config import Settings
from kartenn_cli.runtime import Runtime
from kartenn_cli.tree import layout

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion


class Refusing:
    """A transport whose server cannot be reached."""

    def send(self, request: object) -> Any:  # noqa: ANN401
        raise NetworkError("client.network", f"connection refused: {request}")

    def stream(self, request: object) -> Any:  # noqa: ANN401
        return self.send(request)

    def sse(self, request: object) -> Any:  # noqa: ANN401
        return self.send(request)

    def sleep(self, seconds: float) -> None:
        pass

    def close(self) -> None:
        pass


PREFIX = "7c1e0000-0000-7000-8000-000000000001"


def refusing(url: str, profile: object) -> Any:  # noqa: ANN401 - a transport
    del url, profile
    return Refusing()


def labo(server: FakeServer) -> None:
    """The server `labo`: 0.1.4, without `policy` nor `flow`."""
    server.contract = without(CONTRACT, "policy", "flow")
    server.capabilities = capabilities(
        version="0.1.4",
        modules=[{"name": "vlan", "version": "0.1.4"}, {"name": "machines", "version": "0.1.4"}],
    )


def demo_contract() -> dict[str, Any]:
    """The reference contract and a module of tomorrow, `demo`, with one collection."""
    contract = copy.deepcopy(CONTRACT)
    listing = copy.deepcopy(contract["paths"]["/v1/sites"]["get"])
    listing.update(operationId="list_demo_widgets", tags=["demo"], summary="The widgets.")
    listing["parameters"] = [
        {"name": "shape", "in": "query", "schema": {"type": "string", "enum": ["round", "flat"]}}
    ]
    show = copy.deepcopy(contract["paths"]["/v1/sites/{id}"]["get"])
    show.update(operationId="show_demo_widget", tags=["demo"], summary="A widget.")
    contract["paths"]["/v1/demo-widgets"] = {"get": listing}
    contract["paths"]["/v1/demo-widgets/{id}"] = {"get": show}
    return contract


def test_root_help_lists_the_modules_served(
    server: FakeServer, snapshot: SnapshotAssertion
) -> None:
    result = kartenn("--help")
    assert result.exit_code == 0, result.output
    assert result.stdout == snapshot
    assert result.stderr.startswith("Découverte du serveur… 0.2.0")
    again = kartenn("--help")
    assert again.stderr == ""
    assert server.seen == []


def test_root_help_of_a_server_without_policy(server: FakeServer) -> None:
    labo(server)
    result = kartenn("--help")
    modules = result.stdout.split("Modules servis:\n", 1)[1].split("\n\n", 1)[0]
    assert modules.splitlines() == [
        "  vlan      VLAN et groupes de VLAN.   [module vlan 0.1.4]",
        "  machines  Machines, interfaces, grappes.   [module machines 0.1.4]",
    ]


@pytest.mark.parametrize(
    "path",
    [
        ("policy",),
        ("flow",),
        ("vlan",),
        ("machines",),
        ("prefixes",),
        ("audit",),
        ("policy", "rules", "list"),
        ("prefixes", "properties"),
        ("service-accounts", "secrets"),
    ],
)
def test_help_of_each_group(
    server: FakeServer, snapshot: SnapshotAssertion, path: tuple[str, ...]
) -> None:
    del server
    result = kartenn(*path, "--help")
    assert result.exit_code == 0, result.output
    assert result.stdout == snapshot


def test_help_of_a_derived_verb_names_its_operation(server: FakeServer) -> None:
    del server
    result = kartenn("prefixes", "history", "--help")
    assert "GET /v1/prefixes/{id}/history  (prefix_history) — module core" in result.stdout
    assert "Usage: kartenn prefixes history [OPTIONS] REF" in result.stdout
    result = kartenn("--lang", "en", "vlan", "list", "--help")
    assert "GET|QUERY /v1/vlans  (list_vlans) — module vlan" in result.stdout


def test_a_module_not_served_answers_its_error(server: FakeServer) -> None:
    labo(server)
    kartenn("--help")
    result = kartenn("policy", "check", "machine:bastion-1", "--service", "ssh")
    assert result.exit_code == 2
    assert result.stderr == (
        "erreur [client.module-unavailable] Le module « policy » n'est pas servi par ce "
        "serveur.\n"
        "  Profil prod — https://kartenn.example.org, version 0.1.4\n"
        "  Modules servis : vlan, machines\n"
        "  La commande existe quand le serveur sert le module ; aucune mise à jour du client "
        "n'est nécessaire.\n"
    )
    help_asked = kartenn("policy", "--help")
    assert help_asked.exit_code == 2
    assert "client.module-unavailable" in help_asked.stderr


def test_a_sub_resource_of_a_missing_module(server: FakeServer) -> None:
    labo(server)
    kartenn("--help")
    result = kartenn("addresses", "flows", "2b10")
    assert result.exit_code == 2
    assert result.stderr.splitlines()[0] == (
        "erreur [client.module-unavailable] « addresses flows » est servi par le module "
        "« flow », absent de ce serveur."
    )
    assert kartenn("addresses", "nothing").exit_code == 2
    assert kartenn("nothing").exit_code == 2


def test_a_module_added_to_the_server_is_discovered(
    server: FakeServer, monkeypatch: pytest.MonkeyPatch
) -> None:
    labo(server)
    assert kartenn("--help").exit_code == 0
    server.contract = CONTRACT
    server.capabilities = capabilities(
        version="0.2.0",
        modules=[*capabilities()["modules"][:2], {"name": "policy", "version": "0.2.0"}],
    )
    monkeypatch.setattr(Runtime, "cache", property(lambda _self: CatalogCache(max_age=0)))
    result = kartenn("policy", "--help")
    assert result.exit_code == 0, result.output
    assert result.stderr == (
        "Découverte du serveur… 0.2.0 (avant 0.1.4), modules : vlan, machines, policy "
        "(catalogue mis à jour).\n"
    )
    assert result.stdout.splitlines()[2].startswith("  Politique de flux")


def test_a_module_of_tomorrow_is_derived(server: FakeServer) -> None:
    server.contract = demo_contract()
    server.capabilities = capabilities(modules=[*MODULES, {"name": "demo", "version": "0.0.1"}])
    root = kartenn("--help")
    assert "  demo      Module demo.   [module demo 0.0.1]" in root.stdout
    result = kartenn("demo", "--help")
    assert result.exit_code == 0
    assert "widgets" in result.stdout
    listing = kartenn("demo", "widgets", "list", "--help")
    assert "--shape [round|flat]" in listing.stdout


def test_layout_of_the_reference_contract(server: FakeServer) -> None:
    del server
    found = layout(Runtime(Settings()).catalog)
    assert found.mains == {"flow": "flows", "machines": "machines", "vlan": "vlans"}
    assert found.modules["policy"]["rules"] == "policy-rules"
    assert found.modules["flow"] == {"nonconformities": "flow-nonconformities"}
    assert found.modules["machines"] == {"clusters": "clusters", "interfaces": "interfaces"}
    assert found.resources["audit"] == "audit-events"
    assert "changesets" not in found.resources
    assert found.sections["roles"] == "rights"
    assert found.sections["log-filters"] == "tools"


def test_a_derived_verb_calls_its_operation(server: FakeServer) -> None:
    server.route("GET", "/v1/prefixes/{id}/occupancy", {"used": "3", "total": "256"})
    result = kartenn("prefixes", "occupancy", PREFIX, "-o", "json")
    assert result.exit_code == 0, result.output
    assert result.stdout == '{"used": "3", "total": "256"}\n'
    assert server.seen[-1].url.path == f"/v1/prefixes/{PREFIX}/occupancy"
    server.route("GET", "/v1/vlans", {"items": [], "links": {}})
    listed = kartenn("vlan", "list", "--group", "a", "--group", "b", "--fields", "vid")
    assert listed.exit_code == 0, listed.output
    assert server.seen[-1].url.params.get_list("group") == ["a", "b"]


def test_without_a_server_the_local_commands_stay(server: FakeServer) -> None:
    runtime.TRANSPORT["factory"] = refusing
    del server
    result = kartenn("--help")
    assert result.exit_code == 0
    assert "config" in result.stdout
    assert result.stderr.startswith("Catalogue indisponible")
