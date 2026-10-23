"""Completion for bash, zsh and fish (REQ-023-24; T14; maquette § 5).

- commands, options and the values of the enumerations come from the catalog in the
  cache: no request;
- property keys come from the capabilities in the cache;
- the slugs of references (`--domain`, `--entity`, `--service`…) and the values of
  closed lists (`--prop key=<TAB>`) are read on the server, kept 5 minutes in
  `$XDG_CACHE_HOME/kartenn/complete/`, never a masked value;
- a completion never waits more than a second: past it, only the local proposals stay.
  Nothing is ever written on the standard error.
"""

from __future__ import annotations

import contextlib
import hashlib
import json
import os
import threading
import time
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import click
from click.shell_completion import CompletionItem, get_completion_class

from kartenn_cli.base import command

if TYPE_CHECKING:
    from collections.abc import Callable

    from kartenn import Client
    from kartenn.catalog import Catalog

__all__ = [
    "complete_prop",
    "completion_command",
    "reference_completer",
    "reference_of",
    "server_values",
]

CACHE_SECONDS = 300.0
DEADLINE_SECONDS = 0.8
SHELLS = ("bash", "zsh", "fish")
Values = list[tuple[str, str]]


def _directory() -> Path:
    base = os.environ.get("XDG_CACHE_HOME") or str(Path.home() / ".cache")
    return Path(base) / "kartenn" / "complete"


class ValueCache:
    """Values read on a server, by name, for five minutes; a file in 0600 per server."""

    def __init__(self, url: str, *, clock: Callable[[], float] = time.time) -> None:
        name = hashlib.sha256(url.encode()).hexdigest()[:32]
        self.path = _directory() / f"{name}.json"
        self._clock = clock

    def _read(self) -> dict[str, Any]:
        try:
            data = json.loads(self.path.read_text())
        except (OSError, ValueError):
            return {}
        return cast("dict[str, Any]", data) if isinstance(data, dict) else {}

    def get(self, name: str) -> Values | None:
        entry = self._read().get(name)
        if not isinstance(entry, dict):
            return None
        found = cast("dict[str, Any]", entry)
        at = found.get("at")
        if not isinstance(at, int | float) or self._clock() - at >= CACHE_SECONDS:
            return None
        return [(str(v), str(h)) for v, h in cast("list[list[object]]", found.get("values", []))]

    def set(self, name: str, values: Values) -> None:
        from kartenn._files import write_private  # noqa: PLC0415 - 0600, atomic

        data = self._read()
        data[name] = {"at": self._clock(), "values": [list(pair) for pair in values]}
        write_private(self.path, json.dumps(data, separators=(",", ":")))


def server_values(ctx: click.Context, name: str, fetch: Callable[[Client], Values]) -> Values:
    """Values read on the server through `fetch`, from the cache when fresh; nothing past
    the deadline or on any failure."""
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    found = runtime(ctx)
    profile = found.profile_or_none()
    if profile is None:
        return []
    cache = ValueCache(profile.url.rstrip("/"))
    kept = cache.get(name)
    if kept is not None:
        return kept
    result: dict[str, Values] = {}

    def work() -> None:
        with contextlib.suppress(Exception):  # a completion fails silently
            result["values"] = fetch(found.client)

    worker = threading.Thread(target=work, daemon=True)
    worker.start()
    worker.join(DEADLINE_SECONDS)
    values = result.get("values")
    if values is None:
        return []
    cache.set(name, values)
    return values


def _matching(values: Values, incomplete: str, prefix: str = "") -> list[CompletionItem]:
    return [
        CompletionItem(f"{prefix}{value}", help=text or None)
        for value, text in values
        if value.startswith(incomplete)
    ]


def _shown(item: dict[str, Any]) -> bool:
    return item.get("masked") is not True


def _label(item: dict[str, Any]) -> str:
    for name in ("label", "name", "title"):
        found = item.get(name)
        if isinstance(found, str) and found:
            return found
    return ""


def reference_of(name: str, catalog: Catalog, module: str | None = None) -> str | None:
    """The collection a filter names (`domain` → `domains`, `flow_type` → `flow-types`);
    in a module, its own collection first (`group` of `vlan` → `vlan-groups`)."""
    base = name.replace("_", "-")
    candidates = [f"{base}s", f"{base}es", base]
    if base.endswith("y"):
        candidates.insert(0, f"{base[:-1]}ies")
    if module:
        candidates = [f"{module}-{c}" for c in candidates] + candidates
    return next((c for c in candidates if catalog.has_resource(c)), None)


def _references(collection: str) -> Callable[[Client], Values]:
    def fetch(client: Client) -> Values:
        page = client.resource(collection).list(page_size=200).first_page()
        values: Values = []
        for raw in page.items:
            item = dict(raw)
            key = item.get("slug") or item.get("key")
            if isinstance(key, str) and _shown(item):
                values.append((key, _label(item)))
        return values

    return fetch


def reference_completer(
    collection: str,
) -> Callable[[click.Context, click.Parameter, str], list[CompletionItem]]:
    """The completion of an option that names objects of `collection` by their slug."""

    def complete(
        ctx: click.Context, param: click.Parameter, incomplete: str
    ) -> list[CompletionItem]:
        del param
        values = server_values(ctx, f"ref:{collection}", _references(collection))
        return _matching(values, incomplete)

    return complete


def _property_keys(ctx: click.Context) -> list[str]:
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    try:
        catalog = runtime(ctx).cached_catalog()
    except Exception:  # noqa: BLE001 - a completion fails silently
        return []
    if catalog is None:
        return []
    declared = cast("list[object]", catalog.capabilities.get("properties") or [])
    keys = [cast("dict[str, Any]", d).get("key") for d in declared if isinstance(d, dict)]
    return sorted(k for k in keys if isinstance(k, str))


def _closed_values(key: str) -> Callable[[Client], Values]:
    def fetch(client: Client) -> Values:
        listing = client.resource("property-declarations").pages_of("values", key)
        page = listing.first_page()
        values: Values = []
        for raw in page.items:
            item = dict(raw)
            value = item.get("key")
            if isinstance(value, str) and _shown(item):
                values.append((value, _label(item)))
        return values

    return fetch


def complete_prop(
    ctx: click.Context, param: click.Parameter, incomplete: str
) -> list[CompletionItem]:
    """`--prop <TAB>`: the keys declared; `--prop key=<TAB>`: the values of a closed list."""
    del param
    left, sep, value = incomplete.partition("=")
    if not sep:
        return [CompletionItem(f"{key}=") for key in _property_keys(ctx) if key.startswith(left)]
    key = left.partition(":")[0]
    values = server_values(ctx, f"prop:{key}", _closed_values(key))
    return _matching(values, value, prefix=f"{left}=")


@command(name="completion", help="completion.help")
@click.argument("shell", type=click.Choice(SHELLS))
def completion_command(shell: str) -> None:
    """The completion script of a shell, to source from its configuration."""
    found = get_completion_class(shell)
    if found is None:  # pragma: no cover - the choice holds known shells only
        raise click.BadParameter(shell)
    from kartenn_cli.main import cli  # noqa: PLC0415

    click.echo(found(cli, {}, "kartenn", "_KARTENN_COMPLETE").source())
