"""Writes (REQ-023-28, 30, 31; T17; maquette § 8, 13).

- `create` and `update` take the members typed from the schema of the contract, or
  `--set path=value`, or a document (`--from-file`, `-` for the standard input);
- `update` sends a Merge Patch (RFC 7396) under the `If-Match` of a read; `close` needs a
  motive;
- `--dry-run` simulates the same write as a changeset (`?simulate=true`): nothing is
  written, the exit code is 8 when it would be refused;
- `--into-changeset` adds the write to the local changeset (`kartenn changeset new`);
- a write held for a decision ends with 0, or 10 with `--require-applied`.
"""

from __future__ import annotations

import json
import sys
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import Option
from kartenn_cli.errors import DRY_RUN_REFUSED, PENDING, USAGE, CliError
from kartenn_cli.i18n import t
from kartenn_cli.tree import VerbCommand, first_line

if TYPE_CHECKING:
    from collections.abc import Mapping

    from kartenn import Applied, Pending
    from kartenn.catalog import Catalog, Resource
    from kartenn_cli.runtime import Runtime

__all__ = [
    "CloseCommand",
    "CreateCommand",
    "UpdateCommand",
    "body_members",
    "changeset_operation",
    "parse_value",
    "render_outcome",
    "write_params",
]

RESERVED = frozenset({"motive", "valid_from", "valid_to", "local", "expected", "op"})
_SCALARS = {
    "string": click.STRING,
    "integer": click.INT,
    "number": click.FLOAT,
    "boolean": click.BOOL,
}
MEMBER = "member__"


# Schemas ------------------------------------------------------------------------------


def _as_dict(value: object) -> dict[str, Any]:
    return cast("dict[str, Any]", value) if isinstance(value, dict) else {}


def resolve_schema(catalog: Catalog, schema: object, depth: int = 6) -> dict[str, Any]:
    """A schema with its references followed and its `allOf` merged."""
    found = _as_dict(schema)
    while isinstance(found.get("$ref"), str) and depth > 0:
        found = _as_dict(catalog.schemas.get(str(found["$ref"]).rsplit("/", 1)[-1]))
        depth -= 1
    parts = found.get("allOf")
    if not isinstance(parts, list) or depth <= 0:
        return found
    merged: dict[str, Any] = {"properties": {}, "required": []}
    for part in cast("list[object]", parts):
        resolved = resolve_schema(catalog, part, depth - 1)
        merged["properties"].update(_as_dict(resolved.get("properties")))
        merged["required"] += list(cast("list[str]", resolved.get("required") or []))
    return merged


def _kind(catalog: Catalog, schema: object) -> tuple[str, tuple[str, ...]]:
    """`scalar` (with its type), `enum` (with its values), `reference`, or `other`."""
    raw = _as_dict(schema)
    options = raw.get("oneOf") or raw.get("anyOf")
    if isinstance(options, list):
        kept = [o for o in cast("list[object]", options) if _as_dict(o).get("type") != "null"]
        if len(kept) == 1:
            raw = _as_dict(kept[0])
    found = resolve_schema(catalog, raw)
    enum = found.get("enum")
    if isinstance(enum, list):
        return "enum", tuple(str(v) for v in cast("list[object]", enum))
    kind = found.get("type")
    if isinstance(kind, list):
        kind = next((k for k in cast("list[str]", kind) if k != "null"), None)
    properties = _as_dict(found.get("properties"))
    if kind == "object" and set(properties) == {"id"}:
        return "reference", ()
    if isinstance(kind, str) and kind in _SCALARS:
        return "scalar", (kind,)
    return "other", ()


def body_members(catalog: Catalog, schema: object) -> dict[str, tuple[str, tuple[str, ...], str]]:
    """The members of a body an option can carry: kind, details, description."""
    resolved = resolve_schema(catalog, schema)
    members: dict[str, tuple[str, tuple[str, ...], str]] = {}
    for name, raw in _as_dict(resolved.get("properties")).items():
        if name in RESERVED:
            continue
        kind, details = _kind(catalog, raw)
        if kind != "other":
            members[name] = (kind, details, first_line(str(_as_dict(raw).get("description", ""))))
    return members


def parse_value(text: str) -> object:
    """A value of `--set`: JSON when it reads as JSON (`140`, `true`, `{...}`), else text."""
    try:
        return json.loads(text)
    except ValueError:
        return text


def _set_path(body: dict[str, Any], path: str, value: object) -> None:
    keys = path.split(".")
    target = body
    for key in keys[:-1]:
        nested = target.get(key)
        if not isinstance(nested, dict):
            nested = target[key] = {}
        target = cast("dict[str, Any]", nested)
    target[keys[-1]] = value


