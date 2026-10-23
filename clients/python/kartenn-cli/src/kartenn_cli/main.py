"""Root of the `kartenn` command (REQ-023-18, 39; T12; design § Racine et chargement).

This module imports click and the configuration only. The commands are made when asked
(`get_command`): the static ones by importing their module, the derived ones from the
catalog in the cache. `--version` imports neither the library, nor httpx2, nor rich.

Errors of the library and of the CLI are rendered with their code first, and the process
ends with the exit code of their class (`kartenn_cli.errors`).
"""

from __future__ import annotations

import importlib
import sys
from pathlib import Path
from typing import TYPE_CHECKING, Any, ClassVar, cast

import click

from kartenn_cli import __version__
from kartenn_cli.base import CONTEXT, Option, Section, TGroup
from kartenn_cli.config import OUTPUTS, read_config
from kartenn_cli.errors import CliError
from kartenn_cli.i18n import LANGS, resolve, set_lang, t

if TYPE_CHECKING:
    from collections.abc import Sequence

    from kartenn_cli.runtime import Runtime

__all__ = ["KartennGroup", "cli", "main"]

# Static commands: module, attribute, section of the root help.
STATIC: dict[str, tuple[str, str, str]] = {
    "config": ("kartenn_cli.config", "config_group", "tools"),
}
SECTIONS = ("core", "writes", "modules", "rights", "tools", "others")
_LAST: dict[str, Runtime] = {}


def _runtime(ctx: click.Context) -> Runtime:
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    found = runtime(ctx)
    if not _LAST or _LAST.get("runtime") is not found:
        _LAST["runtime"] = found
        ctx.find_root().call_on_close(found.close)
    return found


def _setting(ctx: click.Context, param: click.Parameter, value: Any) -> Any:  # noqa: ANN401
    """Root options land in the settings as soon as they are parsed (the eager ones before
    `--help`)."""
    if value is not None:
        _runtime(ctx).apply(str(param.name), value)
    return value


def fail(error: BaseException, found: Runtime | None) -> int:
    """Write an error on the standard error; its exit code."""
    from kartenn_cli.errors import exit_code  # noqa: PLC0415
    from kartenn_cli.render.problems import render_error  # noqa: PLC0415

    json_output = found.json_errors if found is not None else False
    color = found.error_color if found is not None else False
    click.echo(render_error(error, json_output=json_output, color=color), err=True, nl=False)
    return exit_code(error)


def _handled() -> tuple[type[BaseException], ...]:
    """The errors rendered with their code: the CLI's, and the library's once loaded."""
    kartenn = sys.modules.get("kartenn")
    found: tuple[type[BaseException], ...] = (CliError,)
    if kartenn is not None and hasattr(kartenn, "KartennError"):
        found += (kartenn.KartennError,)
    return found


def _early_lang() -> None:
    """The language before any option: `KARTENN_LANG`, the profile, the locale. The
    usage line is written before anything else asks for the profile."""
    import os  # noqa: PLC0415

    profile_lang = None
    try:
        config = read_config()
    except CliError:
        config = {}
    name = os.environ.get("KARTENN_PROFILE") or config.get("default")
    profiles = config.get("profiles")
    if isinstance(name, str) and isinstance(profiles, dict):
        raw = cast("dict[str, Any]", profiles).get(name)
        if isinstance(raw, dict):
            found = cast("dict[str, Any]", raw).get("lang")
            profile_lang = found if isinstance(found, str) else None
    set_lang(resolve(os.environ.get("KARTENN_LANG"), profile_lang))


