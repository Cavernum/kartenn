import copy
import json
import os
import stat
from pathlib import Path
from typing import Any, cast

import pytest
from hypothesis import given
from hypothesis import strategies as st

from kartenn._core.messages import Headers, Request, Response
from kartenn._core.plans import run
from kartenn._core.problems import ClientError
from kartenn.catalog import Catalog, CatalogCache, UnknownResource, derive, discover

ROOT = Path(__file__).resolve().parents[4]
CONTRACT: dict[str, Any] = json.loads((ROOT / "specs/006-api-rest/openapi-v1.json").read_text())
SNAPSHOT = Path(__file__).parent / "snapshots" / "catalog-openapi-v1.json"
SERVER = "https://kartenn.test"
MODULES = [
    {"name": "policy", "version": "0.1.0"},
    {"name": "flow", "version": "0.1.0"},
    {"name": "vlan", "version": "0.1.0"},
    {"name": "machines", "version": "0.1.0"},
]


def capabilities(
    version: str = "0.1.0", modules: list[dict[str, str]] | None = None
) -> dict[str, Any]:
    return {
        "version": version,
        "api_versions": ["v1"],
        "modules": MODULES if modules is None else modules,
        "properties": [],
        "ui_plugins": [],
        "limits": {
            "page_size_default": 100,
            "page_size_max": 1000,
            "changeset_operations": 10000,
            "chunk_elements": 10000,
            "body_bytes": 1 << 24,
            "idempotency_retention_seconds": 86400,
        },
        "events": None,
    }


def summary(catalog: Catalog) -> dict[str, Any]:
    return {
        resource.name: {
            "module": resource.module,
            "verbs": sorted(f"{v.level}:{v.name}:{v.operation_id}" for v in resource.verbs),
        }
        for resource in catalog.resources
    }


def test_snapshot_of_the_reference_contract() -> None:
    catalog = derive(SERVER, capabilities(), CONTRACT)
    found = summary(catalog)
    if os.environ.get("KARTENN_SNAPSHOT_UPDATE"):
        SNAPSHOT.parent.mkdir(exist_ok=True)
        SNAPSHOT.write_text(json.dumps(found, indent=1, sort_keys=True) + "\n")
    assert found == json.loads(SNAPSHOT.read_text())


def test_conventions_of_the_reference_contract() -> None:
    catalog = derive(SERVER, capabilities(), CONTRACT)
    prefixes = catalog.resource("prefixes")
    assert prefixes.module == "core"
    assert not catalog.is_module(prefixes)
    assert {
        "list",
        "query",
        "create",
        "get",
        "update",
        "close",
        "history",
        "children",
        "properties",
        "properties.explain",
        "properties.suggest",
        "flows",
    } <= set(prefixes.names)
    flows = prefixes.verb("flows")
    assert flows is not None
    assert catalog.operation(flows.operation_id).module == "flow"
    rules = catalog.resource("policy-rules")
    assert rules.module == "policy"
    assert catalog.is_module(rules)
    interfaces = catalog.resource("interfaces")
    assert {"attachments", "attachments.delete"} <= set(interfaces.names)
    assert catalog.resource("graph").names == ("edges",)
    assert catalog.resource("roles").verb("set") is not None
    listing = catalog.operation("list_prefixes")
    assert [p.name for p in listing.filters] == ["domain", "value"]
    assert catalog.operation("modify_prefix").needs_if_match
    assert catalog.operation("modify_prefix").body_media == "application/merge-patch+json"
    assert catalog.operation("create_prefix").takes_idempotency_key
    assert catalog.operation("query_prefixes").method == "QUERY"
    assert catalog.operation("list_changesets").path_params == ()
    assert catalog.missing_socle == ()
    assert catalog.has("vlan")
    assert catalog.limits["chunk_elements"] == 10000
    assert catalog.events is None
    assert not catalog.has_resource("auth")
    assert catalog.has_operation("live")


def without(contract: dict[str, Any], tag: str) -> dict[str, Any]:
    """The contract of a build without a module: its operations, and the sub-resources the
    core serves on its collections (properties), are gone."""
    trimmed = copy.deepcopy(contract)
    paths: dict[str, dict[str, Any]] = trimmed["paths"]

    def tags_of(value: object) -> set[str]:
        return (
            set(cast("dict[str, list[str]]", value).get("tags", []))
            if isinstance(value, dict)
            else set()
        )

    def tags(item: dict[str, Any]) -> set[str]:
        return {t for op in item.values() for t in tags_of(op)}

    gone = {path.split("/")[2] for path, item in paths.items() if tag in tags(item)}
    gone -= {path.split("/")[2] for path, item in paths.items() if tags(item) - {tag, "properties"}}
    for path in list(paths):
        if path.split("/")[2] in gone:
            del paths[path]
        else:
            paths[path] = {k: v for k, v in paths[path].items() if tag not in tags_of(v)}
    return trimmed


