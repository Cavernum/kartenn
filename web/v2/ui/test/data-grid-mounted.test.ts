// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, shallowReactive, type App, type Component } from "vue";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import DataGrid from "../src/components/DataGrid/DataGrid.vue";
import type { CellRef, GridColumn } from "../src/components/DataGrid/grid.ts";
import { ANCHORED_ROWS, headShift } from "../src/components/internal/virtualRows.ts";

// A paged list past the pages its cache keeps (task 021-22f): the cache drops the first
// page as it reads the next one, every row moves up a page among the rows of the grid, and
// the grid keeps the rows in view where they stand, whichever way it was scrolled, the
// focus and an open editor with them.

const apps: App[] = [];
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

/** The rows from `from` to `to` (excluded), 100 by page. */
const rowsOf = (from: number, to: number) =>
  Array.from({ length: to - from }, (_, i) => ({ id: `a${from + i}`, label: `Ligne ${from + i}` }));
const columns: GridColumn[] = [
  { id: "id", label: "Adresse", width: "md", mono: true },
  { id: "label", label: "Libellé", width: "fill" },
];

// The grid scrolls in a box of 600 px, its rows 28 px high; the document lays nothing out,
// the scroll offset of the grid is its own, written by the virtualiser (`scrollTo`).
const ROW = 28;
const sizes = { clientHeight: "k-data-grid", offsetHeight: "k-data-grid__row" } as const;
const heights = { clientHeight: 600, offsetHeight: ROW } as const;
const offsets = new WeakMap<Element, number>();
beforeEach(() => {
  for (const name of ["clientHeight", "offsetHeight"] as const)
    Object.defineProperty(HTMLElement.prototype, name, {
      configurable: true,
      get(this: HTMLElement) {
        return this.classList.contains(sizes[name]) ? heights[name] : 0;
      },
    });
  // The height of what the grid holds: its header, then its body, gaps and rows.
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return this.classList.contains("k-data-grid") ? ROW + bodyHeight(this) : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "scrollTop", {
    configurable: true,
    get(this: HTMLElement) {
      return offsets.get(this) ?? 0;
    },
    set(this: HTMLElement, top: number) {
      offsets.set(this, Math.max(0, top));
    },
  });
  Object.defineProperty(HTMLElement.prototype, "scrollTo", {
    configurable: true,
    value(this: HTMLElement, options: ScrollToOptions) {
      if (options.top === undefined) return;
      this.scrollTop = options.top;
      this.dispatchEvent(new Event("scroll"));
    },
  });
});
afterEach(() => {
  for (const name of ["clientHeight", "offsetHeight", "scrollHeight", "scrollTop", "scrollTo"])
    delete (HTMLElement.prototype as unknown as Record<string, unknown>)[name];
});

/** A grid of 1 000 rows (ten pages) mounted, its props reactive, its models followed. */
async function mountGrid() {
  const props = shallowReactive<Record<string, unknown>>({
    label: "Adresses",
    columns,
    rows: rowsOf(0, 1000),
    rowKey: "id",
    editable: () => true,
    active: undefined as CellRef | undefined,
    editing: undefined as CellRef | undefined,
  });
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp({
    render: () =>
      h(DataGrid as Component, {
        ...props,
        "onUpdate:active": (cell: CellRef) => (props.active = cell),
        "onUpdate:editing": (cell: CellRef | undefined) => (props.editing = cell),
      }),
  });
  app.config.errorHandler = (error) => {
    throw error;
  };
  apps.push(app);
  app.mount(root);
  await settle();
  const grid = root.querySelector<HTMLElement>("[role='grid']")!;
  expect(grid.dataset.virtual).toBe("true");
  return { props, grid };
}

async function settle(): Promise<void> {
  for (let i = 0; i < 3; i++) await nextTick();
}

/** The grid scrolled to `top`, as the user's wheel would. */
async function scrollGrid(grid: HTMLElement, top: number): Promise<void> {
  grid.scrollTop = top;
  grid.dispatchEvent(new Event("scroll"));
  await settle();
}

