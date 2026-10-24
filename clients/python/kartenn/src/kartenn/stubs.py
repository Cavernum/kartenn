"""Type stubs of a server (REQ-023-17, T4): `python -m kartenn stubs generate`.

For one server, at one revision, writes `<out>/kartenn_server/__init__.pyi`:

- a `TypedDict` per object schema of the contract, a `Literal` per enumeration;
- a `Protocol` per resource, its verbs with their named parameters (filters, members of
  a creation or a modification);
- a `Client` whose attributes and `__getattr__` give those protocols, and refuse any
  other name.

The stubs serve the type checkers only (`typings/` as their stub path):

```python
from typing import TYPE_CHECKING, cast
import kartenn
if TYPE_CHECKING:
    from kartenn_server import Client
kn = cast("Client", kartenn.Client.from_profile("prod"))
```

They are never imported at run time, and the library works without them.
"""

from __future__ import annotations

import json
import keyword
import re
from typing import TYPE_CHECKING, Any, cast

if TYPE_CHECKING:
    from pathlib import Path

    from kartenn.catalog import Catalog, Operation, Param, Resource

__all__ = ["generate", "render"]

PACKAGE = "kartenn_server"
_DEPTH = 6
_RESERVED_MEMBERS = frozenset({"motive", "valid_from", "valid_to"})
_TAKEN = frozenset({"Client"})
# Only the names of this server: narrower than the methods of kartenn.Client, on purpose.
_OVERRIDE = "# pyright: ignore[reportIncompatibleMethodOverride]"
_PRIMITIVES = {
    "string": "str",
    "integer": "int",
    "number": "float",
    "boolean": "bool",
    "null": "None",
}


def _identifier(name: str) -> str:
    found = re.sub(r"\W", "_", name)
    if not found or found[0].isdigit():
        found = f"_{found}"
    if keyword.iskeyword(found) or found in _TAKEN:
        found = f"{found}_"
    return found


def _camel(name: str) -> str:
    return "".join(part.capitalize() for part in re.split(r"[^0-9A-Za-z]+", name) if part)


def _as_dict(value: object) -> dict[str, Any]:
    return cast("dict[str, Any]", value) if isinstance(value, dict) else {}


