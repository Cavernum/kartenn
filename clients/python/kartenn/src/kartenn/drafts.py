"""Drafts kept by the server (spec 024, REQ-024-1 to 7).

```python
draft = kn.drafts.create("Lot 3", operations=[{"op": "create_domain", "slug": "lot3"}])
draft = kn.drafts.update(draft, summary="ouverture du lot 3")   # If-Match = draft.etag
for d in kn.drafts.list():                                       # without their operations
    ...
result = kn.drafts.submit(draft)    # Applied | Pending, or raises Refused (the draft stays)
```

A draft belongs to its author alone; a service account has none. Each modification sends
the version read in `If-Match` (a draft given as an identifier is read first). A submission
asks to apply its changeset (`approval=self`) as every write of the client, and carries an
`Idempotency-Key`: retried after a cut, the first answer comes back.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

from kartenn._core.plans import checked
from kartenn._core.problems import ClientError
from kartenn.resources import Applied, Pending, Record, outcome

if TYPE_CHECKING:
    from collections.abc import Callable, Mapping, Sequence

    from kartenn._core.plans import Plan
    from kartenn.resources import Context

__all__ = ["AsyncDrafts", "Drafts"]

_APPROVAL = "approval"


def _record(answer: Any) -> Record:  # noqa: ANN401 - a response
    data = checked(answer).json()
    return Record(cast("dict[str, Any]", data) if isinstance(data, dict) else {}, etag=answer.etag)


def _reason(summary: str | None, description: str | None) -> dict[str, str] | None:
    if summary is None and description is None:
        return None
    reason: dict[str, str] = {}
    if summary is not None:
        reason["summary"] = summary
    if description is not None:
        reason["description"] = description
    return reason


class _Drafts:
    """The plans of the drafts, shared by both clients."""

    def __init__(self, context: Context) -> None:
        self._context = context

    def list(self) -> Plan[tuple[Record, ...]]:
        catalog = yield from self._context.catalog()
        answer = yield self._context.request(catalog.operation("list_drafts"))
        data = checked(answer).json()
        items = cast("dict[str, Any]", data).get("items") if isinstance(data, dict) else None
        found = cast("list[object]", items) if isinstance(items, list) else []
        return tuple(Record(cast("dict[str, Any]", i)) for i in found if isinstance(i, dict))

    def get(self, ref: object) -> Plan[Record]:
        catalog = yield from self._context.catalog()
        return _record((yield self._context.request(catalog.operation("get_draft"), ref=ref)))

    def create(
        self,
        title: str,
        operations: Sequence[Mapping[str, Any]] | None,
        reason: dict[str, str] | None,
    ) -> Plan[Record]:
        catalog = yield from self._context.catalog()
        body: dict[str, Any] = {"title": title}
        if operations is not None:
            body["operations"] = [dict(o) for o in operations]
        if reason is not None:
            body["reason"] = reason
        request = self._context.request(catalog.operation("create_draft"), body=body)
        return _record((yield request))

    def _read(self, ref: object, etag: str | None) -> Plan[tuple[object, str | None]]:
        if etag is None and not (isinstance(ref, Record) and ref.etag is not None):
            ref = yield from self.get(ref)
        return ref, etag

    def update(self, ref: object, changes: dict[str, Any], etag: str | None) -> Plan[Record]:
        if not changes:
            msg = "nothing to change in the draft"
            raise ClientError("client.missing-parameter", msg)
        ref, etag = yield from self._read(ref, etag)
        catalog = yield from self._context.catalog()
        request = self._context.request(
            catalog.operation("update_draft"), ref=ref, body=changes, etag=etag
        )
        return _record((yield request))

    def delete(self, ref: object, etag: str | None) -> Plan[None]:
        ref, etag = yield from self._read(ref, etag)
        catalog = yield from self._context.catalog()
        request = self._context.request(catalog.operation("delete_draft"), ref=ref, etag=etag)
        checked((yield request))

    def submit(
        self, ref: object, *, approval_self: bool, key: str | None
    ) -> Plan[Applied | Pending]:
        # The version read: a draft modified since is not submitted (412).
        ref, etag = yield from self._read(ref, None)
        catalog = yield from self._context.catalog()
        params: dict[str, object] = {} if approval_self else {_APPROVAL: None}
        request = self._context.request(
            catalog.operation("submit_draft"), ref=ref, params=params, key=key, etag=etag
        )
        result = outcome((yield request))
        if not isinstance(result, Applied | Pending):
            msg = "the server answered a submission without its changeset"
            raise ClientError("client.unexpected-answer", msg)
        return result


def _changes(
    title: str | None,
    summary: str | None,
    description: str | None,
    operations: Sequence[Mapping[str, Any]] | None,
) -> dict[str, Any]:
    changes: dict[str, Any] = {}
    if title is not None:
        changes["title"] = title
    reason = _reason(summary, description)
    if reason is not None:
        changes["reason"] = reason
    if operations is not None:
        changes["operations"] = [dict(o) for o in operations]
    return changes


class Drafts:
    """The drafts of the caller (synchronous client)."""

    def __init__(self, context: Context, run: Callable[[Plan[Any]], Any]) -> None:
        self._plans = _Drafts(context)
        self._run = run

    def list(self) -> tuple[Record, ...]:
        """The drafts of the caller, the most recently modified first, without their
        operations (`operation_count`)."""
        return cast("tuple[Record, ...]", self._run(self._plans.list()))

    def get(self, ref: object) -> Record:
        """A draft whole, its version as ETag."""
        return cast("Record", self._run(self._plans.get(ref)))

    def create(
        self,
        title: str,
        *,
        operations: Sequence[Mapping[str, Any]] | None = None,
        summary: str | None = None,
        description: str | None = None,
    ) -> Record:
        """A new draft: a title, its first operations, the motive it will submit."""
        plan = self._plans.create(title, operations, _reason(summary, description))
        return cast("Record", self._run(plan))

    def update(  # noqa: PLR0913 - the members of a draft
        self,
        ref: object,
        *,
        title: str | None = None,
        summary: str | None = None,
        description: str | None = None,
        operations: Sequence[Mapping[str, Any]] | None = None,
        etag: str | None = None,
    ) -> Record:
        """Its title, motive or operations (the list replaced), under the version read."""
        changes = _changes(title, summary, description, operations)
        return cast("Record", self._run(self._plans.update(ref, changes, etag)))

    def delete(self, ref: object, *, etag: str | None = None) -> None:
        self._run(self._plans.delete(ref, etag))

    def submit(
        self, ref: object, *, approval_self: bool = True, idempotency_key: str | None = None
    ) -> Applied | Pending:
        """Its changeset: applied when `approval_self` and its author may, pending otherwise;
        the draft is then deleted. Refused, it raises `Refused` and the draft stays."""
        plan = self._plans.submit(ref, approval_self=approval_self, key=idempotency_key)
        return cast("Applied | Pending", self._run(plan))


class AsyncDrafts:
    """The drafts of the caller (asynchronous client)."""

    def __init__(self, context: Context, run: Callable[[Plan[Any]], Any]) -> None:
        self._plans = _Drafts(context)
        self._run = run

    async def list(self) -> tuple[Record, ...]:
        return cast("tuple[Record, ...]", await self._run(self._plans.list()))

    async def get(self, ref: object) -> Record:
        return cast("Record", await self._run(self._plans.get(ref)))

    async def create(
        self,
        title: str,
        *,
        operations: Sequence[Mapping[str, Any]] | None = None,
        summary: str | None = None,
        description: str | None = None,
    ) -> Record:
        plan = self._plans.create(title, operations, _reason(summary, description))
        return cast("Record", await self._run(plan))

    async def update(  # noqa: PLR0913 - the members of a draft
        self,
        ref: object,
        *,
        title: str | None = None,
        summary: str | None = None,
        description: str | None = None,
        operations: Sequence[Mapping[str, Any]] | None = None,
        etag: str | None = None,
    ) -> Record:
        changes = _changes(title, summary, description, operations)
        return cast("Record", await self._run(self._plans.update(ref, changes, etag)))

    async def delete(self, ref: object, *, etag: str | None = None) -> None:
        await self._run(self._plans.delete(ref, etag))

    async def submit(
        self, ref: object, *, approval_self: bool = True, idempotency_key: str | None = None
    ) -> Applied | Pending:
        plan = self._plans.submit(ref, approval_self=approval_self, key=idempotency_key)
        return cast("Applied | Pending", await self._run(plan))
