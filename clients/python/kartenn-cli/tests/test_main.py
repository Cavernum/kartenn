"""The root of `kartenn`: version, help, languages, lazy imports (REQ-023-37, 39; T24)."""

from __future__ import annotations

import json
import subprocess
import sys
from typing import TYPE_CHECKING

import pytest

from fakes import kartenn, profile
from kartenn_cli import base
from kartenn_cli.i18n import resolve, set_lang, t, tn

if TYPE_CHECKING:
    from syrupy.assertion import SnapshotAssertion


def test_version_names_the_command_and_the_distribution_version() -> None:
    result = kartenn("--version")
    assert result.exit_code == 0
    assert result.output == "kartenn, version 0.1.0.dev0\n"


def test_the_version_is_the_one_of_the_distribution() -> None:
    from importlib.metadata import version  # noqa: PLC0415

    import kartenn_cli  # noqa: PLC0415

    assert kartenn_cli.__version__ == version("kartenn-cli")


def test_version_imports_neither_the_library_nor_httpx2_nor_rich() -> None:
    script = (
        "import sys\n"
        "from kartenn_cli.main import cli\n"
        "try:\n"
        "    cli.main(['--version'], prog_name='kartenn')\n"
        "except SystemExit:\n"
        "    pass\n"
        "loaded = sorted(m for m in sys.modules if m.split('.')[0] in "
        "{'kartenn', 'httpx2', 'rich', 'rich_click', 'keyring', 'yaml', 'jmespath'})\n"
        "print(loaded, file=sys.stderr)\n"
    )
    done = subprocess.run(
        [sys.executable, "-c", script], capture_output=True, text=True, check=True
    )
    assert done.stdout == "kartenn, version 0.1.0.dev0\n"
    assert done.stderr.strip() == "[]"


def test_help_without_profile(snapshot: SnapshotAssertion) -> None:
    result = kartenn("--help")
    assert result.exit_code == 0
    assert result.output == snapshot


def test_help_in_english(snapshot: SnapshotAssertion) -> None:
    result = kartenn("--lang", "en", "config", "--help")
    assert result.exit_code == 0
    assert result.output == snapshot


def test_the_language_of_the_profile_beats_the_locale(monkeypatch: pytest.MonkeyPatch) -> None:
    profile(lang="en")
    assert "This help." in kartenn("config", "--help").output
    monkeypatch.setenv("KARTENN_LANG", "fr")
    assert "Cette aide." in kartenn("config", "--help").output


def test_help_on_a_terminal_is_rendered_by_rich_click(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(base, "rich_wanted", lambda: True)
    result = kartenn("config", "--help")
    assert result.exit_code == 0
    assert "╭─ Options" in result.output
    assert "╭─ Commandes" in result.output
    assert "add-profile" in result.output


def test_rich_is_wanted_on_a_terminal_without_no_color(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(sys.stdout, "isatty", lambda: True)
    assert not base.rich_wanted()
    monkeypatch.delenv("NO_COLOR")
    assert base.rich_wanted()
    monkeypatch.setenv("TERM", "dumb")
    assert not base.rich_wanted()


@pytest.mark.parametrize(
    ("option", "profile_lang", "env", "expected"),
    [
        ("en", "fr", {"LANG": "fr_FR.UTF-8"}, "en"),
        (None, "en", {"LANG": "fr_FR.UTF-8"}, "en"),
        (None, None, {"LC_ALL": "fr_BE.UTF-8", "LANG": "en_US.UTF-8"}, "fr"),
        (None, None, {"LC_MESSAGES": "fr_FR", "LANG": "en_US"}, "fr"),
        (None, None, {"LANG": "de_DE.UTF-8"}, "en"),
        (None, None, {"LC_ALL": "C", "LANG": "fr_FR.UTF-8"}, "en"),
        (None, None, {}, "en"),
        (None, "es", {"LANG": "fr_FR"}, "fr"),
    ],
)
def test_language_resolution(
    option: str | None, profile_lang: str | None, env: dict[str, str], expected: str
) -> None:
    assert resolve(option, profile_lang, env) == expected


def test_plural_follows_the_language() -> None:
    set_lang("fr")
    assert tn("error.violations", 0) == "0 violation"
    assert tn("error.violations", 2) == "2 violations"
    set_lang("en")
    assert tn("error.violations", 0) == "0 violations"
    assert tn("error.violations", 1) == "1 violation"
    assert t("no.such.key") == "no.such.key"


def test_unknown_language_falls_back_to_english() -> None:
    set_lang("xx")
    assert t("opt.help") == "This help."


def test_errors_of_the_configuration_have_their_code(server: object) -> None:
    del server
    result = kartenn("-o", "json", "-p", "absent", "config", "show")
    assert result.exit_code == 2
    assert json.loads(result.stderr)["code"] == "client.unknown-profile"
