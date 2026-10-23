"""The catalog of a server (REQ-023-1 to 3, 16): what it serves, derived from its
capabilities and its contract, as frozen dataclasses that serialise to compact JSON.
"""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field, replace
from functools import cached_property
from typing import Any, cast

from kartenn._core.problems import ClientError

__all__ = [
    "Catalog",
    "Module",
    "Operation",
    "Param",
    "Resource",
    "UnknownResource",
    "Verb",
]

FORMAT = 1
# The tags of the base (spec 006): `GET /v1/capabilities` lists them among the modules
# of the build, each once (ADR 0026, clause 1), but their collections are the first level
# of a client, not a module group (maquette § 2). Seen by the end-to-end sessions
# (task 26): the simulated servers of the tests served the business modules only.
BASE_TAGS = frozenset(
    {
        "audit",
        "authentication",
        "changesets",
        "core",
        "graph",
        "imports",
        "operations",
        "properties",
        "rights",
    }
)
# The operations of the base the library codes by hand (REQ-023-1): checked by their
# `operationId` when used, `client.operation-unavailable` when absent.
SOCLE = (
    "capabilities",
    "openapi",
    "me",
    "submit_changeset",
    "show_changeset",
    "list_changesets",
    "changeset_diff",
    "validate_changeset",
    "reject_changeset",
    "withdraw_changeset",
    "undo_changeset",
    "list_proposals",
    "create_import",
    "deposit_import_chunk",
    "start_import",
    "resume_import",
    "cancel_import",
    "show_import",
    "list_imports",
    "breakglass",
    "logout",
)
_DATES = frozenset({"valid_at", "as_of", "cursor", "page_size"})


class UnknownResource(ClientError, AttributeError):
    """A resource the server does not serve (`client.unknown-resource`); an
    `AttributeError` too, since `kn.<name>` reaches a resource."""


@dataclass(frozen=True)
class Param:
    """A parameter of an operation: `location` is `path`, `query` or `header`."""

    name: str
    location: str
    required: bool = False
    type: str | None = None
    enum: tuple[str, ...] = ()
    repeated: bool = False
    description: str = ""


@dataclass(frozen=True)
class Operation:
    """An operation of the contract; `method` is in capitals, `QUERY` for `x-query`."""

    operation_id: str
    method: str
    path: str
    module: str
    summary: str = ""
    params: tuple[Param, ...] = ()
    body: dict[str, Any] | None = None
    body_media: str | None = None
    body_required: bool = False
    response: dict[str, Any] | None = None
    command: str | None = None

    def param(self, name: str) -> Param | None:
        return next((p for p in self.params if p.name == name), None)

    @property
    def path_params(self) -> tuple[str, ...]:
        return tuple(p.name for p in self.params if p.location == "path")

    @property
    def filters(self) -> tuple[Param, ...]:
        """The query parameters that are filters: all but the dates, the cursor and the
        size of a page."""
        return tuple(p for p in self.params if p.location == "query" and p.name not in _DATES)

    @property
    def needs_if_match(self) -> bool:
        found = self.param("If-Match")
        return found is not None and found.location == "header"

    @property
    def takes_idempotency_key(self) -> bool:
        return self.param("Idempotency-Key") is not None


@dataclass(frozen=True)
class Verb:
    """What a resource offers: `level` is `collection` or `object`; `read` tells a
    reading (`GET`, `QUERY`) from an action."""

    name: str
    level: str
    operation_id: str
    read: bool


@dataclass(frozen=True)
class Resource:
    """A collection of `/v1` and its sub-resources. `module` is the tag of its
    collection operation; `key` and `columns` come from `x-kartenn` when served."""

    name: str
    module: str
    path: str
    verbs: tuple[Verb, ...] = ()
    key: tuple[str, ...] = ()
    columns: tuple[str, ...] = ()

    def verb(self, name: str) -> Verb | None:
        return next((v for v in self.verbs if v.name == name), None)

    @property
    def names(self) -> tuple[str, ...]:
        return tuple(v.name for v in self.verbs)


@dataclass(frozen=True)
class Module:
    name: str
    version: str


