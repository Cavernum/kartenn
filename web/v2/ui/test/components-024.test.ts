// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createCommentVNode, createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";
import { describe, expect, it } from "vitest";
import Button from "../src/components/Button/Button.vue";
import CommitForm from "../src/components/CommitForm/CommitForm.vue";
import DataGrid from "../src/components/DataGrid/DataGrid.vue";
import { nextSort, type GridColumn } from "../src/components/DataGrid/grid.ts";
import DraftCounter from "../src/components/DraftCounter/DraftCounter.vue";
import IconButton from "../src/components/IconButton/IconButton.vue";
import { moveInGrid } from "../src/components/internal/gridKeys.ts";
import MatrixGrid from "../src/components/MatrixGrid/MatrixGrid.vue";
import SidePanel from "../src/components/SidePanel/SidePanel.vue";
import Skeleton from "../src/components/Skeleton/Skeleton.vue";
import SplitButton from "../src/components/SplitButton/SplitButton.vue";
import Toolbar from "../src/components/Toolbar/Toolbar.vue";
import { foldActions, type ToolbarAction } from "../src/components/Toolbar/toolbar.ts";
import TreeGrid from "../src/components/TreeGrid/TreeGrid.vue";
import { icons } from "../src/icons/index.ts";

// The components of the 024 (spec 020, inventory 30 to 34; REQ-020-23, 34 to 37;
// 024/M8, M9, M12, M14, M15): roles, names and ARIA states as rendered on the server, and
// the pure rules behind folding, sorting and moving in a grid. Their keyboard is played
// in the browser (e2e/visual/clavier.spec.ts).

/** The markup, then what went to the layer of floating layers (tooltips, by Teleport). */
async function render(component: Component, props: Record<string, unknown> = {}) {
  const context: { teleports?: Record<string, string> } = {};
  const html = await renderToString(createSSRApp({ render: () => h(component, props) }), context);
  return html + Object.values(context.teleports ?? {}).join("");
}
const text = (html: string) => html.replace(/<[^>]+>/g, "").replace(/\s+/g, " ");

describe("refused actions (024/M14)", () => {
  it("Button: focusable, aria-disabled, described by « Il faut <permission> »", async () => {
    const html = await render(Button, { forbidden: "prefix.edit" });
    expect(html).not.toMatch(/\sdisabled/);
    expect(html).toContain('aria-disabled="true"');
    const id = /aria-describedby="([^"]+)"/.exec(html)![1]!;
    expect(html).toMatch(new RegExp(`id="${id}"[^>]*role="tooltip"[^>]*hidden`));
    expect(text(html)).toContain("Il faut prefix.edit");
  });

  it("IconButton: the reason of a policy as its tooltip", async () => {
    const html = await render(IconButton, {
      icon: "trash",
      label: "Supprimer",
      reason: "Retenu par une politique.",
    });
    expect(html).toContain('aria-disabled="true"');
    expect(text(html)).toContain("Retenu par une politique.");
  });

  it("an allowed button has no tooltip", async () => {
    const html = await render(Button, {});
    expect(html).not.toContain("tooltip");
    expect(html).not.toContain("aria-describedby");
  });
});

