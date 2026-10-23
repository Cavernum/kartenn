"""Imports by chunks (REQ-023-15; 006 § Imports et tâches de fond).

`POST /v1/imports {source}`, then `PUT …/chunks/{n}` of `chunk_elements` at most, then
`POST …/start`; the import is followed by `GET /v1/imports/{id}` at the pace of
`Retry-After`. The elements are read as they come, one chunk in memory at a time.

The local state (identifier, chunks confirmed, digest of the source) is kept in
`$XDG_STATE_HOME/kartenn/imports/`, in 0600. `resume` reads the source again up to the
next chunk, after checking its digest (`client.import-source-changed` otherwise). When
the sort of the server refuses the import, its violations name an element: the session
gives back its line in the source.
"""

from __future__ import annotations

import hashlib
import json
import re
from array import array
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING, Any, cast

import platformdirs

from kartenn._core.plans import checked
from kartenn._core.problems import ClientError, KartennError, Violation, parse_violations
from kartenn._files import write_private
from kartenn.resources import Record

if TYPE_CHECKING:
    from collections.abc import Awaitable, Callable, Iterable, Iterator, Mapping

    from kartenn._core.plans import Plan
    from kartenn.catalog import Catalog, Operation
    from kartenn.resources import Context

__all__ = [
    "AsyncImportSession",
    "ImportProgress",
    "ImportRefused",
    "ImportSession",
    "file_digest",
    "ndjson_elements",
]

DEFAULT_CHUNK = 1_000
FINAL = frozenset({"done", "cancelled", "paused"})
_INDEX = re.compile(r"(\d+)(?!.*\d)")
_READ_SIZE = 1 << 20


@dataclass(frozen=True)
class ImportProgress:
    import_id: str
    chunks: int
    elements: int


class ImportRefused(KartennError):
    """The sort of the server refused the import: `violations` and, for each, the line of
    the source of its element (`None` when no element is named)."""

    def __init__(self, problem: Mapping[str, Any], lines: tuple[int | None, ...]) -> None:
        slug = str(problem.get("type", "invalid-import")).rsplit(":", 1)[-1]
        self.problem = dict(problem)
        self.violations: tuple[Violation, ...] = parse_violations(problem.get("violations"))
        self.lines = lines
        detail = problem.get("detail") or problem.get("title") or "import refused"
        super().__init__(slug, f"[{slug}] {detail}")


def file_digest(path: Path) -> str:
    """The SHA-256 of a source file, read by blocks."""
    digest = hashlib.sha256()
    with path.open("rb") as file:
        while block := file.read(_READ_SIZE):
            digest.update(block)
    return digest.hexdigest()


def ndjson_elements(path: Path) -> Iterator[tuple[int, Mapping[str, Any]]]:
    """The `ImportElement` of an NDJSON file with their line, blank lines skipped."""
    with path.open(encoding="utf-8") as file:
        for number, line in enumerate(file, start=1):
            if line.strip():
                data = json.loads(line)
                if not isinstance(data, dict):
                    msg = f"{path}:{number}: an element is a JSON object"
                    raise ClientError("client.invalid-element", msg)
                yield number, cast("dict[str, Any]", data)


def default_state_dir() -> Path:
    return Path(platformdirs.user_state_dir("kartenn")) / "imports"


