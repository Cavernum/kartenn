"""What a verb of the tree does: its parameters and its run.

Every verb reaches its operation of the contract: path parameters as arguments, query
parameters as options. Reads add the dates and the output options.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import Option
from kartenn_cli.tree import VerbCommand, query_option

if TYPE_CHECKING:
    from kartenn.catalog import Catalog, Resource, Verb

__all__ = [
    "command_class",
    "extra_command",
    "extra_verbs",
    "output_params",
    "run_verb",
    "verb_params",
]

_SKIPPED = frozenset({"valid_at", "as_of", "cursor", "page_size"})


def output_params() -> list[click.Parameter]:
    """`-o`, `--fields`, `--query`, `--keys/--labels`, `--ids`: the output of a read."""
    from kartenn_cli.config import OUTPUTS  # noqa: PLC0415

    section = "help.section.output"
    return [
        Option(
            ["-o", "--output"],
            type=click.Choice(OUTPUTS),
            metavar="FORMAT",
            help="opt.output",
            section=section,
            callback=_output,
        ),
        Option(["--fields"], metavar="A,B", help="opt.fields", section=section),
        Option(["--query"], metavar="JMESPATH", help="opt.query", section=section),
        Option(["--keys/--labels"], default=None, help="opt.keys", section=section),
        Option(["--ids"], is_flag=True, help="opt.ids", section=section),
    ]


def _output(ctx: click.Context, param: click.Parameter, value: str | None) -> str | None:
    """`-o` of a command counts as the root's: a problem is then written in JSON."""
    del param
    if value is not None:
        from kartenn_cli.runtime import runtime  # noqa: PLC0415

        runtime(ctx).settings.output = value
    return value


def path_arguments(command: VerbCommand) -> list[click.Parameter]:
    return [
        click.Argument([name.lower()], metavar="REF" if index == 0 else name.upper())
        for index, name in enumerate(command.operation.path_params)
    ]


def verb_params(command: VerbCommand) -> list[click.Parameter]:
    operation = command.operation
    params = path_arguments(command)
    read = operation.method in {"GET", "QUERY"}
    listing = command.verb == "list"
    section = "help.section.filters" if listing else "help.section.params"
    for param in operation.params:
        if param.location == "query" and param.name not in _SKIPPED:
            params.append(query_option(param, section=section, multiple=listing))
    if read:
        from kartenn_cli.reads import date_params  # noqa: PLC0415

        if operation.param("valid_at") or operation.param("as_of"):
            params += date_params()
        params += output_params()
    return params


def command_class(resource: Resource, verb: Verb, catalog: Catalog) -> type[VerbCommand]:  # noqa: ARG001
    """The class of a verb: lists, reads, then the other operations."""
    from kartenn_cli.reads import GetCommand, ListCommand, ReadCommand  # noqa: PLC0415
    from kartenn_cli.writes import (  # noqa: PLC0415
        BY_OPERATION,
        ActionCommand,
        CloseCommand,
        CreateCommand,
        UpdateCommand,
    )

    special = BY_OPERATION.get(verb.operation_id)
    if special is not None:
        return special
    if verb.name == "list":
        return ListCommand
    if verb.name == "get":
        return GetCommand
    if verb.read:
        return ReadCommand
    return {"create": CreateCommand, "update": UpdateCommand, "close": CloseCommand}.get(
        verb.name, ActionCommand
    )


def extra_verbs(resource: Resource) -> list[str]:
    """`edit`, for a resource read and updated."""
    return ["edit"] if resource.verb("get") and resource.verb("update") else []


def extra_command(name: str, resource: Resource, catalog: Catalog) -> click.Command | None:
    if name != "edit" or name not in extra_verbs(resource):
        return None
    from kartenn_cli.edit import EditCommand  # noqa: PLC0415

    get = resource.verb("get")
    assert get is not None  # noqa: S101 - checked by extra_verbs
    return EditCommand(
        name, resource=resource, operation=catalog.operation(get.operation_id), catalog=catalog
    )


def run_verb(command: VerbCommand, ctx: click.Context) -> Any:  # noqa: ANN401
    """Call the operation with the arguments and options given, and write its answer."""
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    found = runtime(ctx)
    operation = command.operation
    params: dict[str, object] = {}
    for name in operation.path_params:
        params[name] = ctx.params[name.lower()]
    for param in operation.params:
        if param.location != "query" or param.name in _SKIPPED:
            continue
        value: object = ctx.params.get(param.name)
        if isinstance(value, tuple):
            value = list(cast("tuple[object, ...]", value)) or None
        if value is not None:
            params[param.name] = value
    result = found.client.call(operation.operation_id, **cast("dict[str, Any]", params))
    output = found.output(
        output=ctx.params.get("output"),
        fields=_fields(ctx.params.get("fields")),
        query=ctx.params.get("query"),
        keys=ctx.params.get("keys"),
        ids=bool(ctx.params.get("ids")),
    )
    output.value(result)


def _fields(text: object) -> list[str] | None:
    from kartenn_cli.render.data import parse_fields  # noqa: PLC0415

    return parse_fields(text if isinstance(text, str) else None)
