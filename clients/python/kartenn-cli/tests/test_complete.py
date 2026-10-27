"""Completion for bash, zsh and fish (REQ-023-24; maquette § 5)."""

from __future__ import annotations

import os
import shutil
import sys
import time
from typing import TYPE_CHECKING, Any

import httpx2
import pytest
from click.testing import CliRunner

from fakes import CONTRACT, URL, FakeServer, capabilities, kartenn, profile, with_keys
from kartenn.catalog import CatalogCache, derive
from kartenn_cli import complete, runtime
from kartenn_cli.main import cli

if TYPE_CHECKING:
    from pathlib import Path

DOMAINS = [
    {"id": "1", "slug": "dmz", "label": "Zone démilitarisée"},
    {"id": "2", "slug": "interne"},
    {"id": "3", "slug": "labo"},
    {"id": "4", "slug": "public"},
]
ENVIRONMENTS = [
    {"key": "prod", "label": "Production"},
    {"key": "recette", "label": "Recette"},
    {"key": "dev", "label": "Développement"},
    {"key": "secret", "masked": True},
]
PROPERTIES = [
    {"key": "core.owner", "type": "text"},
    {"key": "dns.ttl", "type": "integer"},
    {"key": "policy.environment", "type": "enum"},
]


def complete_in(shell: str, line: str) -> list[str]:
    """The proposals of `line` as the script of `shell` asks them (click's protocol)."""
    words = line.split(" ")
    env = {
        "_KARTENN_COMPLETE": f"{shell}_complete",
        "COMP_WORDS": line,
        "COMP_CWORD": str(len(words) - 1) if shell != "fish" else words[-1],
    }
    result = CliRunner().invoke(cli, [], prog_name="kartenn", env=env)
    assert result.exit_code == 0, result.output
    assert result.stderr == ""
    return [line for line in result.stdout.splitlines() if line]


@pytest.fixture
def served(server: FakeServer) -> FakeServer:
    server.contract = with_keys(CONTRACT)
    server.capabilities = capabilities(properties=PROPERTIES)
    server.route("GET", "/v1/domains", {"items": DOMAINS, "links": {}})
    server.route(
        "GET",
        "/v1/property-declarations/{key}/values",
        {"items": ENVIRONMENTS, "links": {}},
    )
    kartenn("--help")
    return server


def bash_values(lines: list[str]) -> list[str]:
    return [line.split(",", 1)[1] for line in lines]


def test_scripts_of_the_three_shells() -> None:
    assert "_KARTENN_COMPLETE=bash_complete" in kartenn("completion", "bash").stdout
    assert "#compdef kartenn" in kartenn("completion", "zsh").stdout
    assert "complete --no-files --command kartenn" in kartenn("completion", "fish").stdout
    assert kartenn("completion", "powershell").exit_code == 2


@pytest.mark.usefixtures("served")
def test_commands_and_verbs_from_the_cache_only(served: FakeServer) -> None:
    assert bash_values(complete_in("bash", "kartenn pre")) == ["prefixes"]
    verbs = bash_values(complete_in("bash", "kartenn vlans "))
    assert {"list", "get", "history", "properties", "prefixes"} <= set(verbs)
    assert served.seen == []


def test_slugs_of_references_from_the_server_kept_five_minutes(served: FakeServer) -> None:
    lines = complete_in("zsh", "kartenn prefixes list --domain ")
    assert lines == [
        "plain", "dmz", "Zone démilitarisée",
        "plain", "interne", "_",
        "plain", "labo", "_",
        "plain", "public", "_",
    ]  # fmt: skip
    asked = len(served.seen)
    assert bash_values(complete_in("bash", "kartenn prefixes list --domain in")) == ["interne"]
    assert len(served.seen) == asked
    cache = complete.ValueCache(URL, clock=lambda: time.time() + complete.CACHE_SECONDS)
    assert cache.get("ref:domains") is None
    assert oct(cache.path.stat().st_mode & 0o777) == "0o600"


