"""A kartenn server simulated under `httpx2.MockTransport`: the reference contract, its
capabilities, and the routes a test declares."""

from __future__ import annotations

import json
import re
from collections.abc import Callable
from pathlib import Path
from typing import Any

import httpx2

from kartenn.catalog import CatalogCache
from kartenn.client import AsyncClient, Client
from kartenn.transport.httpx2 import AsyncHttpx2Transport, Httpx2Transport

ROOT = Path(__file__).resolve().parents[4]
CONTRACT: dict[str, Any] = json.loads((ROOT / "specs/006-api-rest/openapi-v1.json").read_text())
URL = "https://kartenn.test"
Handler = Callable[[httpx2.Request], httpx2.Response]
MODULES = [
    {"name": "policy", "version": "0.1.0"},
    {"name": "flow", "version": "0.1.0"},
    {"name": "vlan", "version": "0.1.0"},
    {"name": "machines", "version": "0.1.0"},
]


def capabilities(**changes: object) -> dict[str, Any]:
    base: dict[str, Any] = {
        "version": "0.1.0",
        "api_versions": ["v1"],
        "modules": MODULES,
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
    return {**base, **changes}


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

    def route(self, method: str, path: str, answer: Handler | httpx2.Response) -> None:
        pattern = re.compile(re.sub(r"\\\{[^}]+\\\}", "[^/]+", re.escape(path)) + "$")
        handler: Handler = answer if callable(answer) else (lambda _request: answer)
        self.routes.insert(0, (method, pattern, handler))

    def __call__(self, request: httpx2.Request) -> httpx2.Response:
        path = request.url.path
        if path == "/v1/capabilities":
            return httpx2.Response(200, json=self.capabilities)
        if path == "/v1/openapi.json":
            return httpx2.Response(200, json=self.contract)
        self.seen.append(request)
        for method, pattern, handler in self.routes:
            if method == request.method and pattern.match(path):
                return handler(request)
        return problem(404, "not-found")

    def transport(self) -> Httpx2Transport:
        return Httpx2Transport(URL, mock=httpx2.MockTransport(self))

    def client(self, tmp_path: Path, **options: Any) -> Client:  # noqa: ANN401
        return Client(URL, transport=self.transport(), cache=CatalogCache(tmp_path), **options)

    def async_client(self, tmp_path: Path, **options: Any) -> AsyncClient:  # noqa: ANN401
        transport = AsyncHttpx2Transport(URL, mock=httpx2.MockTransport(self))
        return AsyncClient(URL, transport=transport, cache=CatalogCache(tmp_path), **options)
