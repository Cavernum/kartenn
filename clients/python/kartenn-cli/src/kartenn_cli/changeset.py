"""`kartenn changeset` (REQ-023-31; T17; maquette § 8): compose a changeset in a local
file, simulate it, apply it; read, follow and decide on those of the server.

The local changeset is a YAML file (`./.kartenn-changeset.yaml`, or `KARTENN_CHANGESET`)
that keeps the motive, the dates, the operations and the `Idempotency-Key` of its
submission: applying it again after a cut is recognised by the server.
"""

from __future__ import annotations

import os
import uuid
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import Option, TGroup, group
from kartenn_cli.errors import CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from collections.abc import Mapping

    from kartenn_cli.runtime import Runtime

__all__ = ["add_operation", "changeset_group", "local_path"]

DEFAULT = ".kartenn-changeset.yaml"


def local_path() -> Path:
    return Path(os.environ.get("KARTENN_CHANGESET") or DEFAULT)


def _read() -> dict[str, Any]:
    import yaml  # noqa: PLC0415

    path = local_path()
    try:
        data = yaml.safe_load(path.read_text("utf-8"))
    except FileNotFoundError as error:
        raise CliError("client.no-changeset", t("changeset.none", path=path)) from error
    if not isinstance(data, dict) or not isinstance(
        cast("dict[str, Any]", data).get("operations"), list
    ):
        raise CliError("client.invalid-changeset", t("changeset.invalid", path=path))
    return cast("dict[str, Any]", data)


def _write(data: Mapping[str, Any]) -> Path:
    import yaml  # noqa: PLC0415

    path = local_path()
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(yaml.safe_dump(dict(data), allow_unicode=True, sort_keys=False), "utf-8")
    temporary.chmod(0o600)
    temporary.replace(path)
    return path


def add_operation(operation: Mapping[str, Any], *, summary: str = "") -> int:
    """Append an operation to the local changeset; its index."""
    if not isinstance(operation.get("op"), str):
        raise CliError("client.unknown-operation", t("changeset.no_op"))
    data = _read()
    operations = cast("list[object]", data["operations"])
    operations.append(dict(operation))
    _write(data)
    index = len(operations) - 1
    click.echo(t("changeset.added", index=index, op=operation["op"], summary=summary).rstrip())
    return index


def _found(ctx: click.Context) -> Runtime:
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    return runtime(ctx)


class ChangesetGroup(TGroup):
    """The local and remote changesets; `list` and `proposals` are the lists of the
    catalog."""

    def list_commands(self, ctx: click.Context) -> list[str]:
        return sorted({*super().list_commands(ctx), "list", "proposals"})

    def get_command(self, ctx: click.Context, cmd_name: str) -> click.Command | None:
        if cmd_name in {"list", "proposals"}:
            from kartenn_cli.reads import ListCommand  # noqa: PLC0415

            tree = _found(ctx).tree(cached=ctx.resilient_parsing)
            collection = "changesets" if cmd_name == "list" else "proposals"
            if tree is None or not tree[0].has_resource(collection):
                return None
            catalog = tree[0]
            resource = catalog.resource(collection)
            verb = resource.verb("list")
            if verb is None:
                return None
            return ListCommand(
                cmd_name,
                resource=resource,
                operation=catalog.operation(verb.operation_id),
                catalog=catalog,
            )
        return super().get_command(ctx, cmd_name)


@group(name="changeset", cls=ChangesetGroup, help="changeset.help")
def changeset_group() -> None:
    pass


