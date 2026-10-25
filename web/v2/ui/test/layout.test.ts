// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";
import { describe, expect, it } from "vitest";
import DataGrid from "../src/components/DataGrid/DataGrid.vue";
import {
  chooserItems,
  columnTemplates,
  maxPriority,
  renderedColumns,
  toggleColumn,
  type GridColumn,
} from "../src/components/DataGrid/grid.ts";
import {
  CONTAINER_WIDTHS,
  WIDTH_CLASSES,
  widthClass,
  widthQuery,
} from "../src/components/layout.ts";
import { foldActions, mainAction, type ToolbarAction } from "../src/components/Toolbar/toolbar.ts";
import TreeGrid from "../src/components/TreeGrid/TreeGrid.vue";

// Adaptation to the width (spec 020, « Adaptation à la largeur »): the thresholds of the
// frame equal `WIDTH_CLASSES`, the container queries of the library use `CONTAINER_WIDTHS`
// only, and a grid never hides a column without a way back (the column chooser).

const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));
/** The lengths in px of the conditions of the `@<rule>` at-rules of a sheet. */
const thresholds = (text: string, rule: "media" | "container") =>
  [...text.matchAll(new RegExp(`@${rule}\\s+([^{]*)\\{`, "g"))].flatMap((m) =>
    [...m[1]!.matchAll(/(\d+)px/g)].map((n) => Number(n[1])),
  );

describe("width classes of the frame", () => {
  it("the media queries of the frame copy WIDTH_CLASSES, all of them", () => {
    const frame = readFileSync(here("../../shell/src/_ui/ecrans/EcranCadre.vue"), "utf8");
    const used = new Set(thresholds(frame, "media"));
    const listed = new Set(WIDTH_CLASSES.map((c) => c.min).filter((min) => min > 0));
    expect([...used].sort()).toEqual([...listed].sort());
  });

  it("a window falls in one class, the query of its class matches it", () => {
    expect(widthClass(320)).toBe("compact");
    expect(widthClass(599)).toBe("compact");
    expect(widthClass(600)).toBe("narrow");
    expect(widthClass(1023)).toBe("narrow");
    expect(widthClass(1024)).toBe("standard");
    expect(widthClass(1280)).toBe("wide");
    expect(widthQuery("compact")).toBe("(width < 600px)");
    expect(widthQuery("narrow")).toBe("(600px <= width < 1024px)");
    expect(widthQuery("wide")).toBe("(width >= 1280px)");
  });
});

describe("container queries of the library", () => {
  const sheets = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) return sheets(path);
      return /\.(vue|css)$/.test(entry.name) ? [path] : [];
    });

  it("query only the widths of CONTAINER_WIDTHS, and never the window", () => {
    const allowed = new Set<number>(Object.values(CONTAINER_WIDTHS));
    const wrong = sheets(here("../src/components")).flatMap((path) => {
      const text = readFileSync(path, "utf8");
      const name = path.split("/components/")[1];
      return [
        ...thresholds(text, "container")
          .filter((n) => !allowed.has(n))
          .map((n) => `${name}: @container ${n}px`),
        // Interaction media features (pointer, hover) are allowed; widths are not.
        ...thresholds(text, "media").map((n) => `${name}: @media ${n}px`),
      ];
    });
    expect(wrong).toEqual([]);
  });
});

const columns: GridColumn[] = [
  { id: "value", label: "Valeur", width: "md" },
  { id: "usage", label: "Usage", width: "sm", priority: 2 },
  { id: "site", label: "Site", width: "md", priority: 3 },
  { id: "owner", label: "Propriétaire", width: "fill" },
];
const rows = [{ value: "192.0.2.1", usage: "serveur", site: "nord", owner: "réseau" }];
const render = (component: Component, props: Record<string, unknown>) =>
  renderToString(createSSRApp({ render: () => h(component, props) }));

