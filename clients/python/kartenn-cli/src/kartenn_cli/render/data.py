"""Data formats (REQ-023-25, 26; design § Sorties): `json`, `ndjson`, `yaml`, `csv`, the
projection of `--fields` and the flattening of nested members.

The data formats always carry keys, never labels. A member path is dotted
(`properties.policy.environment`); a key that holds dots itself (`policy.environment`, a
property key) is matched as a whole, the longest first.
"""

from __future__ import annotations

import csv
import io
import json
from typing import TYPE_CHECKING, Any, cast

if TYPE_CHECKING:
    from collections.abc import Callable, Iterable, Mapping, Sequence

__all__ = [
    "csv_rows",
    "dump_json",
    "dump_json_list",
    "dump_yaml",
    "flatten",
    "parse_fields",
    "pick",
    "project",
]

_MISSING = object()


def parse_fields(text: str | None) -> list[str] | None:
    if not text:
        return None
    return [part.strip() for part in text.split(",") if part.strip()]


def pick(data: object, path: str) -> tuple[list[str], Any]:
    """The keys walked and the value of a dotted path; `_MISSING` when absent."""
    parts = path.split(".")
    walked: list[str] = []
    current: Any = data
    while parts:
        if not isinstance(current, dict):
            return walked, _MISSING
        found = cast("dict[str, Any]", current)
        for size in range(len(parts), 0, -1):
            key = ".".join(parts[:size])
            if key in found:
                walked.append(key)
                current = found[key]
                parts = parts[size:]
                break
        else:
            return walked, _MISSING
    return walked, current


def missing(value: object) -> bool:
    return value is _MISSING


def project(item: Mapping[str, Any], fields: Sequence[str]) -> dict[str, Any]:
    """The members named by `--fields`, nested as in the item."""
    result: dict[str, Any] = {}
    for path in fields:
        walked, value = pick(item, path)
        if missing(value):
            continue
        target = result
        for key in walked[:-1]:
            target = cast("dict[str, Any]", target.setdefault(key, {}))
        target[walked[-1]] = value
    return result


def flatten(
    item: Mapping[str, Any],
    prefix: str = "",
    *,
    atom: Callable[[Mapping[str, Any]], bool] | None = None,
) -> dict[str, Any]:
    """Nested members as `a.b`; lists stay values, and the objects `atom` tells."""
    flat: dict[str, Any] = {}
    for key, value in item.items():
        name = f"{prefix}{key}"
        nested = cast("dict[str, Any]", value) if isinstance(value, dict) else None
        if nested and not (atom and atom(nested)):
            flat.update(flatten(nested, f"{name}.", atom=atom))
        else:
            flat[name] = value
    return flat


def dump_json(data: object) -> str:
    return json.dumps(data, ensure_ascii=False)


def dump_json_list(items: Iterable[object]) -> Iterable[str]:
    """Fragments of a JSON array, one element per line (maquette § 4), to write in a row:
    the list is never held in memory."""
    separator = "["
    for item in items:
        yield f"{separator}\n  {dump_json(item)}"
        separator = ","
    yield "[]\n" if separator == "[" else "\n]\n"


def dump_yaml(data: object) -> str:
    import yaml  # noqa: PLC0415 - only for -o yaml

    return yaml.safe_dump(data, allow_unicode=True, sort_keys=False)


def csv_rows(
    items: Iterable[Mapping[str, Any]], columns: Sequence[str], cell: Callable[[object], str]
) -> Iterable[str]:
    """CSV lines: the header, then one line per item (RFC 4180, `\\r\\n`)."""
    buffer = io.StringIO()
    writer = csv.writer(buffer)
    writer.writerow(columns)
    yield buffer.getvalue()
    for item in items:
        buffer.seek(0)
        buffer.truncate()
        writer.writerow([cell(pick(item, column)[1]) for column in columns])
        yield buffer.getvalue()