def test_the_tags_of_the_base_listed_as_modules_stay_the_core() -> None:
    """kartenn-server lists the tags of its base (`core`, `rights`…) among its modules
    (seen by the end-to-end sessions, task 26): their collections stay at the root."""
    base = [{"name": n, "version": "0.1.0"} for n in ("core", "rights", "audit", "operations")]
    catalog = derive(SERVER, capabilities(modules=[*base, *MODULES]), CONTRACT)
    assert catalog.has("core")
    assert catalog.served_modules == ("policy", "flow", "vlan", "machines")
    assert not catalog.is_module(catalog.resource("prefixes"))
    assert not catalog.is_module(catalog.resource("bindings"))
    assert catalog.is_module(catalog.resource("vlans"))


def test_contract_without_policy_has_no_policy_resource() -> None:
    served = [m for m in MODULES if m["name"] != "policy"]
    catalog = derive(SERVER, capabilities(modules=served), without(CONTRACT, "policy"))
    assert all(r.module != "policy" for r in catalog.resources)
    assert not catalog.has("policy")
    with pytest.raises(UnknownResource) as caught:
        catalog.resource("policy-rules")
    assert caught.value.code == "client.unknown-resource"
    assert "module 'policy' is not served" in str(caught.value)
    assert "flow, vlan, machines" in str(caught.value)
    with pytest.raises(ClientError) as missing:
        catalog.operation("show_verdicts")
    assert missing.value.code == "client.operation-unavailable"


def demo_contract() -> dict[str, Any]:
    def op(op_id: str, **extra: object) -> dict[str, Any]:
        return {"operationId": op_id, "tags": ["demo"], "responses": {"200": {}}, **extra}

    item = {"$ref": "#/components/parameters/Id"}
    return {
        "paths": {
            "/v1/demo-widgets": {
                "x-kartenn": {"key": ["name"], "columns": ["name", "size"]},
                "get": op(
                    "list_widgets",
                    parameters=[
                        {
                            "name": "colour",
                            "in": "query",
                            "schema": {"$ref": "#/components/schemas/Colour"},
                        },
                        {
                            "name": "tags",
                            "in": "query",
                            "schema": {"type": "array", "items": {"type": "string"}},
                        },
                    ],
                ),
                "post": op(
                    "create_widget",
                    requestBody={
                        "content": {
                            "application/json": {"schema": {"$ref": "#/components/schemas/Widget"}}
                        }
                    },
                ),
                "x-query": op("query_widgets"),
            },
            "/v1/demo-widgets/{id}": {
                "parameters": [item],
                "get": op("show_widget"),
                "patch": op("modify_widget"),
                "delete": op("close_widget"),
            },
            "/v1/demo-widgets/{id}/polish": {
                "post": op("polish_widget", **{"x-kartenn": {"command": "shine"}})
            },
            "/v1/demo-widgets/{id}/history": {"get": op("widget_history")},
            "/v1/demo-widgets/{id}/history2": {"get": op("widget_history_again")},
            "/v1/demo-widgets/{id}/parts/{part}": {
                "get": op("widget_part"),
                "put": op("set_widget_part"),
            },
        },
        "components": {
            "parameters": {
                "Id": {
                    "name": "id",
                    "in": "path",
                    "required": True,
                    "schema": {"type": ["string", "null"]},
                }
            },
            "schemas": {
                "Colour": {"enum": ["red", "blue"], "type": "string"},
                "Widget": {"type": "object"},
            },
        },
    }


def test_module_unknown_to_the_library_is_derived() -> None:
    catalog = derive(
        SERVER, capabilities(modules=[{"name": "demo", "version": "1"}]), demo_contract()
    )
    widgets = catalog.resource("demo-widgets")
    assert widgets.module == "demo"
    assert catalog.is_module(widgets)
    assert widgets.key == ("name",)
    assert widgets.columns == ("name", "size")
    assert set(widgets.names) == {
        "list",
        "create",
        "query",
        "get",
        "update",
        "close",
        "shine",
        "history",
        "history2",
        "parts",
        "parts.put",
    }
    colour, tags = catalog.operation("list_widgets").filters
    assert colour.enum == ("red", "blue")
    assert tags.repeated
    assert catalog.operation("show_widget").param("id") is not None
    assert catalog.operation("show_widget").params[0].type == "string"
    assert catalog.operation("create_widget").body == {"$ref": "#/components/schemas/Widget"}
    assert set(catalog.missing_socle) >= {"capabilities", "openapi"}


collection_names = st.from_regex(r"[a-z]{2,8}(-[a-z]{2,8}){0,2}", fullmatch=True).filter(
    lambda name: name not in {"auth", "capabilities", "me"}
)