class _Writer:
    def __init__(self, catalog: Catalog) -> None:
        self.catalog = catalog
        self.schemas = catalog.schemas
        self.names = {name: _identifier(name) for name in self.schemas}
        self.lines: list[str] = []

    # Types -----------------------------------------------------------------------

    def ref(self, schema: object) -> str | None:
        ref = _as_dict(schema).get("$ref")
        return ref.rsplit("/", 1)[-1] if isinstance(ref, str) else None

    def type_of(self, schema: object, depth: int = 0) -> str:
        data = _as_dict(schema)
        if depth > _DEPTH or not data:
            return "_t.Any"
        if (name := self.ref(data)) is not None:
            return self.names.get(name, "_t.Any")
        enum = data.get("enum")
        if isinstance(enum, list):
            values = [v for v in cast("list[object]", enum) if isinstance(v, str)]
            if values:
                return f"_t.Literal[{', '.join(json.dumps(v) for v in values)}]"
        for combined in ("oneOf", "anyOf"):
            parts = data.get(combined)
            if isinstance(parts, list):
                return self.union(self.type_of(p, depth + 1) for p in cast("list[object]", parts))
        parts = data.get("allOf")
        if isinstance(parts, list) and len(cast("list[object]", parts)) == 1:
            return self.type_of(cast("list[object]", parts)[0], depth + 1)
        kind = data.get("type")
        kinds = cast("list[object]", kind) if isinstance(kind, list) else [kind]
        return self.union(self.primitive(str(k), data, depth) for k in kinds)

    def primitive(self, kind: str, data: dict[str, Any], depth: int) -> str:
        if kind in _PRIMITIVES:
            return _PRIMITIVES[kind]
        if kind == "array":
            return f"list[{self.type_of(data.get('items'), depth + 1)}]"
        if kind == "object":
            return "dict[str, _t.Any]"
        return "_t.Any"

    @staticmethod
    def union(types: Any) -> str:  # noqa: ANN401 - an iterable of names
        unique = list(dict.fromkeys(cast("list[str]", list(types))))
        if "_t.Any" in unique:
            return "_t.Any"
        return " | ".join(unique) if unique else "_t.Any"

    def properties(self, schema: object, depth: int = 0) -> tuple[dict[str, Any], set[str]]:
        """The members of an object schema, `allOf` merged, and the required ones."""
        data = _as_dict(schema)
        if (name := self.ref(data)) is not None and depth <= _DEPTH:
            return self.properties(self.schemas.get(name), depth + 1)
        members = dict(_as_dict(data.get("properties")))
        required = {str(r) for r in cast("list[object]", data.get("required") or [])}
        for part in cast("list[object]", data.get("allOf") or []):
            more, needed = self.properties(part, depth + 1)
            members.update(more)
            required |= needed
        return members, required

    def is_object(self, schema: object) -> bool:
        data = _as_dict(schema)
        return "properties" in data or (
            "allOf" in data and len(cast("list[object]", data["allOf"])) > 1
        )

    # Declarations ----------------------------------------------------------------

    def typed_dict(self, name: str, schema: object, *, total: bool, drop: frozenset[str]) -> None:
        members, required = self.properties(schema)
        fields: list[str] = []
        for member, value in sorted(members.items()):
            if member in drop:
                continue
            kind = self.type_of(value)
            if total and member not in required:
                kind = f"_t.NotRequired[{kind}]"
            fields.append(f"{json.dumps(member)}: {kind}")
        self.lines.append(
            f"{name} = _t.TypedDict({json.dumps(name)}, {{{', '.join(fields)}}}, total={total})"
        )

    def components(self) -> None:
        for name, schema in sorted(self.schemas.items()):
            identifier = self.names[name]
            if self.is_object(schema):
                self.typed_dict(identifier, schema, total=True, drop=frozenset())
            else:
                self.lines.append(f"{identifier}: _t.TypeAlias = {self.type_of(schema, 1)}")

    def fields_of(self, operation: Operation) -> str | None:
        """A `TypedDict` of the members of a body, for `**members`; `None` when the body
        is not an object of the contract."""
        if operation.body is None or not self.is_object(self.resolved(operation.body)):
            return None
        name = f"_{_camel(operation.operation_id)}Members"
        self.typed_dict(name, operation.body, total=False, drop=_RESERVED_MEMBERS)
        return name

    def resolved(self, schema: object) -> object:
        name = self.ref(schema)
        return self.schemas.get(name) if name is not None else schema

    def filter_type(self, param: Param) -> str:
        base = (
            f"_t.Literal[{', '.join(json.dumps(v) for v in param.enum)}]"
            if param.enum
            else _PRIMITIVES.get(param.type or "string", "str")
        )
        return f"{base} | _t.Sequence[{base}] | None"

    def protocol(self, resource: Resource) -> str:
        name = f"{_camel(resource.name)}Resource"
        if name in self.names.values():
            name = f"{name}_"
        catalog = self.catalog
        out = [f"class {name}(_t.Protocol):", f"    name: str  # {resource.name}"]
        dates = "valid_at: _Date = ..., as_of: _Date = ..."
        for verb in resource.verbs:
            operation = catalog.operation(verb.operation_id)
            if verb.name == "list":
                filters = "".join(
                    f", {_identifier(p.name)}: {self.filter_type(p)} = ..."
                    for p in operation.filters
                    if p.name.isidentifier() and not keyword.iskeyword(p.name)
                )
                out.append(
                    f"    def list(self, *, {dates}, page_size: int | None = ..., "
                    f"cursor: str | None = ..., prop: _t.Mapping[str, object] | None = ...{filters}"
                    ") -> _r.Listing: ..."
                )
            elif verb.name == "get":
                out.append("    def get(self, ref: object, /, **params: object) -> _k.Record: ...")
            elif verb.name in {"create", "update"}:
                out.append(self.write(verb.name, operation))
            elif verb.name == "close":
                out.append(
                    "    def close(self, ref: object, /, *, motive: str | None = ..., "
                    "valid_from: _Date = ..., etag: str | None = ..., simulate: bool = ...) "
                    "-> _Outcome: ..."
                )
            elif verb.name != "query":
                method = _identifier(re.sub(r"[-.]", "_", verb.name))
                out.append(
                    f"    def {method}(self, ref: object = ..., /, *, body: object = ..., "
                    "etag: str | None = ..., **params: object) -> _t.Any: ..."
                )
        out.append(
            "    def verb(self, verb: str, ref: object = ..., /, *, body: object = ..., "
            "etag: str | None = ..., **params: object) -> _t.Any: ..."
        )
        out.append(
            "    def pages_of(self, verb: str, ref: object = ..., /, **params: object)"
            " -> _r.Listing: ..."
        )
        self.lines.append("\n".join(out))
        return name

    def write(self, verb: str, operation: Operation) -> str:
        fields = self.fields_of(operation)
        members = f"**members: _t.Unpack[{fields}]" if fields else "**members: object"
        body = "_t.Mapping[str, object] | None = ..."
        if verb == "create":
            return (
                f"    def create(self, body: {body}, /, *, motive: str | None = ..., "
                "valid_from: _Date = ..., valid_to: _Date = ..., "
                f"idempotency_key: str | None = ..., simulate: bool = ..., {members}) "
                "-> _Outcome: ..."
            )
        return (
            f"    def update(self, ref: object, changes: {body}, /, *, motive: str | None = ..., "
            "valid_from: _Date = ..., etag: str | None = ..., simulate: bool = ..., "
            f"{members}) -> _Outcome: ..."
        )

    def client(self, protocols: dict[str, str]) -> None:
        out = ["class Client(_k.Client):"]
        for resource, protocol in protocols.items():
            attribute = resource.replace("-", "_")
            if attribute.isidentifier() and not keyword.iskeyword(attribute):
                out.extend(["    @property", f"    def {attribute}(self) -> {protocol}: ..."])
        for resource, protocol in protocols.items():
            name = f"_t.Literal[{json.dumps(resource)}]"
            out.append("    @_t.overload")
            out.append(f"    def resource(self, name: {name}) -> {protocol}: ...  {_OVERRIDE}")
        out.append(f"    def __getattr__(self, attribute: _t.Never) -> _t.Never: ...  {_OVERRIDE}")
        self.lines.append("\n".join(out))


def render(catalog: Catalog) -> str:
    """The text of the stub of a catalog."""
    writer = _Writer(catalog)
    writer.lines.append(
        f'"""Types of the kartenn server {catalog.server} (version {catalog.version}, revision '
        f"{catalog.revision}).\n\nGenerated by `python -m kartenn stubs generate`: do not edit, "
        'generate again when the server changes. Never imported at run time."""'
    )
    writer.lines.append(
        "import datetime as _datetime\nimport typing as _t\n\n"
        "import kartenn as _k\nimport kartenn.resources as _r\n\n"
        "_Date: _t.TypeAlias = _datetime.datetime | str | None\n"
        "_Outcome: _t.TypeAlias = _k.Applied | _k.Pending | _k.Record | _k.Simulation | None"
    )
    writer.components()
    protocols = {r.name: writer.protocol(r) for r in catalog.resources}
    writer.client(protocols)
    return "\n\n".join(writer.lines) + "\n"


def generate(catalog: Catalog, out: Path) -> Path:
    """Write the stub of `catalog` under `out` and give its path."""
    package = out / PACKAGE
    package.mkdir(parents=True, exist_ok=True)
    path = package / "__init__.pyi"
    path.write_text(render(catalog), encoding="utf-8")
    return path