@changeset_group.command(name="new", help="changeset.new.help")
@click.option("--motive", required=True, help="opt.motive")
@click.option("--valid-from", metavar="DATE", help="opt.valid_from")
@click.option("--valid-to", metavar="DATE", help="opt.valid_to")
@click.option("--on-behalf-of", metavar="ID", help="opt.on_behalf_of")
@click.option("--force", is_flag=True, help="changeset.opt.force")
def new(
    motive: str,
    valid_from: str | None,
    valid_to: str | None,
    on_behalf_of: str | None,
    *,
    force: bool,
) -> None:
    path = local_path()
    if path.exists() and not force:
        raise CliError("client.changeset-exists", t("changeset.exists", path=path))
    data: dict[str, str | list[object] | None] = {
        "motive": motive,
        "valid_from": valid_from,
        "valid_to": valid_to,
        "on_behalf_of": on_behalf_of,
        "idempotency_key": str(uuid.uuid4()),
        "operations": [],
    }
    shown = _write({k: v for k, v in data.items() if v is not None})
    click.echo(t("changeset.created", path=f"./{shown}" if not shown.is_absolute() else shown))


@changeset_group.command(name="add", help="changeset.add.help")
@click.argument("operation", required=False)
@click.option("--from-file", cls=Option, metavar="metavar.file", help="opt.from_file")
def add(operation: str | None, from_file: str | None) -> None:
    """An operation in JSON (or YAML), given or read from a file."""
    import yaml  # noqa: PLC0415

    from kartenn_cli.writes import read_document  # noqa: PLC0415

    if operation is None and from_file is None:
        raise CliError("client.no-operation", t("changeset.no_op"))
    text = operation if operation is not None else read_document(str(from_file))
    data = yaml.safe_load(text)
    items = cast("list[object]", data) if isinstance(data, list) else [data]
    for item in items:
        if not isinstance(item, dict):
            raise CliError("client.unknown-operation", t("changeset.no_op"))
        add_operation(cast("dict[str, Any]", item))


def _builder(found: Runtime, data: Mapping[str, Any]) -> Any:  # noqa: ANN401 - a ChangesetBuilder
    from datetime import UTC, datetime  # noqa: PLC0415

    from kartenn_cli.reads import resolve_date_text  # noqa: PLC0415

    now = datetime.now(UTC).replace(microsecond=0)
    builder = found.client.changeset(
        motive=str(data["motive"]),
        valid_from=resolve_date_text(data.get("valid_from"), found, now),
        valid_to=resolve_date_text(data.get("valid_to"), found, now),
        on_behalf_of=data.get("on_behalf_of"),
        idempotency_key=data.get("idempotency_key"),
    )
    for operation in cast("list[dict[str, Any]]", data["operations"]):
        builder.add(operation)
    return builder


@changeset_group.command(name="simulate", help="changeset.simulate.help")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "yaml")),
    metavar="FORMAT",
    help="opt.output",
)
@click.pass_context
def simulate_local(ctx: click.Context, output: str | None) -> None:
    if output:
        _found(ctx).settings.output = output
    from kartenn_cli.writes import simulate  # noqa: PLC0415

    found = _found(ctx)
    data = _read()
    from datetime import UTC, datetime  # noqa: PLC0415

    from kartenn_cli.reads import resolve_date_text  # noqa: PLC0415

    now = datetime.now(UTC).replace(microsecond=0)
    simulate(
        found,
        cast("list[dict[str, Any]]", data["operations"]),
        motive=str(data["motive"]),
        valid_from=resolve_date_text(data.get("valid_from"), found, now),
        valid_to=resolve_date_text(data.get("valid_to"), found, now),
        on_behalf_of=data.get("on_behalf_of"),
    )


@changeset_group.command(name="apply", help="changeset.apply.help")
@click.option("--require-applied", is_flag=True, help="opt.require_applied")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "yaml")),
    metavar="FORMAT",
    help="opt.output",
)
@click.pass_context
def apply(ctx: click.Context, output: str | None, *, require_applied: bool) -> None:
    from kartenn_cli.writes import render_outcome  # noqa: PLC0415

    found = _found(ctx)
    data = _read()
    result = _builder(found, data).submit()
    local_path().unlink(missing_ok=True)
    render_outcome(found, result, output=output, require_applied=require_applied)


