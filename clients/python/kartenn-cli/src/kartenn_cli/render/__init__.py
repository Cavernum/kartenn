"""Outputs of the CLI (REQ-023-25, 26; design § Sorties).

`table` by default on a terminal, `json` otherwise; `ndjson`, `yaml`, `csv` on demand.
`--query` (JMESPath) projects the JSON before any rendering; `--fields` picks members.
A list is written as it is read: `ndjson`, `json` and `csv` never hold it in memory.
"""

from __future__ import annotations

import itertools
from dataclasses import dataclass, field
from datetime import datetime, tzinfo
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.render.data import (
    csv_rows,
    dump_json,
    dump_json_list,
    dump_yaml,
    flatten,
    project,
)

if TYPE_CHECKING:
    from collections.abc import Callable, Iterable, Iterator, Mapping

__all__ = ["Output", "format_moment"]

_PEEK = 20


def format_moment(text: str | None, zone: tzinfo | None) -> str | None:
    """An RFC 3339 date of the server in the time zone of the profile, to the second."""
    if not text:
        return None
    try:
        moment = datetime.fromisoformat(text)
    except ValueError:
        return text
    if moment.tzinfo is None:
        return text
    local = moment.astimezone(zone) if zone is not None else moment
    return local.isoformat(sep=" ", timespec="seconds")


def _objects(value: object) -> list[Mapping[str, Any]] | None:
    """A list of JSON objects, else `None`."""
    if not isinstance(value, list):
        return None
    found = cast("list[object]", value)
    if not all(isinstance(item, dict) for item in found):
        return None
    return cast("list[Mapping[str, Any]]", found)


def _object(value: object) -> Mapping[str, Any] | None:
    return cast("Mapping[str, Any]", value) if isinstance(value, dict) else None


@dataclass
class Output:
    """How to write what a command reads."""

    format: str = "json"
    fields: list[str] | None = None
    query: str | None = None
    keys: bool = False
    ids: bool = False
    color: bool = False
    width: int | None = None
    zone: tzinfo | None = None
    columns: tuple[str, ...] = field(default_factory=tuple[str, ...])

    @property
    def table(self) -> bool:
        return self.format == "table"

    def _write(self, text: str) -> None:
        click.echo(text, nl=False, color=self.color)

    def _query(self, data: object) -> object:  # a JMESPath result
        import jmespath  # noqa: PLC0415 - only for --query

        return jmespath.search(cast("str", self.query), data)

    def _projected(self, items: Iterable[Mapping[str, Any]]) -> Iterator[Mapping[str, Any]]:
        if not self.fields:
            yield from items
            return
        for item in items:
            yield project(item, self.fields)

    def items(
        self,
        items: Iterable[Mapping[str, Any]],
        *,
        footer: Callable[[int], str | None] | None = None,
    ) -> int:
        """Write a list; its count. `footer` gives the line under a table."""
        if self.query:
            found = list(items)
            self.value(found)
            return len(found)
        count = 0

        def counted(source: Iterable[Mapping[str, Any]]) -> Iterator[Mapping[str, Any]]:
            nonlocal count
            for item in source:
                count += 1
                yield item

        self._write_items(counted(items))
        if self.table and footer is not None:
            line = footer(count)
            if line:
                self._write(line + "\n")
        return count

    def _write_items(self, source: Iterator[Mapping[str, Any]]) -> None:
        if self.format == "ndjson":
            for item in self._projected(source):
                self._write(dump_json(item) + "\n")
        elif self.format == "json":
            for fragment in dump_json_list(self._projected(source)):
                self._write(fragment)
        elif self.format == "yaml":
            self._write(dump_yaml(list(self._projected(source))))
        elif self.format == "csv":
            from kartenn_cli.render.table import csv_cell  # noqa: PLC0415

            head = list(itertools.islice(source, _PEEK))
            columns = self.fields or self._columns(head)
            for line in csv_rows(itertools.chain(head, source), columns, csv_cell):
                self._write(line)
        else:
            self._table(list(source))

    def _columns(self, items: list[Mapping[str, Any]]) -> list[str]:
        from kartenn_cli.render.table import default_columns  # noqa: PLC0415

        return default_columns(items, self.columns, ids=self.ids)

    def _table(self, rows: list[Mapping[str, Any]]) -> None:
        from kartenn_cli.render.table import render_table  # noqa: PLC0415

        columns = self.fields or self._columns(rows)
        if columns:
            self._write(
                render_table(rows, columns, keys=self.keys, color=self.color, width=self.width)
            )

    def value(self, data: object) -> None:
        """Write an object, or any JSON value."""
        if self.query:
            data = self._query(data)
            self.query = None
        rows = _objects(data)
        if rows is not None:
            self.items(rows)
            return
        found = _object(data)
        if found is not None and self.fields:
            found = data = project(found, self.fields)
        if self.format in {"json", "ndjson"}:
            self._write(dump_json(data) + "\n")
        elif self.format == "yaml":
            self._write(dump_yaml(data))
        elif found is not None:
            if self.format == "csv":
                self.items([found])
            else:
                self.record(found)
        else:
            self._write(f"{'' if data is None else data}\n")

    def record(self, data: Mapping[str, Any]) -> None:
        """An object in a table of two columns: member, value."""
        from kartenn_cli.render.table import atom, cell  # noqa: PLC0415

        shown = {k: v for k, v in data.items() if k != "links" and (self.ids or k != "id")}
        rows = flatten(shown, atom=atom)
        width = max((len(k) for k in rows), default=0)
        for key, value in rows.items():
            self._write(f"{key.ljust(width)}  {cell(value, keys=self.keys)}\n")
