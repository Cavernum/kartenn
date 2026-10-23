import json
from typing import Any

import pytest
from hypothesis import given
from hypothesis import strategies as st

from kartenn._core.messages import Headers, Response
from kartenn._core.problems import (
    ApiError,
    BadRequest,
    Conflict,
    NotFound,
    PreconditionFailed,
    Refused,
    ServerError,
    Unavailable,
    Violation,
    problem_error,
)


def answer(status: int, body: object, **headers: str) -> Response:
    content = body if isinstance(body, bytes) else json.dumps(body).encode()
    return Response(status, Headers.of(headers), content)


REFUSED = {
    "type": "urn:kartenn:problem:changeset-refused",
    "title": "Changeset refused",
    "status": 422,
    "detail": "2 violations",
    "instance": "urn:uuid:5b0e",
    "changeset": "0c41",
    "violations": [
        {
            "rule": "core.prefix.overlap",
            "severity": "error",
            "detail": "overlap",
            "pointer": "/operations/0",
            "subjects": ["a", "b"],
            "operation": 0,
            "hint": "x",
        },
        {"rule": "core.prefix.unused", "severity": "warning", "detail": "unused", "pointer": None},
        "not an object",
    ],
}


def test_refused_carries_everything() -> None:
    error = problem_error(answer(422, REFUSED))
    assert isinstance(error, Refused)
    assert error.code == "changeset-refused"
    assert (error.title, error.detail, error.status, error.instance) == (
        "Changeset refused",
        "2 violations",
        422,
        "urn:uuid:5b0e",
    )
    assert error.extensions == {"changeset": "0c41"}
    first, second = error.violations
    assert first == Violation(
        rule="core.prefix.overlap",
        severity="error",
        detail="overlap",
        pointer="/operations/0",
        subjects=("a", "b"),
        operation=0,
        extra={"hint": "x"},
    )
    assert first.blocking
    assert not second.blocking
    assert second.pointer is None
    assert str(error) == "[changeset-refused] Changeset refused (422): 2 violations"


def test_precondition_failed_gives_the_current_etag() -> None:
    body = {"type": "urn:kartenn:problem:precondition-failed", "title": "Stale", "status": 412}
    error = problem_error(answer(412, body, ETag='"c2.00ff"'))
    assert isinstance(error, PreconditionFailed)
    assert error.current_etag == '"c2.00ff"'
    error = problem_error(answer(412, {**body, "current_etag": '"c3"'}))
    assert isinstance(error, PreconditionFailed)
    assert error.current_etag == '"c3"'


@pytest.mark.parametrize(
    ("status", "cls", "code"),
    [
        (400, BadRequest, "http-400"),
        (404, NotFound, "http-404"),
        (409, Conflict, "http-409"),
        (503, Unavailable, "http-503"),
        (500, ServerError, "http-500"),
        (502, ServerError, "http-502"),
        (418, ApiError, "http-418"),
        (499, ApiError, "http-499"),
    ],
)
def test_non_problem_bodies(status: int, cls: type[ApiError], code: str) -> None:
    error = problem_error(Response(status, Headers(), b"<html>oops</html>"))
    assert type(error) is cls
    assert error.code == code
    assert error.violations == ()


def test_status_member_of_the_wrong_type() -> None:
    error = problem_error(answer(404, {"status": "x", "type": "about:blank", "title": "Gone"}))
    assert error.status == 404
    assert "Too Long" in problem_error(Response(414)).title


extension_names = st.text(alphabet="abcdefghij_", min_size=1, max_size=8).filter(
    lambda name: name not in {"type", "title", "status", "detail", "instance", "violations"}
)
json_values = st.recursive(
    st.none() | st.booleans() | st.integers() | st.text(max_size=5),
    lambda inner: (
        st.lists(inner, max_size=3) | st.dictionaries(st.text(max_size=3), inner, max_size=3)
    ),
    max_leaves=6,
)


@given(st.dictionaries(extension_names, json_values, max_size=5))
def test_unknown_members_stay_readable(extensions: dict[str, Any]) -> None:
    body = {
        "type": "urn:kartenn:problem:conflict",
        "title": "Conflict",
        "status": 409,
        **extensions,
    }
    error = problem_error(answer(409, body))
    assert error.extensions == extensions
    assert error.problem == body
