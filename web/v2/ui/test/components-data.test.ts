// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";
import { describe, expect, it } from "vitest";
import DataGrid from "../src/components/DataGrid/DataGrid.vue";
import { placeCellError } from "../src/components/DataGrid/cellError.ts";
import { bodyParts, keptIndexes } from "../src/components/internal/virtualRows.ts";
import { useCellEditor, useCommitWatch } from "../src/components/internal/cellEditor.ts";
import { nextTick, ref } from "vue";
import type { CellRef, CellState, GridCommit } from "../src/components/DataGrid/grid.ts";
import {
  columnsTemplate,
  columnTemplates,
  nextPresetWidth,
  selectRange,
  type GridColumn,
} from "../src/components/DataGrid/grid.ts";
import BarChart from "../src/components/BarChart/BarChart.vue";
import { layoutBars, niceCeil } from "../src/components/BarChart/chart.ts";
import BarList from "../src/components/BarList/BarList.vue";
import DiffList from "../src/components/DiffList/DiffList.vue";
import DiffValue from "../src/components/DiffValue/DiffValue.vue";
import ExpandableGrid from "../src/components/ExpandableGrid/ExpandableGrid.vue";
import { ApiProblem } from "../../sdk/src/api/problem.ts";
import ErrorState from "../src/components/ErrorState/ErrorState.vue";
import type { ProblemFacts } from "../src/components/ErrorState/problem.ts";
import FilterChip from "../src/components/FilterChip/FilterChip.vue";
import InlineAlert from "../src/components/InlineAlert/InlineAlert.vue";
import Skeleton from "../src/components/Skeleton/Skeleton.vue";
import TreeGrid from "../src/components/TreeGrid/TreeGrid.vue";
import {
  flatten,
  misplaced,
  type TreeNode,
  type TreeRow,
} from "../src/components/TreeGrid/tree.ts";

// The visual version of the data views (spec 020, task 12; REQ-020-16, 19, 20, 22, 23, 32):
// grid and treegrid structure, states said in words, codes shown, the display invariant.

const render = (component: Component, props: Record<string, unknown> = {}) =>
  renderToString(createSSRApp({ render: () => h(component, props) }));
const text = (html: string) => html.replace(/<[^>]+>/g, "").replace(/\s+/g, " ");

const columns: GridColumn[] = [
  { id: "prefix", label: "Préfixe", width: "md", mono: true, sortable: true },
  { id: "label", label: "Libellé", width: "fill" },
];
const rows = [
  { prefix: "192.0.2.0/24", label: "Serveurs" },
  { prefix: "198.51.100.0/24", label: "Postes" },
];

