// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";
import { describe, expect, it } from "vitest";
import DataGrid from "../src/components/DataGrid/DataGrid.vue";
import { columnsTemplate, type GridColumn } from "../src/components/DataGrid/grid.ts";
import BarChart from "../src/components/BarChart/BarChart.vue";
import { layoutBars, niceCeil } from "../src/components/BarChart/chart.ts";
import BarList from "../src/components/BarList/BarList.vue";
import DiffList from "../src/components/DiffList/DiffList.vue";
import ExpandableGrid from "../src/components/ExpandableGrid/ExpandableGrid.vue";
import ErrorState from "../src/components/ErrorState/ErrorState.vue";
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
    expect(columnsTemplate(columns)).toBe("var(--k-size-col-md) minmax(var(--k-size-col-md), 1fr)");
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
    expect(html).toMatch(/role="grid" aria-label="Préfixes" aria-rowcount="3" aria-colcount="2"/);
    expect(html).toContain('aria-multiselectable="true"');
    expect(html).toContain('style="--k-grid-columns:');
    expect(html).toMatch(/role="columnheader"[^>]*aria-sort="descending"/);
    // The unsortable column has no aria-sort.
    expect(html.match(/aria-sort=/g)).toHaveLength(1);
    expect(html).toContain('aria-rowindex="3" aria-selected="true"');
    // One cell only in the tab order: the roving tabindex.
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
  });

  it("renders the states the host decides, in words and with the code of an error", async () => {
    const html = await render(DataGrid, {
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
    });
    expect(html).toContain('data-state="draft"');
    expect(text(html)).toContain("(en brouillon)");
    expect(html).toContain('aria-invalid="true"');
    expect(html).toMatch(/role="alert"[^>]*>.*Libellé trop long\./s);
    expect(text(html)).toContain("core.label.too-long");
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
    expect(text(html)).toContain("Libellé — 198.51.100.0/24");
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
});

describe("ExpandableGrid (revue du 2026-10-25)", () => {
  const flows = Array.from({ length: 25 }, (_, i) => ({
    key: `f${i}`,
    source: `rece-${i}`,
    destination: "prod-api",
    rule: "flow.verdict.denied",
  }));
  const flowColumns: GridColumn[] = [
    { id: "source", label: "Source", mono: true },
    { id: "destination", label: "Destination", width: "fill", mono: true },
    { id: "rule", label: "Règle", width: "lg" },
  ];
  const base = { label: "Flux cassés", columns: flowColumns, rows: flows, rowKey: "key" };

  it("shows the row count, a few rows high, and a named button that opens a dialog", async () => {
    const html = await render(ExpandableGrid, { ...base, visibleRows: 4 });
    expect(text(html)).toContain("25 lignes");
    expect(html).toContain("--k-expandable-grid-rows:4");
    expect(html).toMatch(/<button[^>]*aria-haspopup="dialog"[^>]*aria-expanded="false"/);
    expect(html).toContain('aria-label="Agrandir : Flux cassés"');
    // Closed, one grid only; all the rows are in it (it scrolls, nothing is cut).
    expect(html.match(/role="grid"/g)).toHaveLength(1);
    expect(html.match(/role="row"/g)).toHaveLength(26);
  });

  it("keeps the compact columns in the compact grid only", async () => {
    const html = await render(ExpandableGrid, {
      ...base,
      compactColumns: ["source", "destination"],
    });
    expect(html).toMatch(
      /role="grid" aria-label="Flux cassés" aria-rowcount="26" aria-colcount="2"/,
    );
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
      code: "urn:kartenn:problem:unavailable",
      requestId: "req_1",
      retryLabel: "Réessayer",
    });
    // The alert is the sentence; codes and action stay out of it.
    expect(html).toMatch(
      /<p class="k-error-state__title" role="alert">Le service ne répond pas\.<\/p>/,
    );
    expect(text(html)).toContain("urn:kartenn:problem:unavailable");
    expect(text(html)).toContain("Copier Request-Id req_1");
    expect(text(html)).toContain("Réessayer");
  });

  it("InlineAlert: alert for an error, status otherwise", async () => {
    expect(await render(InlineAlert, { tone: "ko", title: "Refusé." })).toContain('role="alert"');
    expect(await render(InlineAlert, { tone: "warn", title: "Attention." })).toContain(
      'role="status"',
    );
  });

  it("Skeleton: busy, said in words, the bars hidden", async () => {
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
    const html = await render(Skeleton, { rows: 2, columns: 3 });
    expect(html).toContain('aria-busy="true"');
    expect(text(html)).toContain("Chargement");
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
    expect(html).toContain(
      'role="img" aria-label="Changesets appliqués par jour : 3 valeurs, 13 janv. – 15 janv. ; maximum 11 (14 janv.) ; dernière 7"',
    );
    expect(html).toMatch(/<table class="k-visually-hidden">/);
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