def read_document(path: str) -> str:
    """The text of `--from-file` (`-`: the standard input); a file that cannot be read is
    a usage error, not a trace."""
    if path == "-":
        return sys.stdin.read()
    try:
        with open(path, encoding="utf-8") as file:  # noqa: PTH123 - a path given as is
            return file.read()
    except (OSError, UnicodeDecodeError) as error:
        reason = getattr(error, "strerror", None) or str(error)
        message = t("write.unreadable", path=path, reason=reason)
        raise CliError("client.unreadable-file", message) from error


def _document(path: str) -> dict[str, Any]:
    import yaml  # noqa: PLC0415 - YAML is a superset of JSON

    data = yaml.safe_load(read_document(path))
    if not isinstance(data, dict):
        raise CliError("client.invalid-document", t("write.document", path=path))
    return cast("dict[str, Any]", data)


# Parameters ---------------------------------------------------------------------------


def write_params(
    command: VerbCommand, *, members: bool = True, reserved: bool = True
) -> list[click.Parameter]:
    """Members typed from the schema, `--set`, `--from-file`, the reserved members and the
    options of a changeset."""
    from kartenn_cli.commands import path_arguments  # noqa: PLC0415

    params = path_arguments(command)
    section = "help.section.members"
    if members and command.operation.body is not None:
        found = body_members(command.catalog, command.operation.body)
        for name, (kind, details, text) in found.items():
            if name.replace("_", "-") in _OWNED:
                continue
            kind_type: click.ParamType[Any, Any] | None = None
            if kind == "enum":
                kind_type = click.Choice(details)
            elif kind == "scalar":
                kind_type = _SCALARS[details[0]]
            params.append(
                Option(
                    [f"--{name.replace('_', '-')}", f"{MEMBER}{name}"],
                    type=kind_type,
                    metavar="REF" if kind == "reference" else None,
                    help=text or None,
                    section=section,
                )
            )
        params += [
            Option(
                ["--set", "assignments"],
                multiple=True,
                metavar="metavar.assignment",
                help="opt.set",
                section=section,
            ),
            Option(["--from-file"], metavar="metavar.file", help="opt.from_file", section=section),
        ]
    write = "help.section.write"
    if reserved:
        params += [
            Option(["--motive"], help="opt.motive", section=write),
            Option(["--valid-from"], metavar="DATE", help="opt.valid_from", section=write),
            Option(["--on-behalf-of"], metavar="ID", help="opt.on_behalf_of", section=write),
        ]
    params += [
        Option(["--dry-run"], is_flag=True, help="opt.dry_run", section=write),
        Option(["--into-changeset"], is_flag=True, help="opt.into_changeset", section=write),
        Option(["--require-applied"], is_flag=True, help="opt.require_applied", section=write),
        Option(
            ["-o", "--output"],
            type=click.Choice(("table", "json", "yaml")),
            metavar="FORMAT",
            help="opt.output",
            section="help.section.output",
        ),
    ]
    return params


_OWNED = frozenset(
    {
        "set",
        "from-file",
        "motive",
        "valid-from",
        "valid-to",
        "on-behalf-of",
        "dry-run",
        "into-changeset",
        "require-applied",
        "output",
    }
)


def gather_body(command: VerbCommand, ctx: click.Context, found: Runtime) -> dict[str, Any]:
    """The body of a write: the document, then the typed members, then `--set`."""
    from kartenn_cli.complete import reference_of  # noqa: PLC0415
    from kartenn_cli.reads import resolve_ref  # noqa: PLC0415

    path = ctx.params.get("from_file")
    body = _document(path) if path else {}
    members = (
        body_members(command.catalog, command.operation.body) if command.operation.body else {}
    )
    for name, (kind, _, _) in members.items():
        value = ctx.params.get(f"{MEMBER}{name}")
        if value is None:
            continue
        if kind == "reference":
            own = command.resource
            module = own.module if command.catalog.is_module(own) else None
            collection = reference_of(name, command.catalog, module)
            if collection is None:
                body[name] = {"id": value}
                continue
            resource = command.catalog.resource(collection)
            body[name] = {"id": resolve_ref(found.client, command.catalog, resource, value)}
        else:
            body[name] = value
    for assignment in ctx.params.get("assignments") or ():
        left, sep, right = str(assignment).partition("=")
        if not sep or not left:
            raise CliError("client.invalid-assignment", t("write.assignment", given=assignment))
        _set_path(body, left, parse_value(right))
    return body


# Changeset operations -----------------------------------------------------------------


def _singular(resource: Resource) -> str | None:
    create = resource.verb("create")
    if create is None or not create.operation_id.startswith("create_"):
        return None
    return create.operation_id.removeprefix("create_")