describe("DataGrid", () => {
  it("shares one template of tokens between head and rows", () => {
    expect(columnsTemplate(columns)).toBe(
      "minmax(auto, var(--k-size-col-md)) minmax(var(--k-size-col-md), 1fr)",
    );
  });

  it("is a named grid, rows and columns counted and indexed, sort said", async () => {
    const html = await render(DataGrid, {
      label: "Préfixes",
      columns,
      rows,
      rowKey: "prefix",
      sort: { column: "prefix", direction: "descending" },
      selectable: true,
      selected: ["198.51.100.0/24"],
    });
    // A column of checkboxes first (several selectable): counted, indexed 1.
    expect(html).toMatch(/role="grid" aria-label="Préfixes" aria-rowcount="3" aria-colcount="3"/);
    expect(html).toContain('aria-multiselectable="true"');
    expect(html).toContain('style="--k-grid-columns:calc(');
    expect(html).toMatch(/role="columnheader"[^>]*aria-sort="descending"/);
    expect(html).toMatch(/data-column="prefix"[^>]*aria-colindex="2"/);
    expect(text(html)).toContain("Sélectionner toutes les lignes");
    expect(text(html)).toContain("Sélectionner 198.51.100.0/24");
    // The unsortable column has no aria-sort.
    expect(html.match(/aria-sort=/g)).toHaveLength(1);
    expect(html).toContain('aria-rowindex="3" aria-selected="true"');
    // One cell only in the tab order: the roving tabindex.
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
    // Nothing editable: read only.
    expect(html).toContain('aria-readonly="true"');
  });

  it("templates: a width the user gave, the track of the checkboxes", () => {
    expect(columnsTemplate(columns, { label: 300.4 })).toBe(
      "minmax(auto, var(--k-size-col-md)) minmax(auto, 300px)",
    );
    const templates = columnTemplates(columns, {}, {}, true);
    expect(templates["--k-grid-columns"]).toMatch(/^calc\(var\(--k-control-height-sm\)/);
  });

  it("selects a range from its anchor, the earlier choice kept", () => {
    const keys = ["a", "b", "c", "d", "e"];
    expect(selectRange(keys, ["e"], "b", "d")).toEqual(["e", "b", "c", "d"]);
    expect(selectRange(keys, [], "d", "b")).toEqual(["b", "c", "d"]);
    expect(selectRange(keys, ["a"], "z", "b")).toEqual(["a"]);
  });

  it("steps a click on a handle through the token widths, then back to the narrowest", () => {
    expect(nextPresetWidth(100, [56, 96, 160, 240])).toBe(160);
    expect(nextPresetWidth(240, [56, 96, 160, 240])).toBe(56);
    expect(nextPresetWidth(159.5, [56, 96, 160, 240])).toBe(240);
  });

  it("lays the body out: rendered rows, and gaps of the heights left out", () => {
    const start = (i: number) => 30 + i * 28;
    const end = (i: number) => start(i) + 28;
    // The window 10 to 12, and the row of the focus far below.
    expect(bodyParts([10, 11, 12, 500, 11], 1000, start, end, 30)).toEqual([
      { gap: 280, key: "gap-10" },
      { index: 10 },
      { index: 11 },
      { index: 12 },
      { gap: (500 - 13) * 28, key: "gap-500" },
      { index: 500 },
      { gap: 499 * 28, key: "gap-end" },
    ]);
    expect(bodyParts([], 0, start, end, 30)).toEqual([]);
    // The rows of the focus and of the editor join the window, wherever they are.
    const at = new Map([
      ["a", 3],
      ["z", 900],
    ]);
    expect(keptIndexes([10, 11], ["z", undefined, "a", "?"], (k) => at.get(k))).toEqual([
      10, 11, 900, 3,
    ]);
  });

  // The contract of in-place editing (design « Contrat d'édition »), without a DOM.
  function editorOf(text = "Serveurs", check?: (value: string) => string | undefined) {
    const editing = ref<CellRef | undefined>(undefined);
    const commits: GridCommit[] = [];
    const refusals: string[] = [];
    const moves: number[] = [];
    const editor = useCellEditor({
      editing,
      textOf: () => text,
      driven: () => true,
      check: (_, value) => check?.(value),
      cellElement: () => null,
      moveFocus: (_, by) => moves.push(by),
      commit: (commit) => commits.push(commit),
      refuse: (_, error) => refusals.push(error),
    });
    return { editing, editor, commits, refusals, moves };
  }
  const cell: CellRef = { row: "192.0.2.0/24", column: "label" };
  // Events as the editor reads them (the tests run without a DOM).
  const key = (k: string, shiftKey = false) =>
    ({ key: k, shiftKey, defaultPrevented: false, preventDefault() {} }) as KeyboardEvent;

  it("editing: an unchanged text never commits; Tab moves on, Escape cancels", async () => {
    const { editing, editor, commits, moves } = editorOf();
    editing.value = cell;
    await nextTick();
    expect(editor.draft.value).toBe("Serveurs");
    editor.onKeydown(key("Tab"));
    expect(commits).toEqual([]);
    expect(editing.value).toBeUndefined();
    expect(moves).toEqual([1]);
    editing.value = cell;
    await nextTick();
    editor.draft.value = "Serveurs du nord";
    editor.onKeydown(key("Escape"));
    expect(commits).toEqual([]);
    await editor.start(cell);
    expect(editor.draft.value).toBe("Serveurs");
    editor.draft.value = "Serveurs du nord";
    editor.onKeydown(key("Enter"));
    expect(commits).toEqual([{ ...cell, value: "Serveurs du nord" }]);
    expect(moves).toEqual([1, 0, 0]);
  });

  it("editing: a refused text stays, said; the focus leaving the window keeps the editor", async () => {
    const { editing, editor, commits, refusals } = editorOf("Serveurs", (v) =>
      v.trim() ? undefined : "Le libellé est obligatoire.",
    );
    editing.value = cell;
    await nextTick();
    editor.draft.value = "";
    editor.onKeydown(key("Tab"));
    expect(editing.value).toEqual(cell);
    expect(editor.error.value).toBe("Le libellé est obligatoire.");
    expect(refusals).toEqual(["Le libellé est obligatoire."]);
    editor.draft.value = "Bases";
    await nextTick();
    expect(editor.error.value).toBeUndefined();
    // Nowhere to go (another window): still open, nothing committed.
    const input = { closest: (selector: string) => selector === ".k-data-grid__editor" };
    const away = (to: object | null) =>
      editor.onFocusout({ relatedTarget: to, target: input } as unknown as FocusEvent);
    away(null);
    expect(editing.value).toEqual(cell);
    expect(commits).toEqual([]);
    // Elsewhere in the page: the changed text commits.
    away({});
    expect(commits).toEqual([{ ...cell, value: "Bases" }]);
    expect(editing.value).toBeUndefined();
  });

  it("follows commits until the host settles them, bounded", async () => {
    const states = ref<Record<string, CellState | undefined>>({});
    const texts = ref<Record<string, string>>({});
    const said: string[] = [];
    const watch = useCommitWatch(
      (c) => states.value[c.row],
      (c) => texts.value[c.row],
      (c, error) => said.push(`${c.row}: ${error.error}`),
    );
    watch.follow({ row: "a", column: "x", value: "1" });
    watch.follow({ row: "b", column: "x", value: "2" });
    states.value = { a: "pending" };
    await nextTick();
    expect(watch.awaiting.value).toHaveLength(2);
    // Applied without a state (the text is the committed one), refused with an error.
    texts.value = { b: "2" };
    states.value = { a: { error: "Refusé." } };
    await nextTick();
    expect(watch.awaiting.value).toHaveLength(0);
    expect(said).toEqual(["a: Refusé."]);
    for (let i = 0; i < 100; i++) watch.follow({ row: `r${i}`, column: "x", value: "v" });
    expect(watch.awaiting.value.length).toBeLessThanOrEqual(64);
  });

  // REQ-020-19: a grid of 10⁴ rows holds a few hundred at most in the document.
  // The number of rows in the document under a real scroll is the e2e script's
  // (data-grid.spec.ts, 10⁴ rows); here, the counts and the row kept for the focus.
  it("counts 10 000 rows, keeps the row of the focus at its index, offsets the indexes", async () => {
    const many = Array.from({ length: 10_000 }, (_, i) => ({
      prefix: `p-${i}`,
      label: `Ligne ${i}`,
    }));
    const html = await render(DataGrid, {
      label: "Préfixes",
      columns,
      rows: many,
      rowKey: "prefix",
      active: { row: "p-9999", column: "prefix" },
      rowOffset: 500,
      rowCount: 20_000,
    });
    expect(html).toContain('data-virtual="true"');
    expect(html).toContain('aria-rowcount="20001"');
    // The row of the focus stays in the document, at its index after the dropped pages.
    expect(html).toContain('aria-rowindex="10501"');
    expect(html).toContain("k-data-grid__gap");
    // A paged list: the whole count, or -1 while unknown.
    const paged = await render(DataGrid, {
      label: "Préfixes",
      columns,
      rows,
      rowKey: "prefix",
      rowCount: 1284,
    });
    expect(paged).toContain('aria-rowcount="1285"');
    const unknown = await render(DataGrid, {
      label: "Préfixes",
      columns,
      rows,
      rowKey: "prefix",
      rowCount: -1,
    });
    expect(unknown).toContain('aria-rowcount="-1"');
  });

  it("holds the element of an extension column per row, and says editable cells", async () => {
    const html = await render(DataGrid, {
      label: "Préfixes",
      columns: [...columns, { id: "ext", label: "Extension", element: "k-demo-cell" }],
      rows,
      rowKey: "prefix",
      editable: (_: unknown, column: GridColumn) => column.id === "label",
    });
    expect(html.match(/<k-demo-cell/g)).toHaveLength(rows.length);
    expect(html).not.toMatch(/role="grid"[^>]*aria-readonly/);
    expect(html).toMatch(/data-column="prefix"[^>]*aria-readonly="true"/);
    expect(html).not.toMatch(/data-column="label"[^>]*aria-readonly/);
    // An extension cell is never editable.
    expect(html).toMatch(/data-column="ext"[^>]*aria-readonly="true"/);
  });

  it("selects one row only when `single`: no aria-multiselectable", async () => {
    const html = await render(DataGrid, {
      label: "Préfixes",
      columns,
      rows,
      rowKey: "prefix",
      selectable: "single",
      selected: ["198.51.100.0/24"],
    });
    expect(html).not.toContain("aria-multiselectable");
    expect(html).toContain('aria-selected="true"');
  });

  it("renders the states the host decides, in words and with the code of an error", async () => {
    const context: { teleports?: Record<string, string> } = {};
    const grid = await renderToString(
      createSSRApp({
        render: () =>
          h(DataGrid as Component, {
            label: "Préfixes",
            columns,
            rows,
            rowKey: "prefix",
            cellState: (row: (typeof rows)[number], column: GridColumn) =>
              column.id !== "label"
                ? undefined
                : row.prefix === "192.0.2.0/24"
                  ? "draft"
                  : { error: "Libellé trop long.", code: "core.label.too-long" },
          }),
      }),
      context,
    );
    const layers = Object.values(context.teleports ?? {}).join("");
    expect(grid).toContain('data-state="draft"');
    expect(text(grid)).toContain("(en brouillon)");
    expect(grid).toContain('aria-invalid="true"');
    // The message is not in the cell: a hidden layer, named by the cell.
    expect(text(grid)).not.toContain("Libellé trop long.");
    const id = /aria-describedby="([^"]+)"/.exec(grid)?.[1];
    expect(id).toBeTruthy();
    expect(layers).toMatch(/class="k-cell-error"[^>]*hidden/);
    expect(layers).toMatch(new RegExp(`id="${id}"[^>]*>.*Libellé trop long\\.`, "s"));
    expect(text(layers)).toContain("core.label.too-long");
  });

  it("places the error of a cell under it, joined by one outline, flipped near the edges", () => {
    const base = {
      cell: { x: 100, y: 100, width: 120, height: 32 },
      message: { width: 200, height: 40 },
      viewport: { width: 1000, height: 800 },
      radius: 5,
      stroke: 1,
      margin: 8,
      rtl: false,
    };
    const below = placeCellError(base);
    expect(below).toMatchObject({ x: 100, y: 100, width: 200, height: 72 });
    expect(below).toMatchObject({ side: "block-end", align: "start", messageTop: 32 });
    // Flush at the start, a concave fillet at the end of the junction (sweep 0).
    expect(below.outline).toMatch(
      /^M0\.5 32 L0\.5 0\.5 L119\.5 0\.5 L119\.5 27 A5 5 0 0 0 124\.5 32/,
    );
    // The fill never enters the cell: it starts on the junction.
    expect(below.fill).toMatch(/^M0\.5 32 L119\.5 32 /);
    // At least as wide as the cell.
    expect(placeCellError({ ...base, message: { width: 40, height: 40 } }).width).toBe(120);
    // Near the bottom: above the cell; near the inline end: aligned on the cell's end.
    const edge = placeCellError({ ...base, cell: { x: 860, y: 740, width: 120, height: 32 } });
    expect(edge).toMatchObject({ side: "block-start", align: "end", x: 780, y: 700 });
    expect(edge.messageTop).toBe(0);
    // Turned over: the fillet's sweep turns too, and the cell is at the bottom of the layer.
    expect(edge.outline).toContain("L75.5 40 A5 5 0 0 1 80.5 45");
    // Right to left: aligned on the cell's inline start, its right edge.
    expect(placeCellError({ ...base, rtl: true })).toMatchObject({ x: 20, align: "start" });
  });

  // The error of a cell never resizes it (024, cell errors in a bubble): the editor of an
  // invalid cell is described by the layer of the message, which stays out of the cell.
  it("gives the error of a cell being edited to its layer, the editor described by it", async () => {
    const context: { teleports?: Record<string, string> } = {};
    const html = await renderToString(
      createSSRApp({
        render: () =>
          h(DataGrid as Component, {
            label: "Préfixes",
            columns,
            rows,
            rowKey: "prefix",
            editing: { row: "198.51.100.0/24", column: "label" },
            cellState: (row: (typeof rows)[number], column: GridColumn) =>
              column.id === "label" && row.prefix === "198.51.100.0/24"
                ? { error: "Libellé trop long.", code: "core.label.too-long" }
                : undefined,
          }),
      }),
      context,
    );
    expect(text(html)).not.toContain("Libellé trop long.");
    const id = /<input[^>]*aria-describedby="([^"]+)"/.exec(html)?.[1];
    expect(id).toBeTruthy();
    expect(html).toMatch(/<input[^>]*aria-invalid="true"/);
    const layers = Object.values(context.teleports ?? {}).join("");
    expect(layers).toMatch(new RegExp(`id="${id}"[^>]*>.*Libellé trop long\\.`, "s"));
  });

  it("puts an editor in the cell being edited, named after its column and row", async () => {
    const html = await render(DataGrid, {
      label: "Préfixes",
      columns,
      rows,
      rowKey: "prefix",
      editing: { row: "198.51.100.0/24", column: "label" },
    });
    expect(html).toContain('data-editing="true"');
    expect(html).toMatch(/<input[^>]*value="Postes"/);
    expect(html).toContain('aria-label="Libellé — 198.51.100.0/24"');
  });

  it("lets the cells of a message column wrap, the others stay on one line", async () => {
    const html = await render(DataGrid, {
      label: "Préfixes",
      columns: [columns[0], { ...columns[1], wrap: true }],
      rows,
      rowKey: "prefix",
    });
    expect(html.match(/data-wrap="true"/g)).toHaveLength(rows.length);
    expect(html).toMatch(/data-column="label"[^>]*data-wrap="true"/);
    expect(html).not.toMatch(/data-column="prefix"[^>]*data-wrap/);
  });

  // Design « Adaptation à la largeur »: a simple list stacks its rows under a container
  // `sm`; still a grid of rows and cells, each attribute after the label of its column.
  it("stacks a simple list: labels on the attribute cells, not on the key nor the actions", async () => {
    const stacked = await render(DataGrid, {
      label: "Préfixes",
      columns: [
        ...columns,
        { id: "used", label: "Occupation", priority: 2 },
        { id: "menu", label: "Actions", actions: true },
      ],
      rows,
      rowKey: "prefix",
      stack: true,
    });
    expect(stacked).toMatch(/<div class="k-data-grid-frame"[^>]*data-stack="true"/);
    expect(stacked).toMatch(/role="grid"/);
    expect(stacked.match(/role="row"/g)).toHaveLength(rows.length + 1);
    expect(stacked.match(/data-label="Libellé"/g)).toHaveLength(rows.length);
    expect(stacked.match(/data-label="Occupation"/g)).toHaveLength(rows.length);
    expect(stacked).not.toMatch(/data-column="prefix"[^>]*data-label/);
    expect(stacked).not.toMatch(/data-label="Actions"/);
    expect(stacked).toMatch(/data-column="menu"[^>]*data-actions="true"/);
    const plain = await render(DataGrid, { label: "Préfixes", columns, rows, rowKey: "prefix" });
    expect(plain).not.toContain("data-stack");
    expect(plain).not.toContain("data-label");
  });
});

