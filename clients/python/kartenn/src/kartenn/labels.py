"""The dictionary of labels `GET /v1/labels` (spec 018, REQ-018-21; 023/T15 amended).

Objects carry keys; the dictionary gives their labels for one language, by space
(`value`, `property`, `role`, `permission`, `rule`, `problem`, `kind`, `state`,
`policy.*`…). The client keeps it:

- in memory, for the run of the program, until an event `labels`, `ready` or `reset` of
  the stream (022) says it may have changed;
- on disk, in the cache of the catalog (`<catalog>/labels/`, T20), one file per server,
  profile and language, with its `ETag`: the next run asks again under `If-None-Match`
  and a 304 costs no download.

`resolve` falls back on the key: a code without a label shows as itself.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from http import HTTPStatus
from typing import TYPE_CHECKING, Any, cast

from kartenn._core.messages import Headers, Request
from kartenn._core.plans import Plan, checked
from kartenn._files import write_private

if TYPE_CHECKING:
    from pathlib import Path

__all__ = ["Dictionary", "LabelCache", "fetch", "invalidating"]

PATH = "/v1/labels"
# The events after which the dictionary may have changed (022; spec 018, R23).
INVALIDATING = frozenset({"labels", "ready", "reset"})


def invalidating(event_name: str) -> bool:
    """Whether an event of the stream makes the dictionary in memory stale."""
    return event_name in INVALIDATING


@dataclass(frozen=True)
class Dictionary:
    """The labels of one language, as the server serves them."""

    lang: str
    default_lang: str
    revision: str
    spaces: dict[str, Any] = field(default_factory=dict[str, Any])
    fallback: tuple[str, ...] = ()
    etag: str | None = None

    @classmethod
    def of(cls, body: dict[str, Any], etag: str | None) -> Dictionary:
        fallback = cast("dict[str, Any]", body.get("fallback") or {})
        return cls(
            lang=str(body.get("lang", "")),
            default_lang=str(body.get("default_lang", "")),
            revision=str(body.get("revision", "")),
            spaces=cast("dict[str, Any]", body.get("spaces") or {}),
            fallback=tuple(str(k) for k in fallback.get("keys", ())),
            etag=etag,
        )

    def to_json(self) -> str:
        return json.dumps(
            {
                "lang": self.lang,
                "default_lang": self.default_lang,
                "revision": self.revision,
                "spaces": self.spaces,
                "fallback": {"keys": list(self.fallback)},
                "etag": self.etag,
            },
            separators=(",", ":"),
            ensure_ascii=False,
        )

    def label(self, space: str, key: str, value: str | None = None) -> str | None:
        """The label of `key` in `space` (of the value `value` of the property `key` in
        the space `value`), or `None`."""
        found: object = self.spaces.get(space)
        for part in (key, value):
            if part is None:
                break
            found = cast("dict[str, Any]", found).get(part) if isinstance(found, dict) else None
        return found if isinstance(found, str) else None

    def resolve(self, space: str, key: str, value: str | None = None) -> str:
        """The label, else the key itself (R7)."""
        found = self.label(space, key, value)
        if found is not None:
            return found
        return value if value is not None else key


def _key(server: str, profile: str, lang: str) -> str:
    return hashlib.sha256(f"{server}\0{profile}\0{lang}".encode()).hexdigest()[:32]


class LabelCache:
    """The dictionaries on disk, beside the catalog: files in 0600 written atomically; a
    damaged file is a missing one."""

    def __init__(self, directory: Path) -> None:
        self.directory = directory

    def path(self, server: str, profile: str, lang: str) -> Path:
        return self.directory / f"{_key(server, profile, lang)}.json"

    def load(self, server: str, profile: str, lang: str) -> Dictionary | None:
        try:
            data = json.loads(self.path(server, profile, lang).read_text())
        except (OSError, ValueError):
            return None
        if not isinstance(data, dict):
            return None
        body = cast("dict[str, Any]", data)
        etag = body.get("etag")
        return Dictionary.of(body, etag if isinstance(etag, str) else None)

    def save(self, server: str, profile: str, lang: str, dictionary: Dictionary) -> None:
        write_private(self.path(server, profile, lang), dictionary.to_json())


def fetch(
    lang: str | None,
    known: Dictionary | None,
    *,
    valid_at: str | None = None,
    as_of: str | None = None,
) -> Plan[Dictionary]:
    """The dictionary of `lang` (the default language of the server when `None`), asked
    under `If-None-Match` when one is known: a 304 keeps it."""
    params = tuple(
        (name, value)
        for name, value in (("lang", lang), ("valid_at", valid_at), ("as_of", as_of))
        if value
    )
    headers = Headers.of({"If-None-Match": known.etag} if known and known.etag else {})
    response = yield Request("GET", PATH, params, headers)
    if response.status == HTTPStatus.NOT_MODIFIED and known is not None:
        return known
    body = checked(response).json()
    if not isinstance(body, dict):
        return Dictionary(lang or "", lang or "", "")
    return Dictionary.of(cast("dict[str, Any]", body), response.etag)
