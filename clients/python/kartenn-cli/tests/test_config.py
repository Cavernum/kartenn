"""Profiles, precedence and secrets (REQ-023-35, 36, 40; T19)."""

from __future__ import annotations

import os
import stat
import sys
from typing import TYPE_CHECKING

import keyring
import pytest
from keyring.backends import fail

from conftest import MemoryKeyring
from fakes import URL, kartenn, profile
from kartenn_cli.config import KeyringStore, Settings, config_path, read_config, token_store
from kartenn_cli.errors import CliError
from kartenn_cli.runtime import Runtime

if TYPE_CHECKING:
    from pathlib import Path


def test_add_profile_writes_a_private_file_and_chooses_the_first_by_default() -> None:
    result = kartenn(
        "config", "add-profile", "prod", "--url", URL, "--auth", "oidc", "--issuer", "interne"
    )
    assert result.exit_code == 0, result.output
    path = config_path()
    assert result.output == f"Profil « prod » créé ({path}) et choisi par défaut.\n"
    assert stat.S_IMODE(path.stat().st_mode) == 0o600
    assert read_config() == {
        "default": "prod",
        "profiles": {"prod": {"url": URL, "auth": "oidc", "issuer": "interne"}},
    }
    second = kartenn("config", "add-profile", "labo", "--url", "https://kartenn.example.net")
    assert second.output.endswith("créé (" + str(path) + ").\n")
    assert read_config()["default"] == "prod"
    again = kartenn("config", "add-profile", "labo", "--url", "https://x.example.net", "--default")
    assert "mis à jour" in again.output
    assert read_config()["default"] == "labo"


def test_list_show_use_remove(isolated: MemoryKeyring) -> None:
    assert (
        kartenn("config", "list").output
        == "Aucun profil : kartenn config add-profile NOM --url URL\n"
    )
    kartenn("config", "add-profile", "prod", "--url", URL, "--auth", "basic", "--account", "sync")
    kartenn("config", "add-profile", "labo", "--url", "https://kartenn.example.net")
    assert kartenn("config", "list").output == (
        "  labo  https://kartenn.example.net\n* prod  https://kartenn.example.org\n"
    )
    assert kartenn("config", "show").output == (
        '[profiles.prod]\nurl = "https://kartenn.example.org"\nauth = "basic"\naccount = "sync"\n'
    )
    assert kartenn("config", "use", "labo").output == "Profil par défaut : labo.\n"
    assert "labo" in kartenn("config", "show").output
    isolated.values["kartenn", "kartenn:labo:secret"] = "s3cret"
    assert "supprimé" in kartenn("config", "remove", "labo").output
    assert ("kartenn", "kartenn:labo:secret") not in isolated.values
    assert "default" not in read_config()
    for args in (("use", "absent"), ("remove", "absent"), ("show", "absent")):
        result = kartenn("config", *args)
        assert result.exit_code == 2
        assert "erreur [client.unknown-profile]" in result.stderr


def test_a_broken_configuration_is_refused_with_its_code() -> None:
    path = config_path()
    path.parent.mkdir(parents=True)
    path.write_text("default = [")
    result = kartenn("config", "list")
    assert result.exit_code == 2
    assert result.stderr.startswith("erreur [client.invalid-configuration]")


def test_precedence_of_the_output(monkeypatch: pytest.MonkeyPatch) -> None:
    profile(output="yaml")
    found = Runtime(Settings())
    assert found.output_format() == "yaml"
    monkeypatch.setenv("KARTENN_OUTPUT", "csv")
    assert Runtime(Settings()).output_format() == "csv"
    assert Runtime(Settings(output="ndjson")).output_format() == "ndjson"
    assert Runtime(Settings(output="ndjson")).output_format("table") == "table"
    monkeypatch.setenv("KARTENN_OUTPUT", "xml")
    with pytest.raises(CliError) as refused:
        Runtime(Settings()).output_format()
    assert refused.value.code == "client.invalid-output"


