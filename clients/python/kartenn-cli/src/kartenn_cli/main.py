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

    from kartenn.catalog import Catalog
    from kartenn_cli.runtime import Runtime
    from kartenn_cli.tree import Layout

__all__ = ["KartennGroup", "cli", "main"]

# Static commands: module, attribute, section of the root help.
STATIC: dict[str, tuple[str, str, str]] = {
    "config": ("kartenn_cli.config", "config_group", "tools"),
    "completion": ("kartenn_cli.complete", "completion_command", "tools"),
    "changeset": ("kartenn_cli.changeset", "changeset_group", "writes"),
    "drafts": ("kartenn_cli.drafts", "drafts_group", "writes"),
    "import": ("kartenn_cli.importing", "import_command", "writes"),
    "api": ("kartenn_cli.api", "api_command", "tools"),
    "capabilities": ("kartenn_cli.api", "capabilities_command", "tools"),
    "health": ("kartenn_cli.api", "health_command", "tools"),
    "stats": ("kartenn_cli.api", "stats_command", "tools"),
    "stubs": ("kartenn_cli.api", "stubs_group", "tools"),
    "login": ("kartenn_cli.session", "login_command", "tools"),
    "logout": ("kartenn_cli.session", "logout_command", "tools"),
    "me": ("kartenn_cli.session", "me_command", "rights"),
    "watch": ("kartenn_cli.watch", "watch_command", "tools"),
}
# Static commands shown with their help in the root help; the others by name only.
STATIC_DESCRIBED = frozenset({"api", "changeset", "drafts", "import"})
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

    def _tree(self, ctx: click.Context) -> tuple[Catalog, Layout] | None:
        """The catalog in the cache (or discovered once); a line and nothing more when
        the server cannot be reached: the local commands stay."""
        found = _runtime(ctx)
        try:
            if ctx.resilient_parsing:
                return found.tree(cached=True)
            return found.tree()
        except _handled() as error:
            if "_tree_failed" not in found.__dict__:
                found.__dict__["_tree_failed"] = True
                code = getattr(error, "code", error)
                click.echo(t("discovery.failed", error=code), err=True)
                if code in {"client.login-required", "unauthenticated", "client.secret-required"}:
                    click.echo(t("discovery.login"), err=True)
            return None

    def list_commands(self, ctx: click.Context) -> list[str]:
        names = set(STATIC)
        tree = self._tree(ctx)
        if tree is not None:
            names |= set(tree[1].resources) | set(tree[1].modules) | set(tree[1].mains)
        return sorted(names)

    def get_command(self, ctx: click.Context, cmd_name: str) -> click.Command | None:
        static = STATIC.get(cmd_name)
        if static is not None:
            module = importlib.import_module(static[0])
            command = getattr(module, static[1])
            return command if isinstance(command, click.Command) else None
        from kartenn_cli.tree import KNOWN_MODULES, Unavailable, module_unavailable  # noqa: PLC0415
        from kartenn_cli.tree import resource_command as derived  # noqa: PLC0415

        tree = self._tree(ctx)
        if tree is None:
            return None
        command = derived(cmd_name, *tree)
        if command is not None or ctx.resilient_parsing:
            return command
        found = _runtime(ctx)
        tree = found.tree(discover=True)
        if tree is None:  # pragma: no cover - a profile was read above
            return None
        command = derived(cmd_name, *tree)
        if command is None and cmd_name in KNOWN_MODULES:
            error = module_unavailable(cmd_name, tree[0], found.require_profile())
            return Unavailable(cmd_name, error)
        return command

    def description(self, ctx: click.Context) -> str:
        found = _runtime(ctx)
        profile = found.profile_or_none()
        if profile is None:
            return t("help.root_no_profile")
        tree = self._tree(ctx)
        version = tree[0].version if tree is not None else "?"
        return t("help.root", profile=profile.name, url=profile.url, version=version)

    def command_sections(self, ctx: click.Context) -> list[Section]:
        from kartenn_cli.tree import root_entries  # noqa: PLC0415

        grouped: dict[str, list[tuple[str, str]]] = {name: [] for name in self.sections}
        tree = self._tree(ctx)
        if tree is not None:
            for section, entries in root_entries(*tree).items():
                grouped[section] += entries
        for name, (_, _, section) in sorted(STATIC.items()):
            grouped[section].append((name, self._static_help(ctx, name)))
        return [(t(f"help.section.{key}"), entries) for key, entries in grouped.items() if entries]

    def _static_help(self, ctx: click.Context, name: str) -> str:
        if name not in STATIC_DESCRIBED:
            return ""
        command = self.get_command(ctx, name)
        return t(command.help or "") if command is not None else ""

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
