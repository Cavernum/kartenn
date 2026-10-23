"""Contract and capabilities → catalog, in one pass (design § Catalogue).

The derivation follows the conventions of REST every module keeps (ADR 0026, clause 9):

| Contract | Catalog |
|---|---|
| `/v1/<coll>`: `get`, `post`, `x-query` | `list`, `create`, `query` |
| `/v1/<coll>/{id}`: `get`, `patch`, `delete`, `put` | `get`, `update`, `close`, `set` |
| `/v1/<coll>/{id}/<sub>` in `get` | a reading of a sub-resource (`history`, `children`…) |
| `/v1/<coll>/{id}/<sub>` in another method | an action (`acknowledgement`, `waiver`…) |
| `/v1/<coll>/{id}/properties/{key}/<x>` | `properties.explain`, `properties.suggest` |
| `tags[0]` of the operation | its module |
| `x-kartenn` (optional) | natural key and columns (path), name of a verb (operation) |

Nothing names a module or a resource: a module served tomorrow is derived the same way.
"""

from __future__ import annotations

import hashlib
import json
from typing import Any, cast

from kartenn._core.problems import ClientError
from kartenn.catalog.model import Catalog, Module, Operation, Param, Resource, Verb

__all__ = ["derive", "revision"]

_METHODS = ("get", "post", "put", "patch", "delete", "x-query")
_COLLECTION = {"get": "list", "post": "create", "query": "query", "put": "replace"}
_OBJECT = {"get": "get", "patch": "update", "delete": "close", "put": "set", "post": "post"}
_PROPERTY_VERBS = {"explanation": "properties.explain", "suggestion": "properties.suggest"}
# Not collections: the base the library codes by hand.
_NOT_RESOURCES = frozenset({"auth", "capabilities", "me", "openapi.json"})
_SUCCESS = ("200", "201", "202", "204")


def revision(version: str, modules: list[Module] | tuple[Module, ...], etag: str | None) -> str:
    """The revision of a catalog (T20): the version, the modules and the ETag of the
    contract when it is served."""
    names = sorted(f"{m.name}@{m.version}" for m in modules)
    material = json.dumps([version, names, etag or ""], separators=(",", ":"))
    return hashlib.sha256(material.encode()).hexdigest()[:32]


def _template(segment: str) -> bool:
    return segment.startswith("{") and segment.endswith("}")


def _strings(value: object) -> tuple[str, ...]:
    return tuple(str(v) for v in cast("list[object]", value)) if isinstance(value, list) else ()


def _as_dict(value: object) -> dict[str, Any]:
    return cast("dict[str, Any]", value) if isinstance(value, dict) else {}


class _Resolver:
    def __init__(self, contract: dict[str, Any]) -> None:
        components = _as_dict(contract.get("components"))
        self.schemas = _as_dict(components.get("schemas"))
        self.parameters = _as_dict(components.get("parameters"))

    def schema(self, schema: object, depth: int = 4) -> dict[str, Any]:
        """A schema with its first references followed, to read a type or an enum."""
        found = _as_dict(schema)
        ref = found.get("$ref")
        while isinstance(ref, str) and depth > 0:
            found = _as_dict(self.schemas.get(ref.rsplit("/", 1)[-1]))
            ref = found.get("$ref")
            depth -= 1
        return found

    def param(self, raw: object) -> Param | None:
        data = _as_dict(raw)
        ref = data.get("$ref")
        if isinstance(ref, str):
            data = _as_dict(self.parameters.get(ref.rsplit("/", 1)[-1]))
        if "name" not in data:
            return None
        schema = self.schema(data.get("schema"))
        kind = schema.get("type")
        if isinstance(kind, list):
            kind = next((k for k in cast("list[str]", kind) if k != "null"), None)
        repeated = kind == "array"
        if repeated:
            schema = self.schema(schema.get("items"))
            kind = schema.get("type")
        return Param(
            name=str(data["name"]),
            location=str(data.get("in", "query")),
            required=bool(data.get("required", False)),
            type=kind if isinstance(kind, str) else None,
            enum=_strings(schema.get("enum")),
            repeated=repeated,
            description=str(data.get("description", "")),
        )


def _operation(
    resolver: _Resolver, path: str, method: str, raw: dict[str, Any], shared: list[object]
) -> Operation:
    params = [p for p in (resolver.param(x) for x in [*shared, *raw.get("parameters", [])]) if p]
    body = _as_dict(raw.get("requestBody"))
    content = _as_dict(body.get("content"))
    media = next(iter(content), None)
    responses = _as_dict(raw.get("responses"))
    statuses = [s for s in _SUCCESS if s in responses]
    success = _as_dict(responses[statuses[0]] if statuses else None)
    answer = _as_dict(success.get("content"))
    answer_schema = _as_dict(next(iter(answer.values()), None)).get("schema")
    tags = raw.get("tags")
    command = _as_dict(raw.get("x-kartenn")).get("command")
    return Operation(
        operation_id=str(raw.get("operationId") or f"{method}:{path}"),
        method="QUERY" if method == "x-query" else method.upper(),
        path=path,
        module=str(cast("list[object]", tags)[0]) if isinstance(tags, list) and tags else "",
        summary=str(raw.get("summary", "")),
        params=tuple(params),
        body=None if media is None else _as_dict(_as_dict(content[media]).get("schema")),
        body_media=media,
        body_required=bool(body.get("required", False)),
        response=_as_dict(answer_schema) if answer_schema is not None else None,
        command=command if isinstance(command, str) else None,
    )


