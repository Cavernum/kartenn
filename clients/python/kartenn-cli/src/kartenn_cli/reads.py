"""Reads (REQ-023-22, 23, 25, 26, 27; T14, T15; maquette § 4).

- dates: `--at` (`valid_at`) and `--as-of`, RFC 3339, a date alone (midnight in the time
  zone of the profile) or relative (`-7d`, `+1M`); `--freeze` asks the present
  explicitly and says how to keep it (`KARTENN_AS_OF`);
- filters: the options typed from the contract, and `--prop key[:op]=value`; an unknown
  filter is refused before any request, with the filters there are;
- a list writes its first page, or all of them with `--all`, as it reads them;
- an object is named by its UUID, or by its natural key when the contract declares one
  (`x-kartenn.key`): `interne/192.0.2.0/24`.
"""

from __future__ import annotations

import re
from datetime import UTC, datetime
from typing import TYPE_CHECKING, Any, cast
from urllib.parse import parse_qs, urlsplit

import click

from kartenn_cli.base import Option
from kartenn_cli.errors import NOT_FOUND, USAGE, CliError
from kartenn_cli.i18n import t, tn
from kartenn_cli.tree import VerbCommand

if TYPE_CHECKING:
    from collections.abc import Iterable, Iterator, Mapping

    from kartenn import Client
    from kartenn._core.pages import Page
    from kartenn.catalog import Catalog, Resource
    from kartenn_cli.render import Output
    from kartenn_cli.runtime import Runtime

__all__ = [
    "Dates",
    "GetCommand",
    "ListCommand",
    "ReadCommand",
    "dates_of",
    "list_params",
    "resolve_ref",
]

UUID = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")


# Dates --------------------------------------------------------------------------------


class Dates:
    """The dates of a read, as sent (RFC 3339), and whether `--freeze` asked for them."""

    def __init__(self, valid_at: str | None, as_of: str | None, *, frozen: bool) -> None:
        self.valid_at = valid_at
        self.as_of = as_of
        self.frozen = frozen


def _resolve(text: str | None, found: Runtime, now: datetime) -> str | None:
    if not text:
        return None
    from kartenn._core.dates import format_rfc3339, resolve_date  # noqa: PLC0415

    zone = found.zone or now.astimezone().tzinfo or UTC
    return format_rfc3339(resolve_date(text, zone=zone, now=now))


def dates_of(ctx: click.Context, found: Runtime) -> Dates:
    """`--at`, `--as-of` and `--freeze` of the command, else of the root."""
    now = datetime.now(UTC).replace(microsecond=0)
    at = ctx.params.get("at") or found.settings.at
    as_of = ctx.params.get("as_of") or found.settings.as_of
    frozen = bool(ctx.params.get("freeze")) and not as_of
    if frozen:
        as_of = "now"
    return Dates(_resolve(at, found, now), _resolve(as_of, found, now), frozen=frozen)


def date_params(*, freeze: bool = False) -> list[click.Parameter]:
    section = "help.section.dates"
    params: list[click.Parameter] = [
        Option(["--at"], metavar="DATE", help="opt.at", section=section),
        Option(["--as-of", "as_of"], metavar="DATE", help="opt.as_of", section=section),
    ]
    if freeze:
        params.append(Option(["--freeze"], is_flag=True, help="opt.freeze", section=section))
    return params


# References ---------------------------------------------------------------------------


def resolve_ref(client: Client, catalog: Catalog, resource: Resource, text: str) -> str:
    """The identifier of an object named by its UUID or its natural key (REQ-023-27)."""
    if UUID.fullmatch(text):
        return text
    key = resource.key
    if not key:
        raise CliError(
            "client.key-unknown", t("ref.no_key", resource=resource.name, ref=text), exit_code=USAGE
        )
    parts = text.split("/", len(key) - 1)
    if len(parts) != len(key):
        raise CliError(
            "client.invalid-reference",
            t("ref.shape", ref=text, key="/".join(key), resource=resource.name),
        )
    del catalog
    filters: dict[str, Any] = dict(zip(key, parts, strict=True))
    page = client.resource(resource.name).list(page_size=2, **filters).first_page()
    if not page.items:
        raise CliError(
            "client.reference-not-found",
            t("ref.not_found", ref=text, resource=resource.name),
            exit_code=NOT_FOUND,
        )
    if len(page.items) > 1:
        raise CliError(
            "client.ambiguous-reference", t("ref.ambiguous", ref=text, resource=resource.name)
        )
    return str(next(iter(page.items))["id"])


