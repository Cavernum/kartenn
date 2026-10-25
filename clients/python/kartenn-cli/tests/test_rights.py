"""Groups of kartenn and their members from the CLI (spec 025, REQ-025-11; spec 023):
a group by its key, a member by its identity or `issuer/group`, `bindings create --group`,
`--dry-run`, `--into-changeset`, and an identity with its groups and last session."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import httpx2
import pytest

from fakes import FakeServer, body, kartenn

if TYPE_CHECKING:
    from pathlib import Path

    from syrupy.assertion import SnapshotAssertion

ADMINS = "6a000000-0000-7000-8000-000000000001"
READERS = "6a000000-0000-7000-8000-000000000002"
ALICE = "1d000000-0000-7000-8000-000000000001"
BY_IDENTITY = "4e000000-0000-7000-8000-000000000001"
BY_ISSUER = "4e000000-0000-7000-8000-000000000002"
BINDING = "b1000000-0000-7000-8000-000000000001"
CHANGESET = "9d270000-0000-7000-8000-000000000001"
ISSUER = "https://idp.example.org/realms/interne"
APPLIED: dict[str, Any] = {
    "changeset": {"id": CHANGESET, "href": f"/v1/changesets/{CHANGESET}"},
    "state": "applied",
    "warnings": [],
    "diff": [],
    "cascaded": [],
}
SIMULATED: dict[str, Any] = {
    "valid": True,
    "violations": [],
    "diff": [],
    "sample": [],
    "cascaded": [],
    "sections": [],
}


def group(identifier: str, key: str) -> dict[str, Any]:
    return {
        "id": identifier,
        "href": f"/v1/groups/{identifier}",
        "key": key,
        "labels": {"fr": key.capitalize()},
        "builtin": False,
        "members": 2,
        "bindings": 1,
    }


MEMBERS = [
    {
        "id": BY_IDENTITY,
        "href": f"/v1/groups/{ADMINS}/members/{BY_IDENTITY}",
        "kind": "identity",
        "identity": ALICE,
        "name": "Alice Exemple",
        "issuer": None,
        "group": None,
        "valid_from": "2026-10-01T00:00:00Z",
        "valid_to": None,
    },
    {
        "id": BY_ISSUER,
        "href": f"/v1/groups/{ADMINS}/members/{BY_ISSUER}",
        "kind": "idp_group",
        "identity": None,
        "name": None,
        "issuer": ISSUER,
        "group": "kartenn-admins",
        "valid_from": "2026-10-01T00:00:00Z",
        "valid_to": None,
    },
]


@pytest.fixture
def served(server: FakeServer) -> FakeServer:
    groups = [group(ADMINS, "admins"), group(READERS, "lecteurs")]
    server.route("GET", "/v1/groups", {"items": groups, "links": {}})
    server.route("GET", "/v1/groups/{id}", lambda r: _group(r.url.path))
    server.route("GET", "/v1/groups/{id}/members", {"items": MEMBERS, "links": {}})
    kartenn("--help")
    return server


def _group(path: str) -> httpx2.Response:
    identifier = path.rsplit("/", 1)[-1]
    key = {ADMINS: "admins", READERS: "lecteurs"}[identifier]
    return httpx2.Response(200, json=group(identifier, key), headers={"ETag": '"g1"'})


def created(path: str) -> httpx2.Response:
    return httpx2.Response(201, json=APPLIED, headers={"Location": path})


def test_a_group_is_named_by_its_key(served: FakeServer) -> None:
    result = kartenn("groups", "get", "lecteurs", "-o", "json")
    assert result.exit_code == 0, result.output
    assert served.seen[-1].url.path == f"/v1/groups/{READERS}"
    # The list has no filter by key: the key is matched on its pages.
    assert served.seen[-2].url.path == "/v1/groups"
    missing = kartenn("groups", "get", "absent")
    assert missing.exit_code == 3
    assert "client.reference-not-found" in missing.stderr


def test_rename_and_close_a_group_by_its_key(served: FakeServer) -> None:
    served.route("PATCH", "/v1/groups/{id}", httpx2.Response(200, json=APPLIED))
    renamed = kartenn(
        "groups", "update", "admins", "--set", 'labels={"fr": "Administration"}', "--motive", "m"
    )
    assert renamed.exit_code == 0, renamed.output
    sent = served.seen[-1]
    assert (sent.method, sent.url.path) == ("PATCH", f"/v1/groups/{ADMINS}")
    assert body(sent) == {"labels": {"fr": "Administration"}, "motive": "m"}
    served.route("DELETE", "/v1/groups/{id}", httpx2.Response(200, json=APPLIED))
    closed = kartenn("groups", "close", "lecteurs", "--motive", "fin")
    assert closed.exit_code == 0, closed.output
    assert (served.seen[-1].method, served.seen[-1].url.path) == (
        "DELETE",
        f"/v1/groups/{READERS}",
    )


def test_members_of_a_group(served: FakeServer, snapshot: SnapshotAssertion) -> None:
    result = kartenn("groups", "members", "list", "admins", "-o", "table")
    assert result.exit_code == 0, result.output
    assert served.seen[-1].url.path == f"/v1/groups/{ADMINS}/members"
    assert result.stdout == snapshot


@pytest.mark.parametrize(
    ("member", "sent"),
    [
        (ALICE, {"identity": ALICE}),
        (f"{ISSUER}/kartenn-admins", {"issuer": ISSUER, "group": "kartenn-admins"}),
    ],
)
def test_add_a_member(served: FakeServer, member: str, sent: dict[str, str]) -> None:
    path = f"/v1/groups/{READERS}/members"
    served.route("POST", path, created(f"{path}/{BY_IDENTITY}"))
    result = kartenn("groups", "members", "add", "lecteurs", member, "--motive", "arrivée")
    assert result.exit_code == 0, result.output
    assert served.seen[-1].url.path == path
    assert body(served.seen[-1]) == {**sent, "motive": "arrivée"}


def test_add_a_member_by_its_options(served: FakeServer) -> None:
    path = f"/v1/groups/{READERS}/members"
    served.route("POST", path, created(f"{path}/{BY_ISSUER}"))
    result = kartenn(
        "groups", "members", "add", "lecteurs", "--issuer", ISSUER, "--group", "/équipe/réseau",
        "--motive", "m",
    )  # fmt: skip
    assert result.exit_code == 0, result.output
    assert body(served.seen[-1]) == {"issuer": ISSUER, "group": "/équipe/réseau", "motive": "m"}


def test_a_member_badly_given_is_refused(served: FakeServer) -> None:
    shape = kartenn("groups", "members", "add", "admins", "alice", "--motive", "m")
    assert shape.exit_code == 2
    assert "client.invalid-reference" in shape.stderr
    twice = kartenn(
        "groups", "members", "add", "admins", ALICE, "--identity", ALICE, "--motive", "m"
    )
    assert twice.exit_code == 2
    assert all(r.method == "GET" for r in served.seen)


@pytest.mark.parametrize("member", [BY_ISSUER, f"{ISSUER}/kartenn-admins", ALICE])
def test_remove_a_member(served: FakeServer, member: str) -> None:
    served.route("DELETE", "/v1/groups/{id}/members/{member}", httpx2.Response(200, json=APPLIED))
    result = kartenn("groups", "members", "remove", "admins", member, "--motive", "départ")
    assert result.exit_code == 0, result.output
    sent = served.seen[-1]
    expected = BY_IDENTITY if member == ALICE else BY_ISSUER
    assert (sent.method, sent.url.path) == (
        "DELETE",
        f"/v1/groups/{ADMINS}/members/{expected}",
    )
    assert body(sent)["motive"] == "départ"


def test_show_a_member_and_an_unknown_one(served: FakeServer) -> None:
    path = f"/v1/groups/{ADMINS}/members/{BY_IDENTITY}"
    served.route("GET", path, MEMBERS[0])
    shown = kartenn("groups", "members", "get", "admins", ALICE, "-o", "json")
    assert shown.exit_code == 0, shown.output
    assert served.seen[-1].url.path == path
    unknown = kartenn("groups", "members", "get", "admins", f"{ISSUER}/autre")
    assert unknown.exit_code == 3


def test_add_a_member_simulated(served: FakeServer) -> None:
    path = f"/v1/groups/{ADMINS}/members"
    served.route("POST", path, httpx2.Response(200, json=SIMULATED))
    result = kartenn(
        "groups", "members", "add", "admins", f"{ISSUER}/ops", "--motive", "m", "--dry-run"
    )
    assert result.exit_code == 0, result.output
    sent = served.seen[-1]
    assert sent.url.params["simulate"] == "true"
    assert body(sent)["group"] == "ops"


def test_bind_a_group(served: FakeServer) -> None:
    served.route("POST", "/v1/bindings", created(f"/v1/bindings/{BINDING}"))
    result = kartenn(
        "bindings", "create", "--group", "admins", "--role", "administrator",
        "--set", 'scope={"kind": "global"}', "--motive", "m",
    )  # fmt: skip
    assert result.exit_code == 0, result.output
    assert body(served.seen[-1]) == {
        "group": "admins",
        "role": "administrator",
        "scope": {"kind": "global"},
        "motive": "m",
    }
    served.route("GET", "/v1/bindings", {"items": [], "links": {}})
    listed = kartenn("bindings", "list", "--group", "admins")
    assert listed.exit_code == 0, listed.output
    assert served.seen[-1].url.params["group"] == "admins"


def test_groups_into_the_local_changeset(
    served: FakeServer, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    kartenn("changeset", "new", "--motive", "Équipe réseau")
    steps = [
        ("groups", "create", "--key", "reseau", "--set", 'labels={"fr": "Réseau"}'),
        ("groups", "update", "lecteurs", "--set", 'labels={"fr": "Lecture"}'),
        ("groups", "members", "add", "admins", f"{ISSUER}/reseau"),
        ("groups", "members", "remove", "admins", ALICE),
        ("bindings", "create", "--group", "reseau", "--role", "reader", "--set", "scope={}"),
        ("groups", "close", "lecteurs"),
    ]
    for step in steps:
        added = kartenn(*step, "--into-changeset")
        assert added.exit_code == 0, added.output
    served.route("POST", "/v1/changesets", httpx2.Response(200, json=SIMULATED))
    assert kartenn("changeset", "simulate").exit_code == 0
    assert body(served.seen[-1])["operations"] == [
        {"op": "set_group", "key": "reseau", "labels": {"fr": "Réseau"}},
        {"op": "set_group", "key": "lecteurs", "labels": {"fr": "Lecture"}},
        {
            "op": "add_member",
            "group": "admins",
            "member": {"kind": "idp_group", "issuer": ISSUER, "group": "reseau"},
        },
        {"op": "remove_member", "id": BY_IDENTITY},
        {"op": "bind", "group": "reseau", "role": "reader", "scope": {}},
        {"op": "close_group", "key": "lecteurs"},
    ]


def test_an_identity_with_its_groups_and_those_of_its_last_login(
    served: FakeServer, snapshot: SnapshotAssertion
) -> None:
    identity = {
        "id": ALICE,
        "href": f"/v1/identities/{ALICE}",
        "kind": "person",
        "name": "Alice Exemple",
        "issuer": ISSUER,
        "disabled": False,
        "revoked_after": None,
        "groups": [
            {"key": "admins", "via": "direct"},
            {"key": "admins", "via": "kartenn-admins"},
        ],
        "bindings": [
            {"group": "admins", "role": "administrator", "scope": {"kind": "global"}},
            {"group": "admins", "role": "reader", "scope": {"kind": "site", "id": READERS}},
        ],
        "last_login_idp_groups": ["kartenn-admins", "vpn"],
    }
    served.route("GET", "/v1/identities/{id}", identity)
    table = kartenn("identities", "get", ALICE, "-o", "table")
    assert table.exit_code == 0, table.output
    assert table.stdout == snapshot
    data = kartenn("identities", "get", ALICE, "-o", "json")
    assert '"via": "kartenn-admins"' in data.stdout


def test_me_shows_the_groups_of_its_issuer(served: FakeServer) -> None:
    me = {
        "identity": ALICE,
        "name": "Alice Exemple",
        "issuer": ISSUER,
        "kind": "person",
        "method": "bearer",
        "groups": [{"key": "admins", "via": "kartenn-admins"}],
        "idp_groups": ["kartenn-admins", "vpn"],
        "roles": ["administrator"],
    }
    served.route("GET", "/v1/me", me)
    result = kartenn("me")
    assert result.exit_code == 0, result.output
    assert result.stdout.splitlines()[1:3] == [
        "Groupes      admins (kartenn-admins)",
        "Groupes IdP  kartenn-admins, vpn",
    ]
