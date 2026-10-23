"""Private files of the library (REQ-023-40): directories in 0700, files in 0600, written
atomically (a temporary file in the same directory, then renamed)."""

from __future__ import annotations

import contextlib
import os
import tempfile
from pathlib import Path

__all__ = ["private_dir", "write_private"]


def private_dir(path: Path) -> Path:
    """`path` and its missing parents, each created in 0700."""
    missing: list[Path] = []
    current = path
    while not current.exists():
        missing.append(current)
        current = current.parent
    for folder in reversed(missing):
        with contextlib.suppress(FileExistsError):
            folder.mkdir(mode=0o700)
    return path


def write_private(path: Path, data: str | bytes) -> None:
    """Write `path` in 0600, never half-written."""
    private_dir(path.parent)
    descriptor, temporary = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.")
    try:
        with os.fdopen(descriptor, "wb") as file:
            file.write(data.encode() if isinstance(data, str) else data)
            file.flush()
            os.fsync(file.fileno())
        Path(temporary).replace(path)
    except BaseException:
        Path(temporary).unlink(missing_ok=True)
        raise
