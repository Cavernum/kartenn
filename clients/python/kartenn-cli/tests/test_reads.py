"""Reads: dates, filters, outputs, pages, references (REQ-023-22, 23, 25, 26, 27;
maquette § 4)."""

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

import httpx2
import pytest
from click.testing import CliRunner

from fakes import CONTRACT, FakeServer, kartenn, problem, with_keys
from kartenn_cli.main import cli

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

INTERNE = {
    "id": "3f020000-0000-7000-8000-000000000001",
    "href": "/v1/domains/3f02",
    "slug": "interne",
}


def prefix(number: int, value: str, nature: str, **members: Any) -> dict[str, Any]:  # noqa: ANN401
    return {
        "id": f"7c1e0000-0000-7000-8000-{number:012d}",
        "domain": INTERNE,
        "value": value,
        "nature": nature,
        "point_to_point": False,
        "anycast": False,
        "public": False,
        "links": {"self": f"/v1/prefixes/{number}"},
        **members,
    }


PREFIXES = [
    prefix(1, "198.51.100.0/24", "container"),
    prefix(2, "198.51.100.0/26", "network", properties={"policy.environment": "prod"}),
    prefix(3, "198.51.100.64/26", "network", properties={"policy.environment": "recette"}),
    prefix(4, "198.51.100.128/25", "network"),
]


def page(
    items: list[dict[str, Any]], *, following: str | None = None, **members: object
) -> dict[str, Any]:
    return {"items": items, "links": {"next": following} if following else {}, **members}


@pytest.fixture
def keyed(server: FakeServer) -> FakeServer:
    server.contract = with_keys(CONTRACT)
    kartenn("--help")
    return server


def test_list_in_a_table_as_the_maquette(keyed: FakeServer, snapshot: SnapshotAssertion) -> None:
    keyed.route("GET", "/v1/prefixes", page(PREFIXES, as_of="2026-10-23T08:14:03Z"))
    result = kartenn("prefixes", "list", "--domain", "interne", "-o", "table")
    assert result.exit_code == 0, result.output
    assert result.stdout == snapshot
    assert keyed.seen[-1].url.params.get_list("domain") == ["interne"]


def test_dates_are_resolved_in_the_zone_of_the_profile(keyed: FakeServer) -> None:
    keyed.route("GET", "/v1/prefixes", page(PREFIXES[:2]))
    result = kartenn(
        "prefixes", "list", "--at", "2026-06-01", "--as-of", "2026-07-01", "-o", "table"
    )
    assert result.exit_code == 0, result.output
    params = keyed.seen[-1].url.params
    assert params["valid_at"] == "2026-06-01T00:00:00+02:00"
    assert params["as_of"] == "2026-07-01T00:00:00+02:00"
    assert result.stdout.splitlines()[-1] == (
        "2 éléments — valide au 2026-06-01 00:00:00+02:00, tel qu'affirmé le "
        "2026-07-01 00:00:00+02:00"
    )
    root = kartenn("--at", "2026-06-01", "prefixes", "list", "-o", "table")
    assert root.stdout.splitlines()[-1] == "2 éléments — valide au 2026-06-01 00:00:00+02:00"


def test_a_date_without_offset_is_refused_locally(keyed: FakeServer) -> None:
    result = kartenn("prefixes", "list", "--at", "2026-06-01T10:00:00")
    assert result.exit_code == 2
    assert "client.date-without-offset" in result.stderr
    assert keyed.seen == []


def test_freeze_asks_now_and_says_how_to_keep_it(keyed: FakeServer) -> None:
    keyed.route("GET", "/v1/prefixes", page(PREFIXES[:1]))
    result = kartenn("prefixes", "list", "--freeze", "-o", "table")
    as_of = keyed.seen[-1].url.params["as_of"]
    assert as_of.endswith("Z")
    assert result.stdout.splitlines()[-1] == f"figé : export KARTENN_AS_OF={as_of}"