def changeset_operation(  # noqa: PLR0913, PLR0917 - a write, whole
    catalog: Catalog,
    resource: Resource,
    verb: str,
    operation_id: str,
    members: Mapping[str, Any],
    identifier: str | None = None,
) -> dict[str, Any]:
    """The operation of a changeset that a write of a resource stands for.

    The contract names no such correspondence: the core keeps the operations of its
    schema `Operation` (`create_<x>`, `update_<x>`, `close` of a `kind`), a module its
    own `<module>.<operation>`. A write without one is refused (`client.dry-run-unavailable`).
    """
    from kartenn.changesets import operation_kinds  # noqa: PLC0415

    kinds = operation_kinds(catalog)
    module = resource.module if catalog.is_module(resource) else None
    singular = _singular(resource)
    reference = {} if identifier is None else {"id": {"id": identifier}}
    if verb == "close":
        kind = f"{module}.{singular}" if module else singular
        if singular is not None and _target_kind(catalog, kind):
            return {"op": "close", "kind": kind, **reference}
    elif module is not None:
        return {"op": f"{module}.{operation_id}", **reference, **members}
    else:
        op = f"{verb}_{singular}" if singular else None
        if op is not None and op in kinds:
            return {"op": op, **reference, **members}
    raise CliError(
        "client.dry-run-unavailable", t("write.no_operation", resource=resource.name, verb=verb)
    )


def _target_kind(catalog: Catalog, kind: str | None) -> bool:
    enum = _as_dict(catalog.schemas.get("TargetKind")).get("enum")
    return kind is not None and isinstance(enum, list) and kind in cast("list[object]", enum)


# Outcomes -----------------------------------------------------------------------------


def render_outcome(
    found: Runtime, result: object, *, output: str | None, require_applied: bool
) -> None:
    """The answer of a write; exit 10 when it is held and `--require-applied`."""
    from kartenn import Applied, Pending  # noqa: PLC0415

    fmt = found.asked_output(output) or "table"
    if fmt != "table":
        body: object = result.body if isinstance(result, Applied | Pending) else result
        found.output(output=output).value(
            dict(cast("dict[str, Any]", body)) if isinstance(body, dict) else body
        )
    elif isinstance(result, Applied):
        _applied(found, result)
    elif isinstance(result, Pending):
        _pending(result)
    elif isinstance(result, dict):
        found.output(output=output).value(dict(cast("dict[str, Any]", result)))
    if isinstance(result, Pending) and require_applied:
        raise click.exceptions.Exit(PENDING)


def _applied(found: Runtime, result: Applied) -> None:
    from kartenn import Violation  # noqa: PLC0415
    from kartenn_cli.render.problems import render_violations  # noqa: PLC0415

    click.echo(t("write.applied", id=result.changeset or "—"))
    warnings = [
        Violation.parse(w) for w in cast("list[dict[str, Any]]", result.body.get("warnings") or [])
    ]
    for line in render_violations(warnings, color=found.color):
        click.echo(line)
    if result.location:
        click.echo(t("write.location", location=result.location))


def _pending(result: Pending) -> None:
    click.echo(t("write.pending", id=result.changeset or "—", kind=result.kind or "—"))
    # Spec 024 (REQ-024-13): asked to apply it, its author could not.
    if result.reason is not None:
        click.echo(t("write.not_applied", code=result.reason))
    click.echo(t("write.follow", id=result.changeset or "—"))


def motive_of(ctx: click.Context, found: Runtime, *, required: bool = True) -> str | None:
    """`--motive`, else asked on a terminal; refused without (`client.motive-required`)."""
    motive = ctx.params.get("motive")
    if motive or not required:
        return motive
    if not found.interactive:
        raise CliError("client.motive-required", t("write.motive_required"), exit_code=USAGE)
    return str(click.prompt(t("write.motive")))


def simulate(  # noqa: PLR0913 - the members of a changeset
    found: Runtime,
    operations: list[dict[str, Any]],
    *,
    motive: str,
    valid_from: str | None,
    on_behalf_of: str | None,
    valid_to: str | None = None,
    output: str | None = None,
) -> None:
    """Simulate a changeset of these operations; exit 8 when it would be refused."""
    from kartenn import Refused  # noqa: PLC0415
    from kartenn_cli.render.changesets import render_simulation  # noqa: PLC0415

    builder = found.client.changeset(
        motive=motive, valid_from=valid_from, valid_to=valid_to, on_behalf_of=on_behalf_of
    )
    for operation in operations:
        builder.add(operation)
    from kartenn_cli.render import format_moment  # noqa: PLC0415

    shown = format_moment(valid_from, found.zone)
    # On the standard error when a format is asked for: the output stays a document.
    click.echo(
        t("simulation.head") + (t("simulation.from", date=shown) if shown else ""),
        err=(found.asked_output(output) or "table") != "table",
    )
    try:
        simulation = builder.simulate()
    except Refused as error:
        from kartenn_cli.render.problems import render_error  # noqa: PLC0415

        click.echo(render_error(error, color=found.error_color), err=True, nl=False)
        raise click.exceptions.Exit(DRY_RUN_REFUSED) from error
    render_simulation(found, simulation, operations=len(operations), output=output)
    if simulation.blocking:
        raise click.exceptions.Exit(DRY_RUN_REFUSED)