describe("column priorities", () => {
  it("one template per width: priority 3 leaves under lg, 2 under md", () => {
    expect(columnTemplates(columns, {})).toEqual({
      "--k-grid-columns":
        "var(--k-size-col-md) var(--k-size-col-sm) var(--k-size-col-md) minmax(var(--k-size-col-md), 1fr)",
      "--k-grid-columns-below-lg":
        "var(--k-size-col-md) var(--k-size-col-sm) minmax(var(--k-size-col-md), 1fr)",
      "--k-grid-columns-below-md": "var(--k-size-col-md) minmax(var(--k-size-col-md), 1fr)",
    });
  });

  it("a column the user shows stays at every width; one hidden leaves; never the key", () => {
    const shown = columnTemplates(columns, { site: true });
    expect(shown["--k-grid-columns-below-md"]).toBe(
      "var(--k-size-col-md) var(--k-size-col-md) minmax(var(--k-size-col-md), 1fr)",
    );
    expect(renderedColumns(columns, { usage: false, value: false }).map((c) => c.id)).toEqual([
      "value",
      "site",
      "owner",
    ]);
  });

  it("the chooser lists the columns that may hide, checked when they show", () => {
    const items = chooserItems(columns, { site: false }, (id) => id === "usage");
    expect(items).toEqual([
      { kind: "checkbox", id: "usage", label: "Usage", checked: true },
      { kind: "checkbox", id: "site", label: "Site", checked: false },
    ]);
    expect(toggleColumn({}, "usage", true)).toEqual({ usage: false });
    expect(toggleColumn({}, "site", false)).toEqual({ site: true });
    expect(toggleColumn({ site: true }, "site", true)).toEqual({ site: false });
    expect(maxPriority(columns)).toBe(3);
    expect(maxPriority([columns[0]!, columns[3]!])).toBe(1);
  });

  it("DataGrid: priorities on the cells, a chooser above, the user's choice applied", async () => {
    const html = await render(DataGrid, { label: "Adresses", columns, rows, rowKey: "value" });
    expect(html).toContain('class="k-data-grid-frame"');
    expect(html).toContain('class="k-column-chooser" data-max="3"');
    expect(html).toMatch(/role="columnheader"[^>]*data-column="site"[^>]*data-priority="3"/);
    expect(html).toMatch(/role="gridcell"[^>]*data-column="usage"[^>]*data-priority="2"/);
    expect(html).toContain('aria-colcount="4"');

    const chosen = await render(DataGrid, {
      label: "Adresses",
      columns,
      rows,
      rowKey: "value",
      columnVisibility: { site: true, usage: false },
    });
    expect(chosen).not.toMatch(/data-column="usage"/);
    expect(chosen).toMatch(/role="columnheader"[^>]*data-column="site"(?![^>]*data-priority)/);
    // The last column keeps its index among all of them, the hidden one counted.
    expect(chosen).toMatch(/data-column="owner"[^>]*aria-colindex="4"/);
    expect(chosen).toContain('data-chosen="true"');
  });

  it("DataGrid: no chooser when no column may hide", async () => {
    const html = await render(DataGrid, {
      label: "Adresses",
      columns: [columns[0]!, columns[3]!],
      rows,
      rowKey: "value",
    });
    expect(html).not.toContain("k-column-chooser");
  });

  it("TreeGrid: the same priorities and chooser, the tree column always shown", async () => {
    const html = await render(TreeGrid, {
      label: "Topographie",
      treeLabel: "Nœud",
      nodes: [{ id: "a", label: "192.0.2.0/24", cells: { usage: "serveurs" } }],
      columns: [columns[1]!],
    });
    expect(html).toContain('class="k-column-chooser" data-max="2"');
    expect(html).toMatch(/role="columnheader"[^>]*data-column="usage"[^>]*data-priority="2"/);
  });
});

describe("Toolbar under a container xs", () => {
  const actions: ToolbarAction[] = [
    { id: "free", label: "Prochain libre", priority: 2 },
    { id: "create", label: "Créer", variant: "primary" },
    { id: "copy", label: "Copier" },
  ];
  const widths = new Map([
    ["free", 120],
    ["create", 70],
    ["copy", 80],
  ]);

  it("keeps the main action alone when it must fold", () => {
    expect(mainAction(actions)?.id).toBe("create");
    expect(foldActions(actions, widths, 250, 8, 28)).toEqual(["free", "copy"]);
  });

  it("folds nothing when all fit, whatever the width", () => {
    expect(foldActions(actions, widths, 1000, 8, 28)).toEqual([]);
  });

  it("folds from the lowest priority above xs", () => {
    expect(foldActions(actions, widths, 370, 8, 28)).toEqual([]);
    expect(foldActions(actions, widths, 260, 8, 28, 200)).toEqual(["copy"]);
  });
});
