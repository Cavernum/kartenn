// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, ref, type App } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  CellState,
  GridColumn,
  GridCommit,
  RowState,
} from "../src/components/DataGrid/grid.ts";
import ExpandableGrid from "../src/components/ExpandableGrid/ExpandableGrid.vue";

// Why a state, everywhere a grid carries one (task 021-12n): `ExpandableGrid` gives both
// its grids why a cell offers no editor, why a cell is in its state and why a row is
// (`readonlyReason` with `editable`, `stateReason`, `rowReason`); a row in review says its
// reason on its first cell, beside its hidden word, even when the compact grid leaves out
// the column of the cell held. Mounted: the large view is a dialog, rendered only open.

const apps: App[] = [];
const commits: GridCommit[] = [];
afterEach(() => {
  commits.length = 0;
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

const columns: GridColumn[] = [
  { id: "vlan", label: "VLAN", mono: true },
  { id: "name", label: "Nom", width: "fill" },
  { id: "site", label: "Site" },
];
const rows = [
  { vlan: "100", name: "serveurs", site: "nord" },
  { vlan: "101", name: "postes", site: "sud" },
  { vlan: "102", name: "invités", site: "nord" },
  { vlan: "103", name: "imprimantes", site: "sud" },
];
const HELD = "Une politique d'approbation retient la modification : elle attend une revue.";
const OTHER = "Le site attend l'accord de son responsable.";
const WHY = "Le site vient du bâtiment du VLAN.";
// The cell held is in a column the compact grid leaves out (`site`); row 103 holds two
// writes, one in its first column.
const states: Record<string, CellState> = {
  "101:site": "review",
  "102:name": "draft",
  "103:vlan": "review",
  "103:site": "review",
};
const rowStates: Record<string, RowState> = { "101": "review", "102": "draft", "103": "review" };
const DRAFTED = "Le brouillon en cours renomme le VLAN.";
const REASONS: Record<string, string | string[]> = {
  "101": HELD,
  "102": DRAFTED,
  "103": [HELD, OTHER],
};
const key = (row: Record<string, unknown>) => String(row.vlan);
/** The large dialog is open: the model of the host, which Escape would close. */
const expanded = ref(true);

function mount(): void {
  expanded.value = true;
  const root = document.body.appendChild(document.createElement("div"));
  const app = createApp({
    render: () =>
      h(ExpandableGrid, {
        label: "VLAN",
        columns,
        rows,
        rowKey: "vlan",
        compactColumns: ["vlan", "name"],
        expanded: expanded.value,
        "onUpdate:expanded": (next: boolean) => (expanded.value = next),
        editable: (_: unknown, column: GridColumn) => column.id === "name",
        readonlyReason: (_: unknown, column: GridColumn) =>
          column.id === "site" ? WHY : undefined,
        cellState: (row: Record<string, unknown>, column: GridColumn) =>
          states[`${key(row)}:${column.id}`],
        stateReason: (row: Record<string, unknown>, column: GridColumn) =>
          states[`${key(row)}:${column.id}`] === "review" ? HELD : undefined,
        rowState: (row: Record<string, unknown>) => rowStates[key(row)],
        // Asked of every row: the grid keeps it to the rows with a state.
        rowReason: (row: Record<string, unknown>) => REASONS[key(row)] ?? "Jamais dit.",
        onCommit: (commit: GridCommit) => commits.push(commit),
      }),
  });
  apps.push(app);
  app.mount(root);
}

const description = (el: Element) =>
  (el.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .filter(Boolean)
    .map((id) => document.getElementById(id)?.textContent)
    .join(" | ");

async function grids(): Promise<{ compact: Element; full: Element }> {
  mount();
  await nextTick();
  // The compact grid first, the one of the dialog at the end of the document.
  await vi.waitFor(() => expect(document.querySelectorAll("[role='grid']")).toHaveLength(2));
  const [compact, full] = document.querySelectorAll("[role='grid']");
  return { compact: compact!, full: full! };
}
const cell = (grid: Element, row: string, column: string) =>
  grid.querySelector(`[data-row='${row}'][data-column='${column}']`)!;

describe("ExpandableGrid: why a state, in both grids (task 021-12n)", () => {
  it("says why a cell offers no editor and why a cell is held, in the large grid", async () => {
    const { full } = await grids();
    expect(cell(full, "100", "site").getAttribute("aria-readonly")).toBe("true");
    expect(description(cell(full, "100", "site"))).toBe(WHY);
    // Held, then without an editor: both sentences, its state's first.
    expect(description(cell(full, "101", "site"))).toBe(`${HELD} | ${WHY}`);
    // An editable cell has nothing to say of itself.
    expect(cell(full, "100", "name").getAttribute("aria-readonly")).toBeNull();
    expect(cell(full, "100", "name").getAttribute("aria-describedby")).toBeNull();
  });

  it("describes a row in review by its reason on its first cell, in both grids", async () => {
    const { compact, full } = await grids();
    for (const grid of [compact, full]) {
      const first = cell(grid, "101", "vlan");
      expect(first.querySelector(".k-visually-hidden")?.textContent).toBe("(en revue)");
      expect(description(first)).toBe(HELD);
      // The other cells of the row say nothing of the row.
      expect(cell(grid, "101", "name").getAttribute("aria-describedby")).toBeNull();
      // A row without a state has no reason, whatever the host answers.
      expect(cell(grid, "100", "vlan").getAttribute("aria-describedby")).toBeNull();
    }
    // The compact grid leaves out the column held: the row's first cell still says why.
    expect(cell(compact, "101", "site")).toBeNull();
  });

  it("says the reason the host gives of a row in a draft, and only on its first cell", async () => {
    const { compact } = await grids();
    expect(description(cell(compact, "102", "vlan"))).toBe(DRAFTED);
    // The cell in a draft has no reason of its own (`stateReason` says none).
    expect(cell(compact, "102", "name").getAttribute("aria-describedby")).toBeNull();
    expect(document.body.textContent).not.toContain("Jamais dit.");
  });

  it("says each sentence of a row once, with those its first cell says itself", async () => {
    const { compact } = await grids();
    // The row's two reasons, the first being also the first cell's own: two, not three.
    const first = cell(compact, "103", "vlan");
    expect(first.getAttribute("aria-describedby")!.split(" ")).toHaveLength(2);
    expect(description(first)).toBe(`${HELD} | ${OTHER}`);
  });

  it("Escape in an editor of the large grid cancels it, the dialog kept open", async () => {
    const { full } = await grids();
    const target = cell(full, "100", "name") as HTMLElement;
    target.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    const input = await vi.waitFor(() => {
      const found = target.querySelector("input");
      expect(found).not.toBeNull();
      return found!;
    });
    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    );
    await vi.waitFor(() => expect(target.querySelector("input")).toBeNull());
    await nextTick();
    expect(expanded.value).toBe(true);
    expect(document.querySelector("[role='dialog']")).not.toBeNull();
    expect(commits).toEqual([]);
  });

  it("is editable as a DataGrid: an editor of the large grid commits to the host", async () => {
    const { full } = await grids();
    const target = cell(full, "100", "name") as HTMLElement;
    target.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    const input = await vi.waitFor(() => {
      const found = target.querySelector("input");
      expect(found).not.toBeNull();
      return found!;
    });
    input.value = "serveurs du nord";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await vi.waitFor(() =>
      expect(commits).toEqual([{ row: "100", column: "name", value: "serveurs du nord" }]),
    );
  });
});
