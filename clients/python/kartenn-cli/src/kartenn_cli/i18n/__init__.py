"""Messages of the CLI in French and English (REQ-023-37, T21).

The catalogs are TOML files keyed by dotted names. The language follows `--lang`, then the
profile, then `LC_ALL`, `LC_MESSAGES` and `LANG` (the first one set decides, as POSIX
does); English is the fallback. Codes, keys and names of commands are never translated.
"""

from __future__ import annotations

import os
import tomllib
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

if TYPE_CHECKING:
    from collections.abc import Mapping

__all__ = ["LANGS", "current", "resolve", "set_lang", "t", "tn"]

LANGS = ("fr", "en")
FALLBACK = "en"
_DIRECTORY = Path(__file__).parent
_catalogs: dict[str, dict[str, str]] = {}
_state = {"lang": FALLBACK}


def _code(value: str) -> str:
    """`fr_FR.UTF-8` → `fr`; `C`, `POSIX` stay what they are."""
    return value.split(".", 1)[0].split("@", 1)[0].replace("-", "_").split("_", 1)[0].lower()


def resolve(
    option: str | None = None,
    profile: str | None = None,
    environ: Mapping[str, str] | None = None,
) -> str:
    """The language: the option, the profile, then the first locale variable set."""
    env = os.environ if environ is None else environ
    for given in (option, profile):
        if given and _code(given) in LANGS:
            return _code(given)
    for name in ("LC_ALL", "LC_MESSAGES", "LANG"):
        value = env.get(name)
        if value:
            code = _code(value)
            return code if code in LANGS else FALLBACK
    return FALLBACK


def _flatten(prefix: str, data: Mapping[str, Any], into: dict[str, str]) -> None:
    for key, value in data.items():
        name = f"{prefix}{key}"
        if isinstance(value, dict):
            _flatten(f"{name}.", cast("Mapping[str, Any]", value), into)
        else:
            into[name] = str(value)


def _catalog(lang: str) -> dict[str, str]:
    found = _catalogs.get(lang)
    if found is not None:
        return found
    loaded: dict[str, str] = {}
    _flatten("", tomllib.loads((_DIRECTORY / f"{lang}.toml").read_text("utf-8")), loaded)
    _catalogs[lang] = loaded
    return loaded


def set_lang(lang: str) -> None:
    _state["lang"] = lang if lang in LANGS else FALLBACK


def current() -> str:
    return _state["lang"]


def t(key: str, /, **values: object) -> str:
    """The message `key` in the current language, its `{names}` replaced."""
    text = _catalog(_state["lang"]).get(key)
    if text is None:
        text = _catalog(FALLBACK).get(key, key)
    return text.format(**values) if values else text


def tn(key: str, count: int, /, **values: object) -> str:
    """`key.one` or `key.other` after `count` (French: 0 and 1 are singular)."""
    one = count in {0, 1} if _state["lang"] == "fr" else count == 1
    return t(f"{key}.{'one' if one else 'other'}", count=count, **values)