class KartennGroup(TGroup):
    """The root: static commands, then the commands derived from the catalog."""

    sections: ClassVar[tuple[str, ...]] = SECTIONS

    def list_commands(self, ctx: click.Context) -> list[str]:  # noqa: ARG002 - the catalog, later
        return sorted(STATIC)

    def get_command(self, ctx: click.Context, cmd_name: str) -> click.Command | None:  # noqa: ARG002
        found = STATIC.get(cmd_name)
        if found is not None:
            module = importlib.import_module(found[0])
            command = getattr(module, found[1])
            return command if isinstance(command, click.Command) else None
        return None

    def description(self, ctx: click.Context) -> str:
        found = _runtime(ctx)
        profile = found.profile_or_none()
        if profile is None:
            return t("help.root_no_profile")
        catalog = found.cached_catalog()
        version = catalog.version if catalog is not None else "?"
        return t("help.root", profile=profile.name, url=profile.url, version=version)

    def command_sections(self, ctx: click.Context) -> list[Section]:  # noqa: ARG002
        grouped: dict[str, list[tuple[str, str]]] = {name: [] for name in self.sections}
        for name, (_, _, section) in sorted(STATIC.items()):
            grouped[section].append((name, ""))
        return [(t(f"help.section.{key}"), entries) for key, entries in grouped.items() if entries]

    def invoke(self, ctx: click.Context) -> Any:  # noqa: ANN401 - what the command returns
        try:
            return super().invoke(ctx)
        except _handled() as error:
            ctx.exit(fail(error, _LAST.get("runtime")))

    def main(
        self,
        args: Sequence[str] | None = None,
        prog_name: str | None = None,
        complete_var: str | None = None,
        standalone_mode: bool = True,  # noqa: FBT001, FBT002 - the signature of click
        **extra: Any,  # noqa: ANN401
    ) -> Any:  # noqa: ANN401
        _LAST.clear()
        if "--version" not in (sys.argv[1:] if args is None else args):
            _early_lang()
        if not sys.stdout.isatty():
            extra.setdefault("terminal_width", 100)
        try:
            return super().main(args, prog_name, complete_var, standalone_mode, **extra)
        except _handled() as error:
            sys.exit(fail(error, _LAST.get("runtime")))


@click.group(
    name="kartenn",
    cls=KartennGroup,
    context_settings=CONTEXT,
    help="help.root_no_profile",
)
@click.option(
    "-p",
    "--profile",
    cls=Option,
    metavar="metavar.name",
    is_eager=True,
    expose_value=False,
    callback=_setting,
    help="opt.profile",
)
@click.option(
    "-o",
    "--output",
    cls=Option,
    type=click.Choice(OUTPUTS),
    metavar="FORMAT",
    expose_value=False,
    callback=_setting,
    help="opt.output",
)
@click.option(
    "--at",
    cls=Option,
    metavar="DATE",
    expose_value=False,
    callback=_setting,
    help="opt.at",
)
@click.option(
    "--as-of",
    "as_of",
    cls=Option,
    metavar="DATE",
    expose_value=False,
    callback=_setting,
    envvar="KARTENN_AS_OF",
    help="opt.as_of",
)
@click.option(
    "--keys/--labels",
    "keys",
    cls=Option,
    default=None,
    expose_value=False,
    callback=_setting,
    help="opt.keys",
)
@click.option(
    "--lang",
    cls=Option,
    type=click.Choice(LANGS),
    is_eager=True,
    expose_value=False,
    callback=_setting,
    help="opt.lang",
)
@click.option(
    "--no-input",
    "no_input",
    cls=Option,
    is_flag=True,
    default=None,
    expose_value=False,
    callback=_setting,
    help="opt.no_input",
)
@click.option(
    "--token-file",
    "token_file",
    cls=Option,
    type=click.Path(dir_okay=False, path_type=Path),
    metavar="metavar.file",
    expose_value=False,
    callback=_setting,
    help="opt.token_file",
)
@click.option(
    "-v",
    "--verbose",
    cls=Option,
    count=True,
    default=None,
    expose_value=False,
    callback=_setting,
    help="opt.verbose",
)
@click.version_option(__version__, prog_name="kartenn", help="opt.version")
def cli() -> None:
    pass


def main() -> None:
    """The entry point of the `kartenn` script."""
    cli.main(prog_name="kartenn")
