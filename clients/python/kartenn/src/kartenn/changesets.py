"""Changesets (REQ-023-13, 14; 004, 006 § Écritures).

```python
cs = kn.changeset(motive="ouverture du lot 3", valid_from="2026-11-01T00:00:00+01:00")
domain = cs.add({"op": "create_domain", "slug": "lot3"})
cs.add({"op": "create_prefix", "domain": {"local": domain}, "value": "198.51.100.0/24"})
report = cs.simulate()        # valid, violations, diff, sample, cascaded, sections
result = cs.submit()          # Applied | Pending, or raises Refused
```

A submission asks to apply the changeset (`approval=self`, spec 024): applied when its author
holds `approval.self` on everything it touches and no approval policy holds it, pending
otherwise (`Pending.reason`); `cs.submit(approval_self=False)` submits it only. Every write of
the client asks the same.

An operation is validated locally by its discriminant `op` only, against the schema
`Operation` of the contract. The builder keeps one `Idempotency-Key`: submitting it again
after a cut is recognised by the server (006/H8). Past `changeset_operations` of the
capabilities, the changeset is refused locally with the advice of an import.

`kn.changeset_of(id)` acts on an existing changeset: `approve`, `reject`, `withdraw`
and `undo` send the `If-Match` of the state read; `diff` pages its rows, those of the
sections of modules included, as generic rows.
"""

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any, cast

from kartenn._core.dates import to_param
from kartenn._core.messages import Headers, Request
from kartenn._core.plans import checked
from kartenn._core.problems import ClientError
from kartenn._core.requests import idempotency_key
from kartenn.resources import (
    Applied,
    AsyncListing,
    Listing,
    Pending,
    Record,
    Simulation,
    outcome,
    page,
    simulation_of,
)

if TYPE_CHECKING:
    from collections.abc import Callable, Mapping
    from datetime import datetime

    from kartenn._core.pages import Page
    from kartenn._core.plans import Plan
    from kartenn.catalog import Catalog
    from kartenn.resources import Context

__all__ = [
    "AsyncChangesetBuilder",
    "AsyncChangesetHandle",
    "ChangesetBuilder",
    "ChangesetHandle",
    "Simulation",
    "operation_kinds",
]

ON_BEHALF_OF = "Kartenn-On-Behalf-Of"


def operation_kinds(catalog: Catalog) -> frozenset[str]:
    """The discriminants `op` of the schema `Operation` of the contract."""
    found: set[str] = set()

    def visit(schema: object, depth: int) -> None:
        if not isinstance(schema, dict) or depth > 4:  # noqa: PLR2004 - bounded
            return
        data = cast("dict[str, Any]", schema)
        ref = data.get("$ref")
        if isinstance(ref, str):
            visit(catalog.schemas.get(ref.rsplit("/", 1)[-1]), depth + 1)
        properties = data.get("properties")
        if isinstance(properties, dict):
            discriminant = cast("dict[str, Any]", properties).get("op")
            if isinstance(discriminant, dict):
                enum = cast("dict[str, Any]", discriminant).get("enum")
                if isinstance(enum, list):
                    found.update(str(v) for v in cast("list[object]", enum))
        for key in ("oneOf", "anyOf", "allOf"):
            parts = data.get(key)
            if isinstance(parts, list):
                for part in cast("list[object]", parts):
                    visit(part, depth + 1)

    visit(catalog.schemas.get("Operation"), 0)
    return frozenset(found)


