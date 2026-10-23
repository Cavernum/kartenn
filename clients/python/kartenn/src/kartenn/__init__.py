"""Python client library of the kartenn API (spec 023)."""

from importlib.metadata import version

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
from kartenn.client import AsyncClient, Client
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
    "TokenStore",
    "Unauthenticated",
    "Unavailable",
    "UnknownResource",
    "Violation",
    "__version__",
    "between",
    "exists",
]

__version__ = version("kartenn")
