// @vitest-environment happy-dom
import { DataGrid, TreeGrid, type ExtensionSlotProps, type TreeNode } from "@kartenn/ui";
import { afterEach, beforeEach, describe, expect, it, onTestFinished, vi } from "vitest";
import v8 from "node:v8";
import { runInNewContext } from "node:vm";
import { createApp, h, nextTick, shallowReactive, type App, type Component } from "vue";
import { createShellI18n } from "../src/i18n/index.ts";
import ExtensionElement from "../src/plugins/ExtensionElement.vue";
import type { Placed } from "../src/plugins/registry.ts";
import { cellExtension, gridColumns, stableObjectRefs } from "../src/plugins/slots.ts";

// An error of a plugin confined to its cell (REQ-021-13, task 021-11o2): the columns of the
// plugins in a list (`DataGrid`) and in the tree of the topography (`TreeGrid`), their slot
// `extension` filled by the shell's host (`ExtensionElement`, `cell`), as `ListView` and
// `TopologyView` fill it. An element that throws, at its construction or at its connection,
// leaves its code in its cell; the other cells and the other rows stay as they were.

const BAD = "198.51.100.0/24";

/** Throws as it is created: in every row. */
class ThrowsNew extends HTMLElement {
  constructor() {
    super();
    throw new Error("cells: thrown at construction");
  }
}
/** Throws as it is connected, in the row of `BAD` (or of the node `v1-2`) only. */
class ThrowsOnConnect extends HTMLElement {
  row?: { prefix?: string; id?: string };
  connectedCallback() {
    if (this.row?.prefix === BAD || this.row?.id === "v1-2") throw new Error("cells: connected");
    this.textContent = `vu ${this.row?.prefix ?? this.row?.id}`;
  }
}
/** Tells the object it was given. */
class Fine extends HTMLElement {
  object?: { id: string };
  connectedCallback() {
    this.textContent = `objet ${this.object?.id}`;
  }
}
customElements.define("cells-throws-new", ThrowsNew);
customElements.define("cells-throws-connect", ThrowsOnConnect);
customElements.define("cells-fine", Fine);

const EXTENSIONS: Placed[] = [
  { point: "column", id: "new", element: "cells-throws-new", plugin: "notes" },
  { point: "column", id: "connect", element: "cells-throws-connect", plugin: "notes" },
  { point: "column", id: "fine", element: "cells-fine", plugin: "notes" },
];
const COLUMNS = gridColumns(EXTENSIONS, (key) => key);

