"""HTTP messages of the core: what the client hands to a transport and gets back.

They are not the types of httpx2: the adapter translates (design § Transport), so that no
module but `transport/httpx2.py` depends on it.
"""

from __future__ import annotations

import json
from collections.abc import Mapping
from dataclasses import dataclass, field, replace
from typing import TYPE_CHECKING, Any, cast

if TYPE_CHECKING:
    from collections.abc import Iterable, Iterator

__all__ = ["Headers", "Request", "Response"]


@dataclass(frozen=True)
class Headers:
    """Header fields in order, names compared without case (RFC 9110 §5.1)."""

    items: tuple[tuple[str, str], ...] = ()

    @classmethod
    def of(cls, fields: Mapping[str, str] | Iterable[tuple[str, str]] | None = None) -> Headers:
        if fields is None:
            return cls()
        pairs = cast("Mapping[str, str]", fields).items() if isinstance(fields, Mapping) else fields
        return cls(tuple((str(name), str(value)) for name, value in pairs))

    def get(self, name: str, default: str | None = None) -> str | None:
        lower = name.lower()
        for key, value in self.items:
            if key.lower() == lower:
                return value
        return default

    def get_all(self, name: str) -> list[str]:
        lower = name.lower()
        return [value for key, value in self.items if key.lower() == lower]

    def __contains__(self, name: object) -> bool:
        return isinstance(name, str) and self.get(name) is not None

    def __iter__(self) -> Iterator[tuple[str, str]]:
        return iter(self.items)

    def set(self, name: str, value: str) -> Headers:
        """These headers, `name` replaced by `value`."""
        lower = name.lower()
        kept = tuple(pair for pair in self.items if pair[0].lower() != lower)
        return Headers((*kept, (name, value)))

    def setdefault(self, name: str, value: str) -> Headers:
        return self if name in self else self.set(name, value)

    def without(self, name: str) -> Headers:
        lower = name.lower()
        return Headers(tuple(pair for pair in self.items if pair[0].lower() != lower))


@dataclass(frozen=True)
class Request:
    """A request: `url` is absolute, or a path resolved against the base of the client."""

    method: str
    url: str
    params: tuple[tuple[str, str], ...] = ()
    headers: Headers = field(default_factory=Headers)
    content: bytes | None = None

    def with_header(self, name: str, value: str) -> Request:
        return replace(self, headers=self.headers.set(name, value))

    def with_default_header(self, name: str, value: str) -> Request:
        return replace(self, headers=self.headers.setdefault(name, value))

    @property
    def replayable(self) -> bool:
        """Whether sending it twice does no harm (RFC 9110 §9.2.2), or the server
        recognises the second as the first by its `Idempotency-Key` (006/H8)."""
        return self.method in _IDEMPOTENT or "Idempotency-Key" in self.headers


_IDEMPOTENT = frozenset({"GET", "HEAD", "OPTIONS", "QUERY", "PUT"})


@dataclass(frozen=True)
class Response:
    """A response read in full."""

    status: int
    headers: Headers = field(default_factory=Headers)
    content: bytes = b""

    def json(self) -> Any:  # noqa: ANN401 - JSON is any value
        return json.loads(self.content) if self.content else None

    @property
    def etag(self) -> str | None:
        return self.headers.get("ETag")
