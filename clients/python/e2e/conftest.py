"""Fixtures of the end-to-end sessions of the CLI (spec 023, task 26, `k-py-e2e`).

`k-py-e2e` has run `web/e2e/serve.sh cli-prepare` (builds, databases, break-glass
accounts); the session starts the issuer (`serve.sh idp`) then both instances
(`serve.sh cli`, `serve.sh cli-labo`) and stops them at the end, by the processes it
started. Every command is the installed `kartenn` script, in a subprocess, with its own
XDG directories, French messages and a token file (no keyring on a runner).
"""

from __future__ import annotations

import base64
import hashlib
import hmac
import os
import re
import shutil
import ssl
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Any

import pexpect
import pytest

if TYPE_CHECKING:
    from collections.abc import Iterator

REPO = Path(__file__).resolve().parents[3]
STATE = REPO / "web" / "e2e" / ".state"
HERE = Path(__file__).resolve().parent
PROD = "https://127.0.0.1:9443"
LABO = "https://127.0.0.1:9445"
ISSUER = "https://127.0.0.1:9444"
KARTENN = shutil.which("kartenn", path=str(Path(sys.executable).parent)) or "kartenn"

UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
SHORT_UUID = re.compile(r"\b[0-9a-f]{4}…")
STAMP = re.compile(
    r"\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:\d{2})?)?"
)
URN = re.compile(r"urn:uuid:<uuid>")


def normalized(text: str) -> str:
    """UUIDs and dates replaced, so that a session compares from one run to the next."""
    text = UUID.sub("<uuid>", text)
    text = SHORT_UUID.sub("<id>…", text)
    return STAMP.sub("<date>", text)


def _wait(url: str, process: subprocess.Popen[bytes], log: Path, seconds: float) -> None:
    context = ssl.create_default_context(cafile=str(STATE / "ca.pem"))
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if process.poll() is not None:
            pytest.fail(f"{url}: the server stopped ({log})\n{log.read_text()[-2000:]}")
        try:
            with urllib.request.urlopen(url, context=context, timeout=2):  # noqa: S310
                return
        except (OSError, urllib.error.URLError, ssl.SSLError):
            time.sleep(0.2)
    pytest.fail(f"{url} does not answer after {seconds} s ({log})")


@pytest.fixture(scope="session")
def servers(tmp_path_factory: pytest.TempPathFactory) -> Iterator[None]:
    """The issuer, then the instance with every module and the one without `policy`."""
    if os.environ.get("E2E_SERVERS") == "running":  # servers started by hand, to debug
        yield
        return
    logs = tmp_path_factory.mktemp("servers")
    started: list[subprocess.Popen[bytes]] = []

    def start(what: str, ready: str, seconds: float) -> None:
        log = logs / f"{what}.log"
        with log.open("wb") as out:
            process = subprocess.Popen(
                ["dash", "web/e2e/serve.sh", what],  # noqa: S607 - dash of the devenv shell
                cwd=REPO,
                stdout=out,
                stderr=subprocess.STDOUT,
            )
        started.append(process)
        _wait(ready, process, log, seconds)

    try:
        start("idp", f"{ISSUER}/.well-known/openid-configuration", 300)
        start("cli", f"{PROD}/health/ready", 120)
        start("cli-labo", f"{LABO}/health/ready", 120)
        yield
    finally:
        for process in reversed(started):
            process.terminate()
        for process in started:
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()


@dataclass
class Run:
    code: int
    out: str
    err: str

    @property
    def both(self) -> str:
        return normalized(self.out + self.err)


