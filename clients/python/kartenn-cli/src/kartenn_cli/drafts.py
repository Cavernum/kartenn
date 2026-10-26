"""`kartenn drafts` (spec 024, REQ-024-1 to 7): the drafts of the caller kept by the server,
listed, read, submitted or deleted. Composing one stays with the interface and the
library (`kn.drafts`).
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import click

from kartenn_cli.base import TGroup, group
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from kartenn_cli.runtime import Runtime

__all__ = ["drafts_group"]

_OUTPUT = click.Choice(("table", "json", "yaml"))


def _found(ctx: click.Context) -> Runtime:
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    return runtime(ctx)


@group(name="drafts", cls=TGroup, help="drafts.help")
def drafts_group() -> None:
    pass


@drafts_group.command(name="list", help="drafts.list.help")
@click.option("-o", "--output", type=_OUTPUT, metavar="FORMAT", help="opt.output")
@click.pass_context
def list_drafts(ctx: click.Context, output: str | None) -> None:
    found = _found(ctx)
    found.output(output=output).value([dict(d) for d in found.client.drafts.list()])


@drafts_group.command(name="show", help="drafts.show.help")
@click.argument("draft")
@click.option("-o", "--output", type=_OUTPUT, metavar="FORMAT", help="opt.output")
@click.pass_context
def show(ctx: click.Context, draft: str, output: str | None) -> None:
    found = _found(ctx)
    found.output(output=output).value(dict(found.client.drafts.get(draft)))


@drafts_group.command(name="submit", help="drafts.submit.help")
@click.argument("draft")
@click.option("--require-applied", is_flag=True, help="opt.require_applied")
@click.option("--submit-only", is_flag=True, help="opt.submit_only")
@click.option("-o", "--output", type=_OUTPUT, metavar="FORMAT", help="opt.output")
@click.pass_context
def submit(
    ctx: click.Context,
    draft: str,
    output: str | None,
    *,
    require_applied: bool,
    submit_only: bool,
) -> None:
    from kartenn_cli.writes import render_outcome  # noqa: PLC0415

    found = _found(ctx)
    # Spec 024 (REQ-024-13): applied by its author who may, unless only submitted.
    result = found.client.drafts.submit(draft, approval_self=not submit_only)
    render_outcome(found, result, output=output, require_applied=require_applied)


@drafts_group.command(name="delete", help="drafts.delete.help")
@click.argument("draft")
@click.pass_context
def delete(ctx: click.Context, draft: str) -> None:
    _found(ctx).client.drafts.delete(draft)
    click.echo(t("drafts.deleted", id=draft))
