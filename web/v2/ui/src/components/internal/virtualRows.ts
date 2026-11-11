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
import { computed, onMounted, onUpdated, ref, type ComponentPublicInstance, type Ref } from "vue";

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

/** The rows rendered last by a body that recycles them: their keys in order, their slots. */
export interface RenderedSlots {
  order: readonly string[];
  slots: ReadonlyMap<string, number>;
}

/**
 * The slots of the rows of a virtualised body whose elements are recycled (task 021-22e:
 * at each frame of a tree of 10⁴ rows scrolled, the rows that came were mounted and those
 * that left unmounted). A row rendered before keeps its slot, so its element; a row that
 * comes takes the slot of one that left, and its element, patched in place.
 *
 * The elements of the `guarded` rows (the focus, an editor, now and at the last render)
 * are never moved: moved, an element loses the focus. A slot is therefore taken again
 * only on the same side of each guarded row as before (otherwise its element would cross
 * it, and the reconciliation might move the guarded one instead); and never one of
 * `reserved`, the slot of a row guarded until now that leaves (its element may hold the
 * focus, which another row must not inherit: it is unmounted). Otherwise a slot no row
 * had, the smallest.
 */
export function rowSlots(
  keys: readonly string[],
  before: RenderedSlots,
  {
    guarded = new Set(),
    reserved = new Set(),
    reuse = true,
  }: {
    guarded?: ReadonlySet<string>;
    reserved?: ReadonlySet<number>;
    /** Slots of rows that left taken again; without, each row that comes gets a new one. */
    reuse?: boolean;
  } = {},
): Map<string, number> {
  const slots = new Map<string, number>();
  const taken = new Set<number>(reserved);
  for (const key of keys) {
    const slot = before.slots.get(key);
    if (slot !== undefined && !slots.has(key)) {
      slots.set(key, slot);
      taken.add(slot);
    }
  }
  // The side of each row against the guarded rows that already had an element.
  const sides = (order: readonly string[]) => {
    const side = new Map<string, number>();
    let passed = 0;
    for (const key of order) {
      if (guarded.has(key) && before.slots.has(key)) passed += 1;
      side.set(key, passed);
    }
    return side;
  };
  const was = sides(before.order);
  const now = sides(keys);
  const freed = new Map<number, number[]>();
  for (const key of before.order) {
    const slot = before.slots.get(key)!;
    if (slots.has(key) || taken.has(slot)) continue;
    const side = was.get(key)!;
    freed.set(side, [...(freed.get(side) ?? []), slot]);
  }
  const had = new Set(before.slots.values());
  let fresh = 0;
  for (const key of keys) {
    if (slots.has(key)) continue;
    let slot = reuse ? freed.get(now.get(key)!)?.shift() : undefined;
    if (slot === undefined) {
      while (taken.has(fresh) || had.has(fresh)) fresh += 1;
      slot = fresh;
    }
    slots.set(key, slot);
    taken.add(slot);
  }
  return slots;
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
  /** The key each element was last measured under: a recycled row changes it. */
  const measuredAs = new WeakMap<HTMLElement, string>();
  function measure(el: Element | ComponentPublicInstance | null): void {
    const node = el instanceof Element ? el : (el?.$el as Element | undefined);
    if (!virtual.value || !(node instanceof HTMLElement)) return;
    const v = virtualizer.value;
    // An element shows another row now: its former key forgets it, or measuring that
    // key's next element would stop observing this one, still in use.
    const index = Number(node.dataset.index);
    const key = Number.isInteger(index) ? host.keyAt(index) : undefined;
    const before = measuredAs.get(node);
    if (before !== undefined && before !== key && v.elementsCache.get(before) === node)
      v.elementsCache.delete(before);
    if (key !== undefined) measuredAs.set(node, key);
    v.measureElement(node);
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
  /**
   * The last render committed (`onMounted`, `onUpdated`): its rows, their slots, the rows
   * kept then. `keys` reads it and never writes it: it may be read outside a render (an
   * event handler reads `parts`), or twice before one, and gives the same keys each time.
   */
  let rendered: RenderedSlots & { kept: readonly (string | undefined)[] } = {
    order: [],
    slots: new Map(),
    kept: [],
  };
  /** The state `keys` computed last, committed once its render is done. */
  let computedLast: typeof rendered | undefined;
  const commit = () => {
    if (computedLast) rendered = computedLast;
  };
  onMounted(commit);
  onUpdated(commit);
  /**
   * The key of each part (`rowSlots`): `r` and the slot of its row, the gaps by their
   * order between the guarded rows. Virtualised, a row that comes takes the element of
   * one that left; below `VIRTUAL_ROWS`, each row that comes a new slot, as a list keyed
   * by its rows. Either way a row rendered before keeps its slot, so the rows of the focus
   * and of an editor keep their elements when the list crosses `VIRTUAL_ROWS` (a node
   * opened, or closed).
   */
  const keys = computed<string[]>(() => {
    const list = parts.value;
    const order = list.flatMap((part) => ("index" in part ? [host.keyAt(part.index)] : []));
    const now = new Set(order);
    const kept = host.kept();
    const guarded = new Set(
      [...kept, ...rendered.kept].filter((key): key is string => key !== undefined),
    );
    const reserved = new Set<number>();
    for (const key of guarded) {
      const slot = now.has(key) ? undefined : rendered.slots.get(key);
      if (slot !== undefined) reserved.add(slot);
    }
    const slots = rowSlots(order, rendered, { guarded, reserved, reuse: virtual.value });
    // The gaps by their order between the guarded rows: never across one either.
    let side = 0;
    let gap = 0;
    const out = list.map((part) => {
      if ("gap" in part) return `g${side}-${gap++}`;
      const key = host.keyAt(part.index);
      if (guarded.has(key) && rendered.slots.has(key)) {
        side += 1;
        gap = 0;
      }
      return `r${slots.get(key)}`;
    });
    computedLast = { order, slots, kept };
    return out;
  });
  /** The row height of the density, read again when the density changes. */
  function readEstimate(): void {
    const el = host.scroller.value;
    const row = el ? parseFloat(getComputedStyle(el).getPropertyValue("--k-row-height")) : NaN;
    if (Number.isFinite(row) && row !== estimate.value) estimate.value = row;
  }
  return { virtual, parts, keys, measure, estimate, readEstimate };
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
