// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, ref, shallowReactive, type App } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import GridRowsHost from "./fixtures/GridRowsHost.vue";
import type { CellRef, CellState, GridColumn, RowState } from "../src/components/DataGrid/grid.ts";
import { setLocale } from "../src/i18n.ts";

// The rows of `DataGrid` are components of their own (task 021-22o-b): when the grid renders
// — the roving focus moves, a press on a link of a cell focuses it, an editor opens, the
// host renders —, a row renders again only when its row changes or what it shows does, and
// each state of the host is asked once per cell. A row that does not render again still
// shows what is true now: its stop, its selection, its editor, its marks, its reasons, the
// gate of writes, the language.

const apps: App[] = [];
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
  setLocale("fr");
});

const ROWS = Array.from({ length: 30 }, (_, i) => ({ id: `r${i}`, label: `Ligne ${i}` }));
const columns: GridColumn[] = [
  { id: "id", label: "Clé", width: "md", mono: true },
  { id: "label", label: "Libellé", width: "fill" },
];

/** A grid of 30 rows, its host's states reactive, what it asks counted by row. */
function mounted() {
  const props = shallowReactive<Record<string, unknown>>({
    label: "Lignes",
    active: undefined as CellRef | undefined,
    editing: undefined as CellRef | undefined,
    selected: [] as string[],
  });
  /** The host's states: the cells', the rows', the gate (`as_of`, the width). */
  const states = shallowReactive<Record<string, CellState>>({});
  const rowStates = shallowReactive<Record<string, RowState>>({});
  const held = shallowReactive<Record<string, string>>({});
  const gate = ref<"open" | "past">("open");
  const asked = new Map<string, number>();
  const slotted = new Map<string, number>();
  const count = (map: Map<string, number>, key: string) => map.set(key, (map.get(key) ?? 0) + 1);
  // The same functions at each render, as a host's (a new one is a new prop).
  const cellState = (row: { id: string }, column: GridColumn) => {
    count(asked, row.id);
    return states[`${row.id}/${column.id}`];
  };
  const rowState = (row: { id: string }) => rowStates[row.id];
  const editable = (_: unknown, column: GridColumn) =>
    gate.value === "open" && column.id === "label";
  const readonlyReason = (_: unknown, column: GridColumn) =>
    gate.value === "past" ? "Lu à une date passée" : column.id === "id" ? "Clé figée" : undefined;
  const stateReason = (row: { id: string }, column: GridColumn) => held[`${row.id}/${column.id}`];
  // The slot `cell` compiled, as a screen's (a slot of a render function is new at each
  // render of its host, and renders the rows again with the grid, as Vue does any child).
  const seen = (row: string) => {
    count(slotted, row);
    return row;
  };
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp({
    render: () =>
      h(GridRowsHost, {
        seen,
        grid: {
          label: props.label,
          columns,
          rows: ROWS,
          rowKey: "id",
          selectable: true,
          active: props.active,
          "onUpdate:active": (cell: CellRef) => (props.active = cell),
          editing: props.editing,
          "onUpdate:editing": (cell: CellRef | undefined) => (props.editing = cell),
          selected: props.selected,
          "onUpdate:selected": (keys: string[]) => (props.selected = keys),
          cellState,
          rowState,
          editable,
          readonlyReason,
          stateReason,
        },
      }),
  });
  app.config.errorHandler = (error) => {
    throw error;
  };
  apps.push(app);
  app.mount(root);
  const reset = () => {
    asked.clear();
    slotted.clear();
  };
  const cell = (row: string, column: string) =>
    root.querySelector<HTMLElement>(
      `[role='gridcell'][data-row="${row}"][data-column="${column}"]`,
    )!;
  const described = (el: HTMLElement) =>
    (el.getAttribute("aria-describedby") ?? "")
      .split(" ")
      .filter(Boolean)
      .map((id) => document.getElementById(id)?.textContent ?? `(${id} absent)`);
  return { props, states, rowStates, held, gate, asked, slotted, reset, cell, described, root };
}

