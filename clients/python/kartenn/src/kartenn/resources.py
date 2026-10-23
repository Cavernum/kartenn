"""Dynamic resources (REQ-023-9, 10, 12, 14, 16): what the catalog derives, reached by
name (`kn.resource("vlans")`) or by attribute (`kn.vlans`, `kn.policy_rules`).

The calls are plans (`kartenn._core.plans`) built here once, run by the synchronous
`Resource` and the asynchronous `AsyncResource`. A `Record` is a `dict` that keeps the
ETag, the links and the `as_of` of its reading: `update` and `close` send its ETag in
`If-Match` without the caller copying it (H19: the ETag is opaque to the client).
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field, replace
from http import HTTPStatus
from typing import TYPE_CHECKING, Any, cast

from kartenn._core.dates import to_param
from kartenn._core.messages import Headers, Request
from kartenn._core.pages import Page, check_cursor, next_request, parse_page
from kartenn._core.plans import checked
from kartenn._core.problems import ClientError
from kartenn._core.requests import filters_from, idempotency_key, list_request
from kartenn.catalog import CatalogCache, discover

if TYPE_CHECKING:
    from collections.abc import AsyncIterator, Awaitable, Callable, Iterator, Mapping
    from datetime import datetime

    from kartenn._core.messages import Response
    from kartenn._core.plans import Plan
    from kartenn.catalog import Catalog, Operation
    from kartenn.catalog import Resource as Model

__all__ = [
    "Applied",
    "AsyncListing",
    "AsyncResource",
    "Context",
    "Listing",
    "Pending",
    "Record",
    "Resource",
    "outcome",
]

_TEMPLATE = re.compile(r"\{([^}]+)\}")
_REL_CHANGESET = "urn:kartenn:rel:changeset"
_JSON = "application/json"
_MERGE_PATCH = "application/merge-patch+json"


class Record(dict[str, Any]):
    """An object as read: its members, and the ETag, links and `as_of` of the reading."""

    def __init__(
        self,
        data: Mapping[str, Any] | None = None,
        *,
        etag: str | None = None,
        as_of: str | None = None,
    ) -> None:
        super().__init__(data or {})
        self.etag = etag
        self.as_of = as_of

    @property
    def id(self) -> str | None:
        found = self.get("id")
        return None if found is None else str(found)

    @property
    def links(self) -> dict[str, Any]:
        links = self.get("links")
        return cast("dict[str, Any]", links) if isinstance(links, dict) else {}

    def __repr__(self) -> str:
        return f"Record({super().__repr__()}, etag={self.etag!r})"


@dataclass(frozen=True)
class Applied:
    """A write applied: its changeset, the body of the answer, `Location` of a creation."""

    changeset: str | None
    state: str
    body: Record
    location: str | None = None


@dataclass(frozen=True)
class Pending:
    """A write held for a decision (202): the changeset to follow, `approval` or
    `proposal`."""

    changeset: str | None
    kind: str | None
    body: Record
    link: str | None = None


def _changeset_link(response: Response) -> str | None:
    for header in response.headers.get_all("Link"):
        for part in header.split(","):
            target, _, attributes = part.partition(";")
            if _REL_CHANGESET in attributes:
                return target.strip().strip("<>")
    return None


def _body(response: Response, as_of: str | None = None) -> Any:  # noqa: ANN401 - JSON
    data = response.json()
    if isinstance(data, dict):
        return Record(cast("dict[str, Any]", data), etag=response.etag, as_of=as_of)
    return data


def outcome(response: Response) -> Applied | Pending | Record | None:
    """The result of a write: `Applied` or `Pending` when the server answers a changeset,
    the object otherwise, `None` without a body."""
    checked(response)
    body = _body(response)
    if not isinstance(body, Record):
        return cast("Record | None", body)
    reference = body.get("changeset")
    if not isinstance(reference, dict) or "state" not in body:
        return body
    changeset = cast("dict[str, Any]", reference).get("id")
    identifier = None if changeset is None else str(changeset)
    if response.status == HTTPStatus.ACCEPTED:
        kind = body.get("kind")
        return Pending(
            identifier, None if kind is None else str(kind), body, _changeset_link(response)
        )
    return Applied(identifier, str(body["state"]), body, response.headers.get("Location"))


@dataclass
class _Shared:
    """What every view of a client shares: the server, the cache, the catalog loaded."""

    url: str
    cache: CatalogCache
    catalog: Catalog | None = None


@dataclass(frozen=True)
class Context:
    """A client seen at some dates (`kn.at(...)`): the plans of its calls."""

    shared: _Shared
    valid_at: str | None = None
    as_of: str | None = None
    defaults: dict[str, str] = field(default_factory=dict[str, str])

    @classmethod
    def of(cls, url: str, cache: CatalogCache | None = None) -> Context:
        return cls(_Shared(url, cache or CatalogCache()))

    def at(self, valid_at: datetime | str | None, as_of: datetime | str | None) -> Context:
        return replace(self, valid_at=to_param(valid_at), as_of=to_param(as_of))

    # The catalog -----------------------------------------------------------------

    def catalog(self, *, refresh: bool = False) -> Plan[Catalog]:
        if self.shared.catalog is None or refresh:
            self.shared.catalog = yield from discover(
                self.shared.url, self.shared.cache, refresh=refresh
            )
        return self.shared.catalog

    def model(self, name: str) -> Plan[Model]:
        catalog = yield from self.catalog()
        return catalog.resource(name)

    # Requests --------------------------------------------------------------------

    def request(  # noqa: PLR0913 - every part of a request
        self,
        operation: Operation,
        *,
        ref: object = None,
        params: Mapping[str, object] | None = None,
        body: object = None,
        etag: str | None = None,
        key: str | None = None,
        headers: Mapping[str, str] | None = None,
    ) -> Request:
        """The request of an operation: path filled, query, `If-Match`, `Idempotency-Key`."""
        rest = dict(params or {})
        path = operation.path
        for index, name in enumerate(_TEMPLATE.findall(path)):
            value = _identifier(ref) if index == 0 and ref is not None else rest.pop(name, None)
            if value is None:
                msg = f"{operation.operation_id} needs {name}"
                raise ClientError("client.missing-parameter", msg)
            path = path.replace(f"{{{name}}}", _quote(str(value)), 1)
        query = self._query(operation, rest)
        sent = Headers.of(headers)
        if etag is None and isinstance(ref, Record) and operation.needs_if_match:
            etag = ref.etag
        if etag is not None:
            sent = sent.set("If-Match", etag)
        elif operation.needs_if_match:
            msg = f"{operation.operation_id} needs the ETag of the object (If-Match): read it first"
            raise ClientError("client.etag-required", msg)
        if operation.method == "POST" and operation.takes_idempotency_key:
            sent = sent.setdefault("Idempotency-Key", key or idempotency_key())
        content = None
        if body is not None:
            sent = sent.setdefault("Content-Type", operation.body_media or _JSON)
            content = json.dumps(body, separators=(",", ":")).encode()
        return Request(operation.method, path, tuple(query), sent, content)

    def _query(self, operation: Operation, rest: dict[str, object]) -> list[tuple[str, str]]:
        accepted = {p.name for p in operation.params if p.location == "query"}
        dates = {"valid_at": self.valid_at, "as_of": self.as_of}
        for name, value in dates.items():
            if name in accepted and value is not None:
                rest.setdefault(name, value)
        unknown = sorted(set(rest) - accepted)
        if unknown:
            names = ", ".join(sorted(accepted)) or "none"
            msg = (
                f"{operation.operation_id} has no parameter {', '.join(unknown)} (it has: {names})"
            )
            raise ClientError("client.unknown-parameter", msg)
        query: list[tuple[str, str]] = []
        for name, value in rest.items():
            if value is None:
                continue
            values = cast("list[object]", value) if isinstance(value, list | tuple) else [value]
            query.extend((name, _text(v)) for v in values)
        return query

    # Plans -----------------------------------------------------------------------

    def first_page(  # noqa: PLR0913 - the filters and the dates of a list
        self,
        name: str,
        *,
        filters: Mapping[str, object],
        prop: Mapping[str, object] | None,
        valid_at: datetime | str | None,
        as_of: datetime | str | None,
        page_size: int | None,
        cursor: str | None,
    ) -> Plan[Page]:
        model = yield from self.model(name)
        listing = _verb_operation(self.shared.catalog, model, "list")
        if cursor is not None:
            check_cursor(
                cursor, others={**filters, "prop": prop, "valid_at": valid_at, "as_of": as_of}
            )
            params = [("cursor", cursor)] + ([("page_size", str(page_size))] if page_size else [])
            return (yield from page(Request("GET", listing.path, tuple(params)), None))
        if prop and not listing.takes_properties:
            msg = f"the list of {name} takes no property filter"
            raise ClientError("client.unknown-filter", msg)
        known = [p.name for p in listing.filters]
        found = filters_from(filters, prop, known=known)
        dates = to_param(valid_at) or self.valid_at, to_param(as_of) or self.as_of
        query = model.verb("query") is not None
        request = list_request(
            listing.path,
            found,
            valid_at=dates[0],
            as_of=dates[1],
            page_size=page_size,
            query=query,
            base=self.shared.url,
        )
        return (yield from page(request, dates[1]))

    def get(self, name: str, ref: object, params: Mapping[str, object]) -> Plan[Record]:
        model = yield from self.model(name)
        operation = _verb_operation(self.shared.catalog, model, "get")
        answer = yield self.request(operation, ref=ref, params=params)
        return cast("Record", _body(checked(answer), self._as_of(params)))

    def create(  # noqa: PLR0913 - the reserved members of a write
        self,
        name: str,
        body: Mapping[str, object],
        *,
        motive: str | None,
        valid_from: datetime | str | None,
        valid_to: datetime | str | None,
        key: str | None,
    ) -> Plan[Applied | Pending | Record | None]:
        model = yield from self.model(name)
        operation = _verb_operation(self.shared.catalog, model, "create")
        content = _reserved(body, motive=motive, valid_from=valid_from, valid_to=valid_to)
        request = self.request(operation, body=content, key=key)
        if "Idempotency-Key" not in request.headers:
            request = request.with_header("Idempotency-Key", key or idempotency_key())
        return outcome((yield request))

    def update(  # noqa: PLR0913 - the reserved members of a write
        self,
        name: str,
        ref: object,
        changes: Mapping[str, object],
        *,
        motive: str | None,
        valid_from: datetime | str | None,
        etag: str | None,
    ) -> Plan[Applied | Pending | Record | None]:
        model = yield from self.model(name)
        operation = _verb_operation(self.shared.catalog, model, "update")
        content = _reserved(changes, motive=motive, valid_from=valid_from, valid_to=None)
        request = self.request(operation, ref=ref, body=content, etag=etag)
        if operation.body_media is None:
            request = replace(request, headers=request.headers.set("Content-Type", _MERGE_PATCH))
        return outcome((yield request))

    def close(
        self,
        name: str,
        ref: object,
        *,
        motive: str | None,
        valid_from: datetime | str | None,
        etag: str | None,
    ) -> Plan[Applied | Pending | Record | None]:
        if not motive:
            msg = "closing an object needs a motive"
            raise ClientError("client.motive-required", msg)
        model = yield from self.model(name)
        operation = _verb_operation(self.shared.catalog, model, "close")
        content = _reserved({}, motive=motive, valid_from=valid_from, valid_to=None)
        return outcome((yield self.request(operation, ref=ref, body=content, etag=etag)))

    def verb(  # noqa: PLR0913 - a verb takes what its operation takes
        self,
        name: str,
        verb: str,
        ref: object,
        *,
        body: object,
        etag: str | None,
        params: Mapping[str, object],
    ) -> Plan[Any]:
        model = yield from self.model(name)
        operation = _verb_operation(self.shared.catalog, model, verb)
        if operation.method in {"GET", "QUERY"} and body is None:
            answer = yield self.request(operation, ref=ref, params=params)
            return _body(checked(answer), self._as_of(params))
        return outcome(
            (yield self.request(operation, ref=ref, params=params, body=body, etag=etag))
        )

    def verb_page(
        self, name: str, verb: str, ref: object, params: Mapping[str, object]
    ) -> Plan[Page]:
        model = yield from self.model(name)
        operation = _verb_operation(self.shared.catalog, model, verb)
        return (
            yield from page(self.request(operation, ref=ref, params=params), self._as_of(params))
        )

    def call(  # noqa: PLR0913 - an operation takes what the contract gives it
        self,
        operation_id: str,
        *,
        body: object,
        etag: str | None,
        key: str | None,
        headers: Mapping[str, str] | None,
        params: Mapping[str, object],
    ) -> Plan[Any]:
        catalog = yield from self.catalog()
        operation = catalog.operation(operation_id)
        request = self.request(
            operation, params=params, body=body, etag=etag, key=key, headers=headers
        )
        answer = checked((yield request))
        if operation.method in {"GET", "QUERY"}:
            return _body(answer, self._as_of(params))
        return outcome(answer)

    def _as_of(self, params: Mapping[str, object]) -> str | None:
        given = params.get("as_of")
        return str(given) if given is not None else self.as_of


def page(request: Request, as_of: str | None) -> Plan[Page]:
    """One page of a list."""
    return parse_page(checked((yield request)), as_of=as_of)


def _verb_operation(catalog: Catalog | None, model: Model, verb: str) -> Operation:
    assert catalog is not None  # noqa: S101 - loaded by the plan that asks
    found = model.verb(verb) or next((v for v in model.verbs if _snake(v.name) == verb), None)
    if found is None:
        offered = ", ".join(model.names) or "none"
        msg = f"{model.name} has no verb {verb!r} on this server (it has: {offered})"
        raise ClientError("client.operation-unavailable", msg)
    return catalog.operation(found.operation_id)


def _snake(name: str) -> str:
    return re.sub(r"[-.]", "_", name)


def _identifier(ref: object) -> object:
    if isinstance(ref, dict):
        found = cast("dict[str, Any]", ref).get("id")
        if found is None:
            msg = "the object given has no id"
            raise ClientError("client.missing-parameter", msg)
        return found
    return ref


def _quote(value: str) -> str:
    from urllib.parse import quote  # noqa: PLC0415 - one use

    return quote(value, safe="")


def _text(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value)


def _reserved(
    members: Mapping[str, object],
    *,
    motive: str | None,
    valid_from: datetime | str | None,
    valid_to: datetime | str | None,
) -> dict[str, object]:
    """A body of a write and its reserved members (006 § Représentations)."""
    reserved = {
        "motive": motive,
        "valid_from": to_param(valid_from),
        "valid_to": to_param(valid_to),
    }
    return {**members, **{k: v for k, v in reserved.items() if v is not None}}


# Synchronous ------------------------------------------------------------------------


class Listing:
    """The elements of a list, page after page: `links.next` followed as given."""

    def __init__(self, run: Callable[[Plan[Page]], Page], first: Callable[[], Plan[Page]]) -> None:
        self._run = run
        self._first = first
        self.as_of: str | None = None

    def pages(self) -> Iterator[Page]:
        current = self._run(self._first())
        self.as_of = current.as_of
        yield current
        while (following := next_request(current)) is not None:
            current = self._run(page(following, current.as_of))
            yield current

    def __iter__(self) -> Iterator[Record]:
        for current in self.pages():
            for item in current.items:
                yield Record(item, as_of=current.as_of)

    def first_page(self) -> Page:
        return next(self.pages())


class Resource:
    """A resource of the catalog, for the synchronous client."""

    def __init__(self, context: Context, run: Callable[[Plan[Any]], Any], name: str) -> None:
        self._context = context
        self._run = run
        self.name = name

    def __repr__(self) -> str:
        return f"Resource({self.name!r})"

    @property
    def model(self) -> Model:
        return cast("Model", self._run(self._context.model(self.name)))

    def list(
        self,
        *,
        valid_at: datetime | str | None = None,
        as_of: datetime | str | None = None,
        page_size: int | None = None,
        cursor: str | None = None,
        prop: Mapping[str, object] | None = None,
        **filters: object,
    ) -> Listing:
        def first() -> Plan[Page]:
            return self._context.first_page(
                self.name,
                filters=filters,
                prop=prop,
                valid_at=valid_at,
                as_of=as_of,
                page_size=page_size,
                cursor=cursor,
            )

        return Listing(self._run, first)

    def get(self, ref: object, /, **params: object) -> Record:
        return cast("Record", self._run(self._context.get(self.name, ref, params)))

    def create(
        self,
        body: Mapping[str, object] | None = None,
        /,
        *,
        motive: str | None = None,
        valid_from: datetime | str | None = None,
        valid_to: datetime | str | None = None,
        idempotency_key: str | None = None,
        **members: object,
    ) -> Applied | Pending | Record | None:
        content = {**(body or {}), **members}
        plan = self._context.create(
            self.name,
            content,
            motive=motive,
            valid_from=valid_from,
            valid_to=valid_to,
            key=idempotency_key,
        )
        return cast("Applied | Pending | Record | None", self._run(plan))

    def update(
        self,
        ref: object,
        changes: Mapping[str, object] | None = None,
        /,
        *,
        motive: str | None = None,
        valid_from: datetime | str | None = None,
        etag: str | None = None,
        **members: object,
    ) -> Applied | Pending | Record | None:
        content = {**(changes or {}), **members}
        plan = self._context.update(
            self.name, ref, content, motive=motive, valid_from=valid_from, etag=etag
        )
        return cast("Applied | Pending | Record | None", self._run(plan))

    def close(
        self,
        ref: object,
        /,
        *,
        motive: str | None = None,
        valid_from: datetime | str | None = None,
        etag: str | None = None,
    ) -> Applied | Pending | Record | None:
        plan = self._context.close(self.name, ref, motive=motive, valid_from=valid_from, etag=etag)
        return cast("Applied | Pending | Record | None", self._run(plan))

    def verb(
        self,
        verb: str,
        ref: object = None,
        /,
        *,
        body: object = None,
        etag: str | None = None,
        **params: object,
    ) -> Any:  # noqa: ANN401 - what the operation answers
        """A sub-resource or an action by its name in the catalog (`history`,
        `properties.explain`, `acknowledgement`…)."""
        return self._run(
            self._context.verb(self.name, verb, ref, body=body, etag=etag, params=params)
        )

    def pages_of(self, verb: str, ref: object = None, /, **params: object) -> Listing:
        """A sub-resource read as a list, page after page (`history`, `children`…)."""
        return Listing(self._run, lambda: self._context.verb_page(self.name, verb, ref, params))

    def __getattr__(self, attribute: str) -> Callable[..., Any]:
        if attribute.startswith("_"):
            raise AttributeError(attribute)

        def call(
            ref: object = None, /, *, body: object = None, etag: str | None = None, **params: object
        ) -> Any:  # noqa: ANN401
            return self.verb(attribute, ref, body=body, etag=etag, **params)

        return call


# Asynchronous -----------------------------------------------------------------------


class AsyncListing:
    def __init__(
        self, run: Callable[[Plan[Page]], Awaitable[Page]], first: Callable[[], Plan[Page]]
    ) -> None:
        self._run = run
        self._first = first
        self.as_of: str | None = None

    async def pages(self) -> AsyncIterator[Page]:
        current = await self._run(self._first())
        self.as_of = current.as_of
        yield current
        while (following := next_request(current)) is not None:
            current = await self._run(page(following, current.as_of))
            yield current

    async def __aiter__(self) -> AsyncIterator[Record]:
        async for current in self.pages():
            for item in current.items:
                yield Record(item, as_of=current.as_of)

    async def first_page(self) -> Page:
        return await anext(aiter(self.pages()))


class AsyncResource:
    """A resource of the catalog, for the asynchronous client."""

    def __init__(
        self, context: Context, run: Callable[[Plan[Any]], Awaitable[Any]], name: str
    ) -> None:
        self._context = context
        self._run = run
        self.name = name

    def __repr__(self) -> str:
        return f"AsyncResource({self.name!r})"

    async def model(self) -> Model:
        return cast("Model", await self._run(self._context.model(self.name)))

    def list(
        self,
        *,
        valid_at: datetime | str | None = None,
        as_of: datetime | str | None = None,
        page_size: int | None = None,
        cursor: str | None = None,
        prop: Mapping[str, object] | None = None,
        **filters: object,
    ) -> AsyncListing:
        def first() -> Plan[Page]:
            return self._context.first_page(
                self.name,
                filters=filters,
                prop=prop,
                valid_at=valid_at,
                as_of=as_of,
                page_size=page_size,
                cursor=cursor,
            )

        return AsyncListing(self._run, first)

    async def get(self, ref: object, /, **params: object) -> Record:
        return cast("Record", await self._run(self._context.get(self.name, ref, params)))

    async def create(
        self,
        body: Mapping[str, object] | None = None,
        /,
        *,
        motive: str | None = None,
        valid_from: datetime | str | None = None,
        valid_to: datetime | str | None = None,
        idempotency_key: str | None = None,
        **members: object,
    ) -> Applied | Pending | Record | None:
        content = {**(body or {}), **members}
        plan = self._context.create(
            self.name,
            content,
            motive=motive,
            valid_from=valid_from,
            valid_to=valid_to,
            key=idempotency_key,
        )
        return cast("Applied | Pending | Record | None", await self._run(plan))

    async def update(
        self,
        ref: object,
        changes: Mapping[str, object] | None = None,
        /,
        *,
        motive: str | None = None,
        valid_from: datetime | str | None = None,
        etag: str | None = None,
        **members: object,
    ) -> Applied | Pending | Record | None:
        content = {**(changes or {}), **members}
        plan = self._context.update(
            self.name, ref, content, motive=motive, valid_from=valid_from, etag=etag
        )
        return cast("Applied | Pending | Record | None", await self._run(plan))

    async def close(
        self,
        ref: object,
        /,
        *,
        motive: str | None = None,
        valid_from: datetime | str | None = None,
        etag: str | None = None,
    ) -> Applied | Pending | Record | None:
        plan = self._context.close(self.name, ref, motive=motive, valid_from=valid_from, etag=etag)
        return cast("Applied | Pending | Record | None", await self._run(plan))

    async def verb(
        self,
        verb: str,
        ref: object = None,
        /,
        *,
        body: object = None,
        etag: str | None = None,
        **params: object,
    ) -> Any:  # noqa: ANN401 - what the operation answers
        plan = self._context.verb(self.name, verb, ref, body=body, etag=etag, params=params)
        return await self._run(plan)

    def pages_of(self, verb: str, ref: object = None, /, **params: object) -> AsyncListing:
        return AsyncListing(
            self._run, lambda: self._context.verb_page(self.name, verb, ref, params)
        )

    def __getattr__(self, attribute: str) -> Callable[..., Awaitable[Any]]:
        if attribute.startswith("_"):
            raise AttributeError(attribute)

        async def call(
            ref: object = None, /, *, body: object = None, etag: str | None = None, **params: object
        ) -> Any:  # noqa: ANN401
            return await self.verb(attribute, ref, body=body, etag=etag, **params)

        return call
