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
}

/**
 * The state of a cell, rendered by the grid and decided by the host (contract of in-place
 * editing, design « Inventaire »): a write under way, in a draft, in review, or refused
 * with its message and code.
 */
export type CellState = "pending" | "draft" | "review" | { error: string; code?: string };

/** The state of a whole row. */
export type RowState = "draft" | "review";

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
