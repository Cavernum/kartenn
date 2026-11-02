// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createApp, h, nextTick, shallowReactive, type App, type Component } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import Tree from "../src/components/Tree/Tree.vue";
import TreeGrid from "../src/components/TreeGrid/TreeGrid.vue";
import {
  indexNodes,
  type MoveCheck,
  type MoveIntent,
  type MoveState,
  type TreeNode,
} from "../src/components/TreeGrid/tree.ts";
import { isUnder, moveNode, noCycle } from "../src/demo/treeHost.ts";

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
