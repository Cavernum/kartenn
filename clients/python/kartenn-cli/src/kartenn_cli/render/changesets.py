"""The rendering of a changeset simulated or read (REQ-023-30, 31; maquette § 8).

```
Simulation — rien n'est écrit (valide à partir du 2026-11-01 00:00:00+01:00)
  Opérations  2   Violations  0   Avertissements  1
  ! [core.prefix.unattached] /operations/0  203.0.113.0/26 n'est rattaché à aucun VLAN

Différentiel
  policy.environment  3 objets  — → "prod"
  policy.combinations  3 lignes
    SOURCE          DESTINATION     SERVICE  BEFORE  AFTER
    Administration  203.0.113.0/26  ssh      —       allowed
```

The sections of the modules are generic rows: their columns are their members.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

import click

from kartenn_cli.i18n import t, tn

if TYPE_CHECKING:
    from collections.abc import Mapping, Sequence

    from kartenn import Simulation
    from kartenn_cli.runtime import Runtime

__all__ = ["render_counts", "render_sections", "render_simulation"]


def _text(value: object) -> str:
    return "—" if value is None else str(value)


def render_counts(diff: Sequence[Mapping[str, Any]]) -> list[str]:
    """The counted differential: a line per property and change."""
    lines: list[str] = []
    width = max((len(str(row.get("property", ""))) for row in diff), default=0)
    for row in diff:
        if row.get("masked"):
            change = t("render.masked")
        else:
            change = f"{_text(row.get('before'))} → {_text(row.get('after'))}"
        count = tn("simulation.objects", int(row.get("count") or 0))
        lines.append(f"  {str(row.get('property', '')).ljust(width)}  {count}  {change}")
    return lines


def render_sections(sections: Sequence[Mapping[str, Any]], *, keys: bool = False) -> list[str]:
    """The sections of the modules: name, size, then their first rows in a table."""
    from kartenn_cli.render.table import default_columns, render_table  # noqa: PLC0415

    lines: list[str] = []
    for section in sections:
        name = str(section.get("name") or section.get("section") or "")
        raw = cast("list[object]", section.get("rows") or [])
        rows = [cast("dict[str, Any]", r) for r in raw if isinstance(r, dict)]
        count = int(section.get("count") or len(rows))
        lines.append(f"  {name}  {tn('simulation.rows', count)}")
        columns = default_columns(rows)
        if rows and columns:
            table = render_table(rows, columns, keys=keys)
            lines += [f"    {line}" for line in table.splitlines()]
    return lines


def render_simulation(found: Runtime, simulation: Simulation, *, operations: int) -> None:
    """What applying would do, on the standard output."""
    from kartenn_cli.render.problems import render_violations  # noqa: PLC0415

    fmt = found.asked_output() or "table"
    if fmt != "table":
        found.output().value(dict(simulation.body))
        return
    warnings = [v for v in simulation.violations if not v.blocking]
    click.echo(
        "  "
        + t(
            "simulation.counts",
            operations=operations,
            violations=len(simulation.blocking),
            warnings=len(warnings),
        )
    )
    for line in render_violations(simulation.violations, color=found.color):
        click.echo(line)
    diff = [cast("dict[str, Any]", r) for r in simulation.diff if isinstance(r, dict)]
    sections = [cast("dict[str, Any]", s) for s in simulation.sections if isinstance(s, dict)]
    click.echo("")
    click.echo(t("simulation.diff"))
    lines = render_counts(diff) + render_sections(sections)
    if simulation.cascaded:
        lines.append("  " + tn("simulation.cascaded", len(simulation.cascaded)))
    for line in lines or ["  " + t("simulation.nothing")]:
        click.echo(line)
