"""Pages and cursors (REQ-023-10, 006 § Curseurs).

A page is `{items, links: {next}}`; `next` is followed as given, never rewritten: the
cursor is signed and freezes the dates and the filters of the first page.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, cast

from kartenn._core.messages import Request
from kartenn._core.problems import ClientError

if TYPE_CHECKING:
    from collections.abc import Mapping

    from kartenn._core.messages import Response

__all__ = ["Page", "check_cursor", "next_link", "next_request", "parse_page"]

_LINK = re.compile(r"<([^>]*)>\s*;([^,]*)")
_REL_NEXT = re.compile(r'rel\s*=\s*"?([^";]*)"?')


@dataclass(frozen=True)
class Page:
    """One page of a list: its elements, the link to the next one, the `as_of` it was read
    at when known, and its (weak) ETag."""

    items: tuple[Mapping[str, Any], ...]
    next: str | None
    as_of: str | None
    etag: str | None
    body: Mapping[str, Any]


def next_link(body: Mapping[str, Any], response: Response) -> str | None:
    """`links.next` of the body, else the `Link` header `rel="next"` (RFC 8288)."""
    links = body.get("links")
    if isinstance(links, dict):
        found = cast("dict[str, Any]", links).get("next")
        if isinstance(found, str) and found:
            return found
    for header in response.headers.get_all("Link"):
        for target, attributes in _LINK.findall(header):
            rel = _REL_NEXT.search(attributes)
            if rel is not None and "next" in rel.group(1).split():
                return target
    return None


def parse_page(response: Response, *, as_of: str | None = None) -> Page:
    """The page of a response; `as_of` is the date asked for, kept when the server does
    not answer the one it read at."""
    raw = response.json()
    if not isinstance(raw, dict) or not isinstance(cast("dict[str, Any]", raw).get("items"), list):
        msg = "the answer is not a page of a list (no items)"
        raise ClientError("client.unexpected-answer", msg)
    body = cast("dict[str, Any]", raw)
    items = tuple(
        cast("Mapping[str, Any]", item)
        for item in cast("list[object]", body["items"])
        if isinstance(item, dict)
    )
    answered = body.get("as_of")
    return Page(
        items=items,
        next=next_link(body, response),
        as_of=answered if isinstance(answered, str) else as_of,
        etag=response.etag,
        body=body,
    )


def next_request(page: Page) -> Request | None:
    """The request of the next page: a `GET` of the link, even after a `QUERY` (the cursor
    carries the filters)."""
    return None if page.next is None else Request("GET", page.next)


def check_cursor(cursor: str | None, *, others: Mapping[str, object]) -> None:
    """A cursor freezes the filters and the dates (006): along with it, only `page_size`."""
    if cursor is None:
        return
    given = sorted(name for name, value in others.items() if value not in (None, (), [], {}))
    if given:
        msg = f"a cursor freezes the filters and the dates; drop {', '.join(given)}"
        raise ClientError("client.cursor-frozen", msg)
