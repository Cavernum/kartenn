"""The state of one invocation (`ctx.obj`): settings after precedence, profile, catalog
and client, each made when first needed.

`--help` and the completion read the catalog from the cache only (`cached_catalog`);
a command that talks to the server reads the capabilities again at most once a minute
(`catalog`), and says so on the standard error when the revision changed.
"""

from __future__ import annotations

import os
import sys
from functools import cached_property
from typing import TYPE_CHECKING, Any

import click

from kartenn_cli.config import OUTPUTS, Settings, load_profile, token_store
from kartenn_cli.errors import USAGE, CliError
from kartenn_cli.i18n import resolve, set_lang, t

if TYPE_CHECKING:
    from collections.abc import Callable
    from datetime import tzinfo

    from kartenn import Client
    from kartenn.auth import TokenStore
    from kartenn.catalog import Catalog, CatalogCache
    from kartenn.profiles import Profile
    from kartenn.transport.port import Transport
    from kartenn_cli.render import Output

__all__ = ["TRANSPORT", "Runtime", "runtime"]

# Tests hand the client a simulated server: `TRANSPORT["factory"] = lambda url, profile: …`.
TRANSPORT: dict[str, Callable[[str, Profile], Transport]] = {}


def runtime(ctx: click.Context | None = None) -> Runtime:
    """The runtime of the invocation, made on first use."""
    context = ctx or click.get_current_context()
    root = context.find_root()
    if not isinstance(root.obj, Runtime):
        root.obj = Runtime(Settings())
    return root.obj