describe("DataGrid: a row renders again only when what it shows changes (task 021-22o-b)", () => {
  it("asks each state once per cell, and a row no more when the grid alone renders", async () => {
    const grid = mounted();
    await nextTick();
    // Once by its row, once by the table of the hidden sentences (`useCellReasons`), which
    // reads again only when a state changes (before: four and five times by cell).
    expect(grid.asked.size).toBe(ROWS.length);
    for (const [row, n] of grid.asked) expect(n, row).toBe(2 * columns.length);
    expect(grid.slotted.size).toBe(ROWS.length);
    grid.reset();
    // The grid renders again (its name): no row asks, nor renders its cells.
    grid.props.label = "Toutes les lignes";
    await nextTick();
    expect(grid.root.querySelector("[role='grid']")?.getAttribute("aria-label")).toBe(
      "Toutes les lignes",
    );
    expect(grid.asked.size).toBe(0);
    expect(grid.slotted.size).toBe(0);
  });

  it("a step of the focus renders the row it leaves and the row it reaches", async () => {
    const grid = mounted();
    grid.props.active = { row: "r3", column: "label" };
    await nextTick();
    grid.reset();
    // At the keyboard: a step down.
    grid.cell("r3", "label").focus();
    grid
      .cell("r3", "label")
      .dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    await nextTick();
    await nextTick();
    expect([...grid.asked.keys()].sort()).toEqual(["r3", "r4"]);
    expect([...grid.slotted.keys()].sort()).toEqual(["r3", "r4"]);
    expect(grid.cell("r4", "label").tabIndex).toBe(0);
    expect(grid.cell("r3", "label").tabIndex).toBe(-1);
    expect(document.activeElement).toBe(grid.cell("r4", "label"));
    expect(grid.root.querySelectorAll("[role='grid'] [tabindex='0']")).toHaveLength(1);
    grid.reset();
    // A press on the link of a cell focuses it (`focusin`): its row and the row of the stop.
    grid.cell("r20", "id").querySelector("a")!.focus();
    await nextTick();
    expect([...grid.asked.keys()].sort()).toEqual(["r20", "r4"]);
    expect(grid.props.active).toEqual({ row: "r20", column: "id" });
    expect(grid.cell("r20", "id").tabIndex).toBe(0);
    expect(grid.cell("r4", "label").tabIndex).toBe(-1);
    // The link stays out of the tab order, rendered again or not (one tab stop).
    for (const link of grid.root.querySelectorAll<HTMLElement>("[role='gridcell'] a"))
      expect(link.tabIndex, link.textContent!).toBe(-1);
  });

  it("the selection, the editor and the error render their rows alone, and show", async () => {
    const grid = mounted();
    await nextTick();
    grid.reset();
    grid.props.selected = ["r5"];
    await nextTick();
    expect([...grid.slotted.keys()]).toEqual(["r5"]);
    expect(grid.asked.size).toBe(0);
    expect(grid.cell("r5", "id").closest("[role='row']")!.getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(grid.cell("r6", "id").closest("[role='row']")!.getAttribute("aria-selected")).toBe(
      "false",
    );
    grid.reset();
    // An editor opens, the host's way: its row alone, the editor in its cell.
    grid.props.editing = { row: "r7", column: "label" };
    await nextTick();
    expect([...grid.slotted.keys()]).toEqual(["r7"]);
    expect([...grid.asked.keys()]).toEqual(["r7"]);
    expect(grid.cell("r7", "label").dataset.editing).toBe("true");
    expect(grid.cell("r7", "label").querySelector("input")?.value).toBe("Ligne 7");
    grid.props.editing = undefined;
    await nextTick();
    expect(grid.cell("r7", "label").querySelector("input")).toBeNull();
    grid.reset();
    // An error comes to a cell: its row alone renders, said by the cell (the table of the
    // hidden sentences reads every cell again: a state changed).
    grid.states["r8/label"] = { error: "Valeur refusée", code: "field.invalid" };
    await nextTick();
    expect([...grid.slotted.keys()]).toEqual(["r8"]);
    expect(grid.cell("r8", "label").getAttribute("aria-invalid")).toBe("true");
    expect(grid.described(grid.cell("r8", "label")).join(" ")).toContain("Valeur refusée");
  });

  it("a row not rendered again shows the marks, reasons, gate and language of now", async () => {
    const grid = mounted();
    await nextTick();
    // A draft mark and a state held in review, with its reason.
    grid.rowStates["r2"] = "draft";
    grid.states["r2/label"] = "review";
    grid.held["r2/label"] = "Tenue par une politique";
    await nextTick();
    const row = grid.cell("r2", "id").closest("[role='row']")!;
    expect(row.getAttribute("data-row-state")).toBe("draft");
    expect(grid.cell("r2", "id").textContent).toContain("(en brouillon)");
    expect(grid.cell("r2", "label").textContent).toContain("(en revue)");
    expect(grid.described(grid.cell("r2", "label"))).toEqual(["Tenue par une politique"]);
    // Its reason changes, then goes.
    grid.held["r2/label"] = "Tenue par une autre politique";
    await nextTick();
    expect(grid.described(grid.cell("r2", "label"))).toEqual(["Tenue par une autre politique"]);
    delete grid.states["r2/label"];
    delete grid.held["r2/label"];
    await nextTick();
    expect(grid.cell("r2", "label").getAttribute("aria-describedby")).toBeNull();
    // The gate closes (`as_of`, or a narrow frame): every cell read only, said why.
    expect(grid.cell("r9", "label").getAttribute("aria-readonly")).toBeNull();
    grid.gate.value = "past";
    await nextTick();
    expect(grid.cell("r9", "label").getAttribute("aria-readonly")).toBe("true");
    expect(grid.described(grid.cell("r9", "label"))).toEqual(["Lu à une date passée"]);
    grid.gate.value = "open";
    await nextTick();
    expect(grid.cell("r9", "label").getAttribute("aria-readonly")).toBeNull();
    expect(grid.described(grid.cell("r9", "id"))).toEqual(["Clé figée"]);
    // The language: the hidden words of the rows that did not render since.
    setLocale("en");
    await nextTick();
    expect(grid.cell("r2", "id").textContent).toContain("(in draft)");
    expect(grid.cell("r2", "id").querySelector("a")).not.toBeNull();
  });

  it("keeps the ids of the reasons to the sentences said now", async () => {
    const grid = mounted();
    await nextTick();
    const hidden = () => grid.root.querySelectorAll("[hidden] > span").length;
    const before = hidden();
    for (let i = 0; i < 20; i++) {
      grid.held["r1/label"] = `Raison ${i}`;
      grid.states["r1/label"] = "review";
      await nextTick();
      expect(grid.described(grid.cell("r1", "label"))).toEqual([`Raison ${i}`]);
    }
    // One sentence held at a time: one more hidden sentence, whatever was said before.
    expect(hidden()).toBe(before + 1);
    const id = grid.cell("r1", "label").getAttribute("aria-describedby")!;
    expect(document.querySelectorAll(`[id="${id}"]`)).toHaveLength(1);
  });
});
