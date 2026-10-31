// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h, type Component } from "vue";
import { renderToString } from "vue/server-renderer";
import { describe, expect, it } from "vitest";
import Tree from "../src/components/Tree/Tree.vue";
import TreeGrid from "../src/components/TreeGrid/TreeGrid.vue";
import {
  closedSiblings,
  flatten,
  indexNodes,
  misplacedRendered,
  searchRow,
  treeStep,
  typesAhead,
  type MoveIntent,
  type TreeNode,
} from "../src/components/TreeGrid/tree.ts";
import { useTreeMove } from "../src/components/internal/treeMove.ts";
import { pageGrowth } from "../src/components/internal/treePages.ts";
import { moveNode, noCycle } from "../src/demo/treeHost.ts";
import { virtualTree } from "../src/demo/trees.ts";

// The trees of task 26 (spec 020; REQ-020-19, 31, 32, 33): the model and its keys, the
// contract of moves, the display invariant under random moves, and both components.

const render = (component: Component, props: Record<string, unknown> = {}) =>
  renderToString(createSSRApp({ render: () => h(component, props) }));

const tree: TreeNode[] = [
  {
    id: "a",
    label: "Alpha",
    children: [
      { id: "a1", label: "Alpha un", children: [{ id: "a11", label: "Bêta" }] },
      { id: "a2", label: "Alpha deux", hasChildren: true },
    ],
    more: true,
  },
  { id: "b", label: "Bravo" },
  { id: "c", label: "Charlie", children: [{ id: "c1", label: "Delta" }] },
];

describe("the model of the trees", () => {
  it("pages left: aria-setsize -1 and a row that reads the next page; an open node to read", () => {
    const rows = flatten(tree, new Set(["a", "a2"]), true);
    expect(rows.map((row) => [row.key, row.kind === "node" ? row.setSize : row.level])).toEqual([
      ["a", -1],
      ["a1", -1],
      ["a2", -1],
      ["more:a2", 3],
      ["more:a", 2],
      ["b", -1],
      ["c", -1],
      ["more:", 1],
    ]);
    expect(rows.find((row) => row.key === "more:a")?.parent).toBe("a");
  });

  it("the keys of the tree pattern over the flattened index", () => {
    const open = new Set(["a", "a1"]);
    const rows = flatten(tree, open);
    const at = (key: string) => rows.findIndex((row) => row.key === key);
    expect(treeStep(rows, at("a"), "ArrowDown", open)).toEqual({ to: at("a1") });
    expect(treeStep(rows, 0, "ArrowUp", open)).toEqual({ to: 0 });
    expect(treeStep(rows, at("a1"), "End", open)).toEqual({ to: rows.length - 1 });
    // Right: opens a closed node, then goes to its first child; nothing on a leaf.
    expect(treeStep(rows, at("c"), "ArrowRight", open)).toEqual({ open: "c" });
    expect(treeStep(rows, at("a1"), "ArrowRight", open)).toEqual({ to: at("a11") });
    expect(treeStep(rows, at("b"), "ArrowRight", open)).toBeNull();
    // Left: closes an open node, else goes to the parent; mirrored in RTL.
    expect(treeStep(rows, at("a1"), "ArrowLeft", open)).toEqual({ close: "a1" });
    expect(treeStep(rows, at("a11"), "ArrowLeft", open)).toEqual({ to: at("a1") });
    expect(treeStep(rows, at("a11"), "ArrowRight", open, true)).toEqual({ to: at("a1") });
    expect(treeStep(rows, at("b"), "ArrowLeft", open)).toBeNull();
    // `*`: the closed siblings with children.
    expect(closedSiblings(rows, at("b"), open)).toEqual(["c"]);
  });

  it("the search by typing: printable characters, from the row after, around the end", () => {
    const rows = flatten(tree, new Set(["a", "a1"]));
    const at = (key: string) => rows.findIndex((row) => row.key === key);
    expect(rows[searchRow(rows, at("a"), "a")]!.key).toBe("a1");
    expect(rows[searchRow(rows, at("a1"), "Alpha u")]!.key).toBe("a1");
    expect(rows[searchRow(rows, at("c"), "b")]!.key).toBe("a11");
    expect(rows[searchRow(rows, 0, "BETA")]!.key).toBe("a11");
    expect(searchRow(rows, 0, "z")).toBe(-1);
    const key = (k: string, mods: Partial<KeyboardEvent> = {}) =>
      typesAhead({ key: k, ctrlKey: false, altKey: false, metaKey: false, ...mods });
    expect(key("B")).toBe(true);
    expect(key("é")).toBe(true);
    for (const k of ["Shift", "Control", "ArrowDown", "F2", "Enter", " ", "Dead"])
      expect(key(k)).toBe(false);
    expect(key("a", { ctrlKey: true })).toBe(false);
  });
});

