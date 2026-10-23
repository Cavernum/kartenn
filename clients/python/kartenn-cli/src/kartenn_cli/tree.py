"""The catalog → click commands (REQ-023-18, 21; T13; design § Nommage).

- the core and the base at the first level (`kartenn prefixes list`);
- a group per module served (`kartenn policy …`), its collections without the prefix of
  the module (`policy-rules` → `policy rules`); the collection named after the module,
  or its plural, puts its verbs on the group (`/v1/flows` → `kartenn flow list`);
- verbs: `list`, `get`, `create`, `update`, `close`, `history`, `properties`… and the
  sub-resources and actions of the contract, named after their path;
- the help of a command comes from the contract: summary, module, HTTP operation.

Only the command asked for is built (`get_command`): its module group, then its resource,
then its verb. Nothing here names a resource; a module served tomorrow is derived the
same way.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import Entry, Option, Section, TCommand, TGroup
from kartenn_cli.errors import USAGE, CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from kartenn.catalog import Catalog, Operation, Param, Resource, Verb
    from kartenn.profiles import Profile

__all__ = [
    "KNOWN_MODULES",
    "NAMED_RESOURCES",
    "Layout",
    "ModuleGroup",
    "ResourceGroup",
    "layout",
    "module_unavailable",
]

# Collections reached by named commands only (`changeset`, `import`).
NAMED_RESOURCES = frozenset({"changesets", "imports", "proposals"})
# The names of the first level that are not the collection's own.
TOP_ALIASES = {"audit-events": "audit"}
# Modules a server may lack: their commands answer `client.module-unavailable`, not
# "no such command". Sub-resources served by a module on a resource of another.
KNOWN_MODULES = frozenset({"policy", "flow", "vlan", "machines"})
SUBRESOURCE_MODULES = {"flows": "flow", "evaluations": "flow"}
# The section of the root help by tag of the contract.
SECTION_OF_TAG = {
    "core": "core",
    "properties": "core",
    "graph": "core",
    "rights": "rights",
    "audit": "rights",
    "operations": "tools",
}
# Names of actions as verbs; `None` hides an operation reached otherwise.
VERB_ALIASES: dict[str, str | None] = {
    "query": None,
    "chunks.put": None,
    "acknowledgement": "acknowledge",
    "validation": "approve",
    "rejection": "reject",
    "withdrawal": "withdraw",
    "waiver": "waive",
    "pseudonymisation": "pseudonymise",
    "attachments": "attach",
    "attachments.delete": "detach",
    "secrets": "secrets.add",
    "secrets.delete": "secrets.revoke",
    "properties": "properties.show",
}
# Query parameters the commands own: a filter of the same name is `--filter-<name>`.
OWNED = frozenset({"output", "fields", "query", "all", "ids", "cursor", "page-size", "at"})
_DATES = frozenset({"valid_at", "as_of"})
_PAGES = frozenset({"cursor", "page_size"})


@dataclass(frozen=True)
class Layout:
    """What the first level shows: resources by name, modules by name."""

    resources: dict[str, str] = field(default_factory=dict[str, str])
    modules: dict[str, dict[str, str]] = field(default_factory=dict[str, dict[str, str]])
    mains: dict[str, str] = field(default_factory=dict[str, str])
    sections: dict[str, str] = field(default_factory=dict[str, str])


def _child(module: str, name: str) -> str | None:
    """The name of a collection in its module group; `None` for its main collection."""
    if name in {module, f"{module}s", f"{module}es"}:
        return None
    return name.removeprefix(f"{module}-")


def layout(catalog: Catalog) -> Layout:
    found = Layout()
    for resource in catalog.resources:
        if resource.name in NAMED_RESOURCES:
            continue
        if catalog.is_module(resource):
            children = found.modules.setdefault(resource.module, {})
            child = _child(resource.module, resource.name)
            if child is None:
                found.mains[resource.module] = resource.name
            else:
                children[child] = resource.name
            continue
        name = TOP_ALIASES.get(resource.name, resource.name)
        found.resources[name] = resource.name
        found.sections[name] = SECTION_OF_TAG.get(resource.module, "others")
    return found


def module_unavailable(
    module: str, catalog: Catalog | None, profile: Profile, *, what: str = ""
) -> CliError:
    """The error of a command whose module the server does not serve (maquette § 12)."""
    served = ", ".join(catalog.served_modules) if catalog is not None else "—"
    version = catalog.version if catalog is not None else "?"
    head = (
        t("module.unavailable_sub", what=what, module=module)
        if what
        else t("module.unavailable", module=module)
    )
    return CliError(
        "client.module-unavailable",
        head,
        t("module.profile", profile=profile.name, url=profile.url, version=version),
        t("module.served", modules=served),
        t("module.no_update"),
        exit_code=USAGE,
    )


class Unavailable(click.Command):
    """A command whose module is absent: it answers its error, help included."""

    def __init__(self, name: str, error: CliError) -> None:
        super().__init__(
            name,
            add_help_option=False,
            context_settings={"ignore_unknown_options": True, "allow_extra_args": True},
        )
        self.error = error

    def parse_args(self, ctx: click.Context, args: list[str]) -> list[str]:  # noqa: ARG002
        raise self.error

    def invoke(self, ctx: click.Context) -> Any:  # noqa: ANN401, ARG002
        raise self.error


# Commands of verbs --------------------------------------------------------------------


def first_line(text: str) -> str:
    return text.strip().splitlines()[0] if text.strip() else ""


def _option_name(name: str) -> str:
    dashed = name.replace("_", "-")
    return f"filter-{dashed}" if dashed in OWNED else dashed


def _type(param: Param) -> click.ParamType[Any, Any] | None:
    if param.enum:
        return click.Choice(param.enum)
    if param.type == "integer":
        return click.INT
    if param.type == "number":
        return click.FLOAT
    if param.type == "boolean":
        return click.BOOL
    return None


def query_option(
    param: Param, *, section: str, multiple: bool = False, catalog: Catalog | None = None
) -> click.Option:
    """The option of a query parameter of the contract; the slugs of the objects it names
    complete from the server."""
    kind = _type(param)
    completer = None
    if catalog is not None and kind is None:
        from kartenn_cli.complete import reference_completer, reference_of  # noqa: PLC0415

        collection = reference_of(param.name, catalog)
        completer = reference_completer(collection) if collection else None
    return Option(
        [f"--{_option_name(param.name)}", param.name],
        type=kind,
        multiple=multiple or param.repeated,
        help=first_line(param.description) or None,
        section=section,
        metavar=None if kind is not None else param.name.upper().replace("_", "-"),
        shell_complete=completer,
    )


class VerbCommand(TCommand):
    """A verb of a resource, bound to its operation."""

    def __init__(
        self,
        name: str,
        *,
        resource: Resource,
        operation: Operation,
        catalog: Catalog,
    ) -> None:
        self.resource = resource
        self.operation = operation
        self.catalog = catalog
        self.verb = name
        super().__init__(name, params=self.build_params(), help=first_line(operation.summary))

    @property
    def operation_ids(self) -> tuple[str, ...]:
        return (self.operation.operation_id,)

    def build_params(self) -> list[click.Parameter]:
        from kartenn_cli.commands import verb_params  # noqa: PLC0415

        return verb_params(self)

    def description(self, ctx: click.Context) -> str:  # noqa: ARG002 - the contract says it
        operation = self.operation
        method = operation.method
        if method == "GET" and self.verb == "list" and self.resource.verb("query") is not None:
            method = "GET|QUERY"
        summary = first_line(operation.summary)
        where = t(
            "help.operation",
            method=method,
            path=operation.path,
            operation=operation.operation_id,
            module=operation.module,
        )
        return f"{summary}\n{where}" if summary else where

    def get_short_help_str(self, limit: int = 45) -> str:
        return first_line(self.operation.summary)[:limit].rstrip()

    def invoke(self, ctx: click.Context) -> Any:  # noqa: ANN401
        from kartenn_cli.commands import run_verb  # noqa: PLC0415

        return run_verb(self, ctx)


@dataclass(frozen=True)
class _Verbs:
    """The verbs of a resource as commands: direct, or nested (`properties explain`)."""

    direct: dict[str, Verb]
    nested: dict[str, dict[str, Verb]]


def _verbs(resource: Resource) -> _Verbs:
    direct: dict[str, Verb] = {}
    nested: dict[str, dict[str, Verb]] = {}
    for verb in resource.verbs:
        name = VERB_ALIASES.get(verb.name, verb.name)
        if name is None:
            continue
        name = name.replace("_", "-")
        if "." in name:
            head, _, tail = name.partition(".")
            nested.setdefault(head, {})[tail.replace(".", "-")] = verb
        else:
            direct[name] = verb
    return _Verbs(direct, nested)


def verb_command(name: str, resource: Resource, verb: Verb, catalog: Catalog) -> click.Command:
    from kartenn_cli.commands import command_class  # noqa: PLC0415

    cls = command_class(resource, verb, catalog)
    return cls(
        name, resource=resource, operation=catalog.operation(verb.operation_id), catalog=catalog
    )


class VerbsGroup(TGroup):
    """Nested verbs of a resource (`properties show|explain|suggest`); the first word
    may be left out when the group has `show` (`properties REF`)."""

    def __init__(
        self, name: str, *, resource: Resource, verbs: dict[str, Verb], catalog: Catalog
    ) -> None:
        super().__init__(name, help=t("help.nested", name=name))
        self.resource = resource
        self.verbs = verbs
        self.catalog = catalog

    def list_commands(self, ctx: click.Context) -> list[str]:  # noqa: ARG002 - click's
        return sorted(self.verbs)

    def get_command(self, ctx: click.Context, cmd_name: str) -> click.Command | None:  # noqa: ARG002
        verb = self.verbs.get(cmd_name)
        return None if verb is None else verb_command(cmd_name, self.resource, verb, self.catalog)

    def resolve_command(
        self, ctx: click.Context, args: list[str]
    ) -> tuple[str | None, click.Command | None, list[str]]:
        if args and args[0] not in self.verbs and "show" in self.verbs:
            args = ["show", *args]
        return super().resolve_command(ctx, args)


class ResourceGroup(TGroup):
    """The verbs of one resource (`kartenn prefixes …`)."""

    def __init__(self, name: str, *, resource: Resource, catalog: Catalog, **attrs: Any) -> None:  # noqa: ANN401
        self.resource = resource
        self.catalog = catalog
        self.found = _verbs(resource)
        super().__init__(name, **attrs)

    def description(self, ctx: click.Context) -> str:  # noqa: ARG002
        return t("help.resource", name=self.resource.name, module=self.resource.module)

    def get_short_help_str(self, limit: int = 45) -> str:  # noqa: ARG002
        return ""

    def verb_names(self) -> list[str]:
        from kartenn_cli.commands import extra_verbs  # noqa: PLC0415

        return sorted({*self.found.direct, *self.found.nested, *extra_verbs(self.resource)})

    def list_commands(self, ctx: click.Context) -> list[str]:  # noqa: ARG002
        return self.verb_names()

    def get_command(self, ctx: click.Context, cmd_name: str) -> click.Command | None:
        verb = self.found.direct.get(cmd_name)
        if verb is not None:
            return verb_command(cmd_name, self.resource, verb, self.catalog)
        nested = self.found.nested.get(cmd_name)
        if nested is not None:
            return VerbsGroup(cmd_name, resource=self.resource, verbs=nested, catalog=self.catalog)
        from kartenn_cli.commands import extra_command  # noqa: PLC0415

        extra = extra_command(cmd_name, self.resource, self.catalog)
        if extra is not None:
            return extra
        return self.unavailable(ctx, cmd_name)

    def unavailable(self, ctx: click.Context, cmd_name: str) -> click.Command | None:
        module = SUBRESOURCE_MODULES.get(cmd_name)
        if module is None or self.catalog.has(module):
            return None
        from kartenn_cli.runtime import runtime  # noqa: PLC0415

        profile = runtime(ctx).require_profile()
        what = f"{ctx.info_name} {cmd_name}"
        return Unavailable(cmd_name, module_unavailable(module, self.catalog, profile, what=what))


class ModuleGroup(ResourceGroup):
    """A module served: the verbs of its main collection and its other collections."""

    def __init__(
        self,
        name: str,
        *,
        catalog: Catalog,
        children: dict[str, str],
        main: str | None,
        named: dict[str, click.Command] | None = None,
    ) -> None:
        from kartenn_cli.named import replaced  # noqa: PLC0415

        self.replaced = replaced(name, catalog)
        self.children = {k: v for k, v in children.items() if k not in self.replaced}
        self.main_collection = main
        self.named = named or {}
        resource = catalog.resource(main) if main else _empty(name)
        super().__init__(name, resource=resource, catalog=catalog)

    def _version(self) -> str:
        return next((m.version for m in self.catalog.modules if m.name == self.name), "")

    def description(self, ctx: click.Context) -> str:
        from kartenn_cli.runtime import runtime  # noqa: PLC0415

        profile = runtime(ctx).profile_or_none()
        host = _host(profile.url) if profile is not None else "?"
        about = module_summary(str(self.name))
        return t("help.module", about=about, module=self.name, version=self._version(), host=host)

    def get_short_help_str(self, limit: int = 45) -> str:  # noqa: ARG002
        return f"{module_summary(str(self.name))}   [module {self.name} {self._version()}]"

    def list_commands(self, ctx: click.Context) -> list[str]:  # noqa: ARG002
        names = set(self.verb_names() if self.main_collection else []) | set(self.children)
        return sorted((names - self.replaced) | set(self.named))

    def get_command(self, ctx: click.Context, cmd_name: str) -> click.Command | None:
        named = self.named.get(cmd_name)
        if named is not None:
            return named
        child = self.children.get(cmd_name)
        if child is not None:
            return ResourceGroup(
                cmd_name, resource=self.catalog.resource(child), catalog=self.catalog
            )
        if self.main_collection is None or cmd_name in self.replaced:
            return None
        return super().get_command(ctx, cmd_name)

    def command_sections(self, ctx: click.Context) -> list[Section]:
        named: list[Entry] = []
        verbs: list[Entry] = []
        children: list[Entry] = []
        for name in self.list_commands(ctx):
            if name in self.named:
                named.append((name, t(self.named[name].get_short_help_str(limit=90))))
            elif name in self.children:
                children.append((name, ""))
            else:
                verbs.append((name, ""))
        return [
            (t("help.section.named"), named),
            (t("help.section.resources"), children),
            (t("help.section.verbs"), verbs),
        ]


def _empty(name: str) -> Resource:
    from kartenn.catalog import Resource  # noqa: PLC0415

    return Resource(name, name, f"/v1/{name}")


def _host(url: str) -> str:
    from urllib.parse import urlsplit  # noqa: PLC0415

    return urlsplit(url).netloc or url


def module_summary(name: str) -> str:
    key = f"module.about.{name}"
    found = t(key)
    return t("module.about_generic", name=name) if found == key else found


def resource_command(name: str, catalog: Catalog, found: Layout) -> click.Command | None:
    """A command of the first level from the catalog, or `None`."""
    resource = found.resources.get(name)
    if resource is not None:
        return ResourceGroup(name, resource=catalog.resource(resource), catalog=catalog)
    if name in found.modules or name in found.mains:
        from kartenn_cli.named import named_commands  # noqa: PLC0415

        return ModuleGroup(
            name,
            catalog=catalog,
            children=found.modules.get(name, {}),
            main=found.mains.get(name),
            named=named_commands(name, catalog),
        )
    return None


def root_entries(catalog: Catalog, found: Layout) -> dict[str, list[Entry]]:
    """The derived entries of the root help, by section."""
    sections: dict[str, list[Entry]] = {}
    for name in sorted(found.resources):
        sections.setdefault(found.sections[name], []).append((name, ""))
    for module in catalog.modules:
        if module.name in found.modules or module.name in found.mains:
            line = f"{module_summary(module.name)}   [module {module.name} {module.version}]"
            sections.setdefault("modules", []).append((module.name, line))
    return sections


def as_dict(value: object) -> dict[str, Any]:
    return cast("dict[str, Any]", value) if isinstance(value, dict) else {}