def _date(found: Runtime, text: str | None) -> str | None:
    from datetime import UTC, datetime  # noqa: PLC0415

    from kartenn_cli.reads import resolve_date_text  # noqa: PLC0415

    return resolve_date_text(text, found, datetime.now(UTC).replace(microsecond=0))


def _headers(ctx: click.Context) -> dict[str, str] | None:
    on_behalf_of = ctx.params.get("on_behalf_of")
    return {"Kartenn-On-Behalf-Of": on_behalf_of} if on_behalf_of else None


# Commands -----------------------------------------------------------------------------


class _Write(VerbCommand):
    """What `create`, `update` and `close` share."""

    def build_params(self) -> list[click.Parameter]:
        return write_params(self, members=self.verb != "close")

    def identifier(self, ctx: click.Context, found: Runtime) -> str | None:
        from kartenn_cli.reads import resolve_ref  # noqa: PLC0415

        names = self.operation.path_params
        if not names:
            return None
        return resolve_ref(found.client, self.catalog, self.resource, ctx.params[names[0].lower()])

    def body(self, ctx: click.Context, found: Runtime) -> dict[str, Any]:
        return gather_body(self, ctx, found)

    def motive_required(self) -> bool:
        return True

    def invoke(self, ctx: click.Context) -> Any:  # noqa: ANN401
        from kartenn_cli.runtime import runtime  # noqa: PLC0415

        found = runtime(ctx)
        staged = bool(ctx.params.get("dry_run") or ctx.params.get("into_changeset"))
        motive = motive_of(ctx, found, required=self.motive_required() and not staged)
        valid_from = _date(found, ctx.params.get("valid_from"))
        identifier = self.identifier(ctx, found)
        members = self.body(ctx, found)
        if ctx.params.get("dry_run") or ctx.params.get("into_changeset"):
            operation = changeset_operation(
                self.catalog,
                self.resource,
                self.verb,
                self.operation.operation_id,
                members,
                identifier,
            )
            if ctx.params.get("into_changeset"):
                from kartenn_cli.changeset import add_operation  # noqa: PLC0415

                add_operation(operation, summary=self.summary(members))
                return
            simulate(
                found,
                [operation],
                motive=motive or t("simulation.motive"),
                valid_from=valid_from,
                on_behalf_of=ctx.params.get("on_behalf_of"),
                output=ctx.params.get("output"),
            )
            return
        content = {**members, "motive": motive} if motive else dict(members)
        if valid_from:
            content["valid_from"] = valid_from
        result = self.send(found, identifier, content, _headers(ctx))
        render_outcome(
            found,
            result,
            output=ctx.params.get("output"),
            require_applied=bool(ctx.params.get("require_applied")),
        )

    def summary(self, members: Mapping[str, Any]) -> str:
        shown: list[str] = []
        for name in self.resource.key:
            part: object = members.get(name)
            if isinstance(part, dict):
                part = cast("dict[str, Any]", part).get("id")
            if part is not None:
                shown.append(str(part))
        return "/".join(shown)

    def send(
        self,
        found: Runtime,
        identifier: str | None,
        content: dict[str, Any],
        headers: dict[str, str] | None,
    ) -> object:
        params = {} if identifier is None else {self.operation.path_params[0]: identifier}
        etag = None
        if self.operation.needs_if_match and identifier is not None:
            etag = found.client.resource(self.resource.name).get(identifier).etag
        return found.client.call(
            self.operation.operation_id, body=content, etag=etag, headers=headers, **params
        )


class CreateCommand(_Write):
    """`create`: a new object of the collection."""


class UpdateCommand(_Write):
    """`update REF`: a Merge Patch under the `If-Match` of a read."""


class CloseCommand(_Write):
    """`close REF --motive`: the end of the validity of an object (D33)."""

    def body(self, ctx: click.Context, found: Runtime) -> dict[str, Any]:  # noqa: ARG002
        return {}


class ActionCommand(_Write):
    """An action of the contract (`acknowledge`, `waive`, `attach`, `set`…): its body
    from the schema; a motive when the schema asks one."""

    def build_params(self) -> list[click.Parameter]:
        return write_params(self, reserved=self.takes_motive)

    @property
    def takes_motive(self) -> bool:
        body = self.operation.body
        if body is None:
            return False
        resolved = resolve_schema(self.catalog, body)
        return "motive" in _as_dict(resolved.get("properties"))

    def motive_required(self) -> bool:
        body = self.operation.body
        if body is None:
            return False
        required = cast("list[str]", resolve_schema(self.catalog, body).get("required") or [])
        return "motive" in required
