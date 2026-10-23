"""`kartenn import` (REQ-023-32; T18; maquette § 10).

- an NDJSON file of `ImportElement`, as it is;
- a CSV or a JSON file of objects of **one** resource (`--resource prefixes`): each row a
  creation, its columns the members (typed by the schema of the contract), `prop.<key>`
  its properties, `--id-column` its identifier in the source;
- read as it comes, sent by chunks, resumed with `--resume` after checking the digest of
  the file, followed to its end; a refusal of the sort names the line of the source.

Progress: rich on a terminal, one line per step elsewhere.
"""

from __future__ import annotations

import csv
import json
import sys
import time
from pathlib import Path
from typing import TYPE_CHECKING, Any, Self, cast

import click

from kartenn_cli.base import command
from kartenn_cli.errors import CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from collections.abc import Callable, Iterator

    from kartenn import ImportProgress
    from kartenn.catalog import Catalog, Resource
    from kartenn_cli.runtime import Runtime

__all__ = ["import_command", "resource_elements"]

PROPERTY = "prop."


def _typed(value: str, kind: tuple[str, tuple[str, ...], str] | None) -> object:
    if kind is None or kind[0] != "scalar":
        return value
    scalar = kind[1][0]
    if scalar == "integer":
        return int(value)
    if scalar == "number":
        return float(value)
    if scalar == "boolean":
        return value.strip().lower() in {"true", "1", "yes", "oui"}
    return value


def _rows(path: Path) -> Iterator[tuple[int, dict[str, Any]]]:
    """The rows of a CSV (line of the file, header on line 1) or of a JSON array."""
    if path.suffix.lower() == ".json":
        data = json.loads(path.read_text("utf-8"))
        items = cast("list[object]", data) if isinstance(data, list) else []
        for index, item in enumerate(items, start=1):
            if isinstance(item, dict):
                yield index, cast("dict[str, Any]", item)
        return
    with path.open(encoding="utf-8", newline="") as file:
        reader = csv.DictReader(file)
        for row in reader:
            yield reader.line_num, {k: v for k, v in row.items() if k is not None}


def _row(
    row: dict[str, Any],
    members: dict[str, tuple[str, tuple[str, ...], str]],
    id_column: str | None,
    reference: Callable[[str, str], dict[str, str]],
) -> tuple[dict[str, Any], str | None]:
    """The members of a row and its identifier in the source."""
    body: dict[str, Any] = {}
    properties: dict[str, Any] = {}
    external = None
    for column, raw in row.items():
        if raw in (None, ""):
            continue
        kind = members.get(column)
        if column == id_column:
            external = str(raw)
        elif column.startswith(PROPERTY):
            properties[column.removeprefix(PROPERTY)] = raw
        elif kind is not None and kind[0] == "reference" and isinstance(raw, str):
            body[column] = reference(column, raw)
        else:
            body[column] = _typed(raw, kind) if isinstance(raw, str) else raw
    if properties:
        body["properties"] = properties
    return body, external


def resource_elements(
    found: Runtime,
    catalog: Catalog,
    resource: Resource,
    path: Path,
    *,
    id_column: str | None,
) -> Iterator[tuple[int, dict[str, Any]]]:
    """`ImportElement`s of the rows of a file of one resource, with their line."""
    from kartenn_cli.complete import reference_of  # noqa: PLC0415
    from kartenn_cli.reads import resolve_ref  # noqa: PLC0415
    from kartenn_cli.writes import body_members, changeset_operation  # noqa: PLC0415

    create = resource.verb("create")
    if create is None:
        raise CliError(
            "client.operation-unavailable", t("import.no_create", resource=resource.name)
        )
    operation = catalog.operation(create.operation_id)
    members = body_members(catalog, operation.body) if operation.body else {}
    resolved: dict[tuple[str, str], str] = {}

    def reference(column: str, text: str) -> dict[str, str]:
        collection = reference_of(column, catalog) or column
        key = (collection, text)
        if key not in resolved:
            target = catalog.resource(collection)
            resolved[key] = resolve_ref(found.client, catalog, target, text)
        return {"id": resolved[key]}

    for line, row in _rows(path):
        body, external = _row(row, members, id_column, reference)
        element: dict[str, Any] = {
            "operation": changeset_operation(catalog, resource, "create", create.operation_id, body)
        }
        if external is not None:
            element["external_id"] = external
        yield line, element


