"""Exit codes, one per class of failure (REQ-023-34, design § Codes de sortie)."""

from __future__ import annotations

import pytest

from kartenn import (
    ApiError,
    ClientError,
    Conflict,
    Forbidden,
    ImportRefused,
    NetworkError,
    NotFound,
    PreconditionFailed,
    PreconditionRequired,
    RateLimited,
    Refused,
    ServerError,
    Unauthenticated,
    Unavailable,
)
from kartenn_cli import errors
from kartenn_cli.errors import CliError, exit_code
from kartenn_cli.render.problems import render_error


def api(cls: type[ApiError], status: int, slug: str = "x") -> ApiError:
    return cls({"type": f"urn:kartenn:problem:{slug}", "status": status}, status=status)


@pytest.mark.parametrize(
    ("error", "code"),
    [
        (CliError("client.done", "fait", exit_code=errors.OK), 0),
        (api(ServerError, 500), 1),
        (ClientError("client.unexpected-answer", "?"), 1),
        (ValueError("bogue"), 1),
        (CliError("client.module-unavailable", "absent"), 2),
        (ClientError("client.unknown-filter", "filtre"), 2),
        (api(ApiError, 400), 2),
        (api(NotFound, 404, "not-found"), 3),
        (ClientError("client.reference-not-found", "?"), 3),
        (api(Unauthenticated, 401), 4),
        (api(Forbidden, 403), 4),
        (ClientError("client.login-required", "login"), 4),
        (api(Conflict, 409), 5),
        (api(PreconditionFailed, 412), 5),
        (api(PreconditionRequired, 428), 5),
        (api(Refused, 422), 6),
        (ImportRefused({"type": "urn:kartenn:problem:invalid-import"}, ()), 6),
        (api(RateLimited, 429), 7),
        (api(Unavailable, 503), 7),
        (NetworkError("client.network", "refused"), 7),
        (NetworkError("client.tls", "certificate"), 7),
        (CliError("client.dry-run-refused", "x", exit_code=errors.DRY_RUN_REFUSED), 8),
        (CliError("client.denied", "x", exit_code=errors.DENIED), 9),
        (CliError("client.pending", "x", exit_code=errors.PENDING), 10),
    ],
)
def test_exit_codes(error: BaseException, code: int) -> None:
    assert exit_code(error) == code


def test_client_errors_render_their_code_first() -> None:
    error = CliError("client.module-unavailable", "Le module « policy » n'est pas servi.", "Ligne")
    assert render_error(error) == (
        "erreur [client.module-unavailable] Le module « policy » n'est pas servi.\n  Ligne\n"
    )
    assert render_error(error, json_output=True) == (
        '{"code": "client.module-unavailable", "detail": "Le module « policy » n\'est pas '
        'servi."}\n'
    )
    login = ClientError("client.login-required", "not logged in: kartenn login")
    assert render_error(login) == (
        "erreur [client.login-required] not logged in: kartenn login\n"
        "  Connectez-vous : kartenn login (ou KARTENN_TOKEN).\n"
    )
    assert render_error(ValueError("bogue")) == "erreur [client.unexpected] bogue\n"


def test_a_stale_version_says_how_to_merge() -> None:
    error = PreconditionFailed(
        {
            "type": "urn:kartenn:problem:precondition-failed",
            "title": "L'objet a changé depuis sa lecture",
            "status": 412,
            "instance": "urn:uuid:c3d1",
        },
        status=412,
        etag='"1aa2"',
    )
    assert render_error(error) == (
        "erreur [precondition-failed] L'objet a changé depuis sa lecture (412)\n"
        "  requête urn:uuid:c3d1\n"
        '  Version courante "1aa2". Relisez l\'objet, ou utilisez « edit » pour fusionner.\n'
    )


def test_an_import_refused_names_its_import() -> None:
    error = ImportRefused(
        {
            "type": "urn:kartenn:problem:invalid-import",
            "title": "L'import n'a pas pu être trié",
            "status": 422,
            "violations": [
                {"rule": "import.unknown-reference", "severity": "error", "detail": "Inconnu"}
            ],
        },
        (73412,),
    )
    error.context = {"import": "61b0"}  # type: ignore[attr-defined]
    error.sources = ("adresses.csv:73412",)  # type: ignore[attr-defined]
    assert render_error(error) == (
        "erreur [invalid-import] L'import n'a pas pu être trié (422)\n"
        "  1 violation — import 61b0\n"
        "  ✗ [import.unknown-reference] adresses.csv:73412  Inconnu\n"
    )
