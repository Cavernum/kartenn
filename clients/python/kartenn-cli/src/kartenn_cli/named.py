"""Named commands of the modules (REQ-023-20; design § Commandes nommées; maquette § 6, 7).

A table of the CLI bound to `operationId`s of the contract. An entry whose operations
are not all served is not offered: the module group answers as for a module absent. An
entry may replace derived commands of its group (`check` stands for `verdicts`).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import Option, TCommand, TGroup
from kartenn_cli.errors import DENIED, CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from collections.abc import Callable, Mapping

    from kartenn.catalog import Catalog
    from kartenn_cli.runtime import Runtime

__all__ = ["NAMED", "NamedCommand", "named_commands", "replaced"]


@dataclass(frozen=True)
class NamedCommand:
    """A command of a module group, the operations it needs and what it replaces."""

    module: str
    name: str
    operation_ids: tuple[str, ...]
    build: Callable[[Catalog], click.Command]
    replaces: tuple[str, ...] = field(default=())


def _found(ctx: click.Context) -> Runtime:
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    return runtime(ctx)


def _dates(ctx: click.Context, found: Runtime) -> tuple[str | None, str | None]:
    from kartenn_cli.reads import dates_of  # noqa: PLC0415

    dates = dates_of(ctx, found)
    return dates.valid_at, dates.as_of


def _asked(found: Runtime, output: str | None) -> bool:
    return (found.asked_output(output) or "table") != "table"


def _endpoint(found: Runtime, catalog: Catalog, text: str) -> object:
    """`kind:ref` (`machine:bastion-1`, `prefix:interne/198.51.100.0/24`) as an endpoint of
    a flow; a JSON object as it is."""
    import json  # noqa: PLC0415

    from kartenn_cli.complete import reference_of  # noqa: PLC0415
    from kartenn_cli.reads import resolve_ref  # noqa: PLC0415

    if text.startswith("{"):
        return json.loads(text)
    kind, sep, ref = text.partition(":")
    collection = reference_of(kind, catalog) if sep else None
    if collection is None:
        raise CliError("client.invalid-endpoint", t("named.endpoint", given=text))
    return {
        "kind": kind,
        "id": {"id": resolve_ref(found.client, catalog, catalog.resource(collection), ref)},
    }


# policy check -------------------------------------------------------------------------

_MARKS = {"allowed": "✓", "allowed_pending": "◐", "partial": "◐", "denied": "✗"}


def _verdict_lines(answer: Mapping[str, Any], zone: Any) -> list[str]:  # noqa: ANN401 - tzinfo
    del zone
    synthesis = cast("dict[str, Any]", answer.get("synthesis") or {})
    verdict = str(synthesis.get("verdict") or "—")
    outcome = str(synthesis.get("outcome") or "—")
    head = f"{t('check.verdict')}  {_MARKS.get(verdict, '?')} {t('check.' + verdict)}"
    lines = [f"{head} — {t('check.outcome', outcome=outcome)}"]
    lines.append(
        f"  {t('check.evaluations')}  "
        + t("check.counts", allowed=synthesis.get("allowed", 0), denied=synthesis.get("denied", 0))
    )
    evaluations = cast("list[dict[str, Any]]", answer.get("evaluations") or [])
    if evaluations:
        first = evaluations[0]
        decisive = cast("dict[str, Any]", first.get("decisive") or {})
        shown = decisive.get("rule") or decisive.get("pattern") or decisive.get("kind")
        lines.append(f"  {t('check.rule')}  [{decisive.get('kind', '?')}] {shown}")
        unmet = [
            str(c.get("condition"))
            for c in cast("list[dict[str, Any]]", first.get("conditions") or [])
            if c.get("state") != "met"
        ]
        conditions = ", ".join(unmet) if unmet else t("check.all_met")
        lines.append(f"  {t('check.conditions')}  {conditions}")
        if first.get("service"):
            lines.append(f"  {t('check.service')}  {first['service']}")
    if answer.get("access") == "reduced":
        lines.append("  " + t("check.reduced"))
    if answer.get("hidden"):
        lines.append("  " + t("check.hidden", count=answer["hidden"]))
    return lines


def _check(catalog: Catalog) -> click.Command:
    from kartenn_cli.reads import date_params  # noqa: PLC0415

    def run(ctx: click.Context, source: str, destination: str, **options: Any) -> None:  # noqa: ANN401
        found = _found(ctx)
        valid_at, as_of = _dates(ctx, found)
        params = {
            k: v
            for k, v in {
                "service": options["service"],
                "transport": options["transport"],
                "port": options["port"],
                "flow_type": options["flow_type"],
                "entity": options["entity"],
            }.items()
            if v is not None
        }
        answer = found.client.at(valid_at, as_of).call(
            "show_verdicts", source=source, destination=destination, **params
        )
        data = cast("dict[str, Any]", dict(answer))
        if _asked(found, options["output"]):
            found.output(output=options["output"]).value(data)
        else:
            for line in _verdict_lines(data, found.zone):
                click.echo(line)
        verdict = cast("dict[str, Any]", data.get("synthesis") or {}).get("verdict")
        if options["exit_status"] and verdict == "denied":
            raise click.exceptions.Exit(DENIED)

    from kartenn_cli.complete import reference_completer  # noqa: PLC0415

    services = reference_completer("services") if catalog.has_resource("services") else None
    params: list[click.Parameter] = [
        click.Argument(["source"], metavar="SOURCE"),
        click.Argument(["destination"], metavar="DESTINATION"),
        Option(["--service"], help="check.opt.service", shell_complete=services),
        Option(["--transport"], help="check.opt.transport"),
        Option(["--port"], type=click.INT, help="check.opt.port"),
        Option(["--flow-type"], help="check.opt.flow_type"),
        Option(["--entity"], help="check.opt.entity"),
        Option(["--exit-status"], is_flag=True, help="check.opt.exit_status"),
        *date_params(),
        Option(
            ["-o", "--output"],
            type=click.Choice(("table", "json", "yaml")),
            metavar="FORMAT",
            help="opt.output",
        ),
    ]
    return TCommand("check", params=params, callback=click.pass_context(run), help="check.help")


# policy matrix ------------------------------------------------------------------------

_CELL = {"allow": "✓", "deny": "✗", "conditional": "◐"}


def _matrix(catalog: Catalog) -> click.Command:
    del catalog
    group = TGroup("matrix", help="matrix.help")

    @group.command(name="show", help="matrix.show.help", cls=TCommand)
    @click.option("--flow-type", help="check.opt.flow_type")
    @click.option(
        "-o",
        "--output",
        type=click.Choice(("table", "json", "yaml")),
        metavar="FORMAT",
        help="opt.output",
    )
    @click.pass_context
    def show(ctx: click.Context, flow_type: str | None, output: str | None) -> None:
        found = _found(ctx)
        params: dict[str, Any] = {"flow_type": flow_type} if flow_type else {}
        answer = cast("dict[str, Any]", dict(found.client.call("show_policy_matrix", **params)))
        if _asked(found, output):
            found.output(output=output).value(answer)
            return
        cells = cast("list[dict[str, Any]]", answer.get("cells") or [])
        sources = sorted({str(c["source"]) for c in cells})
        destinations = sorted({str(c["destination"]) for c in cells})
        grid = {(str(c["source"]), str(c["destination"])): str(c["state"]) for c in cells}
        rows = [
            {"source": s, **{d: _CELL.get(grid.get((s, d), "deny"), "?") for d in destinations}}
            for s in sources
        ]
        click.echo(t("matrix.title", flow_type=answer.get("flow_type")))
        from kartenn_cli.render.table import render_table  # noqa: PLC0415

        if rows:
            click.echo(render_table(rows, ["source", *destinations]), nl=False)
        if answer.get("outside"):
            click.echo(t("matrix.outside", count=answer["outside"]))

    @group.command(name="set", help="matrix.set.help", cls=TCommand)
    @click.argument("source")
    @click.argument("flow_type", metavar="FLOW-TYPE")
    @click.argument("destination")
    @click.argument("value")
    @click.option("--motive", help="opt.motive")
    @click.option("--require-applied", is_flag=True, help="opt.require_applied")
    @click.option(
        "-o",
        "--output",
        type=click.Choice(("table", "json", "yaml")),
        metavar="FORMAT",
        help="opt.output",
    )
    @click.pass_context
    def set_cell(  # noqa: PLR0913, PLR0917 - a cell and its write
        ctx: click.Context,
        source: str,
        flow_type: str,
        destination: str,
        value: str,
        motive: str | None,
        output: str | None,
        *,
        require_applied: bool,
    ) -> None:
        from kartenn_cli.writes import motive_of, render_outcome  # noqa: PLC0415

        found = _found(ctx)
        reason = motive_of(ctx, found)
        del motive
        client = found.client
        etag = client.call("show_policy_matrix", flow_type=flow_type).etag
        result = client.call(
            "set_policy_matrix_cell",
            source=source,
            flow_type=flow_type,
            destination=destination,
            body={"state": value, "motive": reason},
            etag=etag,
        )
        render_outcome(found, result, output=output, require_applied=require_applied)

    return group


# policy report ------------------------------------------------------------------------


def _report(catalog: Catalog) -> click.Command:
    del catalog

    @click.command(name="report", cls=TCommand, help="report.help")
    @click.option("--at", metavar="DATE", help="opt.at")
    @click.option("--wait", is_flag=True, help="report.opt.wait")
    @click.option("--rubric", help="report.opt.rubric")
    @click.option("--id", "report_id", metavar="ID", help="report.opt.id")
    @click.option(
        "-o",
        "--output",
        type=click.Choice(("table", "json", "ndjson", "yaml")),
        metavar="FORMAT",
        help="opt.output",
    )
    @click.pass_context
    def report(  # noqa: PLR0913 - the options of a report
        ctx: click.Context,
        at: str | None,
        rubric: str | None,
        report_id: str | None,
        output: str | None,
        *,
        wait: bool,
    ) -> None:
        del at
        found = _found(ctx)
        client = found.client
        valid_at, _ = _dates(ctx, found)
        if report_id is None:
            body = {"valid_at": valid_at} if valid_at else None
            created = client.call("request_policy_report", body=body)
            record = dict(getattr(created, "body", created) or {})
            report_id = str(record.get("id"))
            click.echo(t("report.asked", id=report_id), err=True)
        record = dict(client.call("show_policy_report", id=report_id))
        while wait and record.get("state") != "done":
            client.sleep(2.0)
            record = dict(client.call("show_policy_report", id=report_id))
        if rubric:
            items = client.call("policy_report_items", id=report_id, rubric=rubric)
            found.output(output=output).items(
                cast("list[Mapping[str, Any]]", list(dict(items).get("items") or []))
            )
            return
        if _asked(found, output):
            found.output(output=output).value(record)
            return
        click.echo(t("report.state", id=report_id, state=record.get("state")))
        for name, count in sorted(cast("dict[str, Any]", record.get("counts") or {}).items()):
            click.echo(f"  {name}  {'—' if count is None else count}")

    return report


# flow request, review -----------------------------------------------------------------


def _request(catalog: Catalog) -> click.Command:
    from kartenn_cli.complete import reference_completer  # noqa: PLC0415

    @click.command(name="request", cls=TCommand, help="request.help")
    @click.option("--source", required=True, metavar="KIND:REF", help="request.opt.source")
    @click.option(
        "--destination", required=True, metavar="KIND:REF", help="request.opt.destination"
    )
    @click.option(
        "--service",
        required=True,
        help="request.opt.service",
        shell_complete=reference_completer("services"),
    )
    @click.option("--entity", required=True, help="request.opt.entity")
    @click.option("--ref", "refs", multiple=True, metavar="SYSTEM:ID", help="request.opt.ref")
    @click.option("--justification", help="request.opt.justification")
    @click.option("--end", metavar="DATE", help="request.opt.end")
    @click.option("--motive", help="opt.motive")
    @click.option(
        "--set",
        "assignments",
        cls=Option,
        multiple=True,
        metavar="metavar.assignment",
        help="opt.set",
    )
    @click.option("--require-applied", is_flag=True, help="opt.require_applied")
    @click.option(
        "-o",
        "--output",
        type=click.Choice(("table", "json", "yaml")),
        metavar="FORMAT",
        help="opt.output",
    )
    @click.pass_context
    def request(ctx: click.Context, **options: Any) -> None:  # noqa: ANN401
        from kartenn_cli.reads import resolve_ref  # noqa: PLC0415
        from kartenn_cli.writes import motive_of, parse_value, render_outcome  # noqa: PLC0415

        found = _found(ctx)
        motive = motive_of(ctx, found)
        client = found.client
        body: dict[str, Any] = {
            "source": _endpoint(found, catalog, options["source"]),
            "destination": _endpoint(found, catalog, options["destination"]),
            "service": {
                "id": resolve_ref(client, catalog, catalog.resource("services"), options["service"])
            },
            "entity": {
                "id": resolve_ref(client, catalog, catalog.resource("entities"), options["entity"])
            },
            "external_refs": [
                dict(zip(("system", "id"), ref.split(":", 1), strict=False))
                for ref in options["refs"]
            ],
            "motive": motive,
        }
        if options["justification"]:
            body["justification"] = options["justification"]
        if options["end"]:
            from datetime import UTC, datetime  # noqa: PLC0415

            from kartenn_cli.reads import resolve_date_text  # noqa: PLC0415

            body["end"] = resolve_date_text(
                options["end"], found, datetime.now(UTC).replace(microsecond=0)
            )
        for assignment in options["assignments"]:
            left, _, right = str(assignment).partition("=")
            body[left] = parse_value(right)
        result = client.call("request_flow", body=body)
        render_outcome(
            found, result, output=options["output"], require_applied=options["require_applied"]
        )

    return request


def _review(catalog: Catalog) -> click.Command:
    del catalog

    @click.command(name="review", cls=TCommand, help="review.help")
    @click.argument("flow")
    @click.option("--approve/--reject", "approve", default=None, help="review.opt.approve")
    @click.option("--motive", help="opt.motive")
    @click.option(
        "-o",
        "--output",
        type=click.Choice(("table", "json", "yaml")),
        metavar="FORMAT",
        help="opt.output",
    )
    @click.pass_context
    def review(
        ctx: click.Context,
        flow: str,
        motive: str | None,
        output: str | None,
        *,
        approve: bool | None,
    ) -> None:
        from kartenn_cli.writes import motive_of, render_outcome  # noqa: PLC0415

        del motive
        if approve is None:
            raise CliError("client.decision-required", t("review.decision"))
        found = _found(ctx)
        record = dict(found.client.call("show_flow", id=flow))
        changeset = record.get("changeset")
        if isinstance(changeset, dict):
            changeset = cast("dict[str, Any]", changeset).get("id")
        if not changeset:
            raise CliError("client.nothing-to-review", t("review.none", flow=flow))
        handle = found.client.changeset_of(str(changeset))
        result = handle.approve() if approve else handle.reject(str(motive_of(ctx, found)))
        render_outcome(found, result, output=output, require_applied=False)

    return review


NAMED: tuple[NamedCommand, ...] = (
    NamedCommand("policy", "check", ("show_verdicts",), _check, replaces=("verdicts",)),
    NamedCommand(
        "policy",
        "matrix",
        ("show_policy_matrix", "set_policy_matrix_cell"),
        _matrix,
        replaces=("matrix",),
    ),
    NamedCommand(
        "policy",
        "report",
        ("request_policy_report", "show_policy_report", "policy_report_items"),
        _report,
        replaces=("reports",),
    ),
    NamedCommand("flow", "request", ("request_flow",), _request, replaces=("create",)),
    NamedCommand(
        "flow", "review", ("show_flow", "validate_changeset", "reject_changeset"), _review
    ),
)


def _served(entry: NamedCommand, catalog: Catalog) -> bool:
    return catalog.has(entry.module) and all(catalog.has_operation(o) for o in entry.operation_ids)


def named_commands(module: str, catalog: Catalog) -> dict[str, click.Command]:
    """The named commands of a module the server serves."""
    return {
        entry.name: entry.build(catalog)
        for entry in NAMED
        if entry.module == module and _served(entry, catalog)
    }


def replaced(module: str, catalog: Catalog) -> frozenset[str]:
    """The derived commands of a module that its named commands stand for."""
    return frozenset(
        name
        for entry in NAMED
        if entry.module == module and _served(entry, catalog)
        for name in entry.replaces
    )