class _Progress:
    """What an import says while it runs: rich on a terminal, lines elsewhere."""

    def __init__(self, *, rich: bool) -> None:
        self.rich = rich
        self.started = time.monotonic()
        self._bar: Any = None
        self._task: Any = None
        self.import_id = "?"

    def __enter__(self) -> Self:
        if self.rich:
            from rich.console import Console  # noqa: PLC0415
            from rich.progress import Progress  # noqa: PLC0415

            self._bar = Progress(console=Console(stderr=True), transient=True)
            self._bar.start()
            self._task = self._bar.add_task(t("import.sending"), total=None)
        return self

    def __exit__(self, *_: object) -> None:
        if self._bar is not None:
            self._bar.stop()

    def line(self, text: str) -> None:
        if self._bar is None:
            click.echo(text, err=True)
        else:
            self._bar.console.print(text, markup=False, highlight=False)

    def chunk(self, progress: ImportProgress) -> None:
        self.import_id = progress.import_id
        if self._bar is not None:
            description = t("import.chunks", chunks=progress.chunks, elements=progress.elements)
            self._bar.update(self._task, description=description)


@command(name="import", help="import.help")
@click.argument("file", type=click.Path(exists=True, dir_okay=False, path_type=Path))
@click.option("--resource", help="import.opt.resource")
@click.option("--id-column", help="import.opt.id_column")
@click.option("--source", help="import.opt.source")
@click.option("--resume", "resume", metavar="ID", help="import.opt.resume")
@click.option("--chunk", type=click.IntRange(1), metavar="N", help="import.opt.chunk")
@click.option("--follow/--no-follow", default=True, help="import.opt.follow")
@click.pass_context
def import_command(  # noqa: PLR0913, PLR0917 - the options of an import
    ctx: click.Context,
    file: Path,
    resource: str | None,
    id_column: str | None,
    source: str | None,
    resume: str | None,
    chunk: int | None,
    *,
    follow: bool,
) -> None:
    from kartenn import ImportRefused, file_digest, ndjson_elements  # noqa: PLC0415
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    found = runtime(ctx)
    client = found.client
    catalog = client.catalog
    if resource is None and file.suffix.lower() != ".ndjson":
        raise CliError("client.resource-required", t("import.resource_required", file=file.name))
    elements: Callable[[], Iterator[tuple[int, Any]]]
    if resource is None:
        elements = lambda: ndjson_elements(file)  # noqa: E731
    else:
        model = catalog.resource(resource)
        if model.verb("create") is None:
            raise CliError("client.operation-unavailable", t("import.no_create", resource=resource))
        elements = lambda: resource_elements(found, catalog, model, file, id_column=id_column)  # noqa: E731
    with _Progress(rich=sys.stderr.isatty()) as progress:
        session = client.import_session(
            source or file.name,
            digest=file_digest(file),
            chunk_elements=chunk,
            on_progress=progress.chunk,
        )
        import_id = session.resume(elements()) if resume else session.run(elements())
        if resume and resume != import_id:
            progress.line(t("import.other", given=resume, found=import_id))
        progress.line(t("import.uploaded", id=import_id))
        if not follow:
            return
        progress.line(t("import.sorting", id=import_id))
        try:
            record = session.wait()
        except ImportRefused as refused:
            refused.context = {"import": import_id}  # type: ignore[attr-defined]
            refused.sources = tuple(  # type: ignore[attr-defined]
                f"{file.name}:{line}" if line is not None else None for line in refused.lines
            )
            refused.lines_after = (  # type: ignore[attr-defined]
                t("import.back", id=import_id, file=file.name),
            )
            raise
    elapsed = time.monotonic() - progress.started
    state = record.get("state")
    click.echo(
        t(
            "import.final",
            id=import_id,
            state=state,
            elements=record.get("elements"),
            elapsed=f"{elapsed:.0f}",
        )
    )