describe("SidePanel", () => {
  it("is a named complementary region with tabs and a window splitter of 420 px", async () => {
    const html = await render(SidePanel, {
      label: "Panneau droit",
      id: "panel",
      tabs: [
        { value: "detail", label: "Détail" },
        { value: "draft", label: "Brouillon", count: 3 },
      ],
      tab: "draft",
    });
    expect(html).toMatch(/<aside id="panel" class="k-side-panel" aria-label="Panneau droit"/);
    expect(html).toContain("--k-side-panel-width:420px");
    expect(html).toMatch(
      /role="separator" tabindex="0" aria-orientation="vertical" aria-label="Redimensionner le panneau" aria-controls="panel" aria-valuenow="420" aria-valuemin="320" aria-valuemax="720"/,
    );
    expect(html).toContain('role="tablist"');
    expect(html).toContain('aria-label="Fermer le panneau"');
  });

  it("closed, leaves nothing", async () => {
    const html = await render(SidePanel, { label: "Panneau", tabs: [], open: false });
    expect(html.replace(/<!--[^>]*-->/g, "")).toBe("");
  });

  it("leaves out its footer when the host fills it with nothing for the tab in front", async () => {
    const tabs = [
      { value: "detail", label: "Détail" },
      { value: "draft", label: "Brouillon" },
    ];
    const footer = ({ tab }: { tab: string }) =>
      tab === "draft" ? [h("p", "Soumettre")] : [createCommentVNode("v-if")];
    const shown = (tab: string) =>
      renderToString(
        createSSRApp({ render: () => h(SidePanel, { label: "Panneau", tabs, tab }, { footer }) }),
      );
    expect(await shown("draft")).toContain('class="k-side-panel__footer"');
    expect(await shown("detail")).not.toContain("k-side-panel__footer");
  });

  it("closed with a rail, leaves a strip whose chevron reopens it", async () => {
    const html = await render(SidePanel, { label: "Panneau", tabs: [], open: false, rail: true });
    expect(html).not.toContain("<aside");
    expect(html).toContain('class="k-side-panel-rail"');
    expect(html).toMatch(
      /aria-label="Ouvrir le panneau"[^>]*aria-expanded="false"|aria-expanded="false"[^>]*aria-label="Ouvrir le panneau"/,
    );
    const open = await render(SidePanel, { label: "Panneau", tabs: [], rail: true });
    expect(open).toContain("<aside");
    expect(open).toContain('class="k-side-panel-rail"');
    expect(open).not.toContain("Ouvrir le panneau");
  });

  it("covering a narrow window, has no handle", async () => {
    const html = await render(SidePanel, { label: "Panneau", tabs: [], cover: true });
    expect(html).toContain('data-cover="true"');
    expect(html).not.toContain('role="separator"');
  });
});

describe("TreeGrid (024/M10)", () => {
  const nodes = [
    { id: "d", label: "Domaine" },
    { id: "p", label: "Salle A", objectKey: "192.0.2.0/24" },
  ];

  it("names the « + » of the nodes the host chooses, and only them", async () => {
    const html = await render(TreeGrid, {
      label: "Arbre",
      treeLabel: "Préfixe",
      nodes,
      addLabel: (node: { objectKey?: string }) =>
        node.objectKey ? `Ajouter dans ${node.objectKey}` : undefined,
    });
    expect(html.match(/aria-label="Ajouter dans [^"]+"/g)).toEqual([
      'aria-label="Ajouter dans 192.0.2.0/24"',
    ]);
    expect(html).toMatch(
      /tabindex="-1"[^>]*aria-label="Ajouter dans|aria-label="Ajouter dans[^>]*tabindex="-1"/,
    );
  });

  it("without addLabel, no button", async () => {
    const html = await render(TreeGrid, { label: "Arbre", treeLabel: "Préfixe", nodes });
    expect(html).not.toContain("k-tree-grid__add");
  });
});

describe("CommitForm and SplitButton (REQ-020-37)", () => {
  it("CommitForm: a labelled summary and a labelled optional description", async () => {
    const html = await render(CommitForm, { required: true, summary: "Maintenance" });
    expect(text(html)).toContain("Résumé");
    expect(text(html)).toContain("Description (facultative)");
    expect(html).toMatch(
      /<input[^>]*required[^>]*value="Maintenance"|value="Maintenance"[^>]*required/,
    );
    expect(html).toContain("<textarea");
  });

  it("CommitForm: the error of the host is said under the summary", async () => {
    const html = await render(CommitForm, { error: "Le résumé est obligatoire." });
    expect(html).toContain('aria-invalid="true"');
    expect(text(html)).toContain("Le résumé est obligatoire.");
  });

  it("SplitButton: the chosen action, a chevron that opens a menu", async () => {
    const actions = [
      { id: "apply", label: "Appliquer", forbidden: "changeset.apply_own" },
      { id: "review", label: "Soumettre en revue" },
    ];
    const chosen = await render(SplitButton, { actions, action: "review" });
    expect(text(chosen)).toContain("Soumettre en revue");
    expect(chosen).toContain('aria-haspopup="menu"');
    expect(chosen).toContain('aria-label="Choisir l&#39;action"');
    expect(chosen).not.toContain('aria-disabled="true"');
    // Chosen while forbidden: the button itself is refused, with the permission.
    const refused = await render(SplitButton, { actions, action: "apply" });
    expect(refused).toContain('aria-disabled="true"');
    expect(text(refused)).toContain("Il faut changeset.apply_own");
  });
});

