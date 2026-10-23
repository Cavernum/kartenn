import json
import random

import pytest

from kartenn._core.messages import Headers, Response
from kartenn._core.pages import check_cursor, next_request, parse_page
from kartenn._core.problems import ClientError
from kartenn._core.sse import CAP_SECONDS, EventTracker, RawEvent


def page(body: object, **headers: str) -> Response:
    return Response(200, Headers.of(headers), json.dumps(body).encode())


def test_page_follows_links_next_as_given() -> None:
    parsed = parse_page(
        page({"items": [{"id": "a"}, "junk"], "links": {"next": "/v1/x?cursor=abc"}}, ETag='W/"p"'),
        as_of="2026-01-01T00:00:00Z",
    )
    assert parsed.items == ({"id": "a"},)
    assert parsed.next == "/v1/x?cursor=abc"
    assert parsed.as_of == "2026-01-01T00:00:00Z"
    assert parsed.etag == 'W/"p"'
    following = next_request(parsed)
    assert following is not None
    assert (following.method, following.url, following.params) == ("GET", "/v1/x?cursor=abc", ())


def test_page_end_and_link_header() -> None:
    end = parse_page(page({"items": [], "links": {"next": None}, "as_of": "2026-02-02T00:00:00Z"}))
    assert end.next is None
    assert end.as_of == "2026-02-02T00:00:00Z"
    assert next_request(end) is None
    linked = parse_page(
        page({"items": []}, Link='</v1/x?cursor=1>; rel="prev", </v1/x?cursor=2>; rel="next"')
    )
    assert linked.next == "/v1/x?cursor=2"
    unrelated = parse_page(page({"items": []}, Link="</v1/x>; title=a"))
    assert unrelated.next is None


@pytest.mark.parametrize("body", [[], {"items": 1}, None])
def test_not_a_page(body: object) -> None:
    with pytest.raises(ClientError, match="not a page"):
        parse_page(page(body))


def test_cursor_freezes_filters() -> None:
    check_cursor(None, others={"domain": "x"})
    check_cursor("c", others={"domain": None, "usage": []})
    with pytest.raises(ClientError) as caught:
        check_cursor("c", others={"domain": "x", "valid_at": "2026-01-01T00:00:00Z"})
    assert caught.value.code == "client.cursor-frozen"
    assert "domain, valid_at" in str(caught.value)


def changes(changeset: str, cursor: str) -> RawEvent:
    return RawEvent("changes", json.dumps({"changeset": changeset, "objects": []}), cursor)


def test_tracker_keeps_cursor_and_drops_duplicates() -> None:
    tracker = EventTracker()
    assert tracker.headers() == {}
    ready = tracker.feed(RawEvent("ready", '{"heartbeat":15}', "k1.A", retry=2140))
    assert ready is not None
    assert ready.kind == "ready"
    assert tracker.retry_ms == 2140
    first = tracker.feed(changes("C1", "k1.B"))
    assert first is not None
    assert first.data["changeset"] == "C1"
    assert tracker.feed(RawEvent("message", "", "k1.C")) is None  # a point of resumption
    assert tracker.headers() == {"Last-Event-ID": "k1.C"}
    assert tracker.feed(changes("C1", "k1.D")) is None  # replayed after a resumption
    assert tracker.last_event_id == "k1.D"
    pending = tracker.feed(RawEvent("changeset", '{"changeset":"C2","state":"pending"}', "k1.E"))
    applied = tracker.feed(RawEvent("changeset", '{"changeset":"C2","state":"applied"}', "k1.F"))
    assert pending is not None
    assert applied is not None
    reset = tracker.feed(RawEvent("reset", '{"reason":"overflow"}', "k1.G"))
    assert reset is not None
    assert reset.kind == "reset"
    again = tracker.feed(RawEvent("reset", '{"reason":"overflow"}', "k1.H"))
    assert again is not None


def test_tracker_memory_is_bounded() -> None:
    tracker = EventTracker("k1.0", memory=2)
    assert tracker.headers() == {"Last-Event-ID": "k1.0"}
    for name in ("A", "B", "C"):
        assert tracker.feed(changes(name, "k")) is not None
    assert tracker.feed(changes("A", "k")) is not None  # forgotten
    assert tracker.feed(changes("C", "k")) is None


def test_task_events_and_odd_data() -> None:
    tracker = EventTracker()
    task = tracker.feed(RawEvent("changes", '{"task":"flow:expiry","event":"e1"}'))
    assert task is not None
    assert tracker.feed(RawEvent("changes", '{"task":"flow:expiry","event":"e1"}')) is None
    loose = tracker.feed(RawEvent("changes", "{}"))
    assert loose is not None
    assert tracker.feed(RawEvent("changes", "{}")) is not None
    orphan = tracker.feed(RawEvent("changeset", "{}"))
    assert orphan is not None
    text = tracker.feed(RawEvent("custom", "not json"))
    assert text is not None
    assert text.data == {"raw": "not json"}
    scalar = tracker.feed(RawEvent("custom", "3"))
    assert scalar is not None
    assert scalar.data == {"value": 3}
    empty = tracker.feed(RawEvent("ping-like", ""))
    assert empty is not None
    assert empty.data == {}


def test_backoff_grows_capped_with_jitter() -> None:
    tracker = EventTracker(rng=random.Random(7))  # noqa: S311 - jitter
    tracker.feed(RawEvent("ready", "{}", "k", retry=1000))
    delays = [tracker.backoff() for _ in range(8)]
    for attempt, delay in enumerate(delays):
        nominal = min(2**attempt, CAP_SECONDS)
        assert nominal * 0.8 <= delay <= nominal * 1.2
    tracker.feed(changes("Z", "k2"))
    assert tracker.failures == 0
