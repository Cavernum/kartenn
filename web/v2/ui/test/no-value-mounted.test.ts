// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, type App } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { GridColumn } from "../src/components/DataGrid/grid.ts";
import ExpandableGrid from "../src/components/ExpandableGrid/ExpandableGrid.vue";

// An absent value in a grid that wraps `DataGrid` (task 020-30): the wrapper passes its own
// `cell` slot, whose default content says a `null` cell as `DataGrid` does, in both views.
// Mounted: the large view is a dialog, rendered only open, in the document.

const apps: App[] = [];
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

const columns: GridColumn[] = [
  { id: "vlan", label: "VLAN", mono: true },
  { id: "name", label: "Nom", width: "fill" },
];
const rows = [
  { vlan: "100", name: "serveurs" },
  { vlan: "101", name: null },
];

describe("ExpandableGrid: a cell null (task 020-30)", () => {
  it("is said absent (NoValue) in the compact grid and in the large one", async () => {
    const root = document.body.appendChild(document.createElement("div"));
    const app = createApp({
      render: () =>
        h(ExpandableGrid, { label: "VLAN", columns, rows, rowKey: "vlan", expanded: true }),
    });
    apps.push(app);
    app.mount(root);
    await nextTick();
    // Both grids: the compact one, the one of the dialog (at the end of the document).
    await vi.waitFor(() => expect(document.querySelectorAll("[role='grid']")).toHaveLength(2));
    for (const grid of document.querySelectorAll("[role='grid']")) {
      const cell = grid.querySelector("[data-row='101'][data-column='name']")!;
      expect(cell.querySelector(".k-no-value [aria-hidden='true']")?.textContent).toBe("—");
      expect(cell.querySelector(".k-no-value .k-visually-hidden")?.textContent).toBe(
        "non renseigné",
      );
      // A value given stays as it is.
      const given = grid.querySelector("[data-row='100'][data-column='name']")!;
      expect(given.querySelector(".k-no-value")).toBeNull();
      expect(given.textContent).toContain("serveurs");
    }
  });
});
