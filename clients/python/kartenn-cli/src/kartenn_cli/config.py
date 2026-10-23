"""Profiles, precedence and secrets (REQ-023-36, 40; T19; design § Configuration).

- `$XDG_CONFIG_HOME/kartenn/config.toml` holds the profiles, never a secret;
- precedence: option, then environment (`KARTENN_URL`, `KARTENN_PROFILE`,
  `KARTENN_OUTPUT`, `KARTENN_LANG`), then profile, then default;
- tokens and secrets live in the keyring of the system (`kartenn:<profile>:…`). Without a
  usable keyring nothing is stored: the CLI proposes `KARTENN_TOKEN` or `--token-file`.
"""

from __future__ import annotations

import contextlib
import json
import os
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import group
from kartenn_cli.errors import AUTH, CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from kartenn.auth import TokenStore
    from kartenn.profiles import Profile

__all__ = [
    "OUTPUTS",
    "KeyringStore",
    "Settings",
    "config_group",
    "config_path",
    "load_profile",
    "read_config",
    "token_store",
    "write_config",
]

OUTPUTS = ("table", "json", "ndjson", "yaml", "csv")
AUTHS = ("oidc", "basic", "token", "breakglass", "none")
SERVICE = "kartenn"


@dataclass
class Settings:
    """The options of the root command, before precedence."""

    profile: str | None = None
    output: str | None = None
    at: str | None = None
    as_of: str | None = None
    keys: bool = False
    lang: str | None = None
    no_input: bool = False
    verbose: int = 0
    token_file: Path | None = None


def config_path() -> Path:
    """`$XDG_CONFIG_HOME/kartenn/config.toml` (platformdirs, as the library reads it)."""
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(base) / "kartenn" / "config.toml"


def read_config(path: Path | None = None) -> dict[str, Any]:
    source = path or config_path()
    try:
        return tomllib.loads(source.read_text("utf-8"))
    except FileNotFoundError:
        return {}
    except tomllib.TOMLDecodeError as error:
        raise CliError(
            "client.invalid-configuration", t("config.invalid", path=source, error=error)
        ) from error


def _value(value: object) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int | float):
        return str(value)
    return json.dumps(str(value), ensure_ascii=False)


def write_config(config: dict[str, Any], path: Path | None = None) -> Path:
    """Write the configuration (its comments are not kept), in 0600."""
    target = path or config_path()
    lines = [f"default = {_value(config['default'])}"] if config.get("default") else []
    for name, raw in sorted(cast("dict[str, Any]", config.get("profiles") or {}).items()):
        lines += ["", f"[profiles.{json.dumps(name)}]"]
        profile = cast("dict[str, Any]", raw)
        lines += [f"{key} = {_value(value)}" for key, value in profile.items()]
    target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    temporary = target.with_name(f".{target.name}.tmp")
    temporary.write_text("\n".join(lines) + "\n", "utf-8")
    temporary.chmod(0o600)
    temporary.replace(target)
    return target


def load_profile(name: str | None) -> Profile | None:
    """The profile chosen by precedence, `None` without configuration nor `KARTENN_URL`.

    `KARTENN_URL` overrides the URL of the profile, or stands for a profile of its own
    (`auth` = `token` when `KARTENN_TOKEN` is set)."""
    from kartenn import ClientError  # noqa: PLC0415 - the library, when a server is needed
    from kartenn.profiles import Profile, read_profile  # noqa: PLC0415

    url = os.environ.get("KARTENN_URL")
    try:
        profile = read_profile(name, path=config_path())
    except ClientError as error:
        if error.code != "client.no-configuration" or name:
            raise
        if not url:
            return None
        auth = "token" if os.environ.get("KARTENN_TOKEN") else "none"
        return Profile(name="env", url=url, auth=auth)
    if url:
        from dataclasses import replace  # noqa: PLC0415

        profile = replace(profile, url=url)
    return profile


# Secrets ---------------------------------------------------------------------------


class KeyringStore:
    """The `TokenStore` of the CLI: the keyring of the system, imported on first use.

    Without a usable keyring (no D-Bus, a `fail` or `null` backend), `get` finds nothing
    and `set` refuses with the way out (`KARTENN_TOKEN`, `--token-file`)."""

    def __init__(self) -> None:
        self._backend: Any = None
        self._usable: bool | None = None

    def _keyring(self) -> Any:  # noqa: ANN401 - a backend of keyring
        if self._usable is None:
            try:
                import keyring  # noqa: PLC0415 - 50 ms, only when a secret is needed

                backend = keyring.get_keyring()
                self._usable = float(getattr(backend, "priority", 0)) > 0
                self._backend = backend
            except Exception:  # noqa: BLE001 - any failure of the keyring is its absence
                self._usable = False
        return self._backend if self._usable else None

    def get(self, key: str) -> str | None:
        backend = self._keyring()
        if backend is None:
            return None
        try:
            return cast("str | None", backend.get_password(SERVICE, key))
        except Exception:  # noqa: BLE001 - a locked keyring holds nothing for us
            return None

    def set(self, key: str, value: str) -> None:
        backend = self._keyring()
        if backend is None:
            raise CliError("client.keyring-unavailable", t("keyring.unavailable"), exit_code=AUTH)
        backend.set_password(SERVICE, key, value)

    def delete(self, key: str) -> None:
        backend = self._keyring()
        if backend is None:
            return
        with contextlib.suppress(Exception):  # nothing to forget
            backend.delete_password(SERVICE, key)