/** A drag event of the test: it records whether the drop was allowed. */
const dragEvent = () => {
  const event = {
    defaultPrevented: false,
    dataTransfer: null,
    relatedTarget: null,
    currentTarget: { contains: () => false },
    preventDefault() {
      event.defaultPrevented = true;
    },
  };
  return event as unknown as DragEvent & { defaultPrevented: boolean };
};

describe("the contract of moves (REQ-020-31)", () => {
  const host = (mode: "derived" | "declared", model = tree) => {
    const intents: MoveIntent[] = [];
    const said: string[] = [];
    const moves = useTreeMove({
      mode: () => mode,
      canMove: noCycle,
      nodeOf: (id) => indexNodes(model).get(id)?.node,
      move: (intent) => intents.push(intent),
      announce: (text) => said.push(text),
    });
    return { moves, intents, said };
  };

  it("derived: no drag, Ctrl+X and Ctrl+V do nothing, no target listed", () => {
    const { moves, intents } = host("derived");
    const start = dragEvent();
    moves.onDragStart("a1", start);
    expect(start.defaultPrevented).toBe(true);
    expect(moves.cutNode("a1")).toBe(false);
    expect(moves.pasteInto("b")).toBe(false);
    expect(
      moves.targets(
        "a1",
        [...indexNodes(tree).values()].map((e) => e.node),
        "",
      ),
    ).toEqual([]);
    expect(intents).toEqual([]);
  });

  it("declared: a refused target is checked before it shows, takes no drop, says why", () => {
    const { moves, intents, said } = host("declared");
    moves.onDragStart("a", dragEvent());
    const over = dragEvent();
    moves.onDragOver("a11", over);
    expect(over.defaultPrevented).toBe(false);
    expect(moves.dropOf("a11")).toMatchObject({ ok: false, code: "tree.cycle" });
    expect(said.at(-1)).toContain("tree.cycle");
    moves.onDrop("a11", dragEvent());
    expect(intents).toEqual([]);
    // An accepted one: marked, the drop is an intent with the nodes of the model.
    moves.onDragStart("a1", dragEvent());
    const ok = dragEvent();
    moves.onDragOver("b", ok);
    expect(ok.defaultPrevented).toBe(true);
    expect(moves.dropOf("b")).toEqual({ ok: true });
    moves.onDrop("b", dragEvent());
    expect(intents.map((i) => [i.node.id, i.target?.id])).toEqual([["a1", "b"]]);
    expect(intents[0]!.node).toBe(indexNodes(tree).get("a1")!.node);
  });

  it("declared: Ctrl+X then Ctrl+V; a refused paste keeps the node cut; Escape cancels", () => {
    const { moves, intents } = host("declared");
    expect(moves.cutNode("a")).toBe(true);
    expect(moves.pasteInto("a1")).toBe(true);
    expect(intents).toEqual([]);
    expect(moves.cut.value).toBe("a");
    moves.pasteInto("c");
    expect(intents.map((i) => [i.node.id, i.target?.id])).toEqual([["a", "c"]]);
    expect(moves.cut.value).toBeNull();
    moves.cutNode("b");
    expect(moves.cancelCut()).toBe(true);
    expect(moves.cancelCut()).toBe(false);
  });

  it("« Déplacer vers… » lists the accepted targets only, the first level among them", () => {
    const { moves } = host("declared");
    const all = [...indexNodes(tree).values()].map((e) => e.node);
    const listed = moves.targets("a", all, "").map((o) => o.value);
    expect(listed).toEqual([":root", "b", "c", "c1"]);
    expect(moves.targets("a", all, "del").map((o) => o.label)).toEqual(["Delta"]);
  });

  it("a new model during a drag: the verdict under the pointer is taken again", () => {
    let model = tree;
    const moves = useTreeMove({
      mode: () => "declared",
      canMove: noCycle,
      nodeOf: (id) => indexNodes(model).get(id)?.node,
      move: () => {},
      announce: () => {},
    });
    moves.onDragStart("b", dragEvent());
    moves.onDragOver("c", dragEvent());
    expect(moves.dropOf("c")).toEqual({ ok: true });
    // c now under b: b over c would make a cycle.
    model = moveNode(tree, "c", "b");
    moves.recheck();
    expect(moves.dropOf("c")).toMatchObject({ ok: false, code: "tree.cycle" });
    // The node dragged gone: no drag left.
    model = tree.filter((n) => n.id !== "b");
    moves.recheck();
    expect(moves.dragging.value).toBeNull();
  });

  it("a node whose move is pending is neither dragged, nor cut, nor moved again", () => {
    const intents: MoveIntent[] = [];
    const moves = useTreeMove({
      mode: () => "declared",
      canMove: noCycle,
      nodeOf: (id) => indexNodes(tree).get(id)?.node,
      pending: (id) => id === "b",
      move: (intent) => intents.push(intent),
      announce: () => {},
    });
    const start = dragEvent();
    moves.onDragStart("b", start);
    expect(start.defaultPrevented).toBe(true);
    moves.onDragOver("c", dragEvent());
    moves.onDrop("c", dragEvent());
    expect(moves.cutNode("b")).toBe(false);
    expect(moves.movable("b")).toBe(false);
    expect(moves.movable("c")).toBe(true);
    expect(intents).toEqual([]);
  });

  it("a page is said for its new children only: not a node moved in, not a removal", () => {
    const known = new Set(["x1", "x2"]);
    expect(pageGrowth(known, ["x1", "x2", "x3", "x4"], new Set())).toBe(2);
    expect(pageGrowth(known, ["x1", "x2", "m"], new Set(["m"]))).toBe(0);
    expect(pageGrowth(known, ["x1"], new Set())).toBe(0);
    expect(pageGrowth(known, ["x1", "m", "x3"], new Set(["m"]))).toBe(1);
  });

  // REQ-020-33: the prototype deleted the last child; the node moved is the one chosen,
  // taken by its id from the model of now.
  it("the node moved is the one chosen, by its id", () => {
    const { moves, intents } = host("declared");
    moves.ask("a1", "c");
    expect(intents[0]!.node.id).toBe("a1");
    const after = moveNode(tree, "a1", "c");
    expect(indexNodes(after).get("a1")?.parent).toBe("c");
    expect(indexNodes(after).get("a2")?.parent).toBe("a");
  });
});