@given(st.lists(collection_names, min_size=1, max_size=5, unique=True))
def test_any_collection_name_is_derived(names: list[str]) -> None:
    paths: dict[str, Any] = {}
    for name in names:
        paths[f"/v1/{name}"] = {"get": {"operationId": f"list_{name}", "tags": ["m"]}}
        paths[f"/v1/{name}/{{id}}"] = {"delete": {"operationId": f"close_{name}", "tags": ["m"]}}
    catalog = derive(
        SERVER, capabilities(modules=[{"name": "m", "version": "1"}]), {"paths": paths}
    )
    assert sorted(r.name for r in catalog.resources) == sorted(names)
    for name in names:
        assert catalog.resource(name).names == ("list", "close")


def test_unsupported_api_version() -> None:
    with pytest.raises(ClientError) as caught:
        derive(SERVER, {**capabilities(), "api_versions": ["v2", "v3"]}, CONTRACT)
    assert caught.value.code == "client.api-unsupported"
    assert "v2, v3" in str(caught.value)


def test_json_round_trip() -> None:
    catalog = derive(SERVER, capabilities(), CONTRACT)
    assert Catalog.from_json(catalog.to_json()) == catalog
    with pytest.raises(ClientError):
        Catalog.from_json('{"format": 0}')


class Server:
    """A server of capabilities and contract, counting the downloads of the contract."""

    def __init__(self, version: str = "0.1.0", etag: str | None = '"c1"') -> None:
        self.version = version
        self.etag = etag
        self.seen: list[Request] = []

    def __call__(self, request: Request) -> Response:
        self.seen.append(request)
        if request.url == "/v1/capabilities":
            return Response(200, content=json.dumps(capabilities(self.version)).encode())
        if self.etag is not None and request.headers.get("If-None-Match") == self.etag:
            return Response(304)
        headers = Headers.of({"ETag": self.etag} if self.etag else {})
        return Response(200, headers, json.dumps(CONTRACT).encode())

    @property
    def downloads(self) -> int:
        return sum(
            1 for r in self.seen if r.url == "/v1/openapi.json" and "If-None-Match" not in r.headers
        )


class Clock:
    def __init__(self) -> None:
        self.now = 1_000.0

    def __call__(self) -> float:
        return self.now


def test_same_revision_downloads_nothing(tmp_path: Path) -> None:
    clock = Clock()
    cache = CatalogCache(tmp_path / "cache" / "catalog", clock=clock)
    server = Server()
    first = run(discover(SERVER, cache), server)
    assert server.downloads == 1
    assert first.contract_etag == '"c1"'
    again = run(discover(SERVER, cache), server)
    assert len(server.seen) == 2  # less than a minute: no request at all
    assert again == first
    clock.now += 61
    run(discover(SERVER, cache), server)
    assert server.downloads == 1
    assert server.seen[-1].headers.get("If-None-Match") == '"c1"'
    server.version = "0.1.1"
    updated = run(discover(SERVER, cache, refresh=True), server)
    assert server.downloads == 2
    assert updated.version == "0.1.1"
    assert updated.revision != first.revision
    for path in (tmp_path / "cache").rglob("*"):
        mode = stat.S_IMODE(path.stat().st_mode)
        assert mode == (0o700 if path.is_dir() else 0o600), path


def test_contract_without_etag_is_kept_by_version(tmp_path: Path) -> None:
    clock = Clock()
    cache = CatalogCache(tmp_path, clock=clock)
    server = Server(etag=None)
    run(discover(SERVER, cache), server)
    clock.now += 120
    run(discover(SERVER, cache), server)
    assert server.downloads == 1
    assert [r.url for r in server.seen] == [
        "/v1/capabilities",
        "/v1/openapi.json",
        "/v1/capabilities",
    ]


def test_damaged_cache_is_a_missing_one(tmp_path: Path) -> None:
    cache = CatalogCache(tmp_path)
    assert cache.last(SERVER) is None
    assert cache.fresh(SERVER) is None
    catalog = run(discover(SERVER, cache), Server())
    cache.path(SERVER, catalog.revision).write_text("{")
    assert cache.last(SERVER) is None
    next(p for p in tmp_path.glob("*.json") if "-" not in p.name).write_text("[]")
    assert cache.last(SERVER) is None
    assert CatalogCache(tmp_path).last("https://other.test") is None


def test_errors_of_discovery(tmp_path: Path) -> None:
    cache = CatalogCache(tmp_path)

    def broken(request: Request) -> Response:
        if request.url == "/v1/capabilities":
            return Response(200, content=b"[]")
        return Response(500)

    with pytest.raises(ClientError) as caught:
        run(discover(SERVER, cache), broken)
    assert caught.value.code == "client.unexpected-answer"