# Lists --------------------------------------------------------------------------------


def _prop_values(entries: Iterable[str]) -> dict[str, object]:
    """`--prop` entries as the library takes them: values, `between`, `exists`."""
    from kartenn import between, exists  # noqa: PLC0415
    from kartenn._core.requests import parse_prop  # noqa: PLC0415

    found: dict[str, object] = {}
    for entry in entries:
        condition = parse_prop(entry)
        key = condition.field.removeprefix("prop.")
        if condition.op == "exists":
            found[key] = exists(bool(condition.value))
        elif condition.op == "between":
            low, high = cast("tuple[Any, Any]", condition.value)
            found[key] = between(low, high)
        else:
            value = cast("tuple[Any, ...]", condition.value)[0]
            previous = found.get(key)
            found[key] = (
                [*cast("list[object]", previous), value]
                if isinstance(previous, list)
                else ([previous, value] if previous is not None else value)
            )
    return found


def _cursor(link: str | None) -> str | None:
    if not link:
        return None
    values = parse_qs(urlsplit(link).query).get("cursor")
    return values[0] if values else link


def list_params(command: VerbCommand) -> list[click.Parameter]:
    """The parameters of `list`: filters, `--prop`, dates, pages and output."""
    from kartenn_cli.commands import output_params  # noqa: PLC0415
    from kartenn_cli.tree import query_option  # noqa: PLC0415

    section = "help.section.filters"
    params: list[click.Parameter] = [
        query_option(param, section=section, multiple=True) for param in command.operation.filters
    ]
    params.append(
        Option(
            ["--prop"],
            multiple=True,
            metavar="metavar.prop",
            help="opt.prop",
            section=section,
        )
    )
    params += date_params(freeze=True)
    params += output_params()
    pages = "help.section.output"
    params += [
        Option(["--all", "every"], is_flag=True, help="opt.all", section=pages),
        Option(
            ["--page-size"],
            type=click.IntRange(1),
            metavar="N",
            help="opt.page_size",
            section=pages,
        ),
        Option(["--cursor"], metavar="CURSOR", help="opt.cursor", section=pages),
    ]
    return params


def write_pages(
    output: Output,
    pages: Iterator[Page],
    *,
    every: bool,
    dates: Dates,
    zone: Any,  # noqa: ANN401 - a tzinfo
) -> None:
    """Write a list page after page; under a table, its count, dates and what follows."""
    from kartenn_cli.render import format_moment  # noqa: PLC0415

    state: dict[str, Any] = {"next": None, "as_of": dates.as_of}

    def items() -> Iterator[Mapping[str, Any]]:
        for page in pages:
            state["next"], state["as_of"] = page.next, page.as_of or state["as_of"]
            yield from page.items
            if not every:
                return

    def footer(count: int) -> str:
        parts = [tn("list.count", count)]
        as_of = format_moment(state["as_of"], zone)
        if dates.valid_at:
            valid = format_moment(dates.valid_at, zone)
            parts.append(
                t("list.valid_as_of", valid=valid, as_of=as_of)
                if as_of
                else t("list.valid", valid=valid)
            )
        elif as_of:
            parts.append(t("list.as_of", as_of=as_of))
        line = " — ".join(parts)
        following = _cursor(state["next"])
        if following:
            line += "\n" + t("list.next", cursor=following)
        if dates.frozen and dates.as_of:
            line += "\n" + t("list.frozen", as_of=dates.as_of)
        return line

    output.items(items(), footer=footer)
    if not output.table and state["next"] and not every:
        click.echo(t("list.next", cursor=_cursor(state["next"])), err=True)


def _output(found: Runtime, ctx: click.Context, resource: Resource) -> Output:
    from kartenn_cli.render.data import parse_fields  # noqa: PLC0415

    return found.output(
        output=ctx.params.get("output"),
        fields=parse_fields(ctx.params.get("fields")),
        query=ctx.params.get("query"),
        keys=ctx.params.get("keys"),
        ids=bool(ctx.params.get("ids")),
        columns=resource.columns,
    )