class _Import:
    """The plans of an import, shared by both sessions."""

    def __init__(  # noqa: PLR0913 - the settings of an import
        self,
        context: Context,
        *,
        source: str,
        digest: str,
        chunk_elements: int | None = None,
        state_dir: Path | None = None,
        on_progress: Callable[[ImportProgress], None] | None = None,
    ) -> None:
        self._context = context
        self.source = source
        self.digest = digest
        self._chunk = chunk_elements
        self._directory = state_dir or default_state_dir()
        self._progress = on_progress
        self.import_id: str | None = None
        self.lines: array[int] = array("Q")

    @property
    def state_path(self) -> Path:
        key = f"{self._context.shared.url}\0{self.source}".encode()
        return self._directory / f"{hashlib.sha256(key).hexdigest()[:32]}.json"

    def _save(self, state: dict[str, Any]) -> None:
        write_private(self.state_path, json.dumps(state))

    def _load(self) -> dict[str, Any] | None:
        try:
            data = json.loads(self.state_path.read_text())
        except (OSError, ValueError):
            return None
        return cast("dict[str, Any]", data) if isinstance(data, dict) else None

    def _size(self, catalog: Catalog) -> int:
        limit = catalog.limits.get("chunk_elements")
        wanted = self._chunk or (limit if isinstance(limit, int) and limit > 0 else DEFAULT_CHUNK)
        return min(wanted, limit) if isinstance(limit, int) and limit > 0 else wanted

    def run(self, elements: Iterable[Any], *, resume: bool) -> Plan[str]:
        catalog = yield from self._context.catalog()
        state = self._load() if resume else None
        if resume:
            if state is None:
                msg = f"no import of {self.source!r} to resume on this server"
                raise ClientError("client.no-import", msg)
            if state.get("digest") != self.digest:
                msg = f"{self.source!r} changed since its import began: its digest differs"
                raise ClientError("client.import-source-changed", msg)
        if state is None:
            operation = catalog.operation("create_import")
            answer = yield self._context.request(operation, body={"source": self.source})
            created = checked(answer).json()
            identifier = (
                cast("dict[str, Any]", created).get("id") if isinstance(created, dict) else None
            )
            if identifier is None:
                msg = "the server created an import without an identifier"
                raise ClientError("client.unexpected-answer", msg)
            state = {
                "import": str(identifier),
                "source": self.source,
                "digest": self.digest,
                "chunk_elements": self._size(catalog),
                "confirmed": 0,
            }
            self._save(state)
        self.import_id = str(state["import"])
        yield from self._chunks(catalog, state, elements)
        start = catalog.operation("start_import")
        checked((yield self._context.request(start, ref=self.import_id)))
        self.state_path.unlink(missing_ok=True)
        return self.import_id

    def _chunks(
        self, catalog: Catalog, state: dict[str, Any], elements: Iterable[Any]
    ) -> Plan[None]:
        size, confirmed = int(state["chunk_elements"]), int(state["confirmed"])
        deposit = catalog.operation("deposit_import_chunk")
        batch: list[Mapping[str, Any]] = []
        count = 0
        for position, item in enumerate(elements):
            line, element = (
                cast("tuple[int, object]", item)
                if isinstance(item, tuple)
                else (position + 1, item)
            )
            self.lines.append(line)
            count = position + 1
            if position // size < confirmed:
                continue
            batch.append(cast("Mapping[str, Any]", element))
            if len(batch) == size:
                yield from self._deposit(deposit, state, batch, count)
                batch = []
        if batch:
            yield from self._deposit(deposit, state, batch, count)

    def _deposit(
        self,
        deposit: Operation,
        state: dict[str, Any],
        batch: list[Mapping[str, Any]],
        count: int,
    ) -> Plan[None]:
        number = int(state["confirmed"])
        request = self._context.request(
            deposit, ref=self.import_id, params={"n": number}, body={"elements": batch}
        )
        checked((yield request))
        state["confirmed"] = number + 1
        self._save(state)
        if self._progress is not None:
            self._progress(ImportProgress(str(self.import_id), number + 1, count))

    def poll(self) -> Plan[tuple[Record, float]]:
        catalog = yield from self._context.catalog()
        answer = checked(
            (yield self._context.request(catalog.operation("show_import"), ref=self.import_id))
        )
        data = answer.json()
        record = Record(cast("dict[str, Any]", data) if isinstance(data, dict) else {})
        after = answer.headers.get("Retry-After")
        return record, float(after) if after and after.isdigit() else 5.0

    def refusal(self, record: Record) -> ImportRefused | None:
        """The refusal of a sort that failed: back to `uploading` with its `error`."""
        error = record.get("error")
        if record.get("state") != "uploading" or not isinstance(error, dict):
            return None
        problem = cast("dict[str, Any]", error)
        lines = tuple(self._line(v) for v in parse_violations(problem.get("violations")))
        return ImportRefused(problem, lines)

    def _line(self, violation: Violation) -> int | None:
        found = None
        if violation.pointer:
            match = _INDEX.search(violation.pointer)
            found = int(match.group(1)) if match else None
        element = violation.extra.get("element")
        if isinstance(element, int):
            found = element
        if found is None:
            return None
        return self.lines[found] if found < len(self.lines) else found + 1


class ImportSession:
    """An import by chunks, for the synchronous client."""

    def __init__(
        self, plans: _Import, run: Callable[[Plan[Any]], Any], sleep: Callable[[float], None]
    ) -> None:
        self._plans = plans
        self._run = run
        self._sleep = sleep

    @property
    def import_id(self) -> str | None:
        return self._plans.import_id

    @property
    def state_path(self) -> Path:
        return self._plans.state_path

    def run(self, elements: Iterable[Any]) -> str:
        """Create the import, deposit every chunk, start it; its identifier."""
        return cast("str", self._run(self._plans.run(elements, resume=False)))

    def resume(self, elements: Iterable[Any]) -> str:
        """Go on with an import cut short: only the chunks not confirmed are sent."""
        return cast("str", self._run(self._plans.run(elements, resume=True)))

    def wait(self) -> Record:
        """Follow the import until it is done, paused or cancelled; a refused sort
        raises `ImportRefused`."""
        while True:
            record, after = cast("tuple[Record, float]", self._run(self._plans.poll()))
            refused = self._plans.refusal(record)
            if refused is not None:
                raise refused
            if record.get("state") in FINAL:
                return record
            self._sleep(after)


class AsyncImportSession:
    """An import by chunks, for the asynchronous client."""

    def __init__(
        self,
        plans: _Import,
        run: Callable[[Plan[Any]], Awaitable[Any]],
        sleep: Callable[[float], Awaitable[None]],
    ) -> None:
        self._plans = plans
        self._run = run
        self._sleep = sleep

    @property
    def import_id(self) -> str | None:
        return self._plans.import_id

    async def run(self, elements: Iterable[Any]) -> str:
        return cast("str", await self._run(self._plans.run(elements, resume=False)))

    async def resume(self, elements: Iterable[Any]) -> str:
        return cast("str", await self._run(self._plans.run(elements, resume=True)))

    async def wait(self) -> Record:
        while True:
            record, after = cast("tuple[Record, float]", await self._run(self._plans.poll()))
            refused = self._plans.refusal(record)
            if refused is not None:
                raise refused
            if record.get("state") in FINAL:
                return record
            await self._sleep(after)


def plans(  # noqa: PLR0913 - the settings of an import
    context: Context,
    *,
    source: str,
    digest: str,
    chunk_elements: int | None = None,
    state_dir: Path | None = None,
    on_progress: Callable[[ImportProgress], None] | None = None,
) -> _Import:
    return _Import(
        context,
        source=source,
        digest=digest,
        chunk_elements=chunk_elements,
        state_dir=state_dir,
        on_progress=on_progress,
    )