/**
 * The row rendered under the pointer, `y` pixels below the top of the rows in view: the
 * body laid out from what it rendered, its gaps and its rows in order. Its key and where
 * its top stands in the window of the grid.
 */
function rowUnder(grid: HTMLElement, y: number): { key: string; top: number } | null {
  let at = 0;
  for (const part of grid.querySelectorAll<HTMLElement>(".k-data-grid__body > *")) {
    const height = heightOf(part);
    const top = at - grid.scrollTop;
    if (!part.classList.contains("k-data-grid__gap") && top <= y && y < top + height) {
      const key = part.querySelector<HTMLElement>("[data-row]")?.dataset.row;
      return key ? { key, top } : null;
    }
    at += height;
  }
  return null;
}

/** The height of a part of the body: a gap's own, a row's 28 px. */
const heightOf = (part: HTMLElement) =>
  part.classList.contains("k-data-grid__gap")
    ? parseFloat(part.style.getPropertyValue("--k-data-grid-gap"))
    : ROW;
const bodyHeight = (grid: HTMLElement) =>
  [...grid.querySelectorAll<HTMLElement>(".k-data-grid__body > *")].reduce(
    (sum, part) => sum + heightOf(part),
    0,
  );

/** The cache reads the eleventh page and drops the first: the same count, a page up. */
async function dropFirstPage(props: Record<string, unknown>): Promise<void> {
  props.rows = [...(props.rows as ReturnType<typeof rowsOf>).slice(100), ...rowsOf(1000, 1100)];
  await settle();
}

describe("DataGrid: a page dropped at the head of a paged list (task 021-22f)", () => {
  it("keeps the row under the pointer in its place, scrolled down to the end of the rows", async () => {
    const { props, grid } = await mountGrid();
    // Down, a screen at a time, near the end of the ten pages (row 870 or so).
    for (let top = 0; top <= 24_500; top += 3_500) await scrollGrid(grid, top);
    const before = rowUnder(grid, 300)!;
    expect(before.key).toBe(`a${Math.floor((24_500 + 300) / ROW)}`);
    const offset = grid.scrollTop;
    await dropFirstPage(props);
    expect(rowUnder(grid, 300)).toEqual(before);
    expect(grid.scrollTop).toBe(offset - 100 * ROW);
    // The window moved with the scroll: the next rows, then the new page, still come.
    await scrollGrid(grid, grid.scrollTop + 3_000);
    expect(rowUnder(grid, 300)!.key).toBe(`a${100 + Math.floor((offset - 2_800 + 3_300) / ROW)}`);
  });

  it("keeps the row under the pointer in its place, scrolled up from the end", async () => {
    const { props, grid } = await mountGrid();
    for (let top = 28_000; top >= 14_000; top -= 3_500) await scrollGrid(grid, top);
    const before = rowUnder(grid, 120)!;
    expect(before.key).toBe(`a${Math.floor((14_000 + 120) / ROW)}`);
    await dropFirstPage(props);
    expect(rowUnder(grid, 120)).toEqual(before);
    expect(grid.scrollTop).toBe(14_000 - 100 * ROW);
  });

  it("keeps the rows in place when a page comes back before them, and not across a sort", async () => {
    const { props, grid } = await mountGrid();
    props.rows = rowsOf(100, 1100);
    await settle();
    await scrollGrid(grid, 10_000);
    const before = rowUnder(grid, 200)!;
    // A page read before the first one kept: every row moves down a page.
    props.rows = rowsOf(0, 1000);
    await settle();
    expect(rowUnder(grid, 200)).toEqual(before);
    expect(grid.scrollTop).toBe(10_000 + 100 * ROW);
    // Sorted the other way: no row kept in place, the scroll offset kept.
    const offset = grid.scrollTop;
    props.rows = rowsOf(0, 1000).reverse();
    await settle();
    expect(grid.scrollTop).toBe(offset);
  });

  it("anchors nothing for a row created or removed at the head, scrolled or not", async () => {
    const { props, grid } = await mountGrid();
    const created = { id: "aaa", label: "Créée" };
    for (const top of [0, 5_000]) {
      await scrollGrid(grid, top);
      // A row created that sorts first: at the top of the list, seen at its place.
      props.rows = [created, ...rowsOf(0, 1000)];
      await settle();
      expect(grid.scrollTop, `created, at ${top}`).toBe(top);
      if (top === 0) expect(rowUnder(grid, 0)!.key).toBe("aaa");
      // Removed again: the rows move up one row, the scroll offset kept.
      props.rows = rowsOf(0, 1000);
      await settle();
      expect(grid.scrollTop, `removed, at ${top}`).toBe(top);
      // The first row removed.
      props.rows = rowsOf(1, 1000);
      await settle();
      expect(grid.scrollTop, `first removed, at ${top}`).toBe(top);
      props.rows = rowsOf(0, 1000);
      await settle();
    }
  });

  it("anchors nothing for a page dropped while the grid is at its top", async () => {
    const { props, grid } = await mountGrid();
    await dropFirstPage(props);
    expect(grid.scrollTop).toBe(0);
  });

  it("keeps the cell of the focus focused and an open editor with its text", async () => {
    const { props, grid } = await mountGrid();
    for (let top = 0; top <= 21_000; top += 3_500) await scrollGrid(grid, top);
    // The focus on the row under the pointer, the editor of the next one open.
    const focused = rowUnder(grid, 280)!;
    const cell = grid.querySelector<HTMLElement>(`[data-row="${focused.key}"][data-column="id"]`)!;
    props.active = { row: focused.key, column: "id" };
    await settle();
    cell.focus();
    expect(document.activeElement).toBe(cell);
    await dropFirstPage(props);
    expect(document.activeElement).toBe(cell);
    expect(cell.isConnected).toBe(true);
    expect(rowUnder(grid, 280)).toEqual(focused);

    const edited = rowUnder(grid, 280 + ROW)!;
    props.editing = { row: edited.key, column: "label" };
    await settle();
    const input = grid.querySelector<HTMLInputElement>(".k-data-grid__editor input")!;
    expect(input).not.toBeNull();
    input.focus();
    input.value = "Renommée";
    input.dispatchEvent(new Event("input"));
    await settle();
    await dropFirstPage(props);
    expect(input.isConnected).toBe(true);
    expect(document.activeElement).toBe(input);
    expect(input.value).toBe("Renommée");
    expect(props.editing).toEqual({ row: edited.key, column: "label" });
    expect(rowUnder(grid, 280 + ROW)).toEqual(edited);
  });
});

