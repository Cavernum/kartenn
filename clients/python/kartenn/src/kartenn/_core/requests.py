"""Building requests without input or output (REQ-023-10, 14, 41): filters of the 006,
the switch to `QUERY` past 8 KiB, and the headers every request carries.

Filters (006 § Filtres): a field of the core or a property `prop.<key>`, compared by
`eq` (repeated: one of), `between` (bounds open on either side) or `exists`. In the URL
the operator follows the field after `:`; in the body of a `QUERY` it is a member.
"""

from __future__ import annotations

import json
import platform
import re
import secrets
import uuid
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Literal, cast
from urllib.parse import quote, urlencode

from kartenn._core.dates import to_param
from kartenn._core.messages import Headers, Request
from kartenn._core.problems import ClientError

if TYPE_CHECKING:
    from collections.abc import Iterable
    from datetime import datetime

__all__ = [
    "URL_LIMIT",
    "Filter",
    "between",
    "encode",
    "exists",
    "filters_from",
    "idempotency_key",
    "list_request",
    "parse_prop",
    "query_params",
    "traceparent",
    "user_agent",
]

# 006 § Filtres: a URL of more than 8 KiB is refused (414) before the routing.
URL_LIMIT = 8 * 1024
# REQ-003-2: a property key is `<namespace>.<name>`, never with `:`.
_PROPERTY_KEY = re.compile(r"[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*")
_RESERVED = frozenset({"valid_at", "as_of", "cursor", "page_size"})

Op = Literal["eq", "between", "exists"]
Scalar = str | int | float | bool


@dataclass(frozen=True)
class Filter:
    """A condition: `eq` holds one value or more, `between` a pair of optional bounds,
    `exists` a boolean."""

    field: str
    op: Op
    value: tuple[Scalar, ...] | tuple[Scalar | None, Scalar | None] | bool

    def params(self) -> list[tuple[str, str]]:
        """Its form in a query string."""
        if self.op == "exists":
            return [(f"{self.field}:exists", _text(self.value))]
        values = cast("tuple[Scalar | None, ...]", self.value)
        if self.op == "between":
            low, high = values
            bounds = f"{'' if low is None else _text(low)}..{'' if high is None else _text(high)}"
            return [(f"{self.field}:between", bounds)]
        return [(self.field, _text(value)) for value in values]

    def condition(self) -> dict[str, Any]:
        """Its form in the body of a `QUERY` (schema `Wanted`)."""
        if self.op == "exists":
            return {"field": self.field, "op": "exists", "value": self.value}
        values = list(cast("tuple[Scalar | None, ...]", self.value))
        if self.op == "between":
            return {"field": self.field, "op": "between", "value": values}
        return {"field": self.field, "op": "eq", "value": values[0] if len(values) == 1 else values}


@dataclass(frozen=True)
class _Between:
    low: Scalar | None
    high: Scalar | None


@dataclass(frozen=True)
class _Exists:
    present: bool


def between(low: Scalar | None = None, high: Scalar | None = None) -> _Between:
    """`field:between=low..high`; a missing bound leaves that side open."""
    if low is None and high is None:
        msg = "between needs at least one bound"
        raise ClientError("client.invalid-filter", msg)
    return _Between(low, high)


def exists(present: bool = True) -> _Exists:  # noqa: FBT001, FBT002 - the value of the filter
    """`field:exists=true` (present) or `false` (absent)."""
    return _Exists(present)


def _text(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value)


def _filter(field: str, value: object) -> Filter:
    if isinstance(value, _Between):
        return Filter(field, "between", (value.low, value.high))
    if isinstance(value, _Exists):
        return Filter(field, "exists", value.present)
    if isinstance(value, Sequence) and not isinstance(value, str | bytes):
        values = tuple(cast("Sequence[Scalar]", value))
        if not values:
            msg = f"the filter {field} has no value"
            raise ClientError("client.invalid-filter", msg)
        return Filter(field, "eq", values)
    return Filter(field, "eq", (cast("Scalar", value),))


def filters_from(
    members: Mapping[str, object],
    prop: Mapping[str, object] | None = None,
    *,
    known: Iterable[str] | None = None,
) -> list[Filter]:
    """The filters of keyword arguments (`domain="x"`, `usage=["a", "b"]`,
    `within="192.0.2.0/24"`) and of properties (`{"dns.ttl": between(60, 3600)}`).

    With `known`, a field outside it is refused before any request (REQ-023-23)."""
    allowed = None if known is None else frozenset(known)
    found: list[Filter] = []
    for field, value in members.items():
        if value is None:
            continue
        if field in _RESERVED:
            msg = f"{field} is not a filter"
            raise ClientError("client.invalid-filter", msg)
        if allowed is not None and field not in allowed:
            names = ", ".join(sorted(allowed)) or "none"
            msg = f"unknown filter {field}; the filters are: {names}, prop.<key>"
            raise ClientError("client.unknown-filter", msg)
        found.append(_filter(field, value))
    for key, value in (prop or {}).items():
        _check_key(key)
        found.append(_filter(f"prop.{key}", value))
    return found