def token_store(token_file: Path | None) -> TokenStore:
    """`--token-file` when given (explicit, 0600), else the keyring."""
    if token_file is not None:
        from kartenn.auth import FileStore  # noqa: PLC0415

        return FileStore(token_file)
    return KeyringStore()


# The `config` command --------------------------------------------------------------


@group(name="config", help="config.help")
def config_group() -> None:
    pass


def _profiles(config: dict[str, Any]) -> dict[str, Any]:
    return cast("dict[str, Any]", config.setdefault("profiles", {}))


@config_group.command(name="add-profile", help="config.add.help")
@click.argument("name")
@click.option("--url", required=True, help="config.opt.url")
@click.option("--auth", type=click.Choice(AUTHS), default="oidc", help="config.opt.auth")
@click.option("--issuer", help="config.opt.issuer")
@click.option("--account", help="config.opt.account")
@click.option("--ca-bundle", type=click.Path(dir_okay=False), help="config.opt.ca_bundle")
@click.option("--lang", "lang", type=click.Choice(["fr", "en"]), help="config.opt.lang")
@click.option("--timezone", help="config.opt.timezone")
@click.option("--output", type=click.Choice(OUTPUTS), help="config.opt.output")
@click.option("--default/--no-default", "default", default=None, help="config.opt.default")
def add_profile(  # noqa: PLR0913, PLR0917 - the members of a profile
    name: str,
    url: str,
    auth: str,
    issuer: str | None,
    account: str | None,
    ca_bundle: str | None,
    lang: str | None,
    timezone: str | None,
    output: str | None,
    default: bool | None,  # noqa: FBT001 - a flag of click
) -> None:
    config = read_config()
    profiles = _profiles(config)
    members = {
        "url": url,
        "auth": auth,
        "issuer": issuer,
        "account": account,
        "ca_bundle": str(Path(ca_bundle).resolve()) if ca_bundle else None,
        "lang": lang,
        "timezone": timezone,
        "output": output,
    }
    existed = name in profiles
    profiles[name] = {k: v for k, v in members.items() if v is not None}
    chosen = default if default is not None else not config.get("default")
    if chosen:
        config["default"] = name
    path = write_config(config)
    key = "config.updated" if existed else "config.created"
    click.echo(t(key, name=name, path=path) + (t("config.chosen") if chosen else "."))


@config_group.command(name="list", help="config.list.help")
def list_profiles() -> None:
    config = read_config()
    default = config.get("default")
    profiles = cast("dict[str, Any]", config.get("profiles") or {})
    if not profiles:
        click.echo(t("config.none"))
        return
    for name, raw in sorted(profiles.items()):
        mark = "*" if name == default else " "
        click.echo(f"{mark} {name}  {cast('dict[str, Any]', raw).get('url', '')}")


@config_group.command(name="show", help="config.show.help")
@click.argument("name", required=False)
def show_profile(name: str | None) -> None:
    config = read_config()
    profiles = cast("dict[str, Any]", config.get("profiles") or {})
    from kartenn_cli.runtime import runtime  # noqa: PLC0415 - the root options

    given = name or runtime().settings.profile
    chosen = given or os.environ.get("KARTENN_PROFILE") or config.get("default")
    if not isinstance(chosen, str) or chosen not in profiles:
        known = ", ".join(sorted(profiles)) or "—"
        raise CliError("client.unknown-profile", t("config.unknown", name=chosen, known=known))
    click.echo(f"[profiles.{chosen}]")
    for key, value in cast("dict[str, Any]", profiles[chosen]).items():
        click.echo(f"{key} = {_value(value)}")


@config_group.command(name="use", help="config.use.help")
@click.argument("name")
def use_profile(name: str) -> None:
    config = read_config()
    profiles = _profiles(config)
    if name not in profiles:
        known = ", ".join(sorted(profiles)) or "—"
        raise CliError("client.unknown-profile", t("config.unknown", name=name, known=known))
    config["default"] = name
    write_config(config)
    click.echo(t("config.default", name=name))


@config_group.command(name="remove", help="config.remove.help")
@click.argument("name")
def remove_profile(name: str) -> None:
    config = read_config()
    profiles = _profiles(config)
    if profiles.pop(name, None) is None:
        known = ", ".join(sorted(profiles)) or "—"
        raise CliError("client.unknown-profile", t("config.unknown", name=name, known=known))
    if config.get("default") == name:
        config.pop("default")
    write_config(config)
    KeyringStore().delete(f"kartenn:{name}:secret")
    click.echo(t("config.removed", name=name))