def _show(found: Runtime, record: Mapping[str, Any], output: str | None) -> None:
    from kartenn_cli.render.changesets import render_counts  # noqa: PLC0415

    if (found.asked_output(output) or "table") != "table":
        found.output(output=output).value(dict(record))
        return
    click.echo(
        t(
            "changeset.state",
            id=record.get("id"),
            state=record.get("state"),
            kind=record.get("kind") or "—",
        )
    )
    click.echo(f"  {t('changeset.motive')}  {record.get('motive') or '—'}")
    for line in render_counts(cast("list[dict[str, Any]]", record.get("diff") or [])):
        click.echo(line)


@changeset_group.command(name="show", help="changeset.show.help")
@click.argument("changeset")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "yaml")),
    metavar="FORMAT",
    help="opt.output",
)
@click.option("--follow", is_flag=True, help="changeset.opt.follow")
@click.pass_context
def show(ctx: click.Context, changeset: str, output: str | None, *, follow: bool) -> None:
    found = _found(ctx)
    record = found.client.changeset_of(changeset).read()
    _show(found, record, output)
    if follow and record.get("state") == "pending":
        from kartenn_cli.watch import follow_changeset  # noqa: PLC0415

        click.echo(t("changeset.final", id=changeset, state=follow_changeset(found, changeset)))


@changeset_group.command(name="diff", help="changeset.diff.help")
@click.argument("changeset")
@click.option("--section", help="changeset.opt.section")
@click.option("--property", "prop", help="changeset.opt.property")
@click.option("--all", "every", is_flag=True, help="opt.all")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "ndjson", "yaml", "csv")),
    metavar="FORMAT",
    help="opt.output",
)
@click.pass_context
def diff(  # noqa: PLR0913 - the options of a differential
    ctx: click.Context,
    changeset: str,
    section: str | None,
    prop: str | None,
    output: str | None,
    *,
    every: bool,
) -> None:
    from kartenn_cli.reads import Dates, write_pages  # noqa: PLC0415

    found = _found(ctx)
    listing = found.client.changeset_of(changeset).diff(section=section, prop=prop)
    write_pages(
        found.output(output=output),
        listing.pages(),
        every=every,
        dates=Dates(None, None, frozen=False),
        zone=found.zone,
    )


def _decided(found: Runtime, result: object, output: str | None) -> None:
    from kartenn_cli.writes import render_outcome  # noqa: PLC0415

    render_outcome(found, result, output=output, require_applied=False)


@changeset_group.command(name="approve", help="changeset.approve.help")
@click.argument("changeset")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "yaml")),
    metavar="FORMAT",
    help="opt.output",
)
@click.pass_context
def approve(ctx: click.Context, changeset: str, output: str | None) -> None:
    found = _found(ctx)
    _decided(found, found.client.changeset_of(changeset).approve(), output)


@changeset_group.command(name="reject", help="changeset.reject.help")
@click.argument("changeset")
@click.option("--motive", help="opt.motive")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "yaml")),
    metavar="FORMAT",
    help="opt.output",
)
@click.pass_context
def reject(ctx: click.Context, changeset: str, motive: str | None, output: str | None) -> None:
    from kartenn_cli.writes import motive_of  # noqa: PLC0415

    found = _found(ctx)
    del motive
    reason = str(motive_of(ctx, found))
    _decided(found, found.client.changeset_of(changeset).reject(reason), output)


@changeset_group.command(name="withdraw", help="changeset.withdraw.help")
@click.argument("changeset")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "yaml")),
    metavar="FORMAT",
    help="opt.output",
)
@click.pass_context
def withdraw(ctx: click.Context, changeset: str, output: str | None) -> None:
    found = _found(ctx)
    _decided(found, found.client.changeset_of(changeset).withdraw(), output)


@changeset_group.command(name="undo", help="changeset.undo.help")
@click.argument("changeset")
@click.option("--dry-run", is_flag=True, help="opt.dry_run")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "yaml")),
    metavar="FORMAT",
    help="opt.output",
)
@click.pass_context
def undo(ctx: click.Context, changeset: str, output: str | None, *, dry_run: bool) -> None:
    found = _found(ctx)
    result = found.client.changeset_of(changeset).undo(simulate=dry_run)
    _decided(found, result, output)