def test_prop_and_fields_in_json(keyed: FakeServer) -> None:
    keyed.route("GET", "/v1/prefixes", page(PREFIXES[1:3]))
    result = kartenn(
        "prefixes",
        "list",
        "--domain",
        "interne",
        "--prop",
        "policy.environment=prod",
        "--prop",
        "dns.ttl:between=60..",
        "--prop",
        "dns.fqdn:exists=true",
        "-o",
        "json",
        "--fields",
        "value,properties.policy.environment",
    )
    assert result.exit_code == 0, result.output
    assert result.stdout == (
        "[\n"
        '  {"value": "198.51.100.0/26", "properties": {"policy.environment": "prod"}},\n'
        '  {"value": "198.51.100.64/26", "properties": {"policy.environment": "recette"}}\n'
        "]\n"
    )
    params = keyed.seen[-1].url.params
    assert params["prop.policy.environment"] == "prod"
    assert params["prop.dns.ttl:between"] == "60.."
    assert params["prop.dns.fqdn:exists"] == "true"


def test_a_property_repeated_is_one_of(keyed: FakeServer) -> None:
    keyed.route("GET", "/v1/prefixes", page([]))
    kartenn("prefixes", "list", "--prop", "a.b=1", "--prop", "a.b=2", "--prop", "a.b=3")
    assert keyed.seen[-1].url.params.get_list("prop.a.b") == ["1", "2", "3"]


def test_the_filters_of_the_contract_are_options(keyed: FakeServer) -> None:
    keyed.route("GET", "/v1/prefixes", page([]))
    result = kartenn("prefixes", "list", "--within", "198.51.100.0/24", "--nature", "network")
    assert result.exit_code == 0, result.stderr
    params = keyed.seen[-1].url.params
    assert params["within"] == "198.51.100.0/24"
    assert params["nature"] == "network"


def test_an_unknown_filter_is_refused_without_a_request(keyed: FakeServer) -> None:
    result = kartenn("prefixes", "list", "--colour", "red")
    assert result.exit_code == 2
    assert result.stderr == (
        "erreur [client.unknown-filter] Filtre inconnu : --colour. Filtres : --domain, "
        "--value, --within, --nature, --prop CLÉ[:OP]=VALEUR.\n"
    )
    # A list that does not filter by property offers no --prop.
    result = kartenn("entities", "list", "--prop", "a.b=1")
    assert result.exit_code == 2
    assert result.stderr == (
        "erreur [client.unknown-filter] Filtre inconnu : --prop. Filtres : --slug, --parent.\n"
    )
    assert keyed.seen == []
    bad = kartenn("prefixes", "list", "--prop", "nokey=1")
    assert bad.exit_code == 2
    assert "client.invalid-filter" in bad.stderr


def test_next_page_and_all(keyed: FakeServer) -> None:
    following = "/v1/prefixes?cursor=k2"

    def answer(request: httpx2.Request) -> httpx2.Response:
        if request.url.params.get("cursor") == "k2":
            return httpx2.Response(200, json=page(PREFIXES[2:]))
        return httpx2.Response(200, json=page(PREFIXES[:2], following=following))

    keyed.route("GET", "/v1/prefixes", answer)
    first = kartenn("prefixes", "list", "-o", "table")
    assert first.stdout.splitlines()[-2:] == ["2 éléments", "suite : --all ou --cursor k2"]
    as_json = kartenn("prefixes", "list")
    assert len(json.loads(as_json.stdout)) == 2
    assert as_json.stderr == "suite : --all ou --cursor k2\n"
    every = kartenn("prefixes", "list", "--all", "-o", "ndjson")
    assert len(every.stdout.splitlines()) == 4
    cursor = kartenn("prefixes", "list", "--cursor", "k2", "-o", "csv", "--fields", "value")
    assert cursor.stdout.splitlines() == ["value", "198.51.100.64/26", "198.51.100.128/25"]
    frozen = kartenn("prefixes", "list", "--cursor", "k2", "--domain", "interne")
    assert frozen.exit_code == 2
    assert "client.cursor-frozen" in frozen.stderr


def test_ndjson_all_is_written_as_it_is_read(keyed: FakeServer) -> None:
    """Page 1 is on the standard output before page 2 is asked: the list is never held."""
    runner = CliRunner()
    written_before: list[int] = []
    with runner.isolation() as (out, _err, _):

        def answer(request: httpx2.Request) -> httpx2.Response:
            if request.url.params.get("cursor") == "k2":
                written_before.append(out.getvalue().count(b"\n"))
                return httpx2.Response(200, json=page(PREFIXES[2:]))
            return httpx2.Response(200, json=page(PREFIXES[:2], following="/v1/prefixes?cursor=k2"))

        keyed.route("GET", "/v1/prefixes", answer)
        with pytest.raises(SystemExit) as done:
            cli.main(["prefixes", "list", "--all", "-o", "ndjson"], prog_name="kartenn")
        assert done.value.code == 0
        assert out.getvalue().count(b"\n") == 4
    assert written_before == [2]


