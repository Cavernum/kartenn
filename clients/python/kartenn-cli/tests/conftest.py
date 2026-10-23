"""Fixtures of the CLI: private XDG directories, a keyring in memory, a simulated server
behind a profile, and `kartenn` invoked in process."""

from __future__ import annotations

import os
from typing import TYPE_CHECKING

import keyring
import pytest
from hypothesis import HealthCheck, settings
from keyring.backend import KeyringBackend

from fakes import FakeServer, profile
from kartenn_cli import runtime
from kartenn_cli.i18n import set_lang

if TYPE_CHECKING:
    from collections.abc import Iterator
    from pathlib import Path

settings.register_profile(
    "kartenn", deadline=None, suppress_health_check=[HealthCheck.too_slow], max_examples=50
)
settings.load_profile("kartenn")


class MemoryKeyring(KeyringBackend):
    priority = 1  # type: ignore[assignment]

    def __init__(self) -> None:
        super().__init__()
        self.values: dict[tuple[str, str], str] = {}

    def get_password(self, service: str, username: str) -> str | None:
        return self.values.get((service, username))

    def set_password(self, service: str, username: str, password: str) -> None:
        self.values[(service, username)] = password

    def delete_password(self, service: str, username: str) -> None:
        self.values.pop((service, username), None)


@pytest.fixture(autouse=True)
def isolated(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Iterator[MemoryKeyring]:
    """Every test has its own XDG directories, French messages, no colours, and a keyring
    in memory."""
    for name in list(os.environ):
        if name.startswith(("KARTENN_", "LC_")):
            monkeypatch.delenv(name)
    for name in ("XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME", "XDG_RUNTIME_DIR"):
        monkeypatch.setenv(name, str(tmp_path / name.lower()))
    monkeypatch.setenv("LANG", "fr_FR.UTF-8")
    monkeypatch.setenv("NO_COLOR", "1")
    monkeypatch.setenv("TZ", "UTC")
    set_lang("fr")
    backend = MemoryKeyring()
    keyring.set_keyring(backend)
    yield backend
    runtime.TRANSPORT.clear()


@pytest.fixture
def server() -> FakeServer:
    """A simulated server behind the profile `prod`."""
    fake = FakeServer()
    profile()
    runtime.TRANSPORT["factory"] = lambda url, _profile: fake.transport(url)
    return fake
