"""Bench of the CLI (spec 023, task 27, REQ-023-39): `bench-cli [runs]`.

The budgets measured are those of the client alone, so the server is a stub on the
loopback: `GET /v1/capabilities` (four modules) and `GET /v1/openapi.json`, the reference
contract of the repository (specs/006-api-rest/openapi-v1.json, about 0.8 MiB), with its
ETag. No PostgreSQL. Each command is the installed `kartenn` script in a subprocess, as
a user runs it; p50 and p95 over `runs` runs (30 by default), after one run unmeasured.

- `--version` (< 80 ms), `--help` and `policy --help` at warm cache (< 200 ms);
- a local completion, `prefixes list --<TAB>` at warm cache (< 100 ms);
- the first contact: `--help` at cold cache, discovery and derivation of the catalogue
  included (< 1.5 s);
- `--help` past the minute of freshness: capabilities read again, contract answered
  304 (no budget of its own; reported).

Exit code 1 when a budget is exceeded at the p95.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[3]
CONTRACT = (ROOT / "specs/006-api-rest/openapi-v1.json").read_bytes()
ETAG = '"' + hashlib.sha256(CONTRACT).hexdigest()[:32] + '"'
CAPABILITIES = json.dumps(
    {
        "version": "0.2.0",
        "api_versions": ["v1"],
        "modules": [
            {"name": name, "version": "0.2.0"} for name in ("policy", "flow", "vlan", "machines")
        ],
        "properties": [],
        "ui_plugins": [],
        "limits": {
            "page_size_default": 100,
            "page_size_max": 1000,
            "changeset_operations": 10000,
            "chunk_elements": 10000,
            "body_bytes": 1 << 24,
            "idempotency_retention_seconds": 86400,
        },
        "events": None,
    }
).encode()
KARTENN = shutil.which("kartenn", path=str(Path(sys.executable).parent)) or "kartenn"
# Budgets of REQ-023-39, in milliseconds, at the p95.
BUDGETS = {
    "--version": 80,
    "--help": 200,
    "policy --help": 200,
    "complétion locale": 100,
    "premier contact": 1500,
}


class Stub(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self) -> None:
        if self.path == "/v1/capabilities":
            self._send(200, CAPABILITIES)
        elif self.path == "/v1/openapi.json":
            if self.headers.get("If-None-Match") == ETAG:
                self._send(304, b"")
            else:
                self._send(200, CONTRACT)
        else:
            self._send(404, b"{}")

    def _send(self, status: int, body: bytes) -> None:
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("ETag", ETAG)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002, ANN401
        del format, args


def timed(argv: list[str], env: dict[str, str], *, before: Any = None) -> float:  # noqa: ANN401
    """Milliseconds of one run of `argv`, which must succeed."""
    if before is not None:
        before()
    start = time.perf_counter()
    subprocess.run(  # noqa: S603
        argv, env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    return (time.perf_counter() - start) * 1000


def measure(
    argv: list[str],
    env: dict[str, str],
    runs: int,
    *,
    before: Any = None,  # noqa: ANN401
) -> tuple[float, float]:
    timed(argv, env, before=before)
    samples = sorted(timed(argv, env, before=before) for _ in range(runs))
    p95 = samples[min(len(samples) - 1, round(0.95 * len(samples)) - 1)]
    return statistics.median(samples), p95


def main() -> int:
    runs = int(sys.argv[1]) if len(sys.argv) > 1 else 30
    server = ThreadingHTTPServer(("127.0.0.1", 0), Stub)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{server.server_address[1]}"
    with tempfile.TemporaryDirectory(prefix="bench-cli-") as home:
        base = Path(home)
        cache = base / "cache"
        env = {k: v for k, v in os.environ.items() if not k.startswith(("KARTENN_", "_KARTENN"))}
        env |= {
            "XDG_CONFIG_HOME": str(base / "config"),
            "XDG_CACHE_HOME": str(cache),
            "LANG": "fr_FR.UTF-8",
            "NO_COLOR": "1",
        }
        subprocess.run(  # noqa: S603
            [KARTENN, "config", "add-profile", "bench", "--url", url, "--auth", "none"],
            env=env,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        completion = env | {
            "_KARTENN_COMPLETE": "bash_complete",
            "COMP_WORDS": "kartenn prefixes list --",
            "COMP_CWORD": "3",
        }

        # The completion answers the options of the command, not an error.
        subprocess.run([KARTENN, "--help"], env=env, check=True, capture_output=True)  # noqa: S603
        offered = subprocess.run(  # noqa: S603
            [KARTENN], env=completion, check=True, capture_output=True, text=True
        ).stdout
        if "--domain" not in offered:
            print(f"bench-cli : complétion inattendue\n{offered}", file=sys.stderr)
            return 1

        def cold() -> None:
            shutil.rmtree(cache, ignore_errors=True)

        def stale() -> None:
            # The pointer of the server older than the minute of freshness (`checked`):
            # capabilities read again, contract revalidated.
            for pointer in (cache / "kartenn" / "catalog").glob("*.json"):
                if "-" not in pointer.stem:
                    data = json.loads(pointer.read_text())
                    data["checked"] = time.time() - 120
                    pointer.write_text(json.dumps(data))

        rows = [
            ("--version", measure([KARTENN, "--version"], env, runs)),
            ("premier contact", measure([KARTENN, "--help"], env, runs, before=cold)),
            ("--help", measure([KARTENN, "--help"], env, runs)),
            ("policy --help", measure([KARTENN, "policy", "--help"], env, runs)),
            ("complétion locale", measure([KARTENN], completion, runs)),
            ("--help, cache périmé", measure([KARTENN, "--help"], env, runs, before=stale)),
        ]
    server.shutdown()
    over = False
    print(f"bench-cli — {runs} exécutions, contrat de {len(CONTRACT) / 2**20:.2f} Mio")
    print(f"{'mesure':<22} {'p50 ms':>8} {'p95 ms':>8} {'budget':>8}")
    for name, (p50, p95) in rows:
        budget = BUDGETS.get(name)
        mark = "" if budget is None else ("  ok" if p95 < budget else "  DÉPASSÉ")
        over = over or (budget is not None and p95 >= budget)
        shown = "—" if budget is None else f"< {budget}"
        print(f"{name:<22} {p50:>8.0f} {p95:>8.0f} {shown:>8}{mark}")
    return 1 if over else 0


if __name__ == "__main__":
    sys.exit(main())
