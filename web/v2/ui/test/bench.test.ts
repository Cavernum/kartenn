// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, type App } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import { benches } from "../src/catalogue.ts";
import { fullTarget, lockedTarget, STATE_MEASURE } from "../src/demo/benchNames.ts";

// The hosts of the component benches (spec 020, task 27; design « Budgets »), mounted: the
// controls and selectors the Playwright bench (e2e/bench/components.spec.ts) drives are
// there, so that a change breaking the bench shows in `k-web2-check`, not at its next run.

const apps: App[] = [];
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

function mount(name: string): HTMLElement {
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp({ render: () => h(benches[name]!) });
  app.config.errorHandler = (error) => {
    throw error;
  };
  apps.push(app);
  app.mount(root);
  return root;
}

const button = (root: HTMLElement, name: string) => {
  const found = [...root.querySelectorAll("button")].find((b) => b.textContent?.trim() === name);
  if (!found) throw new Error(`no button « ${name} »`);
  return found;
};
const settle = async () => {
  await nextTick();
  await nextTick();
};

describe("component bench hosts", () => {
  it("declares Tree, TreeGrid and DataGrid", () => {
    expect(Object.keys(benches).sort()).toEqual(["DataGrid", "Tree", "TreeGrid"]);
  });

  for (const [name, role, frame] of [
    ["Tree", "tree", ".k-tree-frame"],
    ["TreeGrid", "treegrid", ".k-tree-grid-frame"],
  ] as const) {
    it(`${name}: shown by its button, five domains opened, moveState changed and measured`, async () => {
      const root = mount(name);
      expect(root.querySelector(`[role='${role}']`)).toBeNull();
      button(root, "Afficher l'arbre").click();
      await settle();
      const tree = root.querySelector(`[role='${role}']`)!;
      expect(tree.getAttribute("aria-label")).toBe("Cent mille nœuds");
      expect(tree.querySelector("[data-node='v1']")).not.toBeNull();
      expect(root.querySelector(`${frame} > [role='status']`)).not.toBeNull();
      button(root, "Ouvrir cinq domaines").click();
      await settle();
      expect(tree.querySelector("[data-node='v1']")?.getAttribute("aria-expanded")).toBe("true");
      button(root, "Tout ouvrir").click();
      await settle();
      performance.clearMeasures(STATE_MEASURE);
      button(root, "Changer l'état d'un nœud").click();
      await settle();
      expect(performance.getEntriesByName(STATE_MEASURE)).toHaveLength(1);
    });
  }

  it("refuses targets of rank 7k by its check, of rank 3k by its answer, never a domain", () => {
    expect(["v1-105", "v1-104", "v2"].map(lockedTarget)).toEqual([true, false, false]);
    expect(["v1-102", "v1-104", "v2"].map(fullTarget)).toEqual([true, false, false]);
  });

  it("DataGrid: ten thousand rows, sortable prefix, the filter said", async () => {
    const root = mount("DataGrid");
    await settle();
    const grid = root.querySelector("[role='grid']")!;
    expect(grid.getAttribute("aria-label")).toBe("Dix mille préfixes");
    expect(grid.getAttribute("aria-rowcount")).toBe("10001");
    expect(grid.querySelector("[role='columnheader'][data-column='prefix']")).not.toBeNull();
    const field = root.querySelector<HTMLInputElement>("input[type='search']")!;
    expect(root.querySelector(`label[for='${field.id}']`)?.textContent).toContain(
      "Filtrer les libellés",
    );
    field.value = "Supervision";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    await settle();
    expect(grid.getAttribute("aria-rowcount")).toBe("2001");
    expect(root.querySelector("[role='status']")?.textContent?.replace(/\s/g, "")).toBe(
      "2000préfixes",
    );
  });
});