def _check_key(key: str) -> None:
    if _PROPERTY_KEY.fullmatch(key) is None:
        msg = f"not a property key (namespace.name): {key!r}"
        raise ClientError("client.invalid-filter", msg)


def parse_prop(text: str) -> Filter:
    """A filter of the command line, `key[:op]=value` (REQ-023-23): `dns.ttl:between=60..3600`,
    `dns.fqdn:exists=true`, `policy.environment=prod`."""
    left, sep, value = text.partition("=")
    if not sep:
        msg = f"a property filter is key[:op]=value: {text!r}"
        raise ClientError("client.invalid-filter", msg)
    key, _, op = left.partition(":")
    _check_key(key)
    field = f"prop.{key}"
    if op in {"", "eq"}:
        return Filter(field, "eq", (value,))
    if op == "between":
        low, dots, high = value.partition("..")
        if not dots or (not low and not high):
            msg = f"between takes low..high, one bound at least: {value!r}"
            raise ClientError("client.invalid-filter", msg)
        return Filter(field, "between", (low or None, high or None))
    if op == "exists":
        if value not in {"true", "false"}:
            msg = f"exists takes true or false: {value!r}"
            raise ClientError("client.invalid-filter", msg)
        return Filter(field, "exists", value == "true")
    msg = f"unknown operator {op!r}; the operators are eq, between, exists"
    raise ClientError("client.invalid-filter", msg)


def query_params(
    filters: Iterable[Filter],
    *,
    valid_at: datetime | str | None = None,
    as_of: datetime | str | None = None,
    page_size: int | None = None,
) -> list[tuple[str, str]]:
    params = [pair for found in filters for pair in found.params()]
    for name, value in (("valid_at", to_param(valid_at)), ("as_of", to_param(as_of))):
        if value is not None:
            params.append((name, value))
    if page_size is not None:
        params.append(("page_size", str(page_size)))
    return params


def encode(params: Iterable[tuple[str, str]]) -> str:
    """The query string as sent: `:` stays literal (RFC 3986 §3.4, `pchar`)."""
    return urlencode(list(params), quote_via=quote, safe=":")


def list_request(  # noqa: PLR0913 - the dates and the size of a read
    path: str,
    filters: Sequence[Filter] = (),
    *,
    valid_at: datetime | str | None = None,
    as_of: datetime | str | None = None,
    page_size: int | None = None,
    query: bool = False,
    base: str = "",
) -> Request:
    """The first page of a list: a `GET`, or a `QUERY` when the URL would exceed 8 KiB
    (REQ-023-10). `query` tells whether the collection accepts `QUERY` (`x-query`)."""
    params = query_params(filters, valid_at=valid_at, as_of=as_of, page_size=page_size)
    if len(f"{base}{path}?{encode(params)}".encode()) <= URL_LIMIT:
        return Request("GET", path, tuple(params))
    if not query:
        msg = (
            f"the filters of {path} exceed {URL_LIMIT} bytes of URL and the server "
            "accepts no QUERY on it"
        )
        raise ClientError("client.query-unsupported", msg)
    dates = {"valid_at": to_param(valid_at), "as_of": to_param(as_of)}
    body: dict[str, Any] = {name: value for name, value in dates.items() if value is not None}
    body["filters"] = [found.condition() for found in filters]
    if page_size is not None:
        body["page_size"] = page_size
    return Request(
        "QUERY",
        path,
        headers=Headers.of({"Content-Type": "application/json"}),
        content=json.dumps(body, separators=(",", ":")).encode(),
    )


def idempotency_key() -> str:
    """A new `Idempotency-Key` (REQ-023-14): a UUID v4."""
    return str(uuid.uuid4())


def traceparent() -> str:
    """A W3C Trace Context `traceparent` of a new trace (REQ-023-41), flagged sampled."""
    return f"00-{secrets.token_hex(16)}-{secrets.token_hex(8)}-01"


def user_agent(version: str) -> str:
    return f"kartenn-python/{version} python/{platform.python_version()}"
