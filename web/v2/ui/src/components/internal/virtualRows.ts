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
  /** The keys of every row in order, when the host keeps them: read once per change. */
  keys?(): readonly string[];
  indexOf(key: string): number | undefined;
  scroller: Ref<HTMLElement | null>;
  /** The keys of the rows kept mounted whatever the scroll (the focus, an editor). */
  kept(): (string | undefined)[];
  /** The height above the rows in the scroller (a sticky header). */
  top?(): number;
  /** Not virtualised now (the list is loading). */
  off?(): boolean;
  /**
   * The rows in view kept in their place when rows go or come at the head of the list
   * (`headShift`: a paged list whose cache drops its first page, task 021-22f).
   */
  anchor?: boolean;
}

/**
 * The rows `after` is `before` moved by at its head: dropped from it (positive), or added
 * before it (negative), the rows they share in the same order, whatever comes or goes at
 * the tail; 0 for any other change (a sort, a filter, rows that come or go at the tail
 * alone). A paged list whose cache drops its first page as it reads the next one
 * (`MAX_PAGES` of the SDK) moves by a page.
 */
export function headShift(before: readonly string[], after: readonly string[]): number {
  // `to` is `from` less its first `at` rows, some rows after them (none of those gone).
  const dropped = (from: readonly string[], to: readonly string[]) => {
    const at = from.indexOf(to[0]!);
    if (at <= 0) return 0;
    const shared = Math.min(from.length - at, to.length);
    for (let i = 0; i < shared; i++) if (from[at + i] !== to[i]) return 0;
    const gone = new Set(from.slice(0, at));
    for (let i = shared; i < to.length; i++) if (gone.has(to[i]!)) return 0;
    return at;
  };
  if (before.length === 0 || after.length === 0) return 0;
  const added = dropped(after, before);
  return dropped(before, after) || (added > 0 ? -added : 0);
}

/**
 * The rows at least a move of the head must count for the rows in view to stay in their
 * place (`anchor`): a page of a paged list (100 rows from the server), never the few rows
 * a write creates or removes at the head of a sorted list, which are seen where they come.
 */
export const ANCHORED_ROWS = 50;

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
  /** The keys of the rows at the last options (`anchor`). */
  let lastKeys: readonly string[] = [];
  /** The last options anchor the rows in view (`anchorTo: "end"`), until their render. */
  let anchoring = false;
  const virtualizer = useVirtualizer<HTMLElement, HTMLElement>(
    computed(() => {
      const count = virtual.value ? host.count() : 0;
      const all = count > 0 ? (host.keys?.() ?? null) : null;
      // A page gone or come at the head, the grid scrolled: the virtualiser keeps the row
      // at the top of the scroller where it stands (`anchorTo: "end"`, its anchoring when
      // the keys at its edges change), so the rows in view, the focus and an editor do not
      // move. Only then: a sort, a filter, a row created or removed keep the scroll offset,
      // and at the top of the list the rows that come are seen.
      const keys = !host.anchor
        ? []
        : (all ?? Array.from({ length: count }, (_, i) => host.keyAt(i)));
      const shifted =
        Math.abs(headShift(lastKeys, keys)) >= ANCHORED_ROWS &&
        (host.scroller.value?.scrollTop ?? 0) > 0;
      lastKeys = keys;
      anchoring = shifted;
      return {
        count,
        getScrollElement: () => host.scroller.value,
        estimateSize: () => estimate.value,
        getItemKey: all
          ? (index: number) => all[index] ?? host.keyAt(index)
          : (index: number) => host.keyAt(index),
        overscan: 10,
        scrollMargin: top(),
        anchorTo: shifted ? ("end" as const) : ("start" as const),
      };
    }),
  );
  /** The key each element was last measured under: a recycled row changes it. */
  const measuredAs = new WeakMap<HTMLElement, string>();
  /** The rows to measure once their render is committed (`measure`, `commit`). */
  const unmeasured = new Set<HTMLElement>();
  /**
   * A row element given by its template ref, measured once the render is committed, never
   * at once: Vue calls a function ref in the middle of the patch, where a measure lays out
   * a body half patched (task 021-22f: the gap before the window unmounted, the rows after
   * it not yet mounted, at the end of a list, the scroller's height shrinks to the rows
   * left and the browser clamps the scroll offset there, to the top of the list).
   */
  function measure(el: Element | ComponentPublicInstance | null): void {
    const node = el instanceof Element ? el : (el?.$el as Element | undefined);
    if (virtual.value && node instanceof HTMLElement) unmeasured.add(node);
  }
  function measureNow(node: HTMLElement): void {
    if (!virtual.value || !node.isConnected) return;
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
    const rows = [...unmeasured];
    unmeasured.clear();
    for (const node of rows) measureNow(node);
    // The anchoring done once the rows are rendered: kept, `anchorTo: "end"` would also pin
    // the view to the end of the list while a row there changes size.
    if (anchoring) {
      anchoring = false;
      virtualizer.value.options.anchorTo = "start";
    }
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