describe("ExpandableGrid (revue du 2026-10-25)", () => {
  const flows = Array.from({ length: 25 }, (_, i) => ({
    key: `f${i}`,
    source: `rece-${i}`,
    destination: "prod-api",
    flowType: "Applicatif",
    rule: "flow.verdict.denied",
  }));
  const flowColumns: GridColumn[] = [
    { id: "source", label: "Source", mono: true },
    { id: "destination", label: "Destination", width: "fill", mono: true },
    { id: "flowType", label: "Type de flux", width: "sm" },
    { id: "rule", label: "Règle", width: "lg" },
  ];
  const base = { label: "Flux non conformes", columns: flowColumns, rows: flows, rowKey: "key" };

  it("shows the row count, a few rows high, and a named button that opens a dialog", async () => {
    const html = await render(ExpandableGrid, { ...base, visibleRows: 4 });
    // More rows than it shows: « 4 sur 25 lignes », never a table that looks finished.
    expect(text(html)).toContain("4 sur 25 lignes");
    expect(html).toContain("--k-expandable-grid-rows:4");
    expect(html).toMatch(/<button[^>]*aria-haspopup="dialog"[^>]*aria-expanded="false"/);
    expect(html).toContain('aria-label="Agrandir : Flux non conformes"');
    // Closed, one grid only; all the rows are in it (it scrolls, nothing is cut).
    expect(html.match(/role="grid"/g)).toHaveLength(1);
    expect(html.match(/role="row"/g)).toHaveLength(26);
  });

  it("keeps the compact columns in the compact grid only", async () => {
    const html = await render(ExpandableGrid, {
      ...base,
      compactColumns: ["source", "destination", "flowType"],
    });
    expect(html).toMatch(
      /role="grid" aria-label="Flux non conformes" aria-rowcount="26" aria-colcount="3"/,
    );
    expect(html).toContain("Type de flux");
    expect(html).not.toContain("Règle");
  });

  it("says one row in the singular", async () => {
    expect(text(await render(ExpandableGrid, { ...base, rows: flows.slice(0, 1) }))).toMatch(
      /1 ligne(?!s)/,
    );
  });
});