@pytest.mark.usefixtures("served")
def test_property_keys_then_closed_values() -> None:
    assert bash_values(complete_in("bash", "kartenn prefixes list --prop ")) == [
        "core.owner=",
        "dns.ttl=",
        "policy.environment=",
    ]
    lines = complete_in("fish", "kartenn prefixes list --prop policy.environment=")
    assert lines == [
        "plain,policy.environment=prod\tProduction",
        "plain,policy.environment=recette\tRecette",
        "plain,policy.environment=dev\tDéveloppement",
    ]


def test_a_long_closed_list_is_asked_by_prefix(served: FakeServer) -> None:
    """A list longer than a page is asked again with `q` for each prefix (spec 018,
    REQ-018-22); a whole list, read once and filtered here."""

    def by_prefix(request: httpx2.Request) -> httpx2.Response:
        prefix = request.url.params.get("q", "")
        items = [e for e in ENVIRONMENTS if str(e["key"]).startswith(prefix)]
        return httpx2.Response(200, json={"items": items, "links": {"next": "/v1/more"}})

    served.route("GET", "/v1/property-declarations/{key}/values", by_prefix)
    line = "kartenn prefixes list --prop policy.environment=re"
    assert bash_values(complete_in("bash", line)) == ["policy.environment=recette"]
    asked = served.seen[-1].url.params
    assert asked.get("q") == "re"
    assert asked.get("page_size") == str(complete.SUGGESTIONS)
    count = len(served.seen)
    assert bash_values(complete_in("bash", line)) == ["policy.environment=recette"]
    assert len(served.seen) == count, "kept by prefix"
    line = "kartenn prefixes list --prop policy.environment=d"
    assert bash_values(complete_in("bash", line)) == ["policy.environment=dev"]
    assert served.seen[-1].url.params.get("q") == "d"


def test_slugs_of_a_long_list_are_asked_by_prefix(served: FakeServer) -> None:
    """A list of references longer than a page is asked again with `q` (spec 018,
    REQ-018-23)."""

    def by_prefix(request: httpx2.Request) -> httpx2.Response:
        prefix = request.url.params.get("q", "")
        items = [d for d in DOMAINS if str(d["slug"]).startswith(prefix)]
        return httpx2.Response(200, json={"items": items, "links": {"next": "/v1/more"}})

    served.route("GET", "/v1/domains", by_prefix)
    assert bash_values(complete_in("bash", "kartenn prefixes list --domain la")) == ["labo"]
    assert served.seen[-1].url.params.get("q") == "la"


def test_a_list_without_suggestions_gives_its_first_page() -> None:
    """A list whose contract declares no `q` (a module's): its first page, unfiltered."""
    from kartenn import ClientError  # noqa: PLC0415

    class Page:
        items = ({"slug": "ssh"}, {"slug": "web"})
        next = None

    class Listing:
        def __init__(self, filters: dict[str, object]) -> None:
            if "q" in filters:
                raise ClientError("client.unknown-filter", "no q")

        def first_page(self) -> Page:
            return Page()

    class Resource:
        def list(self, **filters: object) -> Listing:
            return Listing(filters)

    class Client:
        def resource(self, name: str) -> Resource:
            assert name == "services"
            return Resource()

    fetch = complete._references("services")  # noqa: SLF001  # pyright: ignore[reportPrivateUsage]
    assert fetch(Client(), "w") == ([("ssh", ""), ("web", "")], True)  # type: ignore[arg-type]


def test_a_mute_server_leaves_the_local_proposals_within_a_second(
    served: FakeServer,
) -> None:
    def mute(_request: httpx2.Request) -> httpx2.Response:
        time.sleep(3)
        return httpx2.Response(200, json={"items": [], "links": {}})

    served.route("GET", "/v1/domains", mute)
    started = time.monotonic()
    assert complete_in("bash", "kartenn prefixes list --domain ") == []
    assert bash_values(complete_in("bash", "kartenn prefixes list --prop dns")) == ["dns.ttl="]
    assert time.monotonic() - started < 1.5


def test_a_failing_server_proposes_nothing(served: FakeServer) -> None:
    served.route("GET", "/v1/domains", httpx2.Response(500, json={}))
    assert complete_in("bash", "kartenn prefixes list --domain ") == []


