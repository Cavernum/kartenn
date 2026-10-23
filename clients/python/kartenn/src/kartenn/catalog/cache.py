"""The cache of catalogs (T20, REQ-023-2), under `$XDG_CACHE_HOME/kartenn/catalog/`.

- `<sha256(url)>-<revision>.json`: a derived catalog, compact JSON, written once;
- `<sha256(url)>.json`: the pointer of a server, its current revision, the capabilities
  last read and when (`checked`): they are read again at most once a minute.

Files in 0600 in directories in 0700, written atomically. A damaged or foreign file is
a missing one: the catalog is derived again.
"""

from __future__ import annotations

import hashlib
import json
import time
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import platformdirs

from kartenn._core.problems import ClientError
from kartenn._files import write_private
from kartenn.catalog.model import Catalog

if TYPE_CHECKING:
    from collections.abc import Callable

__all__ = ["CatalogCache"]

MAX_AGE_SECONDS = 60.0


def default_directory() -> Path:
    return Path(platformdirs.user_cache_dir("kartenn")) / "catalog"


class CatalogCache:
    def __init__(
        self,
        directory: Path | None = None,
        *,
        max_age: float = MAX_AGE_SECONDS,
        clock: Callable[[], float] = time.time,
    ) -> None:
        self.directory = directory or default_directory()
        self._max_age = max_age
        self._clock = clock

    def _key(self, server: str) -> str:
        return hashlib.sha256(server.encode()).hexdigest()[:32]

    def _pointer(self, server: str) -> dict[str, Any] | None:
        try:
            data = json.loads((self.directory / f"{self._key(server)}.json").read_text())
        except (OSError, ValueError):
            return None
        return cast("dict[str, Any]", data) if isinstance(data, dict) else None

    def path(self, server: str, revision: str) -> Path:
        return self.directory / f"{self._key(server)}-{revision}.json"

    def last(self, server: str) -> Catalog | None:
        """The last catalog of the server, however old, without the network."""
        pointer = self._pointer(server)
        if pointer is None:
            return None
        try:
            catalog = Catalog.from_json(self.path(server, str(pointer["revision"])).read_text())
        except (OSError, ValueError, KeyError, TypeError, ClientError):
            return None
        if catalog.server != server:
            return None
        capabilities = pointer.get("capabilities")
        if isinstance(capabilities, dict):
            catalog = catalog.with_capabilities(cast("dict[str, Any]", capabilities))
        return catalog

    def fresh(self, server: str) -> Catalog | None:
        """The catalog when its capabilities were read less than a minute ago."""
        pointer = self._pointer(server)
        checked = pointer.get("checked") if pointer else None
        if not isinstance(checked, int | float) or self._clock() - checked >= self._max_age:
            return None
        return self.last(server)

    def save(self, catalog: Catalog) -> None:
        """Keep a new catalog and point the server at it."""
        path = self.path(catalog.server, catalog.revision)
        if not path.exists():
            write_private(path, catalog.with_capabilities({}).to_json())
        self.touch(catalog)

    def touch(self, catalog: Catalog) -> None:
        """Record that the capabilities were read now, for the revision of `catalog`."""
        pointer = {
            "revision": catalog.revision,
            "checked": self._clock(),
            "capabilities": catalog.capabilities,
        }
        name = f"{self._key(catalog.server)}.json"
        write_private(self.directory / name, json.dumps(pointer, separators=(",", ":")))
