"""`kartenn <resource> edit REF` (REQ-023-29; T17; maquette § 9).

The object is written in YAML to a file in 0600, in a private directory, and opened in
`$VISUAL` or `$EDITOR`; the members the update does not take are comments. The
difference becomes a Merge Patch (RFC 7396) sent under the `If-Match` of the read. On a
412, the editor opens again on the current version, the changes of the user in comments.
The file is removed in every case.
"""

from __future__ import annotations

import shutil
import tempfile
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import Option
from kartenn_cli.errors import CliError
from kartenn_cli.i18n import t
from kartenn_cli.tree import VerbCommand

if TYPE_CHECKING:
    from collections.abc import Mapping

    from kartenn import Record
    from kartenn_cli.runtime import Runtime

__all__ = ["EditCommand", "editable_members", "merge_patch", "summary"]

_ABSENT = object()


def merge_patch(before: Mapping[str, Any], after: Mapping[str, Any]) -> dict[str, Any]:
    """The Merge Patch (RFC 7396) that turns `before` into `after`."""
    patch: dict[str, Any] = {}
    for key, value in after.items():
        old = before.get(key, _ABSENT)
        if old == value:
            continue
        if isinstance(old, dict) and isinstance(value, dict):
            nested = merge_patch(cast("dict[str, Any]", old), cast("dict[str, Any]", value))
            if nested:
                patch[key] = nested
        else:
            patch[key] = value
    for key in before:
        if key not in after:
            patch[key] = None
    return patch


def _short(value: object) -> str:
    if value is None:
        return "—"
    if isinstance(value, dict | list):
        import json  # noqa: PLC0415

        return json.dumps(value, ensure_ascii=False)
    return str(value)


def summary(before: Mapping[str, Any], patch: Mapping[str, Any], prefix: str = "") -> list[str]:
    """`name: lot3 → lot3-prod` per member changed."""
    lines: list[str] = []
    for key, given in patch.items():
        old: object = before.get(key)
        value: object = given
        if isinstance(value, dict) and isinstance(old, dict):
            nested = cast("dict[str, Any]", value)
            earlier = cast("dict[str, Any]", old)
            lines += summary(earlier, nested, f"{prefix}{key}.")
        else:
            lines.append(f"{prefix}{key}: {_short(before.get(key))} → {_short(given)}")
    return lines


def editable_members(command: VerbCommand) -> set[str]:
    """The members the update takes (its body schema), but the reserved ones."""
    from kartenn_cli.writes import RESERVED, resolve_schema  # noqa: PLC0415

    update = command.resource.verb("update")
    if update is None:
        return set()
    body = command.catalog.operation(update.operation_id).body
    properties = resolve_schema(command.catalog, body).get("properties")
    return set(cast("dict[str, Any]", properties or {})) - RESERVED


def _document(
    record: Mapping[str, Any], editable: set[str], comment: str = ""
) -> tuple[str, dict[str, Any]]:
    import yaml  # noqa: PLC0415

    kept = {k: v for k, v in record.items() if k in editable}
    fixed = {k: v for k, v in record.items() if k not in editable}
    lines = [f"# {t('edit.fixed')}"]
    lines += [
        f"# {line}"
        for line in yaml.safe_dump(fixed, allow_unicode=True, sort_keys=False).splitlines()
    ]
    if comment:
        lines += [f"# {t('edit.yours')}"]
        lines += [f"# {line}" for line in comment.splitlines()]
    body = yaml.safe_dump(kept, allow_unicode=True, sort_keys=False) if kept else ""
    return "\n".join(lines) + "\n" + body, kept


def _edit(text: str) -> str:
    """The text after the editor, through a file in 0600 in a private directory, removed
    whatever happens."""
    directory = Path(tempfile.mkdtemp(prefix="kartenn-edit-"))
    try:
        path = directory / "object.yaml"
        path.write_text(text, "utf-8")
        path.chmod(0o600)
        click.edit(filename=str(path))
        return path.read_text("utf-8")
    finally:
        shutil.rmtree(directory, ignore_errors=True)