const tree: TreeNode[] = [
  {
    id: "a",
    label: "A",
    children: [
      { id: "a1", label: "A1", children: [{ id: "a11", label: "A11" }] },
      { id: "a2", label: "A2", hasChildren: true },
    ],
  },
  { id: "b", label: "B" },
];

describe("TreeGrid", () => {
  it("flattens the expanded nodes, depth first, with level, size and position", () => {
    const flat = flatten(tree, new Set(["a", "a1"]));
    expect(
      flat.map((row) => [row.node.id, row.level, row.setSize, row.posInSet, row.parent]),
    ).toEqual([
      ["a", 1, 2, 1, null],
      ["a1", 2, 2, 1, "a"],
      ["a11", 3, 1, 1, "a1"],
      ["a2", 2, 2, 2, "a"],
      ["b", 1, 2, 2, null],
    ]);
    expect(misplaced(flat)).toEqual([]);
  });

  it("the guard finds a row shown under a parent not its own (REQ-020-32)", () => {
    const flat = flatten(tree, new Set(["a", "a1"]));
    // A move drawn by the view, not by the model: a11 shown right under b.
    const moved: TreeRow[] = [
      ...flat.filter((row) => row.node.id !== "a11"),
      { ...flat[2]!, level: 2 },
    ];
    expect(misplaced(moved)).toEqual(["a11"]);
  });

  it("renders the treegrid pattern and the states of moves", async () => {
    const html = await render(TreeGrid, {
      label: "Arbre",
      treeLabel: "Préfixe",
      nodes: tree,
      expanded: ["a"],
      selected: "a1",
      nodeState: (node: TreeNode) =>
        node.id === "a2"
          ? "move-pending"
          : node.id === "b"
            ? { refused: { reason: "Parent déduit.", code: "tree.derived-parent" } }
            : undefined,
    });
    expect(html).toContain('role="treegrid"');
    expect(html).toMatch(/aria-level="1" aria-setsize="2" aria-posinset="1" aria-expanded="true"/);
    expect(html).toMatch(/aria-level="2"[^>]*aria-expanded="false" aria-selected="true"/);
    expect(html).toMatch(/aria-busy="true" data-state="move-pending"/);
    expect(text(html)).toContain("(déplacement en attente)");
    expect(text(html)).toContain("Parent déduit.");
    expect(text(html)).toContain("tree.derived-parent");
    expect(html).toContain('style="--k-tree-depth:1;"');
  });
});