// ——— Display invariant (REQ-020-32) ———
// The property test runs on mounted instances (test/trees-mounted.test.ts); here, the
// guard itself, on the rows rendered against the model.

describe("the display guard on the rows rendered (REQ-020-32)", () => {
  const open = new Set(["a", "a1", "c"]);
  const rows = flatten(tree, open);
  const parentOf = (key: string) => indexNodes(tree).get(key)?.parent;

  it("a window and a row kept beyond a gap: each under the parent of its model", () => {
    // Rendered: the first two rows, then (after a gap) the last one, kept.
    expect(misplacedRendered(rows, [0, 1, rows.length - 1], parentOf)).toEqual([]);
  });

  it("a row laid out under a parent its model does not give it is found", () => {
    // The layout says a11 is under a1; a model where a11 belongs to c says otherwise.
    const other = (key: string) => (key === "a11" ? "c" : parentOf(key));
    const at = rows.findIndex((row) => row.key === "a11");
    expect(misplacedRendered(rows, [at], other)).toEqual(["a11"]);
    // Not rendered, not checked.
    expect(misplacedRendered(rows, [0], other)).toEqual([]);
  });
});

// ——— The components ———

describe("Tree (APG Tree View, Reka)", () => {
  it("named rows, their detail described, pages counted, the typing marked", async () => {
    const html = await render(Tree, {
      label: "Sites",
      nodes: [
        {
          id: "s1",
          label: "Site A",
          detail: "3 salles",
          children: [{ id: "s11", label: "Salle 1", objectKey: "S1" }],
          more: true,
        },
      ],
      expanded: ["s1"],
      selected: "s11",
    });
    expect(html).toMatch(/aria-label="Sites"[^>]*role="tree"/);
    expect(html).toContain("data-k-typeahead");
    expect(html).toMatch(
      /data-node="s1"[^>]*role="treeitem" aria-selected="false" aria-expanded="true"/,
    );
    // Named by its label (the content left visible), the key in its description, out of
    // the name.
    const described =
      /data-node="s11"[^>]*aria-setsize="-1" aria-posinset="1" aria-describedby="([^"]+)"/.exec(
        html,
      )![1]!;
    expect(html).toContain(`<span class="k-tree__label">Salle 1</span>`);
    expect(html).toMatch(
      new RegExp(`id="${described}" class="k-tree__detail" aria-hidden="true"><code[^>]*>S1<`),
    );
    expect(html).toMatch(/data-node="s11"[^>]*role="treeitem" aria-selected="true"/);
    // The stop on the selected row, for Reka's entry focus too.
    expect(html).toMatch(/tabindex="0"[^>]*data-active[^>]*data-node="s11"/);
    expect(html).toContain('data-more="s1"');
    // Derived by default: nothing to drag.
    expect(html).not.toContain("draggable");
  });

  it("declared: rows draggable, a move pending at its place, a refusal with its code", async () => {
    const html = await render(Tree, {
      label: "Entités",
      nodes: tree,
      expanded: ["a"],
      parentMode: "declared",
      moveState: (node: TreeNode) =>
        node.id === "a1"
          ? "pending"
          : node.id === "b"
            ? { refused: { ok: false, reason: "Verrouillé.", code: "entity.locked" } }
            : undefined,
    });
    expect(html).toContain('draggable="true"');
    expect(html).toMatch(/aria-busy="true"[^>]*data-move="pending"/);
    expect(html).toContain("entity.locked");
    expect(html.replace(/<[^>]+>/g, "")).toContain("(déplacement en attente)");
  });

  it("a hundred thousand nodes: a window in the document, the focused row kept", async () => {
    const html = await render(Tree, {
      label: "Cent mille",
      nodes: virtualTree(),
      expanded: ["v1"],
      selected: "v1-900",
    });
    expect(html).toContain('data-virtual="true"');
    expect((html.match(/role="treeitem"/g) ?? []).length).toBeLessThan(300);
    expect(html).toContain('data-node="v1-900"');
    expect(html).toContain("k-tree__gap");
  });
});

