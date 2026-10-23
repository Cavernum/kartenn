"""Profiles of `config.toml` (design § Configuration, REQ-023-36), read by tomllib.

A profile names a server, a way to authenticate and preferences; never a secret. A
profile that holds one (`secret`, `password`, `token`…) is refused: secrets go through a
`TokenStore`, a file named by `KARTENN_SECRET_FILE`, or the standard input.
"""

from __future__ import annotations

import os
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, cast

import platformdirs

from kartenn._core.problems import ClientError

__all__ = ["Profile", "config_path", "read_profile"]

SECRET_KEYS = frozenset(
    {
        "access_token",
        "client_secret",
        "passphrase",
        "password",
        "refresh_token",
        "secret",
        "token",
        "totp",
    }
)
_KNOWN = frozenset(
    {"url", "auth", "account", "issuer", "ca_bundle", "lang", "timezone", "output", "http2"}
)


@dataclass(frozen=True)
class Profile:
    name: str
    url: str
    auth: str = "none"
    account: str | None = None
    issuer: str | None = None
    ca_bundle: str | None = None
    lang: str | None = None
    timezone: str | None = None
    output: str | None = None
    http2: bool | None = None
    extra: dict[str, Any] = field(default_factory=dict[str, Any])


def config_path() -> Path:
    """`$XDG_CONFIG_HOME/kartenn/config.toml`."""
    return Path(platformdirs.user_config_dir("kartenn")) / "config.toml"


def read_profile(name: str | None = None, *, path: Path | None = None) -> Profile:
    """The profile `name`, else `KARTENN_PROFILE`, else the `default` of the file."""
    source = path or config_path()
    try:
        config = tomllib.loads(source.read_text())
    except FileNotFoundError as error:
        msg = f"no configuration at {source}"
        raise ClientError("client.no-configuration", msg) from error
    except tomllib.TOMLDecodeError as error:
        msg = f"{source} is not valid TOML: {error}"
        raise ClientError("client.invalid-configuration", msg) from error
    chosen = name or os.environ.get("KARTENN_PROFILE") or config.get("default")
    profiles = cast("dict[str, Any]", config.get("profiles") or {})
    if not isinstance(chosen, str) or not isinstance(profiles.get(chosen), dict):
        known = ", ".join(sorted(profiles)) or "none"
        msg = f"no profile {chosen!r} in {source} (profiles: {known})"
        raise ClientError("client.unknown-profile", msg)
    raw = cast("dict[str, Any]", profiles[chosen])
    secrets = sorted(SECRET_KEYS & set(raw))
    if secrets:
        msg = (
            f"the profile {chosen!r} holds a secret ({', '.join(secrets)}): keep it in the "
            "keyring, a file named by KARTENN_SECRET_FILE, or the standard input"
        )
        raise ClientError("client.secret-in-profile", msg)
    url = raw.get("url")
    if not isinstance(url, str) or not url:
        msg = f"the profile {chosen!r} has no url"
        raise ClientError("client.invalid-configuration", msg)
    known = {k: v for k, v in raw.items() if k in _KNOWN and k != "url"}
    http2 = known.pop("http2", None)
    return Profile(
        name=chosen,
        url=url,
        http2=http2 if isinstance(http2, bool) else None,
        extra={k: v for k, v in raw.items() if k not in _KNOWN},
        **{k: str(v) for k, v in known.items()},
    )