class ListCommand(VerbCommand):
    """`list`: the filters of the contract, `--prop`, the dates, the pages."""

    def build_params(self) -> list[click.Parameter]:
        return list_params(self)

    def parse_args(self, ctx: click.Context, args: list[str]) -> list[str]:
        try:
            return super().parse_args(ctx, args)
        except click.NoSuchOption as error:
            filters = ", ".join(f"--{p.name.replace('_', '-')}" for p in self.operation.filters)
            raise CliError(
                "client.unknown-filter",
                t("filter.unknown", option=error.option_name, filters=filters or "—"),
            ) from error

    def invoke(self, ctx: click.Context) -> Any:  # noqa: ANN401
        from kartenn_cli.runtime import runtime  # noqa: PLC0415

        found = runtime(ctx)
        dates = dates_of(ctx, found)
        filters: dict[str, object] = {}
        for param in self.operation.filters:
            values = ctx.params.get(param.name) or ()
            if values:
                filters[param.name] = list(values)
        listing = found.client.resource(self.resource.name).list(
            valid_at=dates.valid_at,
            as_of=dates.as_of,
            page_size=ctx.params.get("page_size"),
            cursor=ctx.params.get("cursor"),
            prop=_prop_values(ctx.params.get("prop") or ()) or None,
            **filters,
        )
        write_pages(
            _output(found, ctx, self.resource),
            listing.pages(),
            every=bool(ctx.params.get("every")),
            dates=dates,
            zone=found.zone,
        )


class ReadCommand(VerbCommand):
    """A read of an object or of a sub-resource (`get`, `history`, `children`…)."""

    def build_params(self) -> list[click.Parameter]:
        from kartenn_cli.commands import output_params, path_arguments  # noqa: PLC0415
        from kartenn_cli.tree import query_option  # noqa: PLC0415

        operation = self.operation
        params = path_arguments(self)
        skipped = {"valid_at", "as_of", "cursor", "page_size"}
        params += [
            query_option(p, section="help.section.params")
            for p in operation.params
            if p.location == "query" and p.name not in skipped
        ]
        if operation.param("valid_at") or operation.param("as_of"):
            params += date_params()
        params += output_params()
        if operation.param("cursor"):
            params.append(
                Option(
                    ["--all", "every"], is_flag=True, help="opt.all", section="help.section.output"
                )
            )
        return params

    def params_of(self, ctx: click.Context, found: Runtime) -> tuple[str | None, dict[str, object]]:
        operation = self.operation
        names = operation.path_params
        ref = None
        params: dict[str, object] = {}
        for index, name in enumerate(names):
            value = ctx.params[name.lower()]
            if index == 0:
                ref = resolve_ref(found.client, self.catalog, self.resource, value)
            else:
                params[name] = value
        for param in operation.params:
            if param.location == "query" and param.name in ctx.params:
                value = ctx.params[param.name]
                if value is not None and value != ():
                    params[param.name] = value
        return ref, params

    def invoke(self, ctx: click.Context) -> Any:  # noqa: ANN401
        from kartenn_cli.runtime import runtime  # noqa: PLC0415

        found = runtime(ctx)
        dates = dates_of(ctx, found)
        client = found.client.at(dates.valid_at, dates.as_of)
        ref, params = self.params_of(ctx, found)
        resource = client.resource(self.resource.name)
        output = _output(found, ctx, self.resource)
        if self.verb == "get":
            output.value(dict(resource.get(ref, **params)))
            return
        verb = next(
            v.name for v in self.resource.verbs if v.operation_id == self.operation.operation_id
        )
        if self.operation.param("cursor") is not None:
            listing = resource.pages_of(verb, ref, **cast("dict[str, Any]", params))
            write_pages(
                output,
                listing.pages(),
                every=bool(ctx.params.get("every")),
                dates=dates,
                zone=found.zone,
            )
            return
        answer: object = resource.verb(verb, ref, **cast("dict[str, Any]", params))
        output.value(answer)


class GetCommand(ReadCommand):
    """`get REF`."""
