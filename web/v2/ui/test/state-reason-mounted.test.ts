// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, type App } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import DataGrid from "../src/components/DataGrid/DataGrid.vue";
import type { CellState, GridColumn } from "../src/components/DataGrid/grid.ts";

// Why a cell is in its state (`stateReason`, task 021-12e): a write held in review says
// why, as the description of its cell, beside the reason of a cell without an editor;
// an error says nothing more than its message.

const apps: App[] = [];
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

const columns: GridColumn[] = [
  { id: "prefix", label: "Préfixe", mono: true },
  { id: "label", label: "Libellé", width: "fill" },
  { id: "env", label: "Environnement" },
];
const rows = [
  { prefix: "192.0.2.0/24", label: "serveurs", env: "production" },
  { prefix: "198.51.100.0/24", label: "postes", env: "recette" },
  { prefix: "203.0.113.0/24", label: "invités", env: "production" },
];
const HELD = "Une politique d'approbation retient la modification : elle attend une revue.";
const WHY = "L'environnement vient du site du préfixe.";
const states: Record<string, CellState> = {
  "192.0.2.0/24:label": "review",
  "198.51.100.0/24:label": { error: "Libellé refusé.", code: "core.label.invalid" },
  // Cells without an editor and with a state: two sentences, or the same one twice.
  "203.0.113.0/24:env": "review",
  "198.51.100.0/24:env": "draft",
};
/** The reason of a state: the held one, or for a draft the very sentence of why not. */
const reasonOfState = (key: string) => (states[key] === "draft" ? WHY : HELD);

function mount(): HTMLElement {
  const root = document.body.appendChild(document.createElement("div"));
  const app = createApp({
    render: () =>
      h(DataGrid, {
        label: "Préfixes",
        columns,
        rows,
        rowKey: "prefix",
        editable: (_: unknown, column: GridColumn) => column.id === "label",
        readonlyReason: (_: unknown, column: GridColumn) => (column.id === "env" ? WHY : undefined),
        cellState: (row: Record<string, unknown>, column: GridColumn) =>
          states[`${String(row.prefix)}:${column.id}`],
        // Asked of every cell: the grid keeps it to the cells with a state, not an error.
        stateReason: (row: Record<string, unknown>, column: GridColumn) =>
          reasonOfState(`${String(row.prefix)}:${column.id}`),
      }),
  });
  apps.push(app);
  app.mount(root);
  return root;
}

const cell = (root: HTMLElement, row: string, column: string) =>
  root.querySelector<HTMLElement>(`[data-row="${row}"][data-column="${column}"]`)!;
const description = (el: HTMLElement) =>
  (el.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .filter(Boolean)
    .map((id) => document.getElementById(id)?.textContent)
    .join(" | ");

describe("DataGrid: why a cell is in its state (task 021-12e)", () => {
  it("describes a cell in review by its reason, an error by its message only", async () => {
    const root = mount();
    await nextTick();
    expect(description(cell(root, "192.0.2.0/24", "label"))).toBe(HELD);
    // A cell without a state has no reason for one; one without an editor keeps its own.
    expect(cell(root, "192.0.2.0/24", "prefix").getAttribute("aria-describedby")).toBeNull();
    expect(description(cell(root, "192.0.2.0/24", "env"))).toBe(WHY);
    // An error is described by its message, never by a state's reason.
    expect(description(cell(root, "198.51.100.0/24", "label"))).toContain("Libellé refusé.");
    expect(description(cell(root, "198.51.100.0/24", "label"))).not.toContain(HELD);
  });

  it("says both sentences of a cell, its state's reason first, and the same one once", async () => {
    const root = mount();
    await nextTick();
    const both = cell(root, "203.0.113.0/24", "env");
    expect(both.getAttribute("aria-describedby")!.split(" ")).toHaveLength(2);
    expect(description(both)).toBe(`${HELD} | ${WHY}`);
    // The same text from both: one id, one sentence.
    const same = cell(root, "198.51.100.0/24", "env");
    expect(same.getAttribute("aria-describedby")!.split(" ")).toHaveLength(1);
    expect(description(same)).toBe(WHY);
  });
});