describe("filters, differential, view states", () => {
  it("FilterChip: one button for the sentence, one to remove it, named", async () => {
    const html = await render(FilterChip, {
      property: "Préfixe",
      operator: "dans",
      value: "192.0.2.0/33",
      state: "invalid",
      reason: "Longueur hors de 0 à 32.",
    });
    const reason = /aria-describedby="([^"]+)"/.exec(html)![1]!;
    expect(html).toMatch(new RegExp(`id="${reason}"[^>]*>Longueur hors de 0 à 32\\.<`));
    expect(html).toContain('aria-label="Retirer le filtre Préfixe dans 192.0.2.0/33"');
    expect(text(html)).toContain("Longueur hors de 0 à 32.");
  });

  it("DiffList: signs, words, before and after, disclosures", async () => {
    const html = await render(DiffList, {
      groups: [
        {
          id: "g",
          title: "Serveurs",
          kind: "changed",
          changes: [{ attribute: "Libellé", kind: "changed", before: "A", after: "B" }],
        },
      ],
    });
    expect(html).toMatch(/aria-expanded="true" aria-controls="[^"]+-g"/);
    expect(text(html)).toContain("~");
    expect(text(html)).toContain("modifié :");
    expect(html).toMatch(/<del[^>]*>.*avant : .*A<\/del>/s);
    expect(html).toMatch(/<ins[^>]*>.*après : .*B<\/ins>/s);
  });

  it("ErrorState: an alert with the problem type and the request identifier", async () => {
    const html = await render(ErrorState, {
      title: "Le service ne répond pas.",
      problem: { type: "urn:kartenn:problem:unavailable", requestId: "req_1" },
      retryLabel: "Réessayer",
    });
    // The alert is the sentence, waiting (hidden) until it is in the tree; codes and
    // action stay out of it.
    expect(html).toMatch(
      /<p class="k-error-state__title" role="alert" data-state="waiting"><span class="k-error-state__sentence">Le service ne répond pas\.<\/span><\/p>/,
    );
    expect(text(html)).toContain("urn:kartenn:problem:unavailable");
    expect(text(html)).toContain("Copier Request-Id req_1");
    expect(text(html)).toContain("Réessayer");
  });

  it("ErrorState: each violation, its message and its code; no code without a problem", async () => {
    const html = await render(ErrorState, {
      title: "Le changeset est refusé.",
      problem: {
        type: "urn:kartenn:problem:changeset-refused",
        requestId: null,
        violations: [{ rule: "prefix.overlaps-sibling", detail: "Le préfixe chevauche." }],
      },
    });
    expect(text(html)).toMatch(/Le préfixe chevauche\.\s*Copier prefix\.overlaps-sibling/);
    expect(text(html)).not.toContain("Request-Id");
    const bare = await render(ErrorState, { title: "Le serveur est injoignable." });
    expect(bare).not.toContain("k-code-badge");
  });

  it("ErrorState takes an ApiProblem of the SDK as it is; a warning says so; no about:blank", async () => {
    // Type level: an ApiProblem is a ProblemFacts (checked by vue-tsc on this file).
    const problem: ProblemFacts = new ApiProblem({
      status: 422,
      type: "about:blank",
      title: "Unprocessable Content",
      violations: [
        {
          rule: "core.owner.unknown",
          detail: "Responsable inconnu.",
          severity: "error",
          subjects: [],
        },
        { rule: "prop.dns.ttl-missing", detail: "TTL absent.", severity: "warning", subjects: [] },
      ],
      requestId: "req_1",
    });
    const html = await render(ErrorState, { title: "Refusé.", problem });
    expect(text(html)).toMatch(/Erreur\s*Responsable inconnu\.\s*Copier core\.owner\.unknown/);
    expect(text(html)).toMatch(/Avertissement\s*TTL absent\.\s*Copier prop\.dns\.ttl-missing/);
    expect(text(html)).not.toContain("about:blank");
    expect(text(html)).toContain("Copier Request-Id req_1");
  });

  it("DiffList: up to 200 lines, every change rendered, no gap", async () => {
    const groups = Array.from({ length: 20 }, (_, g) => ({
      id: `g${g}`,
      title: `Objet ${g}`,
      kind: "changed" as const,
      changes: Array.from({ length: 9 }, (_, c) => ({
        attribute: `a${c}`,
        kind: "changed" as const,
        before: "x",
        after: "y",
      })),
    }));
    // 20 headings + 180 changes = 200 lines: not virtualised.
    const html = await render(DiffList, { groups });
    expect(html).not.toContain("data-virtual");
    expect(html.match(/class="k-diff-list__change"/g)).toHaveLength(180);
    expect(html).not.toContain("aria-setsize");
  });

  it("TreeGrid: loading, skeleton rows in the cells, the tree busy", async () => {
    const html = await render(TreeGrid, {
      label: "Arbre",
      treeLabel: "Préfixe",
      nodes: [{ id: "a", label: "A" }],
      columns: [{ id: "env", label: "Environnement" }],
      loading: true,
      loadingRows: 3,
    });
    expect(html).toContain('aria-busy="true"');
    expect(html.match(/role="row" class="k-tree-grid__row" aria-level="1"/g)).toHaveLength(3);
    expect(html.match(/k-skeleton-fill/g)).toHaveLength(6);
    expect(text(html)).toContain("Chargement");
    expect(text(html)).not.toContain("A");
    // Its tab stop kept: the cell that says it loads.
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
  });

  it("FilterChip: the keys behind the labels in the description of its button", async () => {
    const html = await render(FilterChip, {
      property: "Usage",
      operator: "est",
      value: "Hôte",
      query: "usage=host",
    });
    // Its tooltip, in the layer of the frame (a teleport: e2e/components/filter-picker.spec.ts
    // reads it); without keys, none.
    expect(html).toMatch(/class="k-filter-chip__body"[^>]*aria-describedby="k-tooltip-[^"]+"/);
    const bare = await render(FilterChip, { property: "Usage", operator: "est", value: "Hôte" });
    expect(bare).not.toContain("aria-describedby");
  });

  it("InlineAlert: alert for an error, status otherwise", async () => {
    expect(await render(InlineAlert, { tone: "ko", title: "Refusé." })).toContain('role="alert"');
    expect(await render(InlineAlert, { tone: "warn", title: "Attention." })).toContain(
      'role="status"',
    );
    // Another icon, the colour and the role of the tone kept.
    const warn = await render(InlineAlert, { tone: "warn", title: "Lecture seule." });
    const locked = await render(InlineAlert, {
      tone: "warn",
      title: "Lecture seule.",
      icon: "lock",
    });
    expect(locked).toContain('role="status"');
    expect(locked).toContain('data-tone="warn"');
    expect(locked).not.toEqual(warn);
  });

  it("Skeleton: an empty status region first (filled once mounted), the bars hidden", async () => {
    const html = await render(Skeleton, { rows: 2, columns: 3 });
    // Not busy: a busy region would hold back the announcement of its status.
    expect(html).not.toContain("aria-busy");
    expect(html).toContain('role="status"></span>');
    expect(html.match(/class="k-skeleton__row" aria-hidden="true"/g)).toHaveLength(2);
  });
});

