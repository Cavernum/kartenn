"""Python client library of the kartenn API (spec 023).

The names below are loaded on first use (PEP 562): `import kartenn.catalog` or
`import kartenn.profiles` does not load the clients, the authentication or the metadata
of the distribution. The command line reads its catalog in the cache this way, within
the budgets of REQ-023-39.
"""

from __future__ import annotations

import importlib
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from kartenn._core.problems import (
        ApiError,
        BadRequest,
        ClientError,
        Conflict,
        Forbidden,
        KartennError,
        NetworkError,
        NotFound,
        PreconditionFailed,
        PreconditionRequired,
        RateLimited,
        Refused,
        ServerError,
        Unauthenticated,
        Unavailable,
        Violation,
    )
    from kartenn._core.requests import between, exists
    from kartenn.auth import FileStore, MemoryStore, TokenStore
    from kartenn.catalog import UnknownResource
    from kartenn.changesets import Simulation
    from kartenn.client import AsyncClient, Client
    from kartenn.imports import ImportProgress, ImportRefused, file_digest, ndjson_elements
    from kartenn.resources import Applied, Pending, Record

__all__ = [
    "ApiError",
    "Applied",
    "AsyncClient",
    "BadRequest",
    "Client",
    "ClientError",
    "Conflict",
    "FileStore",
    "Forbidden",
    "ImportProgress",
    "ImportRefused",
    "KartennError",
    "MemoryStore",
    "NetworkError",
    "NotFound",
    "Pending",
    "PreconditionFailed",
    "PreconditionRequired",
    "RateLimited",
    "Record",
    "Refused",
    "ServerError",
    "Simulation",
    "TokenStore",
    "Unauthenticated",
    "Unavailable",
    "UnknownResource",
    "Violation",
    "__version__",
    "between",
    "exists",
    "file_digest",
    "ndjson_elements",
]

__version__: str  # read from the metadata of the distribution on first use

_HOMES = {
    "kartenn._core.problems": (
        "ApiError",
        "BadRequest",
        "ClientError",
        "Conflict",
        "Forbidden",
        "KartennError",
        "NetworkError",
        "NotFound",
        "PreconditionFailed",
        "PreconditionRequired",
        "RateLimited",
        "Refused",
        "ServerError",
        "Unauthenticated",
        "Unavailable",
        "Violation",
    ),
    "kartenn._core.requests": ("between", "exists"),
    "kartenn.auth": ("FileStore", "MemoryStore", "TokenStore"),
    "kartenn.catalog": ("UnknownResource",),
    "kartenn.changesets": ("Simulation",),
    "kartenn.client": ("AsyncClient", "Client"),
    "kartenn.imports": ("ImportProgress", "ImportRefused", "file_digest", "ndjson_elements"),
    "kartenn.resources": ("Applied", "Pending", "Record"),
}
_HOME_OF = {name: module for module, names in _HOMES.items() for name in names}


def __getattr__(name: str) -> Any:  # noqa: ANN401 - any exported name
    if name == "__version__":
        from importlib.metadata import version  # noqa: PLC0415 - 10 ms, on demand

        value: Any = version("kartenn")
    else:
        home = _HOME_OF.get(name)
        if home is None:
            msg = f"module 'kartenn' has no attribute {name!r}"
            raise AttributeError(msg)
        value = getattr(importlib.import_module(home), name)
    globals()[name] = value
    return value


def __dir__() -> list[str]:
    return sorted({*globals(), *__all__})
