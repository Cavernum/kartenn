"""The table (REQ-023-25, 26; T15; design § Sorties), rendered by rich, imported here only.

- columns: `--fields`, else `x-kartenn.columns`, else the scalar members of the first
  level and the references, but `id`, `href` and `links`;
- a reference `{id, href, slug?, label?}` shows its label, else its key; `--keys` forces
  the key; the identifiers show only with `--ids`;
- a masked value shows a placeholder, never its content.
"""

from __future__ import annotations

import io
from typing import TYPE_CHECKING, Any, cast

from kartenn_cli.i18n import t
from kartenn_cli.render.data import dump_json, missing, pick

if TYPE_CHECKING:
    from collections.abc import Callable, Mapping, Sequence

__all__ = ["atom", "cell", "csv_cell", "default_columns", "render_table"]

_HIDDEN = frozenset({"id", "href", "links"})
NONE = "—"


def _reference(value: Mapping[str, Any]) -> bool:
    return "id" in value and ("href" in value or "slug" in value or len(value) == 1)


def _masked(value: Mapping[str, Any]) -> bool:
    return value.get("masked") is True


def _masked_value(value: object) -> bool:
    return isinstance(value, dict) and _masked(cast("dict[str, Any]", value))


def atom(value: Mapping[str, Any]) -> bool:
    """A reference or a masked value: shown as one cell."""
    return _reference(value) or value.get("masked") is True


def _shown(value: object) -> bool:
    if isinstance(value, dict):
        data = cast("dict[str, Any]", value)
        return _reference(data) or data.get("masked") is True
    return not isinstance(value, list)


def default_columns(
    items: Sequence[Mapping[str, Any]], declared: Sequence[str] = (), *, ids: bool = False
) -> list[str]:
    columns: list[str] = list(declared)
    if not columns:
        for item in items[:20]:
            for key, value in item.items():
                if key not in _HIDDEN and key not in columns and _shown(value):
                    columns.append(key)
    if ids and "id" not in columns:
        columns.insert(0, "id")
    return columns


def _object_cell(data: Mapping[str, Any], *, keys: bool) -> str:
    if data.get("masked") is True:
        return t("render.masked")
    if not _reference(data):
        return dump_json(data)
    label = None if keys else data.get("label")
    shown = label or data.get("slug") or data.get("key") or data.get("id")
    return NONE if shown is None else str(shown)


def cell(
    value: object,
    *,
    keys: bool = False,
    label: Callable[[object], str | None] | None = None,
) -> str:
    """The text of a value in a cell; `label` gives the label of a key (spec 018)."""
    if value is None or missing(value):
        return NONE
    if isinstance(value, bool):
        return "true" if value else "false"
    if not keys and label is not None and not isinstance(value, list):
        found = label(value)
        if found is not None:
            return found
    if isinstance(value, dict):
        return _object_cell(cast("dict[str, Any]", value), keys=keys)
    if isinstance(value, list):
        items = cast("list[object]", value)
        return ", ".join(cell(v, keys=keys, label=label) for v in items) or NONE
    return str(value)


def csv_cell(value: object) -> str:
    """A cell of CSV: keys always, empty for no value, never a masked value."""
    if value is None or missing(value) or _masked_value(value):
        return ""
    return "" if (found := cell(value, keys=True)) == NONE else found


def render_table(  # noqa: PLR0913 - the options of a table
    items: Sequence[Mapping[str, Any]],
    columns: Sequence[str],
    *,
    keys: bool = False,
    color: bool = False,
    width: int | None = None,
    labels: Callable[[str, object], str | None] | None = None,
) -> str:
    """The table as text, without trailing spaces; `labels` gives the label of a value of
    a column (a closed-list value, a table of labels), the key showing otherwise."""
    from rich.console import Console  # noqa: PLC0415 - rich only for tables
    from rich.table import Table  # noqa: PLC0415
    from rich.text import Text  # noqa: PLC0415

    def of(column: str) -> Callable[[object], str | None] | None:
        resolve = labels
        if resolve is None:
            return None
        return lambda value: resolve(column, value)

    rows = [
        [cell(pick(item, column)[1], keys=keys, label=of(column)) for column in columns]
        for item in items
    ]
    table = Table(
        box=None,
        show_edge=False,
        pad_edge=False,
        padding=(0, 1),
        header_style="bold" if color else "",
    )
    for column in columns:
        table.add_column(Text(column.upper()), no_wrap=width is None, overflow="fold")
    for row in rows:
        table.add_row(*(Text(value) for value in row))
    natural = sum(
        max([len(column), *(len(row[index]) for row in rows)]) + 2
        for index, column in enumerate(columns)
    )
    buffer = io.StringIO()
    console = Console(
        file=buffer,
        width=width or max(natural + 2, 40),
        color_system="standard" if color else None,
        force_terminal=color,
        highlight=False,
        emoji=False,
        markup=False,
    )
    console.print(table)
    return "\n".join(line.rstrip() for line in buffer.getvalue().splitlines()) + "\n"
