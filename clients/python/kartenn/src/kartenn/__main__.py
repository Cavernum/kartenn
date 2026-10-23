"""`python -m kartenn stubs generate`: the type stubs of a server (REQ-023-17), without
any dependency of the CLI.

The catalog comes from a profile, a URL, or a contract file read offline (with its
capabilities when given, `v1` and no module otherwise).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, cast

from kartenn._core.problems import KartennError
from kartenn.catalog import Catalog, derive
from kartenn.stubs import generate

__all__ = ["main"]


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python -m kartenn")
    commands = parser.add_subparsers(dest="command", required=True)
    stubs = commands.add_parser("stubs", help="type stubs of a server")
    actions = stubs.add_subparsers(dest="action", required=True)
    generate_parser = actions.add_parser("generate", help="write <out>/kartenn_server/__init__.pyi")
    source = generate_parser.add_mutually_exclusive_group()
    source.add_argument("--profile", help="a profile of config.toml")
    source.add_argument("--url", help="the URL of a server")
    source.add_argument("--contract", type=Path, help="a contract file, read offline")
    generate_parser.add_argument("--capabilities", type=Path, help="with --contract")
    generate_parser.add_argument("--out", type=Path, default=Path("typings"))
    return parser


def _read(path: Path) -> dict[str, Any]:
    return cast("dict[str, Any]", json.loads(path.read_text(encoding="utf-8")))


def _catalog(arguments: argparse.Namespace) -> Catalog:
    if arguments.contract is not None:
        capabilities = (
            _read(arguments.capabilities)
            if arguments.capabilities is not None
            else {"version": "offline", "api_versions": ["v1"], "modules": []}
        )
        return derive(str(arguments.contract), capabilities, _read(arguments.contract))
    from kartenn.client import Client  # noqa: PLC0415 - only with a server

    client = Client(arguments.url) if arguments.url else Client.from_profile(arguments.profile)
    with client:
        return client.catalog


def main(argv: list[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        path = generate(_catalog(arguments), arguments.out)
    except KartennError as error:
        sys.stderr.write(f"kartenn: {error}\n")
        return 1
    sys.stdout.write(f"{path}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
