// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, shallowReactive, type App, type Component } from "vue";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import Tree from "../src/components/Tree/Tree.vue";
import type { CellState, GridColumn } from "../src/components/DataGrid/grid.ts";
import TreeGrid from "../src/components/TreeGrid/TreeGrid.vue";
import {
  indexNodes,
  type MoveCheck,
  type MoveIntent,
  type MoveState,
  type TreeNode,
} from "../src/components/TreeGrid/tree.ts";
import { isUnder, moveNode, noCycle } from "../src/demo/treeHost.ts";
import { virtualTree } from "../src/demo/trees.ts";

// The trees mounted (spec 020, task 26; REQ-020-31, 32): one instance per run receives the
// models one after the other with its own state (the focused row kept, moves pending),
// the drops go through its own contract of moves, by the events of the platform; the
// display invariant is read on the rows it rendered.

const apps: App[] = [];
afterEach(() => {
  for (const app of apps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

type Props = Record<string, unknown> & { nodes: TreeNode[] };

/** A tree mounted in the document, its props reactive, its intents recorded. */
function mount(component: Component, initial: Props) {
  const props = shallowReactive({ ...initial });
  const intents: MoveIntent[] = [];
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp({
    render: () =>
      h(component, {
        ...props,
        onMove: (intent: MoveIntent) => intents.push(intent),
        "onUpdate:expanded": (next: string[]) => (props.expanded = next),
        "onUpdate:selected": (next: string) => (props.selected = next),
      }),
  });
  app.config.errorHandler = (error) => {
    throw error;
  };
  apps.push(app);
  app.mount(root);
  return { props, intents, root };
}

const ROW = new Map<Component, string>([
  [TreeGrid as Component, "[role='row'][data-node]"],
  [Tree as Component, "[role='treeitem'][data-node]"],
]);
const rowSelector = (component: Component) => ROW.get(component)!;

/** A drag of `from` over `to`, then dropped, by the events of the platform. */
function drag(root: HTMLElement, component: Component, from: string, to: string): void {
  const row = (id: string) =>
    root.querySelector<HTMLElement>(`${rowSelector(component)}[data-node="${id}"]`)!;
  const fire = (el: HTMLElement, type: string) =>
    el.dispatchEvent(new Event(type, { bubbles: true, cancelable: true }));
  fire(row(from), "dragstart");
  fire(row(to), "dragenter");
  fire(row(to), "dragover");
  fire(row(to), "drop");
  fire(row(from), "dragend");
}

/** The rows the model shows, by an independent walk: key, level, parent. */
function expectedRows(nodes: readonly TreeNode[], expanded: ReadonlySet<string>) {
  const out: { key: string; level: number; parent: string | null }[] = [];
  const walk = (list: readonly TreeNode[], level: number, parent: string | null) => {
    for (const node of list) {
      out.push({ key: node.id, level, parent });
      if (expanded.has(node.id) && node.children) walk(node.children, level + 1, node.id);
    }
  };
  walk(nodes, 1, null);
  return out;
}

/**
 * The invariant on the rows rendered: each one stands at its place of the model (index,
 * level), and its visual parent — the nearest row of level − 1 before it among the rows
 * rendered with no gap between — is the parent of its model.
 */
function checkRendered(root: HTMLElement, component: Component, props: Props): number {
  const model = indexNodes(props.nodes);
  const expected = expectedRows(props.nodes, new Set(props.expanded as string[]));
  const body = root.querySelector(".k-tree-grid__body") ?? root.querySelector(".k-tree__list")!;
  let stack: string[] = [];
  let checked = 0;
  for (const el of body.children) {
    const element = el as HTMLElement;
    if (
      element.classList.contains("k-tree-grid__gap") ||
      element.classList.contains("k-tree__gap")
    ) {
      stack = [];
      continue;
    }
    const key = element.dataset.node!;
    const level = Number(element.getAttribute("aria-level"));
    const at = Number(element.dataset.index);
    expect(expected[at], `row ${key} at ${at}`).toMatchObject({ key, level });
    const parent = model.get(key)!.parent;
    stack.length = level - 1;
    if (level === 1) expect(parent, `row ${key}`).toBeNull();
    else if (stack[level - 2] !== undefined) expect(stack[level - 2], `row ${key}`).toBe(parent);
    stack[level - 1] = key;
    checked += 1;
  }
  return checked;
}

/** A seeded random source (mulberry32): the same suite on each run. */
function random(seed: number): () => number {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const pick = <T>(rand: () => number, list: readonly T[]): T =>
  list[Math.floor(rand() * list.length)]!;

/** A model of more than 200 rows open: 25 groups of 8, a third with 2 children more. */
function bigTree(): TreeNode[] {
  return Array.from({ length: 25 }, (_, g) => ({
    id: `g${g}`,
    label: `Groupe ${g}`,
    children: Array.from({ length: 8 }, (_, c) => ({
      id: `g${g}-${c}`,
      label: `Nœud ${g}.${c}`,
      ...(c % 3 === 0
        ? {
            children: [
              { id: `g${g}-${c}-a`, label: `Feuille ${g}.${c}.a` },
              { id: `g${g}-${c}-b`, label: `Feuille ${g}.${c}.b` },
            ],
          }
        : {}),
    })),
  }));
}

const components: [string, Component][] = [
  ["TreeGrid", TreeGrid],
  ["Tree", Tree],
];

describe.each(components)("%s mounted: display invariant (REQ-020-32)", (name, component) => {
  for (const seed of [3, 11]) {
    it(`seed ${seed}: models one after the other, drops by its contract, a row kept out of the window`, async () => {
      const rand = random(seed);
      const states = new Map<string, MoveState>();
      const nodes = bigTree();
      const { props, intents, root } = mount(component, {
        label: "Arbre",
        treeLabel: "Nœud",
        nodes,
        expanded: [...indexNodes(nodes).keys()],
        parentMode: "declared",
        canMove: noCycle,
        moveState: (node: TreeNode) => states.get(node.id),
      });
      await nextTick();
      // Virtualised: a window, not every row.
      const all = expectedRows(props.nodes, new Set(props.expanded as string[])).length;
      expect(all).toBeGreaterThan(200);
      expect(root.querySelectorAll(rowSelector(component)).length).toBeLessThan(all);
      // The focus on the last row: kept mounted, out of the window.
      // The stop: the node cell of TreeGrid, the row of Tree (not Tree's own entry).
      const stop = root.querySelector<HTMLElement>(
        "[role='row'] [tabindex='0'], [role='treeitem'][tabindex='0']",
      )!;
      stop.focus();
      stop.dispatchEvent(
        new KeyboardEvent("keydown", { key: "End", bubbles: true, cancelable: true }),
      );
      await nextTick();
      await nextTick();
      const kept = (document.activeElement as HTMLElement).closest<HTMLElement>("[data-node]");
      expect(kept?.dataset.node).toBe(
        expectedRows(props.nodes, new Set(props.expanded as string[])).at(-1)!.key,
      );
      const inflight: MoveIntent[] = [];
      let refused = 0;
      for (let step = 0; step < 40; step++) {
        const shown = [...root.querySelectorAll<HTMLElement>(rowSelector(component))].map(
          (el) => el.dataset.node!,
        );
        const r = rand();
        if (r < 0.4) {
          // A drop between two rows rendered, by the tree's own contract.
          const from = pick(rand, shown);
          const to = pick(rand, shown);
          const model = indexNodes(props.nodes);
          const source = model.get(from)!.node;
          const target = model.get(to)!.node;
          const accepted =
            states.get(from) !== "pending" && from !== to && !isUnder(source, target);
          const before = intents.length;
          drag(root, component, from, to);
          await nextTick();
          expect(intents.length, `${from} → ${to}`).toBe(before + (accepted ? 1 : 0));
          if (accepted) {
            inflight.push(intents.at(-1)!);
            states.set(from, "pending");
            props.moveState = (node: TreeNode) => states.get(node.id);
          } else refused += 1;
        } else if (r < 0.7 && inflight.length) {
          // A late answer, on a model that may have changed meanwhile.
          const intent = inflight.splice(Math.floor(rand() * inflight.length), 1)[0]!;
          if (rand() < 0.7) {
            props.nodes = moveNode(props.nodes, intent.node.id, intent.target?.id ?? null);
            states.delete(intent.node.id);
          } else
            states.set(intent.node.id, {
              refused: { ok: false, reason: "Refusé.", code: "server.refused" },
            });
          props.moveState = (node: TreeNode) => states.get(node.id);
        } else {
          // Another change of the model: a node moved elsewhere.
          const ids = [...indexNodes(props.nodes).keys()];
          props.nodes = moveNode(
            props.nodes,
            pick(rand, ids),
            rand() < 0.2 ? null : pick(rand, ids),
          );
        }
        await nextTick();
        await nextTick();
        expect(checkRendered(root, component, props)).toBeGreaterThan(0);
        // The focus never falls to the page while the tree has rows.
        expect(root.contains(document.activeElement), `${name} step ${step}`).toBe(true);
      }
      expect(refused).toBeGreaterThan(0);
    });
  }
});

describe.each(components)("%s mounted: the contract follows its props", (_, component) => {
  const small: TreeNode[] = [
    { id: "a", label: "A", children: [{ id: "a1", label: "A1" }] },
    { id: "b", label: "B" },
  ];

  it("canMove read at each call: given after mounting, then taken away, without error", async () => {
    const { props, intents, root } = mount(component, {
      label: "Arbre",
      treeLabel: "Nœud",
      nodes: small,
      expanded: ["a"],
      parentMode: "declared",
    });
    await nextTick();
    // No check: no move.
    drag(root, component, "a1", "b");
    expect(intents).toHaveLength(0);
    // A check given: the move is asked.
    props.canMove = (): MoveCheck => ({ ok: true });
    await nextTick();
    drag(root, component, "a1", "b");
    expect(intents.map((i) => [i.node.id, i.target?.id])).toEqual([["a1", "b"]]);
    // Taken away again: no move, no TypeError.
    props.canMove = undefined;
    await nextTick();
    expect(() => drag(root, component, "a1", "b")).not.toThrow();
    expect(intents).toHaveLength(1);
  });

  it("a node whose move is pending is neither dragged nor cut, nor moved again", async () => {
    const { intents, root } = mount(component, {
      label: "Arbre",
      treeLabel: "Nœud",
      nodes: small,
      expanded: ["a"],
      parentMode: "declared",
      canMove: noCycle,
      moveState: (node: TreeNode) => (node.id === "a1" ? "pending" : undefined),
    });
    await nextTick();
    const row = root.querySelector<HTMLElement>(`${rowSelector(component)}[data-node="a1"]`)!;
    expect(row.getAttribute("draggable")).toBeNull();
    expect(
      root.querySelector(`${rowSelector(component)}[data-node="b"]`)!.getAttribute("draggable"),
    ).toBe("true");
    drag(root, component, "a1", "b");
    expect(intents).toHaveLength(0);
    const cell = row.matches("[tabindex]") ? row : row.querySelector<HTMLElement>("[tabindex]")!;
    cell.focus();
    cell.dispatchEvent(
      new KeyboardEvent("keydown", { key: "x", ctrlKey: true, bubbles: true, cancelable: true }),
    );
    await nextTick();
    expect(row.getAttribute("data-move")).toBe("pending");
  });

  it("a node a draft moves: its present row and its preview row, each its mark and words (021/A27)", async () => {
    // « A1 » moved by a draft under « B »: its row at its present place, its preview row.
    const { root } = mount(component, {
      label: "Arbre",
      treeLabel: "Nœud",
      nodes: [
        { id: "a", label: "A", children: [{ id: "a1", label: "A1" }] },
        { id: "b", label: "B", children: [{ id: "preview:a1", label: "A1" }] },
      ],
      expanded: ["a", "b"],
      nodeState: (node: TreeNode) =>
        node.id === "a1" ? "moved" : node.id === "preview:a1" ? "preview" : undefined,
    });
    await nextTick();
    const row = (id: string) =>
      root.querySelector<HTMLElement>(`${rowSelector(component)}[data-node="${id}"]`)!;
    const words = (id: string) =>
      [...row(id).querySelectorAll(".k-visually-hidden")].map((el) => el.textContent).join(" ");
    // Every mark, with a modifier or not (`k-tree-grid__mark--ko`).
    const marks = (id: string) => row(id).querySelectorAll(".k-tree-grid__mark, .k-tree__mark");
    expect(row("a1").getAttribute("data-state")).toBe("moved");
    expect(row("preview:a1").getAttribute("data-state")).toBe("preview");
    // Told apart by their words, not their level only; « en brouillon » said by neither.
    expect(words("a1")).toContain("(déplacé dans le brouillon)");
    expect(words("preview:a1")).toContain("(aperçu du brouillon)");
    expect(words("a1")).not.toContain("aperçu");
    expect(words("preview:a1")).not.toContain("déplacé");
    // A visible mark each, of different shapes (not a colour alone).
    expect(marks("a1")).toHaveLength(1);
    expect(marks("preview:a1")).toHaveLength(1);
    expect(marks("a1")[0]!.innerHTML).not.toBe(marks("preview:a1")[0]!.innerHTML);
  });
});

describe("TreeGrid: the node cell and its « + » (021/A20, REQ-021-37)", () => {
  const nodes: TreeNode[] = [
    { id: "a", label: "192.0.2.0/24" },
    { id: "b", label: "198.51.100.0/24" },
  ];
  /** The tree mounted with a « + » on each node, `b`'s refused; its events recorded. */
  function mounted(extra: Record<string, unknown> = {}) {
    const seen: { activate: string[]; add: string[] } = { activate: [], add: [] };
    const root = document.createElement("div");
    document.body.append(root);
    const app = createApp({
      render: () =>
        h(TreeGrid, {
          label: "Arbre",
          treeLabel: "Préfixe",
          nodes,
          addLabel: (node: TreeNode) => `Ajouter dans ${node.label}`,
          addBlocked: (node: TreeNode) =>
            node.id === "b" ? { forbidden: "core.prefix.edit" } : undefined,
          onActivate: (id: string) => seen.activate.push(id),
          onAdd: (id: string) => seen.add.push(id),
          ...extra,
        }),
    });
    apps.push(app);
    app.mount(root);
    const cell = (id: string) =>
      root.querySelector<HTMLElement>(`[data-row="${id}"][data-column=""]`)!;
    const press = (el: HTMLElement, key: string, init: KeyboardEventInit = {}) => {
      const event = new KeyboardEvent("keydown", {
        key,
        bubbles: true,
        cancelable: true,
        ...init,
      });
      el.dispatchEvent(event);
      return event.defaultPrevented;
    };
    const add = (id: string) => cell(id).querySelector<HTMLElement>(".k-tree-grid__add")!;
    return { seen, cell, press, add };
  }

  it("Enter on the node cell activates its row, not its « + »; Tab goes into the « + »", async () => {
    const { seen, cell, press, add } = mounted();
    await nextTick();
    cell("a").focus();
    expect(press(cell("a"), "Enter")).toBe(true);
    expect(seen.activate).toEqual(["a"]);
    expect(document.activeElement).toBe(cell("a"));
    expect(press(cell("a"), "Tab")).toBe(true);
    expect(document.activeElement).toBe(add("a"));
    // Shift+Tab is the browser's: back to the cell, the one stop of the tree.
    expect(press(cell("a"), "Tab", { shiftKey: true })).toBe(false);
    add("a").click();
    expect(seen.add).toEqual(["a"]);
  });

  it("F2 also goes into the « + »; Tab without a « + » is the browser's; an editable node cell edits on Enter", async () => {
    const { seen, cell, press, add } = mounted({
      addLabel: (node: TreeNode) => (node.id === "a" ? "Ajouter dans a" : undefined),
      editable: (node: TreeNode, column: { id: string }) => node.id === "b" && column.id === "",
    });
    await nextTick();
    cell("a").focus();
    expect(press(cell("a"), "F2")).toBe(true);
    expect(document.activeElement).toBe(add("a"));
    cell("b").focus();
    expect(press(cell("b"), "Tab")).toBe(false);
    expect(press(cell("b"), "Enter")).toBe(true);
    await nextTick();
    await nextTick();
    expect(cell("b").querySelector("input")).not.toBeNull();
    expect(seen.activate).toEqual([]);
  });

  it("a refused « + » stays, aria-disabled, its permission in its description; a click adds nothing", async () => {
    const { seen, add } = mounted();
    await nextTick();
    expect(add("b").getAttribute("aria-disabled")).toBe("true");
    const described = add("b").getAttribute("aria-describedby")!;
    expect(document.getElementById(described)?.textContent).toContain("core.prefix.edit");
    expect(add("a").getAttribute("aria-disabled")).toBeNull();
    add("b").click();
    expect(seen.add).toEqual([]);
  });
});

// A level capped (task 32): the row of a notice after the children read, activated by
// Enter or a click, never selected nor moved, out of the count a page says.
describe.each(components)("%s mounted: the notice of a level capped (task 32)", (_, component) => {
  const notice: TreeNode = {
    id: "capped:a",
    label: "La suite dans la liste",
    icon: "info",
    notice: true,
  };
  const nodes: TreeNode[] = [
    { id: "a", label: "A", children: [{ id: "a1", label: "A1" }, notice] },
    { id: "b", label: "B" },
  ];
  /** The focusable element of row `id`: the node cell of TreeGrid, the treeitem of Tree. */
  const stopOf = (root: HTMLElement, id: string) =>
    root.querySelector<HTMLElement>(
      component === TreeGrid ? `[data-row="${id}"][data-column=""]` : `[data-node="${id}"]`,
    )!;
  const press = (el: HTMLElement, key: string, init: KeyboardEventInit = {}) => {
    el.focus();
    const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
    el.dispatchEvent(event);
    return event.defaultPrevented;
  };

  it("Enter or a click activates it; Space and a click select nothing; it is neither cut nor dragged", async () => {
    const activated: string[] = [];
    const { props, intents, root } = mount(component, {
      label: "Arbre",
      treeLabel: "Nœud",
      nodes,
      expanded: ["a"],
      parentMode: "declared",
      canMove: (): MoveCheck => ({ ok: true }),
      onActivate: (id: string) => activated.push(id),
    });
    await nextTick();
    const row = stopOf(root, "capped:a");
    expect(press(row, "Enter")).toBe(true);
    expect(activated).toEqual(["capped:a"]);
    press(row, " ");
    row.click();
    await nextTick();
    expect(activated).toEqual(["capped:a", "capped:a"]);
    expect(props.selected).toBeUndefined();
    // No drag from it, no drop on it, no cut: the paste moves nothing.
    expect(root.querySelector('[data-node="capped:a"][draggable]')).toBeNull();
    drag(root, component, "capped:a", "b");
    drag(root, component, "b", "capped:a");
    press(row, "x", { ctrlKey: true });
    press(stopOf(root, "b"), "v", { ctrlKey: true });
    expect(intents).toEqual([]);
    // Down passes from it as from any row.
    press(stopOf(root, "capped:a"), "ArrowDown");
    await nextTick();
    await nextTick();
    const focused = (document.activeElement as HTMLElement).closest<HTMLElement>("[data-node]");
    expect(focused?.dataset.node).toBe("b");
  });

  it("the page that brings it says its new children, then the notice, never « fin de la liste »", async () => {
    const loads: (string | null)[] = [];
    const { props, root } = mount(component, {
      label: "Arbre",
      treeLabel: "Nœud",
      nodes: [{ id: "a", label: "A", children: [{ id: "a1", label: "A1" }], more: true }],
      expanded: ["a"],
      onLoad: (parent: string | null) => loads.push(parent),
    });
    await nextTick();
    expect(press(stopOf(root, "more:a"), "Enter")).toBe(true);
    expect(loads).toEqual(["a"]);
    props.nodes = [
      {
        id: "a",
        label: "A",
        children: [
          { id: "a1", label: "A1" },
          { id: "a2", label: "A2" },
          { id: "a3", label: "A3" },
          notice,
        ],
      },
    ];
    await nextTick();
    await nextTick();
    await nextTick();
    const said = root.querySelector("[role='status']")!.textContent;
    expect(said).toBe("2 éléments de plus chargés. La suite dans la liste");
  });
});

describe("TreeGrid: why a cell has no editor, why it is in its state (task 021-12f)", () => {
  const columns: GridColumn[] = [
    { id: "env", label: "Environnement" },
    { id: "used", label: "Occupation" },
  ];
  const nodes: TreeNode[] = [
    { id: "a", label: "192.0.2.0/24", cells: { env: "production", used: "12 %" } },
    { id: "b", label: "198.51.100.0/24", cells: { env: "recette", used: "40 %" } },
    { id: "c", label: "203.0.113.0/24", cells: { env: "production", used: "3 %" } },
  ];
  const HELD = "Une politique d'approbation retient la modification : elle attend une revue.";
  const WHY = "L'occupation est calculée par le serveur.";
  const ROOT = "Le préfixe se change dans sa fiche.";
  const states: Record<string, CellState> = {
    "a:env": "review",
    "b:env": { error: "Environnement refusé.", code: "core.env.invalid" },
    // Cells without an editor and with a state: two sentences, or the same one twice.
    "c:used": "review",
    "b:used": "draft",
  };
  /** The tree mounted, the environment editable, the others said why not. */
  function mounted(extra: Record<string, unknown> = {}) {
    const root = document.createElement("div");
    document.body.append(root);
    const app = createApp({
      render: () =>
        h(TreeGrid, {
          label: "Arbre",
          treeLabel: "Préfixe",
          nodes,
          columns,
          editable: (_: TreeNode, column: GridColumn) => column.id === "env",
          readonlyReason: (_: TreeNode, column: GridColumn) =>
            column.id === "used" ? WHY : column.id === "" ? ROOT : undefined,
          cellState: (node: TreeNode, column: GridColumn) => states[`${node.id}:${column.id}`],
          // Asked of every cell: the tree keeps it to the cells with a state, not an error.
          stateReason: (node: TreeNode, column: GridColumn) =>
            states[`${node.id}:${column.id}`] === "draft" ? WHY : HELD,
          ...extra,
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

  it("describes a cell without an editor by why, one in review by its reason, an error by its message", async () => {
    const root = mounted();
    await nextTick();
    expect(description(cell(root, "a", "used"))).toBe(WHY);
    expect(description(cell(root, "a", "env"))).toBe(HELD);
    // The node cell, without an editor, says why too; an editable cell without a state, nothing.
    expect(description(cell(root, "a", ""))).toBe(ROOT);
    expect(cell(root, "c", "env").getAttribute("aria-describedby")).toBeNull();
    // An error is described by its message, never by a state's reason.
    expect(description(cell(root, "b", "env"))).toContain("Environnement refusé.");
    expect(description(cell(root, "b", "env"))).not.toContain(HELD);
    // Each sentence once in the document, hidden, out of the rows.
    const hidden = root.querySelector<HTMLElement>("div[hidden]")!;
    expect(hidden.closest("[role='row']")).toBeNull();
    expect([...hidden.children].map((el) => el.textContent)).toEqual([ROOT, HELD, WHY]);
  });

  it("says both sentences of a cell, its state's reason first, and the same one once", async () => {
    const root = mounted();
    await nextTick();
    const both = cell(root, "c", "used");
    expect(both.getAttribute("aria-describedby")!.split(" ")).toHaveLength(2);
    expect(description(both)).toBe(`${HELD} | ${WHY}`);
    const same = cell(root, "b", "used");
    expect(same.getAttribute("aria-describedby")!.split(" ")).toHaveLength(1);
    expect(description(same)).toBe(WHY);
  });

  it("a read-only tree says no reason why not, only why a cell is in its state", async () => {
    const root = mounted({ editable: undefined });
    await nextTick();
    expect(cell(root, "a", "used").getAttribute("aria-describedby")).toBeNull();
    expect(cell(root, "a", "").getAttribute("aria-describedby")).toBeNull();
    expect(description(cell(root, "a", "env"))).toBe(HELD);
  });
});

// The column of an extension in a tree whose rows are recycled (task 021-11o, 22e): each
// element shows the row it was created for, created again when its row element comes to
// show another row — an element that reads its row once, when connected (as the test
// plugin `notes` does), shows the right one.
describe("TreeGrid: an extension column over recycled rows (task 021-11o)", () => {
  const TAG = "k-test-row-cell";
  if (!customElements.get(TAG))
    customElements.define(
      TAG,
      class extends HTMLElement {
        row: TreeNode | undefined;
        connectedCallback(): void {
          this.textContent = this.row?.id ?? "";
        }
      },
    );
  const shownRows = (root: HTMLElement) =>
    [...root.querySelectorAll<HTMLElement>("[role='row'][data-node]")].map((row) => ({
      row,
      node: row.dataset.node!,
      element: row.querySelector(`[data-column="ext"] ${TAG}`),
    }));

  // The tree scrolls in a box of 600 px, its rows 28 px high (the document lays nothing
  // out): a window of rows.
  const sizes = { clientHeight: "k-tree-grid", offsetHeight: "k-tree-grid__row" } as const;
  const heights = { clientHeight: 600, offsetHeight: 28 } as const;
  beforeEach(() => {
    for (const name of ["clientHeight", "offsetHeight"] as const)
      Object.defineProperty(HTMLElement.prototype, name, {
        configurable: true,
        get(this: HTMLElement) {
          return this.classList.contains(sizes[name]) ? heights[name] : 0;
        },
      });
  });
  afterEach(() => {
    for (const name of ["clientHeight", "offsetHeight"] as const)
      delete (HTMLElement.prototype as Partial<Record<typeof name, number>>)[name];
  });

  it("gives each row its own element, created again when a recycled row shows another node", async () => {
    const { props, root } = mount(TreeGrid as Component, {
      label: "Cent mille",
      treeLabel: "Nœud",
      nodes: virtualTree(),
      expanded: ["v1"],
      columns: [{ id: "ext", label: "Extension", element: TAG }],
    });
    await nextTick();
    const before = shownRows(root);
    expect(before.length).toBeGreaterThan(2);
    expect(before.length).toBeLessThan(300);
    for (const { node, element } of before) expect(element?.textContent, node).toBe(node);
    // Another domain open: the children of the first leave the window, those of the second
    // take their row elements (still virtualised, 1 100 rows).
    props.expanded = ["v2"];
    await nextTick();
    await nextTick();
    const after = shownRows(root);
    const recycled = after.filter(({ row, node }) =>
      before.some((was) => was.row === row && was.node !== node),
    );
    expect(recycled.length).toBeGreaterThan(0);
    for (const { node, element } of after) expect(element?.textContent, node).toBe(node);
  });

  it("creates the host's content of the slot `extension` again when a recycled row shows another node", async () => {
    // Content with a state of its own, the node it was created for: kept, it would follow
    // the row element to the node it comes to show.
    const Host = {
      props: ["row"],
      setup(props: { row: TreeNode }) {
        const created = props.row.id;
        return () => h("output", created);
      },
    };
    const root = document.createElement("div");
    document.body.append(root);
    const expanded = shallowReactive({ value: ["v1"] });
    const app = createApp({
      render: () =>
        h(
          TreeGrid as Component,
          {
            label: "Cent mille",
            treeLabel: "Nœud",
            nodes: virtualTree(),
            expanded: expanded.value,
            columns: [{ id: "ext", label: "Extension", element: TAG }],
          },
          { extension: (given: { row: TreeNode }) => h(Host, { row: given.row }) },
        ),
    });
    apps.push(app);
    app.mount(root);
    await nextTick();
    const shown = () =>
      [...root.querySelectorAll<HTMLElement>("[role='row'][data-node]")].map((row) => ({
        row,
        node: row.dataset.node!,
        content: row.querySelector('[data-column="ext"] output')?.textContent,
      }));
    const before = shown();
    for (const { node, content } of before) expect(content, node).toBe(node);
    expanded.value = ["v2"];
    await nextTick();
    await nextTick();
    const after = shown();
    expect(
      after.some(({ row, node }) => before.some((was) => was.row === row && was.node !== node)),
    ).toBe(true);
    for (const { node, content } of after) expect(content, node).toBe(node);
  });
});
