"""Drafts kept by the server (spec 024, REQ-024-1 to 7): `kn.drafts`."""

from pathlib import Path
from typing import Any

import httpx2
import pytest

from fakes import FakeServer, body, problem
from kartenn import Applied, ClientError, Pending, Record, Refused

DRAFT: dict[str, Any] = {
    "id": "0190d000-0000-7000-8000-000000000001",
    "href": "/v1/drafts/0190d000-0000-7000-8000-000000000001",
    "author": "0190d000-0000-7000-8000-0000000000aa",
    "title": "Lot 3",
    "operation_count": 1,
    "version": 1,
    "created_at": "2026-10-24T10:00:00Z",
    "updated_at": "2026-10-24T10:00:00Z",
    "reason": {},
    "operations": [{"op": "create_domain", "slug": "lot3"}],
}
SUMMARY = {k: v for k, v in DRAFT.items() if k not in {"reason", "operations"}}
APPLIED: dict[str, Any] = {
    "changeset": {"id": "c1", "href": "/v1/changesets/c1"},
    "state": "applied",
    "warnings": [],
    "diff": [],
    "cascaded": [],
}
PENDING: dict[str, Any] = {
    "changeset": {"id": "c2", "href": "/v1/changesets/c2"},
    "state": "pending",
    "kind": "approval",
    "diff": [],
    "reason": {"code": "changeset.approval.self.missing", "message": "m"},
}


def drafted(version: int = 1, **changes: object) -> httpx2.Response:
    return httpx2.Response(
        200, json={**DRAFT, "version": version, **changes}, headers={"ETag": f'"{version}"'}
    )


def test_create_list_read_update_delete(tmp_path: Path) -> None:
    server = FakeServer()
    server.route(
        "POST",
        "/v1/drafts",
        httpx2.Response(201, json=DRAFT, headers={"ETag": '"1"', "Location": DRAFT["href"]}),
    )
    server.route("GET", "/v1/drafts", httpx2.Response(200, json={"items": [SUMMARY]}))
    server.route("GET", "/v1/drafts/{id}", drafted(1))
    server.route("PATCH", "/v1/drafts/{id}", drafted(2, title="Lot 4"))
    server.route("DELETE", "/v1/drafts/{id}", httpx2.Response(204))
    with server.client(tmp_path) as kn:
        draft = kn.drafts.create(
            "Lot 3", operations=[{"op": "create_domain", "slug": "lot3"}], summary="ouverture"
        )
        assert isinstance(draft, Record)
        assert draft.etag == '"1"'
        assert body(server.seen[-1]) == {
            "title": "Lot 3",
            "operations": [{"op": "create_domain", "slug": "lot3"}],
            "reason": {"summary": "ouverture"},
        }
        listed = kn.drafts.list()
        assert [d["title"] for d in listed] == ["Lot 3"]
        assert "operations" not in listed[0]
        # REQ-024-3: the version read goes in If-Match.
        changed = kn.drafts.update(draft, title="Lot 4", description="pour le labo")
        patch = server.seen[-1]
        assert patch.headers["if-match"] == '"1"'
        assert body(patch) == {"title": "Lot 4", "reason": {"description": "pour le labo"}}
        assert (changed["title"], changed.etag) == ("Lot 4", '"2"')
        # An identifier: read first, for its version.
        kn.drafts.update(DRAFT["id"], operations=[])
        read, patch = server.seen[-2:]
        assert read.method == "GET"
        assert patch.headers["if-match"] == '"1"'
        assert body(patch) == {"operations": []}
        kn.drafts.delete(changed)
        assert server.seen[-1].method == "DELETE"
        assert server.seen[-1].headers["if-match"] == '"2"'
        assert kn.drafts.get(DRAFT["id"])["operations"][0]["slug"] == "lot3"
        with pytest.raises(ClientError):
            kn.drafts.update(draft)


def test_submit_applies_or_waits_or_keeps_the_draft(tmp_path: Path) -> None:
    server = FakeServer()
    server.route("GET", "/v1/drafts/{id}", drafted(3))
    server.route(
        "POST",
        "/v1/drafts/{id}/submission",
        httpx2.Response(201, json=APPLIED, headers={"Location": "/v1/changesets/c1"}),
    )
    with server.client(tmp_path) as kn:
        result = kn.drafts.submit(DRAFT["id"], idempotency_key="k1")
        assert isinstance(result, Applied)
        assert result.changeset == "c1"
        read, sent = server.seen[-2:]
        assert read.method == "GET"
        # The version read: a draft modified since is not submitted.
        assert sent.headers["if-match"] == '"3"'
        assert sent.url.params.get("approval") == "self"
        assert sent.headers["idempotency-key"] == "k1"
        server.route("POST", "/v1/drafts/{id}/submission", httpx2.Response(202, json=PENDING))
        result = kn.drafts.submit(Record(DRAFT, etag='"1"'), approval_self=False)
        assert isinstance(result, Pending)
        assert result.reason == "changeset.approval.self.missing"
        assert "approval" not in server.seen[-1].url.params
        server.route(
            "POST",
            "/v1/drafts/{id}/submission",
            problem(422, "changeset-refused", violations=[]),
        )
        with pytest.raises(Refused):
            kn.drafts.submit(DRAFT["id"])
        server.route("POST", "/v1/drafts/{id}/submission", drafted(1))
        with pytest.raises(ClientError):
            kn.drafts.submit(DRAFT["id"])


@pytest.mark.anyio
async def test_the_asynchronous_drafts(tmp_path: Path) -> None:
    server = FakeServer()
    server.route(
        "POST",
        "/v1/drafts",
        httpx2.Response(201, json=DRAFT, headers={"ETag": '"1"'}),
    )
    server.route("GET", "/v1/drafts", httpx2.Response(200, json={"items": [SUMMARY]}))
    server.route("GET", "/v1/drafts/{id}", drafted(1))
    server.route("PATCH", "/v1/drafts/{id}", drafted(2))
    server.route("DELETE", "/v1/drafts/{id}", httpx2.Response(204))
    server.route("POST", "/v1/drafts/{id}/submission", httpx2.Response(202, json=PENDING))
    async with server.async_client(tmp_path) as kn:
        draft = await kn.drafts.create("Lot 3")
        assert len(await kn.drafts.list()) == 1
        assert (await kn.drafts.get(draft))["version"] == 1
        changed = await kn.drafts.update(draft, summary="s", etag='"1"')
        assert changed.etag == '"2"'
        assert isinstance(await kn.drafts.submit(changed), Pending)
        await kn.drafts.delete(changed)
    assert [r.method for r in server.seen if r.url.path.startswith("/v1/drafts")] == [
        "POST",
        "GET",
        "GET",
        "PATCH",
        "POST",
        "DELETE",
    ]