@dataclass
class Cli:
    """`kartenn` as a user runs it, in its own home."""

    home: Path
    env: dict[str, str] = field(default_factory=dict[str, str])

    @property
    def tokens(self) -> Path:
        return self.home / "tokens.json"

    def environ(self, **extra: str) -> dict[str, str]:
        env = {
            k: v
            for k, v in os.environ.items()
            if not k.startswith(("KARTENN_", "LC_")) and k not in {"VISUAL", "EDITOR"}
        }
        env.update(
            {
                "XDG_CONFIG_HOME": str(self.home / "config"),
                "XDG_CACHE_HOME": str(self.home / "cache"),
                "XDG_STATE_HOME": str(self.home / "state"),
                "XDG_RUNTIME_DIR": str(self.home / "run"),
                "LANG": "fr_FR.UTF-8",
                "NO_COLOR": "1",
                "TZ": "UTC",
                "BROWSER": f"{sys.executable} {HERE / 'browser.py'} %s",
                "KARTENN_CHANGESET": str(self.home / "changeset.yaml"),
            }
        )
        env.update(self.env)
        env.update(extra)
        return env

    def argv(self, *args: str) -> list[str]:
        return [KARTENN, "--token-file", str(self.tokens), *args]

    def __call__(self, *args: str, stdin: str | None = None, code: int | None = 0) -> Run:
        """The command, whose exit code must be `code` (`None`: any)."""
        done = subprocess.run(  # noqa: PLW1510 - the code is checked below
            self.argv(*args),
            input=stdin,
            capture_output=True,
            text=True,
            env=self.environ(),
            cwd=self.home,
            timeout=120,
        )
        run = Run(done.returncode, done.stdout, done.stderr)
        if code is not None:
            assert done.returncode == code, f"kartenn {' '.join(args)}\n{done.stdout}{done.stderr}"
        return run

    def terminal(self, *args: str) -> pexpect.spawn[str]:
        """The command on a terminal (questions, browser, hidden prompts)."""
        spawn: Any = pexpect.spawn
        return spawn(
            KARTENN,
            ["--token-file", str(self.tokens), *args],
            env=self.environ(),
            cwd=str(self.home),
            encoding="utf-8",
            timeout=60,
            dimensions=(50, 160),
        )


def totp(secret: str, now: float | None = None) -> str:
    """The current TOTP code of `secret` (RFC 6238: SHA-1, 30 s, 6 digits)."""
    key = base64.b32decode(secret + "=" * (-len(secret) % 8))
    counter = struct.pack(">Q", int((time.time() if now is None else now) // 30))
    digest = hmac.new(key, counter, hashlib.sha1).digest()
    offset = digest[19] & 0x0F
    value = struct.unpack(">I", digest[offset : offset + 4])[0] & 0x7FFFFFFF
    return f"{value % 1_000_000:06d}"


def breakglass(cli: Cli, profile: str, proofs: str) -> None:
    """`kartenn login --breakglass --keep` on a terminal, with the proofs written by
    `serve.sh cli-prepare` (the only secret outside the API, REQ-005-14)."""
    text = (STATE / proofs).read_text()
    passphrase = re.search(r"^passphrase: (.+)$", text, re.MULTILINE)
    secret = re.search(r"secret=([A-Z2-7]+)", text)
    assert passphrase, f"no passphrase in {proofs}"
    assert secret, f"no TOTP secret in {proofs}"
    child = cli.terminal("-p", profile, "login", "--breakglass", "--name", "e2e", "--keep")
    child.expect("Phrase de passe")
    child.sendline(passphrase[1])
    child.expect("Code TOTP")
    child.sendline(totp(secret[1]))
    child.expect(pexpect.EOF)
    child.close()
    assert child.exitstatus == 0, str(child.before)


@pytest.fixture(scope="session")
def cli(servers: None, tmp_path_factory: pytest.TempPathFactory) -> Cli:
    """One user, Alice, and her profiles: `prod` (every module) and `labo` (no `policy`)."""
    del servers
    given = os.environ.get("E2E_HOME")  # a home kept after the run, to debug
    home = Path(given) if given else tmp_path_factory.mktemp("alice")
    for sub in ("config", "cache", "state", "run"):
        (home / sub).mkdir(mode=0o700, parents=True, exist_ok=True)
    return Cli(home)
