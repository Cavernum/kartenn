"""The `TokenStore` port (design § Authentification, REQ-023-36, 40): where tokens and
secrets live between two calls. The library offers a store in memory (the default) and
an explicit file in 0600; the CLI brings the keyring of the system.
"""

from __future__ import annotations

import json
import stat
from typing import TYPE_CHECKING, Protocol, cast

from kartenn._core.problems import ClientError
from kartenn._files import write_private

if TYPE_CHECKING:
    from pathlib import Path

__all__ = ["FileStore", "MemoryStore", "TokenStore"]


class TokenStore(Protocol):
    def get(self, key: str) -> str | None: ...

    def set(self, key: str, value: str) -> None: ...

    def delete(self, key: str) -> None: ...


class MemoryStore:
    """Tokens for the life of the process."""

    def __init__(self) -> None:
        self._values: dict[str, str] = {}

    def get(self, key: str) -> str | None:
        return self._values.get(key)

    def set(self, key: str, value: str) -> None:
        self._values[key] = value

    def delete(self, key: str) -> None:
        self._values.pop(key, None)


class FileStore:
    """Tokens in a JSON file named by the caller (`--token-file`), in 0600. A file that
    others may read is refused rather than used."""

    def __init__(self, path: Path) -> None:
        self.path = path

    def _read(self) -> dict[str, str]:
        try:
            mode = stat.S_IMODE(self.path.stat().st_mode)
        except FileNotFoundError:
            return {}
        if mode & 0o077:
            msg = f"{self.path} may be read by others (mode {mode:o}); chmod 600 it"
            raise ClientError("client.store-permissions", msg)
        try:
            data = json.loads(self.path.read_text())
        except ValueError as error:
            msg = f"{self.path} is not a store of tokens"
            raise ClientError("client.store-invalid", msg) from error
        if not isinstance(data, dict):
            msg = f"{self.path} is not a store of tokens"
            raise ClientError("client.store-invalid", msg)
        return {str(k): str(v) for k, v in cast("dict[object, object]", data).items()}

    def get(self, key: str) -> str | None:
        return self._read().get(key)

    def set(self, key: str, value: str) -> None:
        values = self._read()
        values[key] = value
        write_private(self.path, json.dumps(values))

    def delete(self, key: str) -> None:
        values = self._read()
        if values.pop(key, None) is not None:
            write_private(self.path, json.dumps(values))
