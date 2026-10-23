"""The rendering of an error (REQ-023-33, T16; design § Rendu d'un problème).

```
erreur [changeset-refused] Changeset refusé (422)
  2 violations — requête urn:uuid:5b0e…  changeset 0c41…
  ✗ [core.prefix.overlap] /operations/0  Le préfixe 198.51.100.0/25 chevauche …
  ! [core.prefix.unused]  —              Aucun préfixe enfant (avertissement)
```

The code always comes first, between brackets, readable without colours. With
`--output json` the problem of the server (RFC 9457) is written as it came.
"""

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.errors import CliError
from kartenn_cli.i18n import t, tn

if TYPE_CHECKING:
    from collections.abc import Sequence

    from kartenn import Violation

__all__ = ["render_error", "render_violations"]


def _pointer(violation: Violation) -> str:
    if violation.pointer:
        return violation.pointer
    if violation.operation is not None:
        return f"/operations/{violation.operation}"
    return "—"


def render_violations(
    violations: Sequence[Violation], *, color: bool, sources: Sequence[str | None] = ()
) -> list[str]:
    """One line per violation: mark, rule, pointer (or source line), message."""
    places: list[str] = []
    for index, violation in enumerate(violations):
        source = sources[index] if index < len(sources) else None
        places.append(source or _pointer(violation))
    rules = [f"[{v.rule}]" for v in violations]
    rule_width = max((len(r) for r in rules), default=0)
    place_width = max((len(p) for p in places), default=0)
    lines: list[str] = []
    for violation, rule, place in zip(violations, rules, places, strict=True):
        blocking = violation.blocking
        mark = "✗" if blocking else "!"
        if color:
            mark = click.style(mark, fg="red" if blocking else "yellow", bold=True)
        detail = violation.detail if blocking else f"{violation.detail} ({t('error.warning')})"
        lines.append(f"  {mark} {rule.ljust(rule_width)} {place.ljust(place_width)}  {detail}")
    return [line.rstrip() for line in lines]


def _head(code: str, title: str, *, color: bool) -> str:
    word = t("error.word")
    if color:
        word = click.style(word, fg="red", bold=True)
    return f"{word} [{code}] {title}".rstrip()


def _summary(count: int, problem: dict[str, Any]) -> str | None:
    parts: list[str] = []
    if count:
        parts.append(tn("error.violations", count))
    instance = problem.get("instance")
    found: list[str] = []
    if instance:
        found.append(t("error.request", id=instance))
    changeset = problem.get("changeset")
    if isinstance(changeset, dict):
        changeset = cast("dict[str, Any]", changeset).get("id")
    if changeset:
        found.append(t("error.changeset", id=changeset))
    imported = problem.get("import")
    if imported:
        found.append(t("error.import", id=imported))
    if found:
        parts.append("  ".join(found))
    return "  " + " — ".join(parts) if parts else None


def _hints(code: str, error: object) -> list[str]:
    key = f"hint.{code}"
    text = t(key, current=getattr(error, "current_etag", None) or "—")
    return [f"  {line}" for line in text.splitlines()] if text != key else []


def render_error(error: BaseException, *, json_output: bool = False, color: bool = False) -> str:
    """The text of an error for the standard error, ended by a newline."""
    from kartenn import ApiError, ImportRefused, KartennError  # noqa: PLC0415 - on failure only

    if isinstance(error, ApiError | ImportRefused):
        problem = dict(error.problem)
        if json_output:
            return json.dumps(problem, ensure_ascii=False) + "\n"
        if isinstance(error, ApiError):
            head = _head(error.code, f"{error.title} ({error.status})", color=color)
            detail = error.detail
        else:
            status = problem.get("status")
            title = problem.get("title") or ""
            head = _head(error.code, f"{title} ({status})" if status else title, color=color)
            detail = None if problem.get("detail") == title else problem.get("detail")
        lines = [head]
        if detail:
            lines.append(f"  {detail}")
        summary = _summary(len(error.violations), {**problem, **getattr(error, "context", {})})
        if summary:
            lines.append(summary)
        sources = getattr(error, "sources", ())
        lines += render_violations(error.violations, color=color, sources=sources)
        lines += _hints(error.code, error)
        lines += [f"  {line}" for line in getattr(error, "lines_after", ())]
        return "\n".join(lines) + "\n"
    if isinstance(error, CliError | KartennError):
        code = error.code
        message = error.message if isinstance(error, CliError) else str(error)
        if json_output:
            return json.dumps({"code": code, "detail": message}, ensure_ascii=False) + "\n"
        lines = [_head(code, message, color=color)]
        if isinstance(error, CliError):
            lines += [f"  {line}" for line in error.lines]
        lines += _hints(code, error)
        return "\n".join(lines) + "\n"
    return f"{_head('client.unexpected', str(error), color=color)}\n"
