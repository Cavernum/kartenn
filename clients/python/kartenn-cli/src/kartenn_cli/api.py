"""`kartenn api` and the tools of the base (REQ-023-19; design § Couverture; maquette § 11).

`kartenn api METHOD PATH` reaches any operation of the contract, `QUERY` included, with
the authentication of the profile: `-f key=value` (query), `--data` (a body, `@file`,
`-` for the standard input), `-H 'Name: value'`, `--paginate` (every page, its elements
as they come). `kartenn api spec` writes the contract. `capabilities`, `health` and
`stubs` are the other operations of the base.
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import Option, command, group
from kartenn_cli.errors import CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from collections.abc import Iterator, Mapping

    from kartenn._core.messages import Request, Response
    from kartenn_cli.runtime import Runtime

__all__ = [
    "api_command",
    "capabilities_command",
    "health_command",
    "stats_command",
    "stubs_group",
]

METHODS = ("GET", "HEAD", "QUERY", "POST", "PUT", "PATCH", "DELETE")
OUTPUTS = ("json", "ndjson", "yaml", "table", "csv")


def _found(ctx: click.Context) -> Runtime:
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    return runtime(ctx)


def _pair(text: str, sep: str, code: str) -> tuple[str, str]:
    left, found, right = text.partition(sep)
    if not found or not left.strip():
        raise CliError(code, t("api.pair", given=text, sep=sep))
    return left.strip(), right.strip() if sep == ":" else right


def _data(text: str) -> bytes:
    if text == "-":
        return sys.stdin.read().encode()
    if text.startswith("@"):
        return Path(text[1:]).read_bytes()
    return text.encode()


def _answer(response: Response) -> object:
    from kartenn._core.plans import checked  # noqa: PLC0415

    checked(response)
    if not response.content:
        return None
    try:
        return response.json()
    except ValueError:
        return response.content.decode(errors="replace")


def _pages(found: Runtime, first: Response) -> Iterator[Mapping[str, Any]]:
    from kartenn._core.pages import next_request, parse_page  # noqa: PLC0415
    from kartenn._core.plans import checked  # noqa: PLC0415

    page = parse_page(checked(first))
    while True:
        yield from page.items
        following = next_request(page)
        if following is None:
            return
        page = parse_page(checked(found.client.send(following)))


@command(name="api", help="api.help")
@click.argument("method")
@click.argument("path", required=False)
@click.option("-f", "--field", "fields_", multiple=True, metavar="KEY=VALUE", help="api.opt.field")
@click.option("--data", metavar="JSON|@FILE|-", help="api.opt.data")
@click.option(
    "-H", "--header", "headers", multiple=True, metavar="'NAME: VALUE'", help="api.opt.header"
)
@click.option("--paginate", is_flag=True, help="api.opt.paginate")
@click.option("--at", metavar="DATE", help="opt.at")
@click.option("--as-of", "as_of", metavar="DATE", help="opt.as_of")
@click.option("-o", "--output", type=click.Choice(OUTPUTS), metavar="FORMAT", help="opt.output")
@click.pass_context
def api_command(  # noqa: PLR0913, PLR0917 - the parts of a request
    ctx: click.Context,
    method: str,
    path: str | None,
    fields_: tuple[str, ...],
    data: str | None,
    headers: tuple[str, ...],
    output: str | None,
    *,
    paginate: bool,
    **dates: Any,  # noqa: ANN401
) -> None:
    from kartenn._core.messages import Headers, Request  # noqa: PLC0415
    from kartenn._core.requests import idempotency_key  # noqa: PLC0415
    from kartenn_cli.reads import dates_of  # noqa: PLC0415

    del dates
    found = _found(ctx)
    verb = method.upper()
    if verb == "SPEC" and path is None:
        found.output(output=output or "json").value(found.client.call("openapi"))
        return
    if verb not in METHODS:
        raise CliError(
            "client.invalid-method", t("api.method", given=method, methods=", ".join(METHODS))
        )
    if not path or not path.startswith("/"):
        raise CliError("client.invalid-path", t("api.path", given=path or ""))
    params = [_pair(f, "=", "client.invalid-field") for f in fields_]
    when = dates_of(ctx, found)
    params += [(n, v) for n, v in (("valid_at", when.valid_at), ("as_of", when.as_of)) if v]
    sent = Headers.of([_pair(h, ":", "client.invalid-header") for h in headers])
    content = None
    if data is not None:
        content = _data(data)
        sent = sent.setdefault("Content-Type", "application/json")
    if verb == "POST":
        sent = sent.setdefault("Idempotency-Key", idempotency_key())
    request: Request = Request(verb, path, tuple(params), sent, content)
    response = found.client.send(request)
    if paginate:
        from kartenn._core.plans import checked  # noqa: PLC0415

        body = checked(response).json()
        if isinstance(body, dict) and "items" in cast("dict[str, Any]", body):
            found.output(output=output or "ndjson").items(_pages(found, response))
            return
    answer = _answer(response)
    if isinstance(answer, str):
        click.echo(answer)
    elif answer is not None:
        found.output(output=output or "json").value(answer)


@command(name="capabilities", help="tools.capabilities")
@click.option("-o", "--output", type=click.Choice(OUTPUTS), metavar="FORMAT", help="opt.output")
@click.pass_context
def capabilities_command(ctx: click.Context, output: str | None) -> None:
    found = _found(ctx)
    found.output(output=output).value(dict(found.client.call("capabilities")))


@command(name="stats", help="tools.stats")
@click.option("-o", "--output", type=click.Choice(OUTPUTS), metavar="FORMAT", help="opt.output")
@click.pass_context
def stats_command(ctx: click.Context, output: str | None) -> None:
    found = _found(ctx)
    found.output(output=output).value(found.client.stats())


@command(name="health", help="tools.health")
@click.argument("probe", type=click.Choice(("live", "ready")), default="ready")
@click.pass_context
def health_command(ctx: click.Context, probe: str) -> None:
    from kartenn._core.messages import Request  # noqa: PLC0415

    found = _found(ctx)
    response = found.client.send(Request("GET", f"/health/{probe}"))
    answer = _answer(response)
    click.echo(t("tools.healthy", probe=probe, status=response.status))
    if isinstance(answer, dict):
        found.output().value(cast("dict[str, Any]", answer))


@group(name="stubs", help="tools.stubs")
def stubs_group() -> None:
    pass


@stubs_group.command(name="generate", help="tools.stubs_generate")
@click.option(
    "--out",
    type=click.Path(file_okay=False, path_type=Path),
    default=Path("typings"),
    cls=Option,
    help="tools.stubs_out",
)
@click.pass_context
def stubs_generate(ctx: click.Context, out: Path) -> None:
    from kartenn.stubs import generate  # noqa: PLC0415

    found = _found(ctx)
    click.echo(str(generate(found.client.catalog, out)))