@dataclass(frozen=True)
class Catalog:
    """The catalog of one server at one revision."""

    server: str
    version: str
    api_versions: tuple[str, ...]
    modules: tuple[Module, ...]
    revision: str
    contract_etag: str | None
    resources: tuple[Resource, ...]
    operations: tuple[Operation, ...]
    schemas: dict[str, Any] = field(default_factory=dict[str, Any])
    capabilities: dict[str, Any] = field(default_factory=dict[str, Any])

    @cached_property
    def _resources(self) -> dict[str, Resource]:
        return {resource.name: resource for resource in self.resources}

    @cached_property
    def _operations(self) -> dict[str, Operation]:
        return {operation.operation_id: operation for operation in self.operations}

    @property
    def module_names(self) -> tuple[str, ...]:
        return tuple(module.name for module in self.modules)

    @property
    def limits(self) -> dict[str, Any]:
        limits = self.capabilities.get("limits")
        return cast("dict[str, Any]", limits) if isinstance(limits, dict) else {}

    @property
    def events(self) -> dict[str, Any] | None:
        """The event stream of the 022, `None` when the server does not serve it."""
        events = self.capabilities.get("events")
        return cast("dict[str, Any]", events) if isinstance(events, dict) else None

    def has(self, module: str) -> bool:
        return module in self.module_names

    def has_resource(self, name: str) -> bool:
        return name in self._resources

    @property
    def served_modules(self) -> tuple[str, ...]:
        """The business modules of the build: the modules of the capabilities, without
        the tags of the base the server lists among them (`core`, `rights`…)."""
        return tuple(name for name in self.module_names if name not in BASE_TAGS)

    def is_module(self, resource: Resource) -> bool:
        """Whether a resource belongs to a module of the build (not the core)."""
        return resource.module in self.served_modules

    def resource(self, name: str) -> Resource:
        found = self._resources.get(name)
        if found is not None:
            return found
        served = ", ".join(self.module_names) or "none"
        message = f"no resource {name!r} on this server (version {self.version})"
        module = name.split("-", 1)[0]
        if module not in self.module_names and not any(
            r.name.startswith(module) for r in self.resources
        ):
            message += f"; the module {module!r} is not served"
        raise UnknownResource("client.unknown-resource", f"{message}; modules served: {served}")

    def operation(self, operation_id: str) -> Operation:
        found = self._operations.get(operation_id)
        if found is None:
            msg = f"the server (version {self.version}) has no operation {operation_id!r}"
            raise ClientError("client.operation-unavailable", msg)
        return found

    def has_operation(self, operation_id: str) -> bool:
        return operation_id in self._operations

    @property
    def missing_socle(self) -> tuple[str, ...]:
        """The operations of the base the server does not serve."""
        return tuple(name for name in SOCLE if name not in self._operations)

    def with_capabilities(self, capabilities: dict[str, Any]) -> Catalog:
        return replace(self, capabilities=capabilities)

    def to_json(self) -> str:
        data = asdict(self)
        data["format"] = FORMAT
        return json.dumps(data, separators=(",", ":"), sort_keys=True)

    @classmethod
    def from_json(cls, text: str) -> Catalog:
        data = cast("dict[str, Any]", json.loads(text))
        if data.pop("format", None) != FORMAT:
            msg = "a catalog of another format"
            raise ClientError("client.catalog-format", msg)
        return cls(
            server=data["server"],
            version=data["version"],
            api_versions=tuple(data["api_versions"]),
            modules=tuple(Module(**m) for m in data["modules"]),
            revision=data["revision"],
            contract_etag=data["contract_etag"],
            resources=tuple(_resource(r) for r in data["resources"]),
            operations=tuple(_operation(o) for o in data["operations"]),
            schemas=data["schemas"],
            capabilities=data["capabilities"],
        )


def _resource(data: dict[str, Any]) -> Resource:
    return Resource(
        name=data["name"],
        module=data["module"],
        path=data["path"],
        verbs=tuple(Verb(**v) for v in data["verbs"]),
        key=tuple(data["key"]),
        columns=tuple(data["columns"]),
    )


def _param(data: dict[str, Any]) -> Param:
    data["enum"] = tuple(data["enum"])
    return Param(**data)


def _operation(data: dict[str, Any]) -> Operation:
    params = tuple(_param(p) for p in data.pop("params"))
    return Operation(params=params, **data)
