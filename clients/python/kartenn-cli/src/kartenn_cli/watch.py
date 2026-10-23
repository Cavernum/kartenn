"""`kartenn watch` and the following of a changeset (REQ-023-38; T22; maquette § 15).

`GET /v1/events` (022), through the library: the client sets `Last-Event-ID` on resumption,
sets aside duplicates and hands `reset` over. A line per event in a table (time, kind,
changeset, objects), an object per line in `ndjson`. Ctrl-C closes the stream and says how
to resume (`--since`). Without the stream (`capabilities.events` null) the command answers
`client.operation-unavailable`.
"""

from __future__ import annotations

from datetime import datetime
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import command
from kartenn_cli.errors import USAGE, CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from kartenn._core.sse import Event
    from kartenn_cli.runtime import Runtime

__all__ = ["event_line", "follow_changeset", "watch_command"]

FINAL = frozenset({"applied", "rejected", "withdrawn", "expired", "undone"})


def _split(text: str | None) -> list[str] | None:
    return [part for part in (text or "").split(",") if part] or None


def _objects(data: dict[str, Any]) -> str:
    shown: list[str] = []
    for raw in cast("list[object]", data.get("objects") or []):
        if isinstance(raw, dict):
            item = cast("dict[str, Any]", raw)
            shown.append(str(item.get("key") or item.get("slug") or item.get("id")))
    wide = cast("list[object]", data.get("invalidated") or data.get("types") or [])
    shown.extend(t("watch.wide", what=raw) for raw in wide)
    return " ; ".join(shown)


def event_line(event: Event, zone: Any) -> str:  # noqa: ANN401 - a tzinfo
    """The line of an event in a table."""
    clock = datetime.now(zone).strftime("%H:%M:%S")
    data = event.data
    if event.kind == "ready":
        return f"{clock}  {t('watch.ready')}"
    if event.kind == "reset":
        return f"{clock}  reset      {t('watch.reset')}"
    changeset = data.get("changeset")
    if event.kind == "changeset":
        return f"{clock}  changeset  {changeset}  {data.get('state', '?')}"
    return f"{clock}  {event.kind:<9}  changeset {changeset}  {_objects(data)}".rstrip()


def require_events(found: Runtime) -> None:
    if found.client.catalog.events is None:
        raise CliError("client.operation-unavailable", t("watch.absent"), exit_code=USAGE)


@command(name="watch", help="watch.help")
@click.option("--types", help="watch.opt.types")
@click.option("--domains", help="watch.opt.domains")
@click.option("--objects", help="watch.opt.objects")
@click.option("--kinds", help="watch.opt.kinds")
@click.option("--since", metavar="ID", help="watch.opt.since")
@click.option(
    "-o", "--output", type=click.Choice(("table", "ndjson")), metavar="FORMAT", help="opt.output"
)
@click.pass_context
def watch_command(  # noqa: PLR0913, PLR0917 - the filters of the 022
    ctx: click.Context,
    types: str | None,
    domains: str | None,
    objects: str | None,
    kinds: str | None,
    since: str | None,
    output: str | None,
) -> None:
    import json  # noqa: PLC0415

    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    found = runtime(ctx)
    require_events(found)
    table = (found.asked_output(output) or "table") == "table"
    profile = found.require_profile()
    if table:
        click.echo(t("watch.open", profile=profile.name), err=True)
    last = since
    try:
        for event in found.client.events(
            types=_split(types),
            domains=_split(domains),
            objects=_split(objects),
            kinds=_split(kinds),
            last_event_id=since,
        ):
            last = event.id or last
            if table:
                click.echo(event_line(event, found.zone))
            else:
                line = {"kind": event.kind, "id": event.id, "data": event.data}
                click.echo(json.dumps(line, ensure_ascii=False))
    except KeyboardInterrupt:
        filters = " ".join(
            f"--{name} {value}"
            for name, value in (
                ("types", types),
                ("domains", domains),
                ("objects", objects),
                ("kinds", kinds),
            )
            if value
        )
        resume = f"kartenn watch --since {last} {filters}".rstrip() if last else "kartenn watch"
        click.echo(t("watch.closed", resume=resume), err=True)


def follow_changeset(found: Runtime, changeset: str) -> str:
    """Wait for a pending changeset to be decided: on the stream when the server serves
    it, else by reading it again; its final state."""
    if found.client.catalog.events is not None:
        for event in found.client.events(objects=[changeset], kinds=["changeset"]):
            state = str(event.data.get("state", ""))
            if event.kind == "changeset" and state in FINAL:
                return state
    while True:  # pragma: no branch - left on a final state
        state = str(found.client.changeset_of(changeset).read().get("state"))
        if state in FINAL:
            return state
        found.client.sleep(2.0)