describe("headShift: rows gone or come at the head only (task 021-22f)", () => {
  const keys = (from: number, to: number) => rowsOf(from, to).map((row) => row.id);
  it("is the first page dropped, the next one read or not, or a page read before", () => {
    expect(headShift(keys(0, 1000), keys(100, 1100))).toBe(100);
    expect(headShift(keys(0, 1000), keys(100, 1000))).toBe(100);
    expect(headShift(keys(100, 1100), keys(0, 1000))).toBe(-100);
    // A row: counted as such, below `ANCHORED_ROWS`.
    expect(headShift(keys(0, 1000), keys(1, 1000))).toBe(1);
    expect(headShift(keys(0, 1000), ["aaa", ...keys(0, 1000)])).toBe(-1);
    expect(ANCHORED_ROWS).toBeGreaterThan(1);
    expect(ANCHORED_ROWS).toBeLessThanOrEqual(100);
  });
  it("is neither a page read at the end, nor a sort, nor rows of another list", () => {
    expect(headShift(keys(0, 900), keys(0, 1000))).toBe(0);
    expect(headShift(keys(0, 1000), keys(0, 1000))).toBe(0);
    expect(headShift(keys(0, 1000), keys(0, 1000).reverse())).toBe(0);
    expect(headShift(keys(0, 1000), keys(5000, 6000))).toBe(0);
    expect(headShift([], keys(0, 100))).toBe(0);
    // A row of the middle gone too: not a move of the head alone.
    const holed = keys(100, 1100).filter((key) => key !== "a500");
    expect(headShift(keys(0, 1000), holed)).toBe(0);
  });
});
