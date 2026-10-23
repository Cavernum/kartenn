"""Following the event stream of the 022 without input or output (REQ-023-38).

The tracker keeps the cursor (`Last-Event-ID`, opaque: kept as received), sets aside
the events already seen, and computes the wait before reopening: from the `retry:`
received, doubled at each failure, capped at 30 s, with ±20 % of jitter (022 § Contrat
consommé par les clients).
"""

from __future__ import annotations

import json
import random
from collections import OrderedDict
from dataclasses import dataclass, field
from typing import Any, cast

__all__ = ["Event", "EventTracker", "RawEvent"]

# 023 design § Événements: the identities of the last 10 000 events.
MEMORY = 10_000
CAP_SECONDS = 30.0
DEFAULT_RETRY_MS = 1_000
JITTER = 0.2


@dataclass(frozen=True)
class RawEvent:
    """A dispatched event of `text/event-stream` (WHATWG § 9.2.6), as a transport reads
    it: `id` is the last event identifier of the stream, empty when none was set."""

    event: str = "message"
    data: str = ""
    id: str = ""
    retry: int | None = None


@dataclass(frozen=True)
class Event:
    """An event of `GET /v1/events`: `kind` is `ready`, `changes`, `changeset` or `reset`."""

    kind: str
    id: str | None
    data: dict[str, Any] = field(default_factory=dict[str, Any])


class EventTracker:
    """The state of a subscription across its connections."""

    def __init__(
        self,
        last_event_id: str | None = None,
        *,
        memory: int = MEMORY,
        rng: random.Random | None = None,
    ) -> None:
        self.last_event_id = last_event_id or None
        self.retry_ms = DEFAULT_RETRY_MS
        self.failures = 0
        self._memory = memory
        self._seen: OrderedDict[tuple[str, ...], None] = OrderedDict()
        self._rng = rng or random.Random()  # noqa: S311 - jitter, not a secret

    def headers(self) -> dict[str, str]:
        """The headers of the next opening: the cursor, when there is one."""
        return {} if self.last_event_id is None else {"Last-Event-ID": self.last_event_id}

    def feed(self, raw: RawEvent) -> Event | None:
        """The event to hand to the caller, or `None`: a point of resumption (`id:`
        alone), a comment, or a duplicate."""
        if raw.retry is not None:
            self.retry_ms = raw.retry
        if raw.id:
            self.last_event_id = raw.id
        if raw.event == "message" and not raw.data:
            return None
        self.failures = 0
        event = Event(raw.event, raw.id or None, _data(raw.data))
        identity = _identity(event)
        if identity is not None:
            if identity in self._seen:
                self._seen.move_to_end(identity)
                return None
            self._seen[identity] = None
            if len(self._seen) > self._memory:
                self._seen.popitem(last=False)
        return event

    def backoff(self) -> float:
        """Seconds to wait before reopening after a cut, then count the failure."""
        base = self.retry_ms / 1000 * (2**self.failures)
        self.failures += 1
        jitter = self._rng.uniform(1 - JITTER, 1 + JITTER)
        return min(base, CAP_SECONDS) * jitter


def _data(text: str) -> dict[str, Any]:
    if not text:
        return {}
    try:
        parsed = json.loads(text)
    except ValueError:
        return {"raw": text}
    return cast("dict[str, Any]", parsed) if isinstance(parsed, dict) else {"value": parsed}


def _identity(event: Event) -> tuple[str, ...] | None:
    """The identity of an event (022 § Doublons): its changeset, or the CloudEvents
    identifier of a task, and the state of a changeset; `ready` and `reset` have none."""
    if event.kind == "changes":
        source = event.data.get("changeset") or event.data.get("event")
        return None if source is None else ("changes", str(source))
    if event.kind == "changeset":
        source = event.data.get("changeset")
        state = event.data.get("state")
        return None if source is None else ("changeset", str(source), str(state))
    return None