def _verb_name(rest: list[str], method: str) -> tuple[str, str]:
    """The level and the name of a verb from the path after the collection."""
    verb = "query" if method == "x-query" else method
    if not rest:
        return "collection", _COLLECTION.get(verb, verb)
    if not _template(rest[0]):
        return "collection", ".".join(s for s in rest if not _template(s))
    tail = rest[1:]
    named = [s for s in tail if not _template(s)]
    if not named:
        return "object", _OBJECT.get(verb, verb)
    if len(tail) == 3 and tail[0] == "properties" and tail[2] in _PROPERTY_VERBS:  # noqa: PLR2004
        return "object", _PROPERTY_VERBS[tail[2]]
    name = ".".join(named)
    if verb != "get" and _template(tail[-1]):
        name = f"{name}.{verb}"
    return "object", name


class _Builder:
    def __init__(self, name: str) -> None:
        self.name = name
        self.path = f"/v1/{name}"
        self.verbs: list[Verb] = []
        self.key: tuple[str, ...] = ()
        self.columns: tuple[str, ...] = ()
        self.module = ""

    def add(self, level: str, name: str, operation: Operation) -> None:
        taken = {(v.level, v.name) for v in self.verbs}
        if (level, name) in taken:
            name = f"{name}.{operation.method.lower()}"
        read = operation.method in {"GET", "QUERY"}
        self.verbs.append(Verb(name, level, operation.operation_id, read))
        if level == "collection" and name in {"list", "create"} and not self.module:
            self.module = operation.module

    def build(self, operations: dict[str, Operation]) -> Resource:
        module = self.module or next((operations[v.operation_id].module for v in self.verbs), "")
        return Resource(self.name, module, self.path, tuple(self.verbs), self.key, self.columns)


def _extension(item: dict[str, Any]) -> tuple[tuple[str, ...], tuple[str, ...]]:
    found = _as_dict(item.get("x-kartenn"))
    key, columns = found.get("key"), found.get("columns")
    return _strings(key), _strings(columns)


def derive(
    server: str,
    capabilities: dict[str, Any],
    contract: dict[str, Any],
    *,
    contract_etag: str | None = None,
) -> Catalog:
    """The catalog of a server from its capabilities and its contract."""
    versions = capabilities.get("api_versions")
    served = _strings(versions)
    if "v1" not in served:
        names = ", ".join(served) or "none"
        msg = f"the server serves no v1 of the API (versions served: {names})"
        raise ClientError("client.api-unsupported", msg)
    modules = [
        Module(str(m.get("name")), str(m.get("version", "")))
        for m in (_as_dict(x) for x in capabilities.get("modules", []))
        if m.get("name")
    ]
    resolver = _Resolver(contract)
    operations: dict[str, Operation] = {}
    builders: dict[str, _Builder] = {}
    for path, raw_item in sorted(_as_dict(contract.get("paths")).items()):
        item = _as_dict(raw_item)
        shared = cast("list[object]", item.get("parameters", []))
        segments = path.removeprefix("/v1/").split("/") if path.startswith("/v1/") else []
        builder = None
        if segments and segments[0] not in _NOT_RESOURCES:
            builder = builders.setdefault(segments[0], _Builder(segments[0]))
            if len(segments) == 1 or (len(segments) == 2 and _template(segments[1])):  # noqa: PLR2004
                key, columns = _extension(item)
                builder.key, builder.columns = builder.key or key, builder.columns or columns
        for method in _METHODS:
            if not isinstance(item.get(method), dict):
                continue
            operation = _operation(resolver, path, method, _as_dict(item[method]), shared)
            operations[operation.operation_id] = operation
            if builder is not None:
                level, name = _verb_name(segments[1:], method)
                builder.add(level, operation.command or name, operation)
    etag = contract_etag
    return Catalog(
        server=server,
        version=str(capabilities.get("version", "")),
        api_versions=served,
        modules=tuple(modules),
        revision=revision(str(capabilities.get("version", "")), modules, etag),
        contract_etag=etag,
        resources=tuple(b.build(operations) for _, b in sorted(builders.items())),
        operations=tuple(operations.values()),
        schemas=resolver.schemas,
        capabilities=capabilities,
    )