class Runtime:
    def __init__(self, settings: Settings) -> None:
        self.settings = settings
        self.lang = resolve(settings.lang or os.environ.get("KARTENN_LANG"), None)
        set_lang(self.lang)

    def apply(self, name: str, value: object) -> None:
        """An option of the root, as soon as it is parsed."""
        setattr(self.settings, name, value)
        if name == "profile":
            self.__dict__.pop("profile", None)
        if name in {"lang", "profile"}:
            profile = self.profile_or_none() if name == "profile" else None
            option = self.settings.lang or os.environ.get("KARTENN_LANG")
            self.lang = resolve(option, profile.lang if profile else None)
            set_lang(self.lang)

    # Profile and preferences -----------------------------------------------------

    @cached_property
    def profile(self) -> Profile | None:
        profile = load_profile(self.settings.profile)
        if profile is not None:
            self.lang = resolve(self.settings.lang or os.environ.get("KARTENN_LANG"), profile.lang)
            set_lang(self.lang)
        return profile

    def require_profile(self) -> Profile:
        profile = self.profile
        if profile is None:
            raise CliError("client.no-configuration", t("config.missing"), t("config.missing_hint"))
        return profile

    @property
    def interactive(self) -> bool:
        """Questions, browser and editor only on a terminal and without `--no-input`."""
        return not self.settings.no_input and sys.stdin.isatty() and sys.stdout.isatty()

    def require_input(self, what: str) -> None:
        if not self.interactive:
            raise CliError("client.input-required", t("input.required", what=what))

    @property
    def color(self) -> bool:
        if os.environ.get("NO_COLOR") or os.environ.get("TERM") == "dumb":
            return False
        return sys.stdout.isatty()

    @property
    def error_color(self) -> bool:
        if os.environ.get("NO_COLOR") or os.environ.get("TERM") == "dumb":
            return False
        return sys.stderr.isatty()

    def output_format(self, given: str | None = None) -> str:
        """Option, environment, profile, then `table` on a terminal and `json` elsewhere."""
        profile = self.profile_or_none()
        found = (
            given
            or self.settings.output
            or os.environ.get("KARTENN_OUTPUT")
            or (profile.output if profile else None)
        )
        if found is None:
            return "table" if sys.stdout.isatty() else "json"
        if found not in OUTPUTS:
            raise CliError("client.invalid-output", t("output.invalid", given=found))
        return found

    def profile_or_none(self) -> Profile | None:
        try:
            return self.profile
        except Exception:  # noqa: BLE001 - a broken profile is reported where it is needed
            return None

    @cached_property
    def zone(self) -> tzinfo | None:
        profile = self.profile_or_none()
        if profile is None or not profile.timezone:
            return None
        from zoneinfo import ZoneInfo, ZoneInfoNotFoundError  # noqa: PLC0415

        try:
            return ZoneInfo(profile.timezone)
        except (ZoneInfoNotFoundError, ValueError) as error:
            raise CliError(
                "client.invalid-configuration", t("config.timezone", zone=profile.timezone)
            ) from error

    def output(self, **options: Any) -> Output:  # noqa: ANN401 - the options of a command
        from kartenn_cli.render import Output  # noqa: PLC0415

        columns = options.pop("columns", ())
        fmt = self.output_format(options.pop("output", None))
        keys = bool(options.pop("keys", None) or self.settings.keys)
        return Output(
            format=fmt,
            keys=keys,
            color=self.color,
            zone=self.zone,
            columns=tuple(columns),
            width=_terminal_width() if self.color else None,
            **options,
        )

    @property
    def json_errors(self) -> bool:
        try:
            return self.output_format() == "json"
        except CliError:
            return False

    # Catalog and client ----------------------------------------------------------

    @cached_property
    def cache(self) -> CatalogCache:
        from kartenn.catalog import CatalogCache  # noqa: PLC0415

        return CatalogCache()

    def cached_catalog(self) -> Catalog | None:
        """The catalog of the server from the cache, without the network."""
        profile = self.profile
        if profile is None:
            return None
        return self.cache.last(profile.url.rstrip("/"))

    @cached_property
    def token_store(self) -> TokenStore:
        return token_store(self.settings.token_file)

    def transport(self, profile: Profile) -> Transport | None:
        """The transport of the client: the library's own, or a simulated one in tests;
        wrapped by `--verbose`."""
        factory = TRANSPORT.get("factory")
        transport = factory(profile.url, profile) if factory else None
        if not self.settings.verbose:
            return transport
        if transport is None:
            from kartenn.transport.httpx2 import Httpx2Transport  # noqa: PLC0415

            transport = Httpx2Transport(
                profile.url.rstrip("/"), ca_bundle=profile.ca_bundle, http2=profile.http2
            )
        from kartenn_cli.verbose import Verbose  # noqa: PLC0415

        return Verbose(transport)

    @cached_property
    def client(self) -> Client:
        from kartenn.client import Client, auth_of_profile  # noqa: PLC0415

        profile = self.require_profile()
        transport = self.transport(profile)
        return Client(
            profile.url,
            auth=auth_of_profile(profile, self.token_store),
            ca_bundle=profile.ca_bundle,
            http2=profile.http2,
            token_store=self.token_store,
            transport=transport,
            cache=self.cache,
        )

    @cached_property
    def catalog(self) -> Catalog:
        """The catalog, discovered without authentication (public routes); a line on the
        standard error when it was not in the cache or its revision changed."""
        from kartenn.catalog import discover  # noqa: PLC0415

        profile = self.require_profile()
        url = profile.url.rstrip("/")
        before = self.cache.last(url)
        catalog = self.client.run_unauthenticated(discover(url, self.cache))
        if before is None or before.revision != catalog.revision:
            modules = ", ".join(catalog.module_names) or "—"
            previous = (
                t("discovery.before", version=before.version)
                if before is not None and before.version != catalog.version
                else ""
            )
            key = "discovery.updated" if before is not None else "discovery.cached"
            line = t(key, version=catalog.version, previous=previous, modules=modules)
            click.echo(line, err=True)
        return catalog

    def close(self) -> None:
        if "client" in self.__dict__:
            self.client.close()


def _terminal_width() -> int:
    import shutil  # noqa: PLC0415

    return shutil.get_terminal_size().columns


def usage(message: str, *lines: str) -> CliError:
    return CliError("client.usage", message, *lines, exit_code=USAGE)
