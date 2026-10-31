// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The rows of a virtualised list of rows (`DataGrid`; `TreeGrid`, task 26): the window the
// virtualiser renders, plus the rows that must stay in the document whatever the scroll
// (the row of the focus, the row of an editor: a row unmounted under the focus sends it to
// `<body>`, spike 005), laid out between gaps of the height of the rows left out; and the
// rows in view, read from the boxes of the rendered rows (Page Up, Page Down, the window
// of a paged list).

/**
 * The parts of a virtualised body, in order: the rows to render (`index`), and between
 * them gaps of the height of the rows left out (`gap`, pixels). `rendered`: the indexes in
 * the document; `start(i)` and `end(i)`: where row `i` begins and ends; `origin`: where
 * the first row begins.
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

/**
 * The indexes to render: the window, and the rows of the keys kept mounted (the focus,
 * the editor), wherever they are.
 */
export function keptIndexes(
  window: readonly number[],
  kept: readonly (string | undefined)[],
  indexOf: (key: string) => number | undefined,
): number[] {
  const out = [...window];
  for (const key of kept) {
    const i = key === undefined ? undefined : indexOf(key);
    if (i !== undefined) out.push(i);
  }
  return out;
}

/** The rows wholly in view: their indexes, from the first to the last one. */
export interface RowsInView {
  first: number;
  last: number;
  count: number;
}

/**
 * The rows in view in `scroller` (under its sticky top of `top` pixels) and in the
 * viewport: each row gives its index (`data-index`). Null when none is in view.
 */
export function rowsInView(
  scroller: HTMLElement,
  rows: Iterable<HTMLElement>,
  top: number,
): RowsInView | null {
  const box = scroller.getBoundingClientRect();
  const from = Math.max(box.top + top, 0);
  const to = Math.min(box.top + scroller.clientHeight, window.innerHeight);
  let first = Infinity;
  let last = -1;
  let count = 0;
  for (const row of rows) {
    const index = Number(row.dataset.index);
    if (!Number.isFinite(index)) continue;
    const r = row.getBoundingClientRect();
    if (r.bottom <= from || r.top >= to) continue;
    first = Math.min(first, index);
    last = Math.max(last, index);
    if (r.top >= from - 1 && r.bottom <= to + 1) count += 1;
  }
  return last < 0 ? null : { first, last, count };
}
