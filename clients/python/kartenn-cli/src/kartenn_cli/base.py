"""Click commands whose help is translated and laid out in sections (REQ-023-21, 37).

A help text or an option help may be a key of the catalogs (`opt.profile`), translated
when the help is shown: the language is known only once the options are parsed.

`--help` is rendered in plain text by click, sections included, unless the standard
output is a terminal with colours: rich-click then renders the same sections (T12). It is
imported there and only there.
"""

from __future__ import annotations

import copy
import os
import sys
from typing import TYPE_CHECKING, Any

import click

from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from collections.abc import Callable, Sequence

__all__ = [
    "Entry",
    "Option",
    "Section",
    "TCommand",
    "TGroup",
    "command",
    "group",
    "option",
    "rich_wanted",
]

Entry = tuple[str, str]
Section = tuple[str, list[Entry]]
CONTEXT = {"help_option_names": ["-h", "--help"], "max_content_width": 100}


def rich_wanted() -> bool:
    """rich-click renders the help on a terminal that takes colours (NO_COLOR, `dumb`)."""
    if os.environ.get("NO_COLOR") or os.environ.get("TERM") == "dumb":
        return False
    return sys.stdout.isatty()


class Option(click.Option):
    """An option whose help is a key of the catalogs, and which belongs to a section of the
    help (`section`, a key too; `help.options` by default)."""

    def __init__(self, *args: Any, section: str | None = None, **kwargs: Any) -> None:  # noqa: ANN401
        super().__init__(*args, **kwargs)
        self.section = section or "help.options"

    def make_metavar(self, ctx: click.Context) -> str:
        if self.metavar is not None:
            return t(self.metavar)
        return super().make_metavar(ctx)


def _record(param: click.Parameter, ctx: click.Context) -> tuple[str, str] | None:
    """The help record of a parameter, its help translated."""
    if not isinstance(param, click.Option):
        return param.get_help_record(ctx)
    original = param.help
    param.help = t(original) if original else original
    try:
        return param.get_help_record(ctx)
    finally:
        param.help = original


class _Help(click.Command):
    """What a translated command adds to click: sections of options and commands."""

    def description(self, ctx: click.Context) -> str:  # noqa: ARG002 - for overrides
        return t(self.help) if self.help else ""

    def option_sections(self, ctx: click.Context) -> list[Section]:
        sections: dict[str, list[Entry]] = {}
        for param in self.get_params(ctx):
            record = _record(param, ctx)
            if record is None:
                continue
            section = getattr(param, "section", "help.options")
            sections.setdefault(section, []).append(record)
        return [(t(name), entries) for name, entries in sections.items()]

    def command_sections(self, ctx: click.Context) -> list[Section]:  # noqa: ARG002 - for overrides
        return []

    def get_help_option(self, ctx: click.Context) -> click.Option | None:
        found = super().get_help_option(ctx)
        if isinstance(found, click.Option):
            found.help = "opt.help"
        return found

    def format_help_text(self, ctx: click.Context, formatter: click.HelpFormatter) -> None:
        text = self.description(ctx)
        if text:
            formatter.write_paragraph()
            with formatter.indentation():
                for line in text.splitlines():
                    formatter.write_text(line) if line else formatter.write_paragraph()

    def format_options(self, ctx: click.Context, formatter: click.HelpFormatter) -> None:
        _write_sections(formatter, self.option_sections(ctx))
        _write_sections(formatter, self.command_sections(ctx))

    def get_help(self, ctx: click.Context) -> str:
        if rich_wanted():
            return _rich_help(self, ctx)
        return super().get_help(ctx)


def _write_sections(formatter: click.HelpFormatter, sections: Sequence[Section]) -> None:
    for title, entries in sections:
        if not entries:
            continue
        with formatter.section(title):
            described = [entry for entry in entries if entry[1]]
            bare = [name for name, text in entries if not text]
            if described:
                formatter.write_dl(described)
            if bare:
                formatter.write_text(", ".join(bare))


class TCommand(_Help):
    """A command whose help is translated."""


class TGroup(_Help, click.Group):
    """A group whose help is translated; its commands in one section by default."""

    command_class = TCommand

    def collect_usage_pieces(self, ctx: click.Context) -> list[str]:
        return [*click.Command.collect_usage_pieces(self, ctx), t("help.subcommand")]

    def command_sections(self, ctx: click.Context) -> list[Section]:
        entries: list[Entry] = []
        for name in self.list_commands(ctx):
            found = self.get_command(ctx, name)
            if found is None or found.hidden:
                continue
            entries.append((name, t(found.get_short_help_str(limit=90))))
        return [(t("help.commands"), entries)]

    def format_commands(self, ctx: click.Context, formatter: click.HelpFormatter) -> None:
        """Written with the options, by sections."""


TGroup.group_class = TGroup


def _rich_help(command: _Help, ctx: click.Context) -> str:
    """The same help, rendered by rich-click on a mirror of the command."""
    from rich_click import (  # noqa: PLC0415 - only for --help on a terminal
        RichCommand,
        RichContext,
        RichGroup,
        RichHelpConfiguration,
    )

    params: list[click.Parameter] = []
    by_section: dict[str, list[str]] = {}
    for param in command.get_params(ctx):
        mirror: click.Parameter = copy.copy(param)
        if isinstance(mirror, click.Option):
            mirror.help = t(mirror.help) if mirror.help else mirror.help
            title = t(getattr(param, "section", "help.options"))
            by_section.setdefault(title, []).append(mirror.opts[0])
        params.append(mirror)
    option_groups: Any = [{"name": k, "options": v} for k, v in by_section.items()]
    commands: dict[str, click.Command] = {}
    command_groups: Any = []
    for title, entries in command.command_sections(ctx):
        for name, text in entries:
            commands[name] = click.Command(name, help=text or None)
        command_groups.append({"name": title, "commands": [name for name, _ in entries]})
    help_text = command.description(ctx)
    mirror_command: click.Command = (
        RichGroup(name=ctx.info_name, commands=commands, params=params, help=help_text)
        if commands
        else RichCommand(name=ctx.info_name, params=params, help=help_text)
    )
    path = ctx.command_path
    config = RichHelpConfiguration(
        command_groups={path: command_groups},
        option_groups={path: option_groups},
        text_markup="ansi",
    )
    rich_ctx = RichContext(
        mirror_command, info_name=ctx.info_name, parent=ctx.parent, rich_help_config=config
    )
    return rich_ctx.get_help()


def command(name: str | None = None, **attrs: Any) -> Callable[[Callable[..., Any]], TCommand]:  # noqa: ANN401
    attrs.setdefault("cls", TCommand)
    return click.command(name, **attrs)


def group(name: str | None = None, **attrs: Any) -> Callable[[Callable[..., Any]], TGroup]:  # noqa: ANN401
    attrs.setdefault("cls", TGroup)
    return click.group(name, **attrs)


def option(*decls: str, **attrs: Any) -> Callable[[Callable[..., Any]], Callable[..., Any]]:  # noqa: ANN401
    attrs.setdefault("cls", Option)
    return click.option(*decls, **attrs)