describe("charts (revue du 2026-10-25)", () => {
  const points = [
    { key: "d1", label: "13 janv.", value: 6 },
    { key: "d2", label: "14 janv.", value: 11 },
    { key: "d3", label: "15 janv.", value: 7 },
  ];

  it("niceCeil: a round top of scale, never below the peak", () => {
    expect([0, 1, 7, 11, 12, 13, 141, 18455].map(niceCeil)).toEqual([
      1, 1, 8, 12, 12, 15, 150, 20000,
    ]);
  });

  it("layoutBars: equal slots, crisp edges, heights against the scale", () => {
    const bars = layoutBars([6, 12, 0], 300, 120, 12);
    expect(bars.map((b) => b.slotX)).toEqual([0, 100, 200]);
    expect(bars.map((b) => b.height)).toEqual([60, 120, 0]);
    expect(bars.every((b) => Number.isInteger(b.x) && Number.isInteger(b.width))).toBe(true);
    expect(bars[1]!.x + bars[1]!.width).toBeLessThanOrEqual(200);
    expect(layoutBars([1, 2], 0, 120, 2)).toEqual([]);
  });

  it("BarChart: one image named by a summary, every value in a hidden table", async () => {
    const html = await render(BarChart, {
      label: "Changesets appliqués par jour",
      points,
      pointLabel: "Jour",
      valueLabel: "Appliqués",
    });
    // An image that takes the focus: the arrows show each bar's value.
    expect(html).toContain(
      'role="img" tabindex="0" aria-label="Changesets appliqués par jour : 3 valeurs, 13 janv. – 15 janv. ; maximum 11 (14 janv.) ; dernière 7"',
    );
    expect(html).toContain('aria-live="polite"');
    // Hidden by a box around it: its caption, outside the table box, is hidden too.
    expect(html).toMatch(/<div class="k-visually-hidden"><table><caption>/);
    expect(html.match(/<th scope="row">/g)).toHaveLength(3);
    expect(html).toMatch(/<th scope="col">Jour<\/th><th scope="col">Appliqués<\/th>/);
    // The graduation and the bounds under the plot are hidden: the table says them.
    expect(html).toContain('class="k-bar-chart__scale" aria-hidden="true"');
    expect(html).toContain('class="k-bar-chart__axis" aria-hidden="true"');
  });

  it("BarChart: an empty series says so", async () => {
    const html = await render(BarChart, { label: "Appliqués", points: [] });
    expect(html).toContain('aria-label="Appliqués : aucune donnée"');
  });

  it("BarList: a table of written values, bars hidden, beyond the scale marked", async () => {
    const html = await render(BarList, {
      label: "Occupation",
      items: [
        { key: "a", label: "campus-ouest", value: 87 },
        { key: "b", label: "dmz", value: 104 },
      ],
      max: 100,
      format: (v: number) => `${v} %`,
    });
    expect(html.match(/<th scope="row"/g)).toHaveLength(2);
    expect(html.match(/class="k-bar-list__track" aria-hidden="true"/g)).toHaveLength(2);
    expect(html).toContain("--k-bar-list-ratio:0.87");
    expect(html).toContain("--k-bar-list-ratio:1");
    expect(html.match(/data-state="over"/g)).toHaveLength(1);
    expect(text(html)).toContain("87 %");
  });
});

describe("DiffValue (relecture du 2026-10-26)", () => {
  const value = (kind: "added" | "removed" | "changed", slots: Record<string, () => string>) =>
    renderToString(createSSRApp({ render: () => h(DiffValue, { kind }, slots) }));

  it("added: an ins, its sign hidden from the name, its word said, the value after", async () => {
    const html = await value("added", { default: () => "Lecture des API" });
    expect(html).toMatch(/<ins class="k-diff-value" data-kind="added">/);
    expect(html).toContain('<span class="k-diff-value__sign" aria-hidden="true">+</span>');
    expect(text(html)).toContain("+ajouté : Lecture des API");
  });

  it("removed: a del with « − »; changed: before, then after", async () => {
    expect(await value("removed", { default: () => "x" })).toMatch(/<del[^>]*data-kind="removed"/);
    const changed = await value("changed", { before: () => "Hôte", default: () => "VIP" });
    expect(text(changed)).toMatch(/avant : Hôte.*après : VIP/);
  });
});