class EditCommand(VerbCommand):
    """`edit REF`: the object in the editor, then a Merge Patch under `If-Match`."""

    def build_params(self) -> list[click.Parameter]:
        from kartenn_cli.commands import path_arguments  # noqa: PLC0415

        write = "help.section.write"
        return [
            *path_arguments(self),
            Option(["--motive"], help="opt.motive", section=write),
            Option(["--valid-from"], metavar="DATE", help="opt.valid_from", section=write),
        ]

    def description(self, ctx: click.Context) -> str:  # noqa: ARG002
        return t("edit.help", path=self.operation.path, operation=self.operation.operation_id)

    def get_short_help_str(self, limit: int = 45) -> str:
        return t("edit.short")[:limit]

    def invoke(self, ctx: click.Context) -> Any:  # noqa: ANN401
        from kartenn import PreconditionFailed  # noqa: PLC0415
        from kartenn_cli.reads import resolve_ref  # noqa: PLC0415
        from kartenn_cli.runtime import runtime  # noqa: PLC0415
        from kartenn_cli.writes import motive_of, render_outcome  # noqa: PLC0415

        found = runtime(ctx)
        found.require_input(t("edit.editor"))
        identifier = resolve_ref(
            found.client,
            self.catalog,
            self.resource,
            ctx.params[self.operation.path_params[0].lower()],
        )
        editable = editable_members(self)
        record = found.client.resource(self.resource.name).get(identifier)
        comment = ""
        while True:
            text, before = _document(record, editable, comment)
            after = self.read_back(_edit(text))
            patch = merge_patch(before, after)
            if not patch:
                click.echo(t("edit.nothing"))
                return
            click.echo(t("edit.changes", changes=" ; ".join(summary(before, patch))))
            motive = motive_of(ctx, found)
            try:
                result = self.send(
                    found, identifier, record, patch, motive, ctx.params.get("valid_from")
                )
            except PreconditionFailed:
                click.echo(t("edit.stale"), err=True)
                if not click.confirm(t("edit.reopen"), default=True):
                    raise
                comment = _changes_text(after)
                record = found.client.resource(self.resource.name).get(identifier)
                continue
            render_outcome(found, result, output=None, require_applied=False)
            return

    @staticmethod
    def read_back(text: str) -> dict[str, Any]:
        import yaml  # noqa: PLC0415

        try:
            data = yaml.safe_load(text)
        except yaml.YAMLError as error:
            raise CliError("client.invalid-document", t("edit.invalid", error=error)) from error
        if data is None:
            return {}
        if not isinstance(data, dict):
            raise CliError("client.invalid-document", t("edit.invalid", error=type(data).__name__))
        return cast("dict[str, Any]", data)

    def send(  # noqa: PLR0913, PLR0917 - what a Merge Patch carries
        self,
        found: Runtime,
        identifier: str,
        record: Record,
        patch: dict[str, Any],
        motive: str | None,
        valid_from: str | None,
    ) -> object:
        from datetime import UTC, datetime  # noqa: PLC0415

        from kartenn_cli.reads import resolve_date_text  # noqa: PLC0415

        update = self.resource.verb("update")
        if update is None:  # pragma: no cover - `edit` exists with `update` only
            raise CliError("client.operation-unavailable", self.resource.name)
        operation = self.catalog.operation(update.operation_id)
        content: dict[str, Any] = {**patch, "motive": motive}
        when = resolve_date_text(valid_from, found, datetime.now(UTC).replace(microsecond=0))
        if when:
            content["valid_from"] = when
        params: dict[str, Any] = {operation.path_params[0]: identifier}
        return found.client.call(operation.operation_id, body=content, etag=record.etag, **params)


def _changes_text(after: Mapping[str, Any]) -> str:
    import yaml  # noqa: PLC0415

    return yaml.safe_dump(dict(after), allow_unicode=True, sort_keys=False)
