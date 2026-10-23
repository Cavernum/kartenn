"""Problems (RFC 9457) as typed exceptions (REQ-023-11).

Every exception of the library is a `KartennError` and carries a code: the slug of a
problem of the server (`changeset-refused`, from `urn:kartenn:problem:<slug>`), or a
`client.*` code for a refusal of the library itself. The members of a problem the
library does not know stay readable in `extensions` (RFC 9457 §3.2).
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from http import HTTPStatus
from typing import TYPE_CHECKING, Any, ClassVar, cast

if TYPE_CHECKING:
    from collections.abc import Mapping

    from kartenn._core.messages import Response

__all__ = [
    "ApiError",
    "BadRequest",
    "ClientError",
    "Conflict",
    "Forbidden",
    "KartennError",
    "NetworkError",
    "NotFound",
    "PayloadTooLarge",
    "PreconditionFailed",
    "PreconditionRequired",
    "RateLimited",
    "Refused",
    "ServerError",
    "Unauthenticated",
    "Unavailable",
    "UnsupportedMediaType",
    "Violation",
    "problem_error",
]

PROBLEM_URN = "urn:kartenn:problem:"
_STANDARD = frozenset({"type", "title", "status", "detail", "instance", "violations"})
_VIOLATION = frozenset({"rule", "severity", "detail", "pointer", "subjects", "operation"})


class KartennError(Exception):
    """An error of the library; `code` is stable, the message is for humans."""

    code: str

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class ClientError(KartennError):
    """A refusal of the library itself, before or instead of a request (`client.*`)."""


class NetworkError(ClientError):
    """The server could not be reached: `client.network`, `client.tls`, `client.timeout`."""


@dataclass(frozen=True)
class Violation:
    """A violation of a rule (006 § Erreurs): `operation` and `pointer` locate its source."""

    rule: str
    severity: str
    detail: str
    pointer: str | None = None
    subjects: tuple[Any, ...] = ()
    operation: int | None = None
    extra: Mapping[str, Any] = field(default_factory=dict[str, Any])

    @property
    def blocking(self) -> bool:
        return self.severity == "error"

    @classmethod
    def parse(cls, raw: Mapping[str, Any]) -> Violation:
        subjects = raw.get("subjects")
        operation = raw.get("operation")
        return cls(
            rule=str(raw.get("rule", "")),
            severity=str(raw.get("severity", "error")),
            detail=str(raw.get("detail", "")),
            pointer=None if raw.get("pointer") is None else str(raw["pointer"]),
            subjects=tuple(cast("list[Any]", subjects)) if isinstance(subjects, list) else (),
            operation=operation if isinstance(operation, int) else None,
            extra={k: v for k, v in raw.items() if k not in _VIOLATION},
        )


def parse_violations(raw: object) -> tuple[Violation, ...]:
    if not isinstance(raw, list):
        return ()
    items = cast("list[object]", raw)
    return tuple(
        Violation.parse(cast("Mapping[str, Any]", item)) for item in items if isinstance(item, dict)
    )


class ApiError(KartennError):
    """A problem answered by the server; the subclass follows its status."""

    status_code: ClassVar[int | None] = None

    def __init__(self, problem: Mapping[str, Any], *, status: int, etag: str | None = None) -> None:
        self.problem = dict(problem)
        self.type = str(problem.get("type", "about:blank"))
        self.status = int(problem.get("status", status))
        self.title = str(problem.get("title", HTTPStatus(status).phrase if _known(status) else ""))
        self.detail: str | None = None if problem.get("detail") is None else str(problem["detail"])
        self.instance: str | None = (
            None if problem.get("instance") is None else str(problem["instance"])
        )
        self.violations = parse_violations(problem.get("violations"))
        self.extensions = {k: v for k, v in problem.items() if k not in _STANDARD}
        self.etag = etag
        code = (
            self.type.removeprefix(PROBLEM_URN)
            if self.type.startswith(PROBLEM_URN)
            else f"http-{self.status}"
        )
        message = f"[{code}] {self.title} ({self.status})"
        if self.detail:
            message += f": {self.detail}"
        super().__init__(code, message)


def _known(status: int) -> bool:
    return status in HTTPStatus._value2member_map_


class BadRequest(ApiError):
    status_code = 400


class Unauthenticated(ApiError):
    status_code = 401


class Forbidden(ApiError):
    status_code = 403


class NotFound(ApiError):
    status_code = 404


class Conflict(ApiError):
    status_code = 409


class PreconditionFailed(ApiError):
    """412: the version expected is not the current one; `current_etag` when it is known."""

    status_code = 412

    @property
    def current_etag(self) -> str | None:
        current = self.extensions.get("current_etag")
        return str(current) if current is not None else self.etag


class PayloadTooLarge(ApiError):
    status_code = 413


class UnsupportedMediaType(ApiError):
    status_code = 415


class Refused(ApiError):
    """422: a changeset refused by its rules, or a member the server does not accept."""

    status_code = 422


class PreconditionRequired(ApiError):
    status_code = 428


class RateLimited(ApiError):
    status_code = 429


class Unavailable(ApiError):
    status_code = 503


class ServerError(ApiError):
    """Any other status of 500 and above."""


_BY_STATUS: dict[int, type[ApiError]] = {
    cls.status_code: cls
    for cls in (
        BadRequest,
        Unauthenticated,
        Forbidden,
        NotFound,
        Conflict,
        PreconditionFailed,
        PayloadTooLarge,
        UnsupportedMediaType,
        Refused,
        PreconditionRequired,
        RateLimited,
        Unavailable,
    )
    if cls.status_code is not None
}


def problem_error(response: Response) -> ApiError:
    """The exception of an error response, its body a problem or not (a 414 before the
    routing has a minimal body, a proxy answers what it likes)."""
    problem: dict[str, Any] = {}
    try:
        parsed = json.loads(response.content) if response.content else None
    except ValueError:
        parsed = None
    if isinstance(parsed, dict):
        problem = cast("dict[str, Any]", parsed)
    problem.setdefault("status", response.status)
    if not isinstance(problem.get("status"), int):
        problem["status"] = response.status
    status = response.status
    cls = _BY_STATUS.get(status)
    if cls is None:
        cls = ServerError if status >= HTTPStatus.INTERNAL_SERVER_ERROR else ApiError
    return cls(problem, status=status, etag=response.headers.get("ETag"))
