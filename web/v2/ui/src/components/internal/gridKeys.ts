// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** A position in a grid: row and column indexes from 0, headers included. */
export interface GridPosition {
  row: number;
  column: number;
}

/** The rows a page key moves by. */
export const PAGE_ROWS = 10;

/**
 * Where the focus goes for `event` in a grid of `rows` × `columns` cells (APG Grid,
 * « Keyboard interaction » of data grids): arrows by one cell, Home and End to the ends
 * of the row, Ctrl+Home and Ctrl+End to the first and last cell, Page Up and Page Down by
 * a page of rows; null for a key the grid leaves alone. The inline arrows follow the
 * direction of the text (`rtl`).
 */
export function moveInGrid(
  from: GridPosition,
  event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey">,
  rows: number,
  columns: number,
  rtl = false,
): GridPosition | null {
  const clamp = (row: number, column: number): GridPosition => ({
    row: Math.max(0, Math.min(rows - 1, row)),
    column: Math.max(0, Math.min(columns - 1, column)),
  });
  const ctrl = event.ctrlKey || event.metaKey;
  const forward = rtl ? -1 : 1;
  switch (event.key) {
    case "ArrowRight":
      return clamp(from.row, from.column + forward);
    case "ArrowLeft":
      return clamp(from.row, from.column - forward);
    case "ArrowDown":
      return clamp(from.row + 1, from.column);
    case "ArrowUp":
      return clamp(from.row - 1, from.column);
    case "Home":
      return clamp(ctrl ? 0 : from.row, 0);
    case "End":
      return clamp(ctrl ? rows - 1 : from.row, columns - 1);
    case "PageDown":
      return clamp(from.row + PAGE_ROWS, from.column);
    case "PageUp":
      return clamp(from.row - PAGE_ROWS, from.column);
    default:
      return null;
  }
}

/** Whether `el` is written right to left. */
export const isRtl = (el: Element): boolean => getComputedStyle(el).direction === "rtl";
