"""Renderings of data and of problems on frozen answers (REQ-023-25, 26, 33; T15, T16)."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any
from zoneinfo import ZoneInfo

import pytest
from click.testing import CliRunner

from kartenn import Refused
from kartenn_cli.render import Output, format_moment
from kartenn_cli.render.data import dump_json_list, flatten, parse_fields, pick, project
from kartenn_cli.render.table import cell, default_columns

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion

PREFIXES: list[dict[str, Any]] = [
    {
        "id": "7c1e0000-0000-7000-8000-000000000001",
        "domain": {"id": "3f02", "href": "/v1/domains/3f02", "slug": "interne"},
        "value": "198.51.100.0/24",
        "nature": "container",
        "public": False,
        "links": {"self": "/v1/prefixes/7c1e"},
        "properties": {"policy.environment": "prod"},
    },
    {
        "id": "7c1e0000-0000-7000-8000-000000000002",
        "domain": {"id": "3f02", "href": "/v1/domains/3f02", "slug": "interne", "label": "Interne"},
        "value": "198.51.100.0/26",
        "nature": "network",
        "public": False,
        "links": {"self": "/v1/prefixes/7c1f"},
        "properties": {"policy.environment": "prod", "dns.ttl": {"masked": True}},
    },
]


def rendered(output: Output, data: object, *, items: bool = True) -> str:
    runner = CliRunner()
    with runner.isolation() as (out, _err, _):
        if items:
            output.items(data, footer=lambda count: f"{count} éléments")  # type: ignore[arg-type]
        else:
            output.value(data)
        return out.getvalue().decode()


@pytest.mark.parametrize("fmt", ["table", "json", "ndjson", "yaml", "csv"])
def test_formats_of_a_list(fmt: str, snapshot: SnapshotAssertion) -> None:
    assert rendered(Output(format=fmt), PREFIXES) == snapshot


def test_table_keys_ids_and_fields(snapshot: SnapshotAssertion) -> None:
    assert rendered(Output(format="table", keys=True, ids=True), PREFIXES) == snapshot
    fields = parse_fields("value,properties.policy.environment,properties.dns.ttl")
    assert rendered(Output(format="table", fields=fields), PREFIXES) == snapshot
    assert rendered(Output(format="json", fields=fields), PREFIXES) == snapshot


def test_declared_columns_come_first() -> None:
    assert default_columns(PREFIXES, ("value", "domain")) == ["value", "domain"]
    assert default_columns(PREFIXES) == ["domain", "value", "nature", "public"]
    assert default_columns(PREFIXES, ids=True)[0] == "id"


def test_query_projects_before_rendering() -> None:
    output = Output(format="json", query="[?nature=='network'].value")
    assert rendered(output, PREFIXES) == '["198.51.100.0/26"]\n'


def test_values(snapshot: SnapshotAssertion) -> None:
    item = PREFIXES[1]
    for fmt in ("table", "json", "yaml", "csv"):
        assert rendered(Output(format=fmt), item, items=False) == snapshot(name=fmt)
    assert rendered(Output(format="table"), "texte", items=False) == "texte\n"
    assert rendered(Output(format="csv"), 3, items=False) == "3\n"
    assert rendered(Output(format="json"), PREFIXES, items=False).startswith("[\n  {")
    assert rendered(Output(format="json", query="length(@)"), PREFIXES, items=False) == "2\n"
    assert rendered(Output(format="json", fields=["value"]), item, items=False) == (
        '{"value": "198.51.100.0/26"}\n'
    )


def test_an_empty_list() -> None:
    assert rendered(Output(format="json"), []) == "[]\n"
    assert "".join(dump_json_list([])) == "[]\n"
    assert rendered(Output(format="table"), []) == "0 éléments\n"


def test_cells() -> None:
    assert cell(None) == "—"
    assert cell(True) == "true"  # noqa: FBT003 - a value
    assert cell({"masked": True, "value": None}) == "\u2039masquée\u203a"
    assert cell({"id": "1", "href": "/x", "slug": "s", "label": "L"}) == "L"
    assert cell({"id": "1", "href": "/x", "slug": "s", "label": "L"}, keys=True) == "s"
    assert cell({"id": "1", "href": "/x"}) == "1"
    assert cell({"a": 1}) == '{"a": 1}'
    assert cell([1, {"id": "2", "slug": "b"}]) == "1, b"
    assert cell([]) == "—"


def test_picking_dotted_keys() -> None:
    assert pick(PREFIXES[0], "properties.policy.environment") == (
        ["properties", "policy.environment"],
        "prod",
    )
    assert project(PREFIXES[0], ["value", "properties.policy.environment", "absent"]) == {
        "value": "198.51.100.0/24",
        "properties": {"policy.environment": "prod"},
    }
    assert pick({"a": 1}, "a.b")[0] == ["a"]
    assert flatten({"a": {"b": 1, "c": {}}, "d": [1]}) == {"a.b": 1, "a.c": {}, "d": [1]}
    assert parse_fields(None) is None
    assert parse_fields("a, b,,") == ["a", "b"]


def test_moments_in_the_zone_of_the_profile() -> None:
    zone = ZoneInfo("Europe/Paris")
    assert format_moment("2026-10-23T08:14:03Z", zone) == "2026-10-23 10:14:03+02:00"
    assert format_moment("2026-10-23T08:14:03Z", None) == "2026-10-23 08:14:03+00:00"
    assert format_moment("2026-10-23T08:14:03", zone) == "2026-10-23T08:14:03"
    assert format_moment("demain", zone) == "demain"
    assert format_moment(None, zone) is None


def test_problems(snapshot: SnapshotAssertion) -> None:
    from kartenn_cli.render.problems import render_error  # noqa: PLC0415

    refused = Refused(
        {
            "type": "urn:kartenn:problem:changeset-refused",
            "title": "Changeset refusé",
            "status": 422,
            "instance": "urn:uuid:5b0e",
            "changeset": "0c41",
            "violations": [
                {
                    "rule": "core.prefix.overlap",
                    "severity": "error",
                    "pointer": "/operations/0",
                    "detail": "Le préfixe 198.51.100.0/25 chevauche 198.51.100.0/24 (interne).",
                },
                {"rule": "core.prefix.unused", "severity": "warning", "detail": "Aucun enfant"},
                {"rule": "core.x", "severity": "error", "operation": 3, "detail": "Trois"},
            ],
        },
        status=422,
    )
    assert render_error(refused) == snapshot(name="text")
    assert render_error(refused, json_output=True) == snapshot(name="json")
    assert "\x1b[" in render_error(refused, color=True)