describe("TreeGrid (complete)", () => {
  it("rows counted and indexed, -1 while pages remain; derived offers no move", async () => {
    const html = await render(TreeGrid, {
      label: "Arbre",
      treeLabel: "Nœud",
      nodes: tree,
      expanded: ["a"],
      columns: [{ id: "env", label: "Environnement" }],
    });
    expect(html).toContain('aria-rowcount="-1"');
    expect(html).toMatch(/role="row"[^>]*data-node="a1"[^>]*aria-rowindex="3"/);
    expect(html).toMatch(/role="row"[^>]*data-node="more:a"/);
    expect(html).toContain('aria-readonly="true"');
    expect(html).not.toContain("draggable");
    const done = await render(TreeGrid, {
      label: "Arbre",
      treeLabel: "Nœud",
      nodes: [{ id: "x", label: "X" }],
    });
    expect(done).toContain('aria-rowcount="2"');
  });

  it("editing: the editor in its cell, states and errors carried by the cell", async () => {
    const html = await render(TreeGrid, {
      label: "Arbre",
      treeLabel: "Nœud",
      nodes: [
        { id: "x", label: "X", cells: { env: "Prod" } },
        { id: "y", label: "Y" },
      ],
      columns: [{ id: "env", label: "Environnement" }],
      editable: () => true,
      editing: { row: "x", column: "env" },
      cellState: (node: TreeNode) =>
        node.id === "y" ? { error: "Inconnu.", code: "env.unknown" } : undefined,
    });
    expect(html).toMatch(/class="k-field-frame k-tree-grid__editor"/);
    expect(html).toContain('aria-label="Environnement — X"');
    expect(html).toMatch(/data-row="y" data-column="env"[^>]*aria-invalid="true"/);
  });

  it("a hundred thousand nodes: a window in the document, the focused row kept", async () => {
    const html = await render(TreeGrid, {
      label: "Cent mille",
      treeLabel: "Nœud",
      nodes: virtualTree(),
      expanded: ["v1"],
      selected: "v1-900",
    });
    expect(html).toContain('data-virtual="true"');
    expect((html.match(/role="row"/g) ?? []).length).toBeLessThan(300);
    expect(html).toMatch(/data-node="v1-900"[^>]*aria-rowindex="902"/);
  });
});
