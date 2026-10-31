// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The rows of a virtualised list of rows (`DataGrid`, `TreeGrid`, `Tree`): the window the
// virtualiser renders, plus the rows that must stay in the document whatever the scroll
// (the row of the focus, the row of an editor: a row unmounted under the focus sends it to
// `<body>`, spike 005), laid out between gaps of the height of the rows left out; and the
// rows in view, read from the boxes of the rendered rows (Page Up, Page Down, the window
// of a paged list).

import { useVirtualizer } from "@tanstack/vue-virtual";
import { computed, ref, type ComponentPublicInstance, type Ref } from "vue";

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

/** Rows beyond which a list is virtualised (as `DiffList` and `DataGrid`, design). */
export const VIRTUAL_ROWS = 200;

/** What a virtualised list of rows gives `useVirtualRows`. */
export interface VirtualRowsHost {
  count(): number;
  keyAt(index: number): string;
  indexOf(key: string): number | undefined;
  scroller: Ref<HTMLElement | null>;
  /** The keys of the rows kept mounted whatever the scroll (the focus, an editor). */
  kept(): (string | undefined)[];
  /** The height above the rows in the scroller (a sticky header). */
  top?(): number;
  /** Not virtualised now (the list is loading). */
  off?(): boolean;
}

/**
 * The virtualisation of a list of rows (`TreeGrid`, `Tree`): beyond `VIRTUAL_ROWS`, the
 * window of `@tanstack/vue-virtual` and the rows kept mounted, between gaps of the height
 * of the others (measured, else the row height of the density, read from `--k-row-height`
 * of the scroller); every row below it.
 */
export function useVirtualRows(host: VirtualRowsHost) {
  const virtual = computed(() => !host.off?.() && host.count() > VIRTUAL_ROWS);
  /** The height of a row before it is measured: the row height of the density. */
  const estimate = ref(28);
  const top = () => host.top?.() ?? 0;
  const virtualizer = useVirtualizer<HTMLElement, HTMLElement>(
    computed(() => ({
      count: virtual.value ? host.count() : 0,
      getScrollElement: () => host.scroller.value,
      estimateSize: () => estimate.value,
      getItemKey: (index: number) => host.keyAt(index),
      overscan: 10,
      scrollMargin: top(),
    })),
  );
  function measure(el: Element | ComponentPublicInstance | null): void {
    const node = el instanceof Element ? el : (el?.$el as Element | undefined);
    if (virtual.value && node instanceof HTMLElement) virtualizer.value.measureElement(node);
  }
  const parts = computed<BodyPart[]>(() => {
    const count = host.count();
    if (!virtual.value) return Array.from({ length: count }, (_, index) => ({ index }));
    const v = virtualizer.value;
    const keep = keptIndexes(
      v.getVirtualItems().map((item) => item.index),
      host.kept(),
      host.indexOf,
    );
    const sizes = v.measurementsCache;
    const at = (i: number) => sizes[i]?.start ?? top() + i * estimate.value;
    const end = (i: number) => sizes[i]?.end ?? at(i) + estimate.value;
    return bodyParts(keep, count, at, end, at(0));
  });
  /** The row height of the density, read again when the density changes. */
  function readEstimate(): void {
    const el = host.scroller.value;
    const row = el ? parseFloat(getComputedStyle(el).getPropertyValue("--k-row-height")) : NaN;
    if (Number.isFinite(row) && row !== estimate.value) estimate.value = row;
  }
  return { virtual, parts, measure, estimate, readEstimate };
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