def test_default_output_is_table_on_a_terminal(monkeypatch: pytest.MonkeyPatch) -> None:
    assert Runtime(Settings()).output_format() == "json"
    monkeypatch.setattr(sys.stdout, "isatty", lambda: True)
    assert Runtime(Settings()).output_format() == "table"


def test_kartenn_url_stands_for_a_profile(monkeypatch: pytest.MonkeyPatch) -> None:
    assert Runtime(Settings()).profile is None
    monkeypatch.setenv("KARTENN_URL", "https://other.example.org")
    found = Runtime(Settings()).profile
    assert found is not None
    assert (found.name, found.url, found.auth) == ("env", "https://other.example.org", "none")
    monkeypatch.setenv("KARTENN_TOKEN", "t")
    found = Runtime(Settings()).profile
    assert found is not None
    assert found.auth == "token"
    profile()
    found = Runtime(Settings()).profile
    assert found is not None
    assert (found.name, found.url) == ("prod", "https://other.example.org")


def test_no_profile_is_a_usage_error() -> None:
    with pytest.raises(CliError) as refused:
        Runtime(Settings()).require_profile()
    assert refused.value.code == "client.no-configuration"


def test_an_unknown_time_zone_is_refused() -> None:
    profile(timezone="Mars/Olympus")
    with pytest.raises(CliError) as refused:
        _ = Runtime(Settings()).zone
    assert refused.value.code == "client.invalid-configuration"


def test_no_input_without_a_terminal_refuses_to_ask(monkeypatch: pytest.MonkeyPatch) -> None:
    found = Runtime(Settings())
    with pytest.raises(CliError) as refused:
        found.require_input("un motif")
    assert refused.value.code == "client.input-required"
    monkeypatch.setattr(sys.stdin, "isatty", lambda: True)
    monkeypatch.setattr(sys.stdout, "isatty", lambda: True)
    assert Runtime(Settings()).interactive
    assert not Runtime(Settings(no_input=True)).interactive


def test_no_color(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(sys.stdout, "isatty", lambda: True)
    monkeypatch.setattr(sys.stderr, "isatty", lambda: True)
    assert not Runtime(Settings()).color
    assert not Runtime(Settings()).error_color
    monkeypatch.delenv("NO_COLOR")
    assert Runtime(Settings()).color
    assert Runtime(Settings()).error_color


def test_keyring_store_keeps_secrets_in_the_keyring(isolated: MemoryKeyring) -> None:
    store = KeyringStore()
    store.set("kartenn:prod:access_token", "a")
    assert isolated.values == {("kartenn", "kartenn:prod:access_token"): "a"}
    assert store.get("kartenn:prod:access_token") == "a"
    store.delete("kartenn:prod:access_token")
    store.delete("kartenn:prod:access_token")
    assert store.get("kartenn:prod:access_token") is None


def test_without_a_keyring_nothing_is_stored() -> None:
    keyring.set_keyring(fail.Keyring())
    store = KeyringStore()
    assert store.get("kartenn:prod:access_token") is None
    store.delete("kartenn:prod:access_token")
    with pytest.raises(CliError) as refused:
        store.set("kartenn:prod:access_token", "a")
    assert refused.value.code == "client.keyring-unavailable"
    assert "KARTENN_TOKEN" in refused.value.message
    assert "--token-file" in refused.value.message


def test_a_failing_keyring_holds_nothing(monkeypatch: pytest.MonkeyPatch) -> None:
    class Broken(MemoryKeyring):
        def get_password(self, service: str, username: str) -> str | None:
            raise RuntimeError(service + username)

    keyring.set_keyring(Broken())
    assert KeyringStore().get("k") is None
    monkeypatch.setattr(keyring, "get_keyring", lambda: (_ for _ in ()).throw(RuntimeError()))
    assert KeyringStore().get("k") is None


def test_token_file_is_explicit(tmp_path: Path) -> None:
    path = tmp_path / "tokens.json"
    store = token_store(path)
    store.set("kartenn:prod:access_token", "a")
    assert stat.S_IMODE(path.stat().st_mode) == 0o600
    assert isinstance(token_store(None), KeyringStore)
    assert os.environ["XDG_CONFIG_HOME"] in str(config_path())
