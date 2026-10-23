"""`kartenn login`, `logout` and `me` (REQ-023-6 to 8; T10, T11; maquette § 1).

- `oidc`: the system browser and a loopback listener (PKCE S256), or `--device` (RFC 8628)
  on a host without a browser; the tokens go to the keyring;
- `basic`: the secret of the service account, typed without echo, goes to the keyring;
- `--breakglass`: name, passphrase and TOTP code typed without echo; the session lives in
  memory, or with `--keep` in `$XDG_RUNTIME_DIR/kartenn/<profile>.session` (0600).
No secret is ever taken as an argument.
"""

from __future__ import annotations

import os
import time
import webbrowser
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.base import command
from kartenn_cli.errors import CliError
from kartenn_cli.i18n import t

if TYPE_CHECKING:
    from collections.abc import Callable

    from kartenn.auth import DeviceCode
    from kartenn.profiles import Profile
    from kartenn_cli.runtime import Runtime

__all__ = ["OPEN_BROWSER", "SLEEP", "login_command", "logout_command", "me_command", "session_path"]

# Replaced in tests: the system browser, and the wait of the device flow.
OPEN_BROWSER: dict[str, Callable[[str], object]] = {"open": webbrowser.open}
SLEEP: dict[str, Callable[[float], object]] = {"sleep": time.sleep}


def session_path(profile: Profile) -> Path:
    """Where a break-glass session is kept with `--keep`."""
    base = os.environ.get("XDG_RUNTIME_DIR") or os.environ.get("XDG_STATE_HOME") or "/tmp"  # noqa: S108
    return Path(base) / "kartenn" / f"{profile.name}.session"


def _found(ctx: click.Context) -> Runtime:
    from kartenn_cli.runtime import runtime  # noqa: PLC0415

    return runtime(ctx)


def _show_device(code: DeviceCode) -> None:
    click.echo(t("login.device_warning"), err=True)
    if code.verification_uri_complete:
        click.echo(f"\n  {t('login.open')}  {code.verification_uri_complete}", err=True)
    else:
        click.echo(f"\n  {t('login.open')}  {code.verification_uri}", err=True)
        click.echo(f"  {t('login.code')}    {code.user_code}", err=True)
    click.echo("\n" + t("login.waiting", minutes=round(code.expires_in / 60)), err=True)


def _who(me: dict[str, Any]) -> str:
    name = me.get("name") or me.get("identity") or "?"
    issuer = me.get("issuer")
    return f"{name} ({issuer})" if issuer else str(name)


def _breakglass(
    found: Runtime, profile: Profile, name: str | None, *, keep: bool
) -> dict[str, Any]:
    from kartenn.auth import Breakglass, Secret  # noqa: PLC0415
    from kartenn.client import Client  # noqa: PLC0415

    found.require_input(t("login.prompts"))
    account = name or str(click.prompt(t("login.name")))
    passphrase = Secret(str(click.prompt(t("login.passphrase"), hide_input=True)))
    code = str(click.prompt(t("login.totp"), hide_input=True))
    auth = Breakglass(account, passphrase, code)
    with Client(
        profile.url,
        auth=auth,
        ca_bundle=profile.ca_bundle,
        transport=found.transport(profile),
        cache=found.cache,
    ) as client:
        me = cast("dict[str, Any]", dict(client.call("me")))
    if keep and auth.cookie:
        from kartenn._files import write_private  # noqa: PLC0415

        write_private(session_path(profile), f"{account}\n{auth.cookie}\n")
    return me


@command(name="login", help="login.help")
@click.option("--device", is_flag=True, help="login.opt.device")
@click.option("--breakglass", is_flag=True, help="login.opt.breakglass")
@click.option("--name", help="login.opt.name")
@click.option("--keep", is_flag=True, help="login.opt.keep")
@click.pass_context
def login_command(
    ctx: click.Context, name: str | None, *, device: bool, breakglass: bool, keep: bool
) -> None:
    from kartenn.auth import OidcNative, device_login  # noqa: PLC0415

    found = _found(ctx)
    profile = found.require_profile()
    if breakglass:
        me = _breakglass(found, profile, name, keep=keep)
        click.echo(t("login.done", who=_who(me)))
        return
    if profile.auth == "basic":
        found.require_input(t("login.secret_prompt"))
        secret = str(click.prompt(t("login.secret", account=profile.account), hide_input=True))
        found.token_store.set(f"kartenn:{profile.name}:secret", secret)
    elif profile.auth == "oidc":
        auth = found.auth
        if not isinstance(auth, OidcNative):  # pragma: no cover - a kept break-glass session
            raise CliError("client.login-required", t("login.session_kept"))
        client = found.client
        if device:
            device_login(
                auth,
                client.run_unauthenticated,
                display=_show_device,
                sleep=SLEEP["sleep"],
                clock=time.time,
            )
        else:
            found.require_input(t("login.browser"))

            def opener(url: str) -> object:
                click.echo(t("login.opening", issuer=profile.issuer or "?"), err=True)
                click.echo(t("login.fallback", url=url), err=True)
                return OPEN_BROWSER["open"](url)

            auth.login(client.run_unauthenticated, open_browser=opener)
    else:
        click.echo(t("login.nothing", auth=profile.auth))
        return
    me = cast("dict[str, Any]", dict(found.client.call("me")))
    click.echo(t("login.done", who=_who(me)))
    _ = found.catalog


@command(name="logout", help="logout.help")
@click.pass_context
def logout_command(ctx: click.Context) -> None:
    from kartenn._core.messages import Request  # noqa: PLC0415
    from kartenn.auth import OidcNative  # noqa: PLC0415

    found = _found(ctx)
    profile = found.require_profile()
    kept = session_path(profile)
    if kept.exists():
        found.client.send(Request("POST", "/v1/auth/logout"))
        kept.unlink()
        click.echo(t("logout.session"))
        return
    if profile.auth == "oidc":
        auth = found.auth
        if isinstance(auth, OidcNative):
            auth.logout()
    elif profile.auth == "basic":
        found.token_store.delete(f"kartenn:{profile.name}:secret")
    click.echo(t("logout.done", profile=profile.name))


@command(name="me", help="me.help")
@click.option(
    "-o",
    "--output",
    type=click.Choice(("table", "json", "yaml")),
    metavar="FORMAT",
    help="opt.output",
)
@click.pass_context
def me_command(ctx: click.Context, output: str | None) -> None:
    found = _found(ctx)
    me = cast("dict[str, Any]", dict(found.client.call("me")))
    if (found.asked_output(output) or "table") != "table":
        found.output(output=output).value(me)
        return
    rows = [
        (t("me.identity"), f"{_who(me)}  ({me.get('kind')}, {me.get('method')})"),
        (t("me.groups"), ", ".join(cast("list[str]", me.get("groups") or [])) or "—"),
        (t("me.roles"), ", ".join(cast("list[str]", me.get("roles") or [])) or "—"),
    ]
    width = max(len(label) for label, _ in rows)
    for label, value in rows:
        click.echo(f"{label.ljust(width)}  {value}")