let apps: App[] = [];
beforeEach(() => {
  vi.spyOn(console, "warn").mockImplementation(() => {});
});
afterEach(() => {
  for (const app of apps) app.unmount();
  apps = [];
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

/** A grid mounted with the slot `extension` filled as the screens fill it. */
function mount(component: Component, initial: Record<string, unknown>) {
  const props = shallowReactive({ ...initial });
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp({
    render: () =>
      h(component, props, {
        extension: (given: ExtensionSlotProps) =>
          h(ExtensionElement, { cell: true, ...cellExtension(EXTENSIONS, given) }),
      }),
  });
  app.use(createShellI18n("fr"));
  apps.push(app);
  app.mount(root);
  return { props, root };
}

const cell = (root: HTMLElement, row: string, column: string) =>
  root.querySelector<HTMLElement>(`[role='gridcell'][data-row="${row}"][data-column="${column}"]`);
const failed = (el: HTMLElement | null) =>
  !!el?.querySelector("[data-failed]") &&
  el.textContent!.includes("Extension en erreur") &&
  el.textContent!.includes("plugin.element");

describe("an element of a column that throws stays in its cell (REQ-021-13)", () => {
  it("in a list: its code in its cell, the other cells and rows as they were", async () => {
    const refs = stableObjectRefs();
    const rows = [
      { id: "p1", prefix: "192.0.2.0/24" },
      { id: "p2", prefix: BAD },
    ];
    const { root } = mount(DataGrid, {
      label: "Préfixes",
      columns: [{ id: "prefix", label: "Préfixe" }, ...COLUMNS],
      rows,
      rowKey: "id",
      elementProps: (row: { id: string }) => ({
        object: refs(row, "prefix", row.id, `/v1/prefixes/${row.id}`),
      }),
    });
    await nextTick();
    await nextTick();
    for (const row of ["p1", "p2"]) {
      // Thrown at construction: in each row, said with its code; no element left there.
      expect(failed(cell(root, row, "x.notes.new")), row).toBe(true);
      expect(cell(root, row, "x.notes.new")!.querySelector("cells-throws-new")).toBeNull();
      // The cells of the core and of another extension stay.
      expect(cell(root, row, "x.notes.fine")!.textContent).toBe(`objet ${row}`);
    }
    expect(cell(root, "p1", "prefix")!.textContent).toContain("192.0.2.0/24");
    // Thrown at connection, in one row: only there.
    expect(failed(cell(root, "p2", "x.notes.connect"))).toBe(true);
    expect(failed(cell(root, "p1", "x.notes.connect"))).toBe(false);
    expect(cell(root, "p1", "x.notes.connect")!.textContent).toBe("vu 192.0.2.0/24");
    // Its code is a control of the cell: out of the tab order, as any (APG Grid).
    const badge = cell(root, "p2", "x.notes.connect")!.querySelector("button")!;
    expect(badge.tabIndex).toBe(-1);
  });

  it("in a list: no element, no encart where elementProps gives nothing", async () => {
    const { root } = mount(DataGrid, {
      label: "Préfixes",
      columns: [{ id: "prefix", label: "Préfixe" }, ...COLUMNS],
      rows: [
        { id: "p1", prefix: "192.0.2.0/24" },
        { id: "p2", prefix: BAD },
      ],
      rowKey: "id",
      elementProps: (row: { id: string }) => (row.id === "p2" ? undefined : {}),
    });
    await nextTick();
    await nextTick();
    expect(failed(cell(root, "p1", "x.notes.new"))).toBe(true);
    for (const column of ["x.notes.new", "x.notes.connect", "x.notes.fine"])
      expect(cell(root, "p2", column)!.textContent!.trim(), column).toBe("");
  });

  it("in a list: the encart does not follow its row when the row shows another object", async () => {
    const { props, root } = mount(DataGrid, {
      label: "Préfixes",
      columns: [{ id: "prefix", label: "Préfixe" }, ...COLUMNS],
      rows: [
        { id: "p1", prefix: "192.0.2.0/24" },
        { id: "p2", prefix: BAD },
      ],
      rowKey: "id",
    });
    await nextTick();
    await nextTick();
    expect(failed(cell(root, "p2", "x.notes.connect"))).toBe(true);
    // The same row keys, read again: the prefixes changed places (the cells of a key are
    // kept by the grid, its element created again for the row it now shows).
    props.rows = [
      { id: "p1", prefix: BAD },
      { id: "p2", prefix: "203.0.113.0/24" },
    ];
    await nextTick();
    await nextTick();
    expect(failed(cell(root, "p1", "x.notes.connect"))).toBe(true);
    expect(failed(cell(root, "p2", "x.notes.connect"))).toBe(false);
    expect(cell(root, "p2", "x.notes.connect")!.textContent).toBe("vu 203.0.113.0/24");
  });

  it("in a list: a setter of `object` or `row` that throws stays in its cell", async () => {
    const throwing = (name: "object" | "row") =>
      class extends HTMLElement {
        set [name](_: unknown) {
          throw new Error(`cells: setter ${name}`);
        }
      };
    customElements.define("cells-throws-object", throwing("object"));
    customElements.define("cells-throws-row", throwing("row"));
    const placed: Placed[] = [
      { point: "column", id: "object", element: "cells-throws-object", plugin: "notes" },
      { point: "column", id: "row", element: "cells-throws-row", plugin: "notes" },
      { point: "column", id: "fine", element: "cells-fine", plugin: "notes" },
    ];
    const root = document.createElement("div");
    document.body.append(root);
    const app = createApp({
      render: () =>
        h(
          DataGrid,
          {
            label: "Préfixes",
            columns: gridColumns(placed, (key) => key),
            rows: [{ id: "p1", prefix: "192.0.2.0/24" }],
            rowKey: "id",
            elementProps: () => ({ object: { type: "prefix", id: "p1", href: "/v1/prefixes/p1" } }),
          },
          {
            extension: (given: ExtensionSlotProps) =>
              h(ExtensionElement, { cell: true, ...cellExtension(placed, given) }),
          },
        ),
    });
    app.use(createShellI18n("fr"));
    apps.push(app);
    app.mount(root);
    await nextTick();
    await nextTick();
    expect(failed(cell(root, "p1", "x.notes.object"))).toBe(true);
    expect(failed(cell(root, "p1", "x.notes.row"))).toBe(true);
    expect(cell(root, "p1", "x.notes.fine")!.textContent).toBe("objet p1");
  });

  it("in a list: the same element and the same object while the grid renders again", async () => {
    const refs = stableObjectRefs();
    const rows = [{ id: "p1", prefix: "192.0.2.0/24" }];
    const { props, root } = mount(DataGrid, {
      label: "Préfixes",
      columns: COLUMNS.filter((column) => column.id === "x.notes.fine"),
      rows,
      rowKey: "id",
      elementProps: (row: { id: string }) => ({
        object: refs(row, "prefix", row.id, `/v1/prefixes/${row.id}`),
      }),
    });
    await nextTick();
    await nextTick();
    const element = root.querySelector<Fine>("cells-fine")!;
    const object = element.object;
    expect(object).toBeDefined();
    // Another render of the grid (its label; the focus moving does the same).
    props.label = "Préfixes du site";
    await nextTick();
    await nextTick();
    expect(root.querySelector("cells-fine")).toBe(element);
    expect(element.object).toBe(object);
  });

  it("keeps a reference per row, and none for a row the screen no longer holds", async () => {
    const refs = stableObjectRefs();
    const row = { id: "p1" };
    const ref = refs(row, "prefix", "p1", "/v1/prefixes/p1");
    expect(refs(row, "prefix", "p1", "/v1/prefixes/p1")).toBe(ref);
    // The same object read again (another row object): another reference, the old one free.
    expect(refs({ id: "p1" }, "prefix", "p1", "/v1/prefixes/p1")).not.toBe(ref);
    // Held weakly by its row: gone with it once collected.
    v8.setFlagsFromString("--expose-gc");
    const gc = runInNewContext("gc") as () => void;
    const held = (() => {
      const gone = { id: "p2" };
      return new WeakRef(refs(gone, "prefix", "p2", "/v1/prefixes/p2"));
    })();
    await new Promise((resolve) => setTimeout(resolve, 0));
    gc();
    await new Promise((resolve) => setTimeout(resolve, 0));
    gc();
    expect(held.deref()).toBeUndefined();
  });

  it("in the tree: its code in the cell of its node, and it does not follow a recycled row", async () => {
    // The tree scrolls in a box of 600 px, its rows 28 px high (the document lays nothing
    // out): a window of rows, as in `ui/test/trees-mounted.test.ts`.
    const heights = { clientHeight: ["k-tree-grid", 600], offsetHeight: ["k-tree-grid__row", 28] };
    for (const [name, [owner, height]] of Object.entries(heights))
      Object.defineProperty(HTMLElement.prototype, name, {
        configurable: true,
        get(this: HTMLElement) {
          return this.classList.contains(owner as string) ? height : 0;
        },
      });
    onTestFinished(() => {
      for (const name of Object.keys(heights))
        delete (HTMLElement.prototype as unknown as Record<string, unknown>)[name];
    });
    const nodes: TreeNode[] = ["v1", "v2"].map((domain) => ({
      id: domain,
      label: domain,
      children: Array.from({ length: 300 }, (_, i) => ({
        id: `${domain}-${i + 1}`,
        label: `${domain}.${i + 1}`,
      })),
    }));
    const { props, root } = mount(TreeGrid, {
      label: "Arbre",
      treeLabel: "Préfixe",
      nodes,
      expanded: ["v1"],
      columns: COLUMNS.filter((column) => column.id === "x.notes.connect"),
      // As the topography: no element in the row of a domain.
      elementProps: (node: TreeNode) => (node.children ? undefined : {}),
    });
    await nextTick();
    await nextTick();
    const connect = (row: string) => cell(root, row, "x.notes.connect");
    expect(failed(connect("v1-2"))).toBe(true);
    expect(connect("v1-1")!.textContent).toBe("vu v1-1");
    expect(connect("v1-3")!.textContent).toBe("vu v1-3");
    expect(connect("v1")!.textContent!.trim()).toBe("");
    const rowOf = (node: string) =>
      root.querySelector<HTMLElement>(`[role='row'][data-node="${node}"]`)!;
    const element = rowOf("v1-2");
    // Another domain open: the rows of the first leave the window, those of the second take
    // their row elements (virtualised, 600 rows).
    props.expanded = ["v2"];
    await nextTick();
    await nextTick();
    expect(root.querySelector("[role='row'][data-node='v1-2']")).toBeNull();
    const shown = [...root.querySelectorAll<HTMLElement>("[role='row'][data-node^='v2-']")];
    expect(shown.length).toBeGreaterThan(2);
    // The row element that held the error shows another node now, without it.
    expect(element.isConnected).toBe(true);
    expect(element.dataset.node).toMatch(/^v2/);
    const now = element.querySelector<HTMLElement>("[data-column='x.notes.connect']");
    expect(failed(now)).toBe(false);
    for (const row of shown) {
      const node = row.dataset.node!;
      expect(failed(connect(node)), node).toBe(false);
      expect(connect(node)!.textContent).toBe(`vu ${node}`);
    }
  });
});