class _Changeset:
    """The plans of a changeset being composed, shared by both builders."""

    def __init__(  # noqa: PLR0913 - the members of a submission
        self,
        context: Context,
        *,
        motive: str,
        valid_from: datetime | str | None,
        valid_to: datetime | str | None,
        on_behalf_of: str | None,
        key: str | None,
    ) -> None:
        self._context = context
        self.motive = motive
        self.valid_from = to_param(valid_from)
        self.valid_to = to_param(valid_to)
        self.on_behalf_of = on_behalf_of
        self.idempotency_key = key or idempotency_key()
        self.operations: list[dict[str, Any]] = []
        self._creations: dict[int, str] = {}

    def add(self, operation: Mapping[str, Any]) -> int:
        """Add an operation; its index is the local reference of what it creates
        (`{"local": n}`)."""
        if not isinstance(operation.get("op"), str):
            msg = "an operation needs its discriminant op"
            raise ClientError("client.unknown-operation", msg)
        self.operations.append(dict(operation))
        return len(self.operations) - 1

    def create(self, resource: str, **members: object) -> int:
        """The creation of an object of `resource`, its `op` the one of its create
        operation in the contract (`prefixes` → `create_prefix`)."""
        self.operations.append(dict(members))
        index = len(self.operations) - 1
        self._creations[index] = resource
        return index

    def _body(self, catalog: Catalog) -> dict[str, Any]:
        for index, resource in self._creations.items():
            verb = catalog.resource(resource).verb("create")
            if verb is None:
                msg = f"{resource} has no creation on this server"
                raise ClientError("client.operation-unavailable", msg)
            self.operations[index] = {"op": verb.operation_id, **self.operations[index]}
        self._creations.clear()
        limit = catalog.limits.get("changeset_operations")
        if isinstance(limit, int) and len(self.operations) > limit:
            msg = (
                f"{len(self.operations)} operations exceed the {limit} of a changeset on this "
                "server: import them instead (kn.import_session)"
            )
            raise ClientError("client.changeset-too-large", msg)
        kinds = operation_kinds(catalog)
        for index, operation in enumerate(self.operations):
            # An operation of a module is `<module>.<operation>` (ADR 0026, clause 6): its
            # module served is all the client can check.
            module, dot, _ = str(operation["op"]).partition(".")
            if dot and module in catalog.served_modules:
                continue
            if kinds and operation["op"] not in kinds:
                msg = f"/operations/{index}: unknown operation {operation['op']!r}"
                raise ClientError("client.unknown-operation", msg)
        body: dict[str, Any] = {"motive": self.motive, "operations": self.operations}
        if self.valid_from is not None:
            body["valid_from"] = self.valid_from
        if self.valid_to is not None:
            body["valid_to"] = self.valid_to
        return body

    def _request(self, catalog: Catalog, *, simulate: bool, approval_self: bool = True) -> Request:
        operation = catalog.operation("submit_changeset")
        headers = Headers.of({"Content-Type": "application/json"})
        if self.on_behalf_of is not None:
            headers = headers.set(ON_BEHALF_OF, self.on_behalf_of)
        if not simulate:
            headers = headers.set("Idempotency-Key", self.idempotency_key)
        params: dict[str, object] = {"simulate": "true"} if simulate else {}
        if not approval_self:
            params["approval"] = None
        request = self._context.request(operation, params=params)
        content = json.dumps(self._body(catalog), separators=(",", ":")).encode()
        return Request("POST", request.url, request.params, headers, content)

    def simulate(self) -> Plan[Simulation]:
        catalog = yield from self._context.catalog()
        answer = checked((yield self._request(catalog, simulate=True)))
        return simulation_of(cast("Record", outcome(answer)))

    def submit(self, *, approval_self: bool = True) -> Plan[Applied | Pending]:
        """Submit (spec 024): applied when `approval_self` and its author holds `approval.self` on
        everything it touches with no policy holding it; pending otherwise."""
        catalog = yield from self._context.catalog()
        result = outcome(
            (yield self._request(catalog, simulate=False, approval_self=approval_self))
        )
        if not isinstance(result, Applied | Pending):
            msg = "the server answered a submission without its changeset"
            raise ClientError("client.unexpected-answer", msg)
        return result


class ChangesetBuilder:
    """A changeset composed, then simulated or submitted (synchronous client)."""

    def __init__(self, changeset: _Changeset, run: Callable[[Plan[Any]], Any]) -> None:
        self._changeset = changeset
        self._run = run

    @property
    def operations(self) -> list[dict[str, Any]]:
        return self._changeset.operations

    @property
    def idempotency_key(self) -> str:
        return self._changeset.idempotency_key

    def add(self, operation: Mapping[str, Any]) -> int:
        return self._changeset.add(operation)

    def create(self, resource: str, **members: object) -> int:
        return self._changeset.create(resource, **members)

    def simulate(self) -> Simulation:
        return cast("Simulation", self._run(self._changeset.simulate()))

    def submit(self, *, approval_self: bool = True) -> Applied | Pending:
        return cast(
            "Applied | Pending", self._run(self._changeset.submit(approval_self=approval_self))
        )


class AsyncChangesetBuilder:
    """A changeset composed, then simulated or submitted (asynchronous client)."""

    def __init__(self, changeset: _Changeset, run: Callable[[Plan[Any]], Any]) -> None:
        self._changeset = changeset
        self._run = run

    @property
    def operations(self) -> list[dict[str, Any]]:
        return self._changeset.operations

    @property
    def idempotency_key(self) -> str:
        return self._changeset.idempotency_key

    def add(self, operation: Mapping[str, Any]) -> int:
        return self._changeset.add(operation)

    def create(self, resource: str, **members: object) -> int:
        return self._changeset.create(resource, **members)

    async def simulate(self) -> Simulation:
        return cast("Simulation", await self._run(self._changeset.simulate()))

    async def submit(self, *, approval_self: bool = True) -> Applied | Pending:
        return cast(
            "Applied | Pending",
            await self._run(self._changeset.submit(approval_self=approval_self)),
        )