describe("Toolbar (REQ-020-36)", () => {
  const actions: ToolbarAction[] = [
    { id: "create", label: "Créer", priority: 3 },
    { id: "edit", label: "Modifier", priority: 2 },
    { id: "import", label: "Importer" },
    { id: "export", label: "Exporter" },
    { id: "history", label: "Historique", priority: 1 },
  ];
  const widths = new Map(actions.map((a) => [a.id, 100]));

  it("folds nothing when all fit", () => {
    expect(foldActions(actions, widths, 540, 10, 30)).toEqual([]);
  });

  it("folds the lowest priority first, from the end, room left for « … »", () => {
    expect(foldActions(actions, widths, 530, 10, 30)).toEqual(["export"]);
    expect(foldActions(actions, widths, 300, 10, 30)).toEqual(["export", "import", "history"]);
    expect(foldActions(actions, widths, 30, 10, 30)).toHaveLength(5);
  });

  it("is a named toolbar, one tab stop once mounted", async () => {
    const html = await render(Toolbar, { label: "Actions", actions });
    expect(html).toMatch(/role="toolbar" aria-label="Actions"/);
    expect(html.match(/data-action=/g)).toHaveLength(5);
  });
});

describe("grids (REQ-020-34, 35)", () => {
  it("moves by the keys of the APG Grid, bounded", () => {
    const at = { row: 1, column: 1 };
    const key = (k: string, ctrlKey = false) => ({ key: k, ctrlKey, metaKey: false });
    expect(moveInGrid(at, key("ArrowRight"), 4, 3)).toEqual({ row: 1, column: 2 });
    expect(moveInGrid(at, key("ArrowRight"), 4, 3, true)).toEqual({ row: 1, column: 0 });
    expect(moveInGrid(at, key("ArrowUp"), 4, 3)).toEqual({ row: 0, column: 1 });
    expect(moveInGrid(at, key("Home"), 4, 3)).toEqual({ row: 1, column: 0 });
    expect(moveInGrid(at, key("End", true), 4, 3)).toEqual({ row: 3, column: 2 });
    expect(moveInGrid(at, key("PageDown"), 4, 3)).toEqual({ row: 3, column: 1 });
    expect(moveInGrid({ row: 0, column: 0 }, key("ArrowLeft"), 4, 3)).toEqual({
      row: 0,
      column: 0,
    });
    expect(moveInGrid(at, key("a"), 4, 3)).toBeNull();
  });

  it("sorts ascending first, then the other way", () => {
    expect(nextSort(undefined, "a")).toEqual({ column: "a", direction: "ascending" });
    expect(nextSort({ column: "a", direction: "ascending" }, "a").direction).toBe("descending");
    expect(nextSort({ column: "a", direction: "descending" }, "a").direction).toBe("ascending");
    expect(nextSort({ column: "a", direction: "ascending" }, "b").direction).toBe("ascending");
  });

  const columns: GridColumn[] = [
    { id: "prefix", label: "Préfixe", sortable: true, filterable: true },
    { id: "label", label: "Libellé", filterable: true },
  ];

  it("DataGrid: loading, busy with skeletons in the cells, said in words", async () => {
    const html = await render(DataGrid, {
      label: "Préfixes",
      columns,
      rows: [],
      rowKey: "prefix",
      loading: true,
      loadingRows: 3,
    });
    expect(html).toContain('aria-busy="true"');
    expect(html.match(/class="k-skeleton__bar"/g)).toHaveLength(6);
    expect(text(html)).toContain("Chargement");
  });

  it("DataGrid: a filtered column says so", async () => {
    const html = await render(DataGrid, {
      label: "Préfixes",
      columns,
      rows: [{ prefix: "192.0.2.0/24", label: "Serveurs" }],
      rowKey: "prefix",
      filtered: ["label"],
    });
    expect(text(html)).toContain("(filtrée)");
    expect(html.match(/k-data-grid__filter/g)).toHaveLength(2);
  });

  it("MatrixGrid: row and column headers of the same make, the corner the entry point", async () => {
    const axis = [
      { id: "a", label: "A" },
      { id: "b", label: "B" },
    ];
    const html = await render(MatrixGrid, {
      label: "Matrice",
      corner: "Source → destination",
      rows: axis,
      columns: axis,
      cellState: (row: { id: string }) => (row.id === "a" ? "draft" : undefined),
    });
    expect(html).toMatch(/role="grid" aria-label="Matrice" aria-rowcount="3" aria-colcount="3"/);
    expect(html.match(/role="columnheader"/g)).toHaveLength(3);
    expect(html.match(/role="rowheader"/g)).toHaveLength(2);
    expect(html.match(/role="gridcell"/g)).toHaveLength(4);
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
    expect(html).toMatch(/class="k-matrix__corner"[^>]*tabindex="0"/);
    expect(text(html)).toContain("(en brouillon)");
  });

  it("MatrixGrid: the active cell crosses its row and column, headers included", async () => {
    const axis = [
      { id: "a", label: "A" },
      { id: "b", label: "B" },
    ];
    const html = await render(MatrixGrid, {
      label: "Matrice",
      corner: "Source → destination",
      rows: axis,
      columns: axis,
      active: { row: "a", column: "b" },
      axes: { rows: "Source", columns: "Destination" },
    });
    // Row a (header and two cells), column b (header and the cell of row b).
    expect(html.match(/data-cross="true"/g)).toHaveLength(5);
    expect(html).toMatch(/role="rowheader"[^>]*data-row="a"[^>]*data-cross="true"/);
    expect(html).toMatch(/role="columnheader"[^>]*data-column="b"[^>]*data-cross="true"/);
    expect(html).not.toMatch(/role="rowheader"[^>]*data-row="b"[^>]*data-cross/);
    // The corner says the direction in words; its arrows, and those of the crosshair, hidden.
    expect(text(html)).toContain("Source → destination");
    expect(html).toContain('class="k-matrix__axes" aria-hidden="true"');
    expect(html.match(/k-matrix__flow/g)).toHaveLength(4);
  });

  it("Skeleton: one bar for a cell, hidden", async () => {
    expect(await render(Skeleton, { variant: "cell" })).toMatch(
      /^<span class="k-skeleton__bar"[^>]*aria-hidden="true"/,
    );
  });
});

describe("DraftCounter (024/M8)", () => {
  it("a toggle of the panel, and the list of the drafts behind a chevron", async () => {
    const html = await render(DraftCounter, {
      count: 3,
      controls: "panel",
      drafts: [{ id: "d1", label: "Salle B", count: 3, current: true }],
    });
    expect(html).toContain('aria-expanded="false" aria-controls="panel"');
    expect(html).toContain('aria-label="Liste des brouillons"');
    expect(html).toContain('aria-haspopup="menu"');
    expect(await render(DraftCounter, { count: 3 })).not.toContain("aria-haspopup");
  });

  it("open, its draft icon becomes a cross", async () => {
    const cross = String(icons.x[0]![1].d);
    expect(await render(DraftCounter, { count: 3, expanded: true })).toContain(cross);
    const closed = await render(DraftCounter, { count: 3 });
    expect(closed).toContain('aria-expanded="false"');
    expect(closed).not.toContain(cross);
  });
});
