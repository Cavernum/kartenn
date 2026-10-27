"""A kartenn server simulated under `httpx2.MockTransport` for the CLI: the reference
contract, its capabilities, and the routes a test declares."""

from __future__ import annotations

import copy
import json
import re
from collections.abc import Callable
from pathlib import Path
from typing import Any

import httpx2
from click.testing import CliRunner, Result

from kartenn.transport.httpx2 import Httpx2Transport
from kartenn_cli.config import write_config
from kartenn_cli.main import cli

ROOT = Path(__file__).resolve().parents[4]
CONTRACT: dict[str, Any] = json.loads((ROOT / "specs/006-api-rest/openapi-v1.json").read_text())
URL = "https://kartenn.example.org"
Handler = Callable[[httpx2.Request], httpx2.Response]
MODULES = [
    {"name": "machines", "version": "0.2.0"},
    {"name": "policy", "version": "0.2.0"},
    {"name": "flow", "version": "0.2.0"},
]


def capabilities(**changes: object) -> dict[str, Any]:
    base: dict[str, Any] = {
        "version": "0.2.0",
        "api_versions": ["v1"],
        "modules": MODULES,
        "properties": [],
        "ui_plugins": [],
        "limits": {
            "page_size_default": 100,
            "page_size_max": 1000,
            "changeset_operations": 10000,
            "chunk_elements": 5000,
            "body_bytes": 1 << 24,
            "idempotency_retention_seconds": 86400,
        },
        "events": None,
    }
    return {**base, **changes}


def without(contract: dict[str, Any], *tags: str) -> dict[str, Any]:
    """The contract without the operations of these tags."""
    trimmed = copy.deepcopy(contract)
    for path, item in list(trimmed["paths"].items()):
        for method in list(item):
            operation: dict[str, Any] = item[method] if isinstance(item[method], dict) else {}
            if set(operation.get("tags", [])) & set(tags):
                del item[method]
        if not any(isinstance(v, dict) and "operationId" in v for v in item.values()):
            del trimmed["paths"][path]
    return trimmed


def with_keys(contract: dict[str, Any]) -> dict[str, Any]:
    """The contract with the natural keys and the columns of `x-kartenn` (A5)."""
    keyed = copy.deepcopy(contract)
    paths = keyed["paths"]
    paths["/v1/prefixes"]["x-kartenn"] = {
        "key": ["domain", "value"],
        "columns": ["domain", "value", "nature"],
    }
    paths["/v1/domains"]["x-kartenn"] = {"key": ["slug"]}
    paths["/v1/vlans"]["x-kartenn"] = {"key": ["group", "vid"]}
    return keyed


def problem(status: int, slug: str, **members: object) -> httpx2.Response:
    body = {"type": f"urn:kartenn:problem:{slug}", "title": slug, "status": status, **members}
    return httpx2.Response(status, json=body, headers={"Content-Type": "application/problem+json"})


def body(request: httpx2.Request) -> Any:  # noqa: ANN401 - JSON
    return json.loads(request.content) if request.content else None


class FakeServer:
    """Routes by method and path pattern (`/v1/prefixes/{id}`), the rest a 404."""

    def __init__(
        self,
        contract: dict[str, Any] | None = None,
        caps: dict[str, Any] | None = None,
    ) -> None:
        self.contract = CONTRACT if contract is None else contract
        self.capabilities = capabilities() if caps is None else caps
        self.routes: list[tuple[str, re.Pattern[str], Handler]] = []
        self.seen: list[httpx2.Request] = []
        self.discoveries = 0

    def route(self, method: str, path: str, answer: Handler | httpx2.Response | object) -> None:
        pattern = re.compile(re.sub(r"\\\{[^}]+\\\}", "[^/]+", re.escape(path)) + "$")
        if callable(answer):
            handler: Handler = answer  # type: ignore[assignment]
        elif isinstance(answer, httpx2.Response):
            response = answer

            def handler(_request: httpx2.Request) -> httpx2.Response:
                return response
        else:
            data = answer

            def handler(_request: httpx2.Request) -> httpx2.Response:
                return httpx2.Response(200, json=data)

        self.routes.insert(0, (method, pattern, handler))

    def __call__(self, request: httpx2.Request) -> httpx2.Response:
        path = request.url.path
        if path == "/v1/capabilities":
            self.discoveries += 1
            return httpx2.Response(200, json=self.capabilities)
        if path == "/v1/openapi.json":
            return httpx2.Response(200, json=self.contract, headers={"ETag": '"c1"'})
        self.seen.append(request)
        for method, pattern, handler in self.routes:
            if method == request.method and pattern.match(path):
                return handler(request)
        return problem(404, "not-found")

    def transport(self, url: str = URL) -> Httpx2Transport:
        return Httpx2Transport(url, mock=httpx2.MockTransport(self))


def profile(name: str = "prod", url: str = URL, **members: Any) -> None:  # noqa: ANN401
    """A profile in the configuration, chosen by default."""
    write_config(
        {
            "default": name,
            "profiles": {name: {"url": url, "auth": "none", "timezone": "Europe/Paris", **members}},
        }
    )


def kartenn(*args: str, input: str | None = None) -> Result:  # noqa: A002 - as click names it
    """`kartenn ARGS…` in process."""
    return CliRunner().invoke(cli, list(args), prog_name="kartenn", input=input)
