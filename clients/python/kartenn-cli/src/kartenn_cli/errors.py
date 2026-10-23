"""Exit codes (REQ-023-34, design § Codes de sortie) and the errors of the CLI itself.

| Code | Case |
|---|---|
| 0 | success; a write applied; a write pending without `--require-applied` |
| 1 | unexpected error (500, a bug of the client) |
| 2 | usage: invalid option, module or operation not served, ambiguous reference |
| 3 | 404 `not-found` |
| 4 | 401 and 403 |
| 5 | 409, 412, 428 |
| 6 | 422, import refused |
| 7 | 429 or 503 after retries; network, TLS |
| 8 | `--dry-run`: the simulation has blocking violations |
| 9 | `policy check --exit-status`: verdict `denied` |
| 10 | `--require-applied`: the write waits for an approval |

The library is imported only when an error is classified.
"""

from __future__ import annotations

__all__ = [
    "AUTH",
    "CONFLICT",
    "DENIED",
    "DRY_RUN_REFUSED",
    "NOT_FOUND",
    "OK",
    "PENDING",
    "REFUSED",
    "UNAVAILABLE",
    "UNEXPECTED",
    "USAGE",
    "CliError",
    "exit_code",
]

OK = 0
UNEXPECTED = 1
USAGE = 2
NOT_FOUND = 3
AUTH = 4
CONFLICT = 5
REFUSED = 6
UNAVAILABLE = 7
DRY_RUN_REFUSED = 8
DENIED = 9
PENDING = 10

_BY_STATUS = {
    400: USAGE,
    401: AUTH,
    403: AUTH,
    404: NOT_FOUND,
    409: CONFLICT,
    412: CONFLICT,
    413: USAGE,
    415: USAGE,
    422: REFUSED,
    428: CONFLICT,
    429: UNAVAILABLE,
    503: UNAVAILABLE,
}
_CLIENT = {
    "client.login-required": AUTH,
    "client.secret-required": AUTH,
    "client.oidc-denied": AUTH,
    "client.oidc-expired": AUTH,
    "client.oidc-error": AUTH,
    "client.oidc-state-mismatch": AUTH,
    "client.unknown-issuer": AUTH,
    "client.device-unavailable": AUTH,
    "client.reference-not-found": NOT_FOUND,
    "client.unexpected-answer": UNEXPECTED,
    "client.query-unsupported": UNAVAILABLE,
}
_NETWORK = frozenset({"client.network", "client.tls", "client.timeout"})


class CliError(Exception):
    """A refusal of the CLI itself: its code (`client.*`), its message, more lines to
    show under it, and its exit code."""

    def __init__(
        self, code: str, message: str, *lines: str, exit_code: int = USAGE, **extra: object
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.lines = lines
        self.exit_code = exit_code
        self.extra = extra


def exit_code(error: BaseException) -> int:
    """The exit code of an error of the library or of the CLI."""
    if isinstance(error, CliError):
        return error.exit_code
    from kartenn import ApiError, ClientError, ImportRefused  # noqa: PLC0415 - on failure only

    if isinstance(error, ImportRefused):
        return REFUSED
    if isinstance(error, ApiError):
        return _BY_STATUS.get(error.status, UNEXPECTED)
    if isinstance(error, ClientError):
        if error.code in _NETWORK:
            return UNAVAILABLE
        return _CLIENT.get(error.code, USAGE)
    return UNEXPECTED