def test_without_profile_or_cache_nothing_is_asked() -> None:
    assert "plain,prefixes" not in complete_in("bash", "kartenn ")
    assert bash_values(complete_in("bash", "kartenn conf")) == ["config"]
    profile()
    runtime.TRANSPORT["factory"] = lambda *_args: pytest.fail("no request")  # type: ignore[misc]
    assert complete_in("bash", "kartenn pre") == []
    assert not any("=" in line for line in complete_in("bash", "kartenn prefixes list --prop "))


def test_reference_of_a_filter() -> None:
    catalog = derive(URL, capabilities(), CONTRACT)
    assert complete.reference_of("domain", catalog) == "domains"
    assert complete.reference_of("entity", catalog) == "entities"
    assert complete.reference_of("flow_type", catalog) == "flow-types"
    assert complete.reference_of("prefix", catalog) == "prefixes"
    assert complete.reference_of("nature", catalog) is None


def test_a_broken_cache_is_an_empty_one(tmp_path: Path) -> None:
    del tmp_path
    cache = complete.ValueCache(URL)
    cache.path.parent.mkdir(parents=True)
    cache.path.write_text("[")
    assert cache.get("x") is None
    cache.path.write_text('{"x": 1, "y": {"at": "never"}}')
    assert cache.get("x") is None
    assert cache.get("y") is None


# Real shells --------------------------------------------------------------------------


@pytest.fixture
def warm(tmp_path: Path) -> dict[str, str]:
    """A profile and its catalog in the cache, for a `kartenn` run by a shell."""
    profile()
    CatalogCache().save(derive(URL, capabilities(), CONTRACT, contract_etag='"c1"'))
    return {
        **os.environ,
        "PATH": f"{os.path.dirname(os.path.normpath(sys.executable))}:{os.environ['PATH']}",  # noqa: PTH120 - no symlink followed
        "HOME": str(tmp_path),
        "TERM": "dumb",
    }


def _shell(command: list[str], env: dict[str, str]) -> Any:  # noqa: ANN401 - a pexpect child
    import pexpect  # noqa: PLC0415

    spawn: Any = pexpect.spawn
    return spawn(command[0], command[1:], env=env, encoding="utf-8", timeout=20)


@pytest.mark.skipif(shutil.which("bash") is None, reason="bash absent")
def test_in_bash(warm: dict[str, str]) -> None:
    child = _shell(["bash", "--norc", "--noprofile", "-i"], warm)
    child.sendline("PS1='$ '; source <(kartenn completion bash); echo re''ady")
    child.expect("ready")
    child.expect_exact("$ ")
    time.sleep(0.5)  # the line editor reads keys once the prompt is drawn
    child.send("kartenn prefi\t")
    child.expect("kartenn prefixes ")
    child.sendcontrol("c")
    child.sendline("exit")
    child.close()


@pytest.mark.skipif(shutil.which("zsh") is None, reason="zsh absent")
def test_in_zsh(warm: dict[str, str]) -> None:
    child = _shell(["zsh", "-f", "-i"], warm)
    child.sendline(
        "PS1='$ '; autoload -Uz compinit && compinit -u; "
        'eval "$(kartenn completion zsh)"; echo re""ady'
    )
    child.expect("ready")
    child.expect_exact("$ ")
    time.sleep(0.5)  # the line editor reads keys once the prompt is drawn
    child.send("kartenn prefi\t")
    child.expect("prefixes")
    child.sendcontrol("c")
    child.sendline("exit")
    child.close()


@pytest.mark.skipif(shutil.which("fish") is None, reason="fish absent from this machine")
def test_in_fish(warm: dict[str, str]) -> None:  # pragma: no cover - fish not installed here
    child = _shell(["fish", "--no-config", "-i"], warm)
    child.sendline("kartenn completion fish | source; echo re''ady")
    child.expect("ready")
    child.expect_exact("$ ")
    time.sleep(0.5)  # the line editor reads keys once the prompt is drawn
    child.send("kartenn prefi\t")
    child.expect("prefixes")
    child.sendcontrol("c")
    child.sendline("exit")
    child.close()
