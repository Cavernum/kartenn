"""The runtime of an invocation: catalog from the cache, discovery, `--verbose`
(REQ-023-2, 41)."""

from __future__ import annotations

import httpx2
import pytest
from click.testing import CliRunner

from fakes import FakeServer, capabilities, profile
from kartenn._core.messages import Request
from kartenn.catalog import CatalogCache
from kartenn_cli import runtime
from kartenn_cli.config import Settings
from kartenn_cli.runtime import Runtime
from kartenn_cli.verbose import Verbose


@pytest.mark.usefixtures("server")
def test_discovery_is_said_once_then_read_from_the_cache() -> None:
    runner = CliRunner()
    with runner.isolation() as (_out, err, _):
        first = Runtime(Settings())
        assert first.cached_catalog() is None
        assert first.catalog.version == "0.2.0"
        said = err.getvalue().decode()
    assert said == (
        "Découverte du serveur… 0.2.0, modules : vlan, machines, policy, flow "
        "(catalogue en cache).\n"
    )
    cached = Runtime(Settings()).cached_catalog()
    assert cached is not None
    assert cached.version == "0.2.0"


def test_a_new_version_is_said(server: FakeServer) -> None:
    runner = CliRunner()
    with runner.isolation():
        _ = Runtime(Settings()).catalog
    server.capabilities = capabilities(version="0.3.0")
    with runner.isolation() as (_out, err, _):
        found = Runtime(Settings())
        found.cache = CatalogCache(max_age=0)
        assert found.catalog.version == "0.3.0"
        said = err.getvalue().decode()
    assert said.startswith("Découverte du serveur… 0.3.0 (avant 0.2.0), modules :")
    assert said.endswith("(catalogue mis à jour).\n")


def test_verbose_tells_each_request(server: FakeServer) -> None:
    server.route("GET", "/v1/me", httpx2.Response(200, json={}, headers={"Request-Id": "r-1"}))
    runner = CliRunner()
    with runner.isolation() as (_out, err, _):
        transport = Verbose(server.transport())
        assert transport.send(Request("GET", "/v1/me")).status == 200
        with transport.stream(Request("GET", "/v1/me")) as streamed:
            assert streamed.status == 200
        with transport.sse(Request("GET", "/v1/me")) as stream:
            assert stream.status == 200
        transport.sleep(0)
        transport.close()
        said = err.getvalue().decode().splitlines()
    assert said[0].startswith("GET /v1/me → 200 (")
    assert said[0].endswith("ms)  r-1")
    assert len(said) == 3


def test_verbose_wraps_the_transport(server: FakeServer) -> None:
    del server
    found = Runtime(Settings(verbose=1))
    assert isinstance(found.transport(found.require_profile()), Verbose)
    runtime.TRANSPORT.clear()
    assert isinstance(found.transport(found.require_profile()), Verbose)
    assert Runtime(Settings()).transport(found.require_profile()) is None
    found.close()
    profile()