def test_query_projects_the_list(keyed: FakeServer) -> None:
    keyed.route("GET", "/v1/prefixes", page(PREFIXES))
    result = kartenn("prefixes", "list", "--query", "[?nature=='container'].value")
    assert result.stdout == '["198.51.100.0/24"]\n'


def test_get_by_natural_key(keyed: FakeServer, snapshot: SnapshotAssertion) -> None:
    keyed.route("GET", "/v1/prefixes", page(PREFIXES[:1]))
    keyed.route("GET", "/v1/prefixes/{id}", PREFIXES[0])
    result = kartenn("prefixes", "get", "interne/198.51.100.0/24", "-o", "table", "--keys")
    assert result.exit_code == 0, result.output
    assert result.stdout == snapshot
    lookup, read = keyed.seen[-2:]
    assert lookup.url.params["domain"] == "interne"
    assert lookup.url.params["value"] == "198.51.100.0/24"
    assert read.url.path == f"/v1/prefixes/{PREFIXES[0]['id']}"


def test_references_refused_locally(keyed: FakeServer) -> None:
    keyed.route("GET", "/v1/prefixes", page([]))
    missing = kartenn("prefixes", "get", "interne/203.0.113.0/24")
    assert missing.exit_code == 3
    assert "client.reference-not-found" in missing.stderr
    keyed.route("GET", "/v1/prefixes", page(PREFIXES[:2]))
    ambiguous = kartenn("prefixes", "get", "interne/198.51.100.0/24")
    assert ambiguous.exit_code == 2
    assert "client.ambiguous-reference" in ambiguous.stderr
    shape = kartenn("prefixes", "get", "interne")
    assert shape.exit_code == 2
    assert "client.invalid-reference" in shape.stderr
    no_key = kartenn("machines", "get", "bastion-1")
    assert no_key.exit_code == 2
    assert "client.key-unknown" in no_key.stderr


def test_a_uuid_is_used_as_given(keyed: FakeServer) -> None:
    keyed.route("GET", "/v1/sites/{id}", {"id": "s", "slug": "site-nord", "links": {}})
    result = kartenn("sites", "get", PREFIXES[0]["id"], "-o", "yaml")
    assert result.stdout == "id: s\nslug: site-nord\nlinks: {}\n"
    keyed.route("GET", "/v1/sites/{id}", problem(404, "not-found", title="Introuvable"))
    gone = kartenn("sites", "get", PREFIXES[0]["id"])
    assert gone.exit_code == 3
    assert gone.stderr.startswith("erreur [not-found] Introuvable (404)")


def test_a_paged_sub_resource(keyed: FakeServer) -> None:
    following = f"/v1/prefixes/{PREFIXES[0]['id']}/history?cursor=h2"

    def answer(request: httpx2.Request) -> httpx2.Response:
        if request.url.params.get("cursor") == "h2":
            return httpx2.Response(200, json=page([{"version": 1}]))
        return httpx2.Response(200, json=page([{"version": 2}], following=following))

    keyed.route("GET", "/v1/prefixes/{id}/history", answer)
    result = kartenn("prefixes", "history", PREFIXES[0]["id"], "--all", "-o", "ndjson")
    assert result.stdout == '{"version": 2}\n{"version": 1}\n'
    keyed.route("GET", "/v1/prefixes/{id}/properties", {"properties": {"a.b": 1}, "masked": []})
    shown = kartenn("prefixes", "properties", PREFIXES[0]["id"], "-o", "json")
    assert json.loads(shown.stdout) == {"properties": {"a.b": 1}, "masked": []}
    keyed.route(
        "GET", "/v1/prefixes/{id}/properties/{key}/explanation", {"value": 3, "masked": False}
    )
    explained = kartenn("prefixes", "properties", "explain", PREFIXES[0]["id"], "a.b", "-o", "json")
    assert json.loads(explained.stdout) == {"value": 3, "masked": False}
    assert keyed.seen[-1].url.path.endswith("/properties/a.b/explanation")
