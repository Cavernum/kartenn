// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** A column of a data grid (design « Inventaire », line 20). */
export interface GridColumn {
  id: string;
  label: string;
  /** Width from the column tokens; `fill` takes the rest (at least `md`). */
  width?: "xs" | "sm" | "md" | "lg" | "fill";
  align?: "start" | "end";
  /** Values in fixed pitch (addresses, prefixes, keys, identifiers). */
  mono?: boolean;
  /**
   * A message (a reason, a description): its cells wrap instead of clipping, the row grows.
   * Keys, names and numbers stay on one line.
   */
  wrap?: boolean;
  sortable?: boolean;
  filterable?: boolean;
  /**
   * How long the column stays when its grid narrows (design « Adaptation à la largeur »):
   * 1 always (the default, and always the first column, the key), 2 hidden under a
   * container `md`, 3 under `lg`. A hidden column is one choice away (column chooser) and
   * its values show in the panel of the chosen row.
   */
  priority?: 1 | 2 | 3;
  /**
   * The actions of the row (a menu, buttons): when the rows of a simple list stack (`stack`
   * on the grid), they go beside the key, without their column's label.
   */
  actions?: boolean;
}

/**
 * The user's choice of columns, by id: `true` always shown, `false` never; a column left
 * out follows its priority and the width of the grid.
 */
export type ColumnVisibility = Record<string, boolean>;

/** The priority a column follows: the user's choice first, the key always shown. */
export function priorityOf(
  columns: readonly GridColumn[],
  column: GridColumn,
  visibility: ColumnVisibility,
): 1 | 2 | 3 {
  if (column === columns[0] || visibility[column.id] === true) return 1;
  return column.priority ?? 1;
}

/** The columns rendered: all but the ones the user hid (never the key). */
export function renderedColumns(
  columns: readonly GridColumn[],
  visibility: ColumnVisibility,
): GridColumn[] {
  return columns.filter((column, i) => i === 0 || visibility[column.id] !== false);
}

/**
 * The templates of the grid, set on it by CSSOM: every rendered column, then without the
 * columns its container hides under `lg` (priority 3) and under `md` (priorities 2 and 3).
 */
export function columnTemplates(
  columns: readonly GridColumn[],
  visibility: ColumnVisibility,
): Record<string, string> {
  const rendered = renderedColumns(columns, visibility);
  const upTo = (max: number) =>
    columnsTemplate(rendered.filter((c) => priorityOf(columns, c, visibility) <= max));
  return {
    "--k-grid-columns": upTo(3),
    "--k-grid-columns-below-lg": upTo(2),
    "--k-grid-columns-below-md": upTo(1),
  };
}

/**
 * The entries of the column chooser: one per column that may hide, checked when it shows
 * (`shown` tells which do, as measured when the chooser opens).
 */
export function chooserItems(
  columns: readonly GridColumn[],
  visibility: ColumnVisibility,
  shown: (id: string) => boolean,
): { kind: "checkbox"; id: string; label: string; checked: boolean }[] {
  return columns
    .filter((column, i) => i > 0 && (column.priority ?? 1) > 1)
    .map((column) => ({
      kind: "checkbox",
      id: column.id,
      label: column.label,
      checked: visibility[column.id] ?? shown(column.id),
    }));
}

/** The choice after the user flips the column `id`, shown or not until then. */
export function toggleColumn(
  visibility: ColumnVisibility,
  id: string,
  shownNow: boolean,
): ColumnVisibility {
  return { ...visibility, [id]: !(visibility[id] ?? shownNow) };
}

/** The greatest priority among the columns: 1 when none may hide (no chooser). */
export function maxPriority(columns: readonly GridColumn[]): 1 | 2 | 3 {
  return columns.reduce<1 | 2 | 3>(
    (max, column, i) => (i > 0 && (column.priority ?? 1) > max ? column.priority! : max),
    1,
  );
}

/**
 * The state of a cell, rendered by the grid and decided by the host (contract of in-place
 * editing, design « Inventaire »): a write under way, in a draft, in review, or refused
 * with its message and code.
 */
export type CellState = "pending" | "draft" | "review" | { error: string; code?: string };

/**
 * The state of a whole row: added or changed in a draft, in review, or removed in a draft
 * (struck through until the draft is applied, rather than gone).
 */
export type RowState = "draft" | "review" | "removed";

/** The sort of the grid (`aria-sort` of its header). */
export interface GridSort {
  column: string;
  direction: "ascending" | "descending";
}

/**
 * The sort after a click (or Enter) on the header of `column`: ascending first, then the
 * other way at each new activation.
 */
export function nextSort(sort: GridSort | undefined, column: string): GridSort {
  return {
    column,
    direction:
      sort?.column === column && sort.direction === "ascending" ? "descending" : "ascending",
  };
}

/** The position of a cell: the key of its row, the id of its column. */
export interface CellRef {
  row: string;
  column: string;
}

const WIDTHS = {
  xs: "var(--k-size-col-xs)",
  sm: "var(--k-size-col-sm)",
  md: "var(--k-size-col-md)",
  lg: "var(--k-size-col-lg)",
  fill: "minmax(var(--k-size-col-md), 1fr)",
} as const;

/**
 * The template shared by the header and every row (`--k-grid-columns`, set on the grid by
 * CSSOM): one width per column, from tokens only. Head and body therefore always align,
 * whatever the rows rendered (the width fix of the prototype).
 */
export function columnsTemplate(columns: readonly GridColumn[]): string {
  return columns.map((column) => WIDTHS[column.width ?? "md"]).join(" ");
}
