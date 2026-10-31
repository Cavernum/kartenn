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
  /**
   * A column of an extension (a plugin, point `column`, W5): each cell holds a custom
   * element of this tag, created for its row (again when virtualisation brings the row
   * back), its `row` property set to the row and the others to what `elementProps` of the
   * grid gives. The grid knows nothing of plugins; the host passes the tag it registered.
   */
  element?: string;
}

/**
 * The user's choice of columns, by id: `true` always shown, `false` never; a column left
 * out follows its priority and the width of the grid.
 */
export type ColumnVisibility = Record<string, boolean>;

/**
 * Widths the user gave the columns (`resizable` grid), in CSS pixels by column id; a
 * column left out keeps the width of its token. Never narrower than its header.
 */
export type ColumnWidths = Record<string, number>;

/** The step of a column width by an arrow key, and by Shift with an arrow. */
export const WIDTH_STEP = 16;
export const WIDTH_STEP_LARGE = 64;
/** The widest a column may be made (CSS pixels). */
export const WIDTH_MAX = 960;

/**
 * The widths a click on a column handle steps through (the single-pointer way to resize,
 * WCAG 2.5.7): the column tokens, as measured on the grid.
 */
export function nextPresetWidth(now: number, presets: readonly number[]): number {
  const sorted = [...presets].filter((w) => w > 0).sort((a, b) => a - b);
  return sorted.find((w) => w > now + 1) ?? sorted[0] ?? now;
}

/**
 * What the grid emits when an editor commits (contract of in-place editing, design
 * « Inventaire »): the cell and the text written. The host checks it, writes it (draft,
 * changeset) and renders the cell's state afterwards (`cellState`).
 */
export interface GridCommit {
  row: string;
  column: string;
  value: string;
}

/**
 * The rows of a virtualised grid in the document: `start` the index of the first rendered
 * row, `end` the index after the last one, among the rows the grid was given (the loaded
 * rows, never the whole list: `ListWindow.end` of the shell's `useNextPage`), `size` the
 * rows of one screen.
 */
export interface GridWindow {
  start: number;
  end: number;
  size: number;
}

/**
 * The keys selected by Shift with an arrow: the ones selected when the range began, and
 * every key from the anchor to `to`, both included.
 */
export function selectRange(
  keys: readonly string[],
  base: readonly string[],
  anchor: string,
  to: string,
): string[] {
  const a = keys.indexOf(anchor);
  const b = keys.indexOf(to);
  if (a < 0 || b < 0) return [...base];
  const range = keys.slice(Math.min(a, b), Math.max(a, b) + 1);
  const out = new Set(base);
  for (const key of range) out.add(key);
  return [...out];
}

/**
 * The parts of the body of a virtualised grid, in order: the rows to render (`index`), and
 * between them gaps of the height of the rows left out (`gap`, pixels). `rendered`: the
 * indexes in the document (the window, plus the row of the focus and of the editor, kept
 * mounted whatever the scroll); `start(i)` and `end(i)`: where row `i` begins and ends.
 */
export type BodyPart = { index: number } | { gap: number; key: string };
export function bodyParts(
  rendered: readonly number[],
  count: number,
  start: (i: number) => number,
  end: (i: number) => number,
  origin = 0,
): BodyPart[] {
  const indexes = [...new Set(rendered)].filter((i) => i >= 0 && i < count).sort((a, b) => a - b);
  const parts: BodyPart[] = [];
  let at = origin;
  for (const index of indexes) {
    const gap = start(index) - at;
    if (gap > 0) parts.push({ gap, key: `gap-${index}` });
    parts.push({ index });
    at = end(index);
  }
  const last = count > 0 ? end(count - 1) : origin;
  if (last - at > 0) parts.push({ gap: last - at, key: "gap-end" });
  return parts;
}

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
  widths: ColumnWidths = {},
  select = false,
): Record<string, string> {
  const rendered = renderedColumns(columns, visibility);
  const lead = select ? `${SELECT_TRACK} ` : "";
  const upTo = (max: number) =>
    lead +
    columnsTemplate(
      rendered.filter((c) => priorityOf(columns, c, visibility) <= max),
      widths,
    );
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

// A token is the width of a column, never narrower than its header (title, sort and
// filter whole: the `auto` minimum is the min-content of the header cell, the body cells
// counting for nothing; relecture du 2026-10-26).
const WIDTHS = {
  xs: "minmax(auto, var(--k-size-col-xs))",
  sm: "minmax(auto, var(--k-size-col-sm))",
  md: "minmax(auto, var(--k-size-col-md))",
  lg: "minmax(auto, var(--k-size-col-lg))",
  fill: "minmax(var(--k-size-col-md), 1fr)",
} as const;

/**
 * The template shared by the header and every row (`--k-grid-columns`, set on the grid by
 * CSSOM): one width per column, from tokens only. Head and body therefore always align,
 * whatever the rows rendered (the width fix of the prototype).
 */
export function columnsTemplate(columns: readonly GridColumn[], widths: ColumnWidths = {}): string {
  return columns
    .map((column) => {
      const width = widths[column.id];
      // A width the user gave: a fixed track, still never narrower than the header.
      return width !== undefined && width > 0
        ? `minmax(auto, ${Math.round(width)}px)`
        : WIDTHS[column.width ?? "md"];
    })
    .join(" ");
}

/** The track of the column of row checkboxes (multiple selection): a control and its room. */
export const SELECT_TRACK = "calc(var(--k-control-height-sm) + 2 * var(--k-cell-padding-inline))";

/** The id the column of row checkboxes takes in a `CellRef` (never a host's column). */
export const SELECT_COLUMN = "k-select";