def builder(  # noqa: PLR0913 - the members of a submission
    context: Context,
    *,
    motive: str,
    valid_from: datetime | str | None = None,
    valid_to: datetime | str | None = None,
    on_behalf_of: str | None = None,
    idempotency_key: str | None = None,
) -> _Changeset:
    return _Changeset(
        context,
        motive=motive,
        valid_from=valid_from,
        valid_to=valid_to,
        on_behalf_of=on_behalf_of,
        key=idempotency_key,
    )


class _Existing:
    """The plans of an existing changeset."""

    def __init__(self, context: Context, ref: object) -> None:
        self._context = context
        self.record = ref if isinstance(ref, Record) else None
        self.id = str(cast("Mapping[str, Any]", ref).get("id") if isinstance(ref, dict) else ref)

    def read(self) -> Plan[Record]:
        catalog = yield from self._context.catalog()
        operation = catalog.operation("show_changeset")
        answer = yield self._context.request(operation, ref=self.id)
        data = checked(answer).json()
        self.record = Record(
            cast("dict[str, Any]", data) if isinstance(data, dict) else {}, etag=answer.etag
        )
        return self.record

    def act(self, operation_id: str, body: object, params: Mapping[str, object]) -> Plan[Any]:
        if self.record is None or self.record.etag is None:
            yield from self.read()
        assert self.record is not None  # noqa: S101 - read above
        catalog = yield from self._context.catalog()
        operation = catalog.operation(operation_id)
        request = self._context.request(
            operation, ref=self.id, params=params, body=body, etag=self.record.etag
        )
        result = outcome((yield request))
        self.record = None
        return result

    def diff_page(self, section: str | None, prop: str | None) -> Plan[Page]:
        catalog = yield from self._context.catalog()
        operation = catalog.operation("changeset_diff")
        params = {"section": section, "property": prop}
        return (yield from page(self._context.request(operation, ref=self.id, params=params), None))


class ChangesetHandle:
    """An existing changeset (synchronous client)."""

    def __init__(self, existing: _Existing, run: Callable[[Plan[Any]], Any]) -> None:
        self._existing = existing
        self._run = run

    @property
    def id(self) -> str:
        return self._existing.id

    def read(self) -> Record:
        return cast("Record", self._run(self._existing.read()))

    def approve(self, operations: list[Mapping[str, Any]] | None = None) -> Any:  # noqa: ANN401
        body = None if operations is None else {"operations": operations}
        return self._run(self._existing.act("validate_changeset", body, {}))

    def reject(self, motive: str) -> Any:  # noqa: ANN401 - what the server answers
        return self._run(self._existing.act("reject_changeset", {"motive": motive}, {}))

    def withdraw(self) -> Any:  # noqa: ANN401 - what the server answers
        return self._run(self._existing.act("withdraw_changeset", None, {}))

    def undo(self, *, simulate: bool = False) -> Any:  # noqa: ANN401 - what the server answers
        params = {"simulate": "true"} if simulate else {}
        return self._run(self._existing.act("undo_changeset", None, params))

    def diff(self, *, section: str | None = None, prop: str | None = None) -> Listing:
        """The rows of the differential, or of one section of a module (`policy.combinations`)."""
        return Listing(self._run, lambda: self._existing.diff_page(section, prop))


class AsyncChangesetHandle:
    """An existing changeset (asynchronous client)."""

    def __init__(self, existing: _Existing, run: Callable[[Plan[Any]], Any]) -> None:
        self._existing = existing
        self._run = run

    @property
    def id(self) -> str:
        return self._existing.id

    async def read(self) -> Record:
        return cast("Record", await self._run(self._existing.read()))

    async def approve(self, operations: list[Mapping[str, Any]] | None = None) -> Any:  # noqa: ANN401
        body = None if operations is None else {"operations": operations}
        return await self._run(self._existing.act("validate_changeset", body, {}))

    async def reject(self, motive: str) -> Any:  # noqa: ANN401 - what the server answers
        return await self._run(self._existing.act("reject_changeset", {"motive": motive}, {}))

    async def withdraw(self) -> Any:  # noqa: ANN401 - what the server answers
        return await self._run(self._existing.act("withdraw_changeset", None, {}))

    async def undo(self, *, simulate: bool = False) -> Any:  # noqa: ANN401
        params = {"simulate": "true"} if simulate else {}
        return await self._run(self._existing.act("undo_changeset", None, params))

    def diff(self, *, section: str | None = None, prop: str | None = None) -> AsyncListing:
        return AsyncListing(self._run, lambda: self._existing.diff_page(section, prop))


def existing(context: Context, ref: object) -> _Existing:
    return _Existing(context, ref)
