import { describe, expect, it } from "vitest";
import { moveFocus, ROOT, Tree, type Node, type Page } from "./model.ts";

const prefix = (value: string): Node => ({
  kind: "prefix",
  id: value,
  href: `/v1/prefixes/${value}`,
  value,
  nature: "container",
});

/** Children by parent, served by pages of `size`; the cursor is the next offset. */
function fetcher(children: Record<string, Node[]>, size = 2) {
  const calls: [string, string | null][] = [];
  const fetch = async (parent: string, cursor: string | null): Promise<Page> => {
    calls.push([parent, cursor]);
    const all = children[parent] ?? [];
    const start = cursor ? Number(cursor) : 0;
    const end = start + size;
    return { items: all.slice(start, end), next: end < all.length ? String(end) : null };
  };
  return { fetch, calls };
}

describe("the tree of the prefixes (REQ-007-31)", () => {
  it("reads the children page by page, and closes the set once the last is read", async () => {
    const { fetch, calls } = fetcher({ [ROOT]: [prefix("a"), prefix("b"), prefix("c")] });
    const tree = new Tree(fetch);
    await tree.loadMore(ROOT);
    let rows = tree.rows();
    expect(rows.map((r) => r.key)).toEqual(["a", "b", "more:"]);
    expect(rows[0]).toMatchObject({ posinset: 1, setsize: -1, level: 1, expanded: false });

    await tree.loadMore(ROOT);
    rows = tree.rows();
    expect(rows.map((r) => r.key)).toEqual(["a", "b", "c"]);
    expect(rows[2]).toMatchObject({ posinset: 3, setsize: 3 });
    expect(calls).toEqual([
      [ROOT, null],
      [ROOT, "2"],
    ]);
    // Read to the end: no more request.
    await tree.loadMore(ROOT);
    expect(calls).toHaveLength(2);
  });

  it("opens a prefix on demand, its children one level down; a closed one hides them", async () => {
    const { fetch, calls } = fetcher({ [ROOT]: [prefix("a")], a: [prefix("a1")] });
    const tree = new Tree(fetch);
    await tree.loadMore(ROOT);
    await tree.expand("a");
    expect(tree.rows().map((r) => [r.key, r.level])).toEqual([
      ["a", 1],
      ["a1", 2],
    ]);
    tree.collapse("a");
    expect(tree.rows().map((r) => r.key)).toEqual(["a"]);
    // Opened again: the children read stay, no new request.
    await tree.expand("a");
    expect(calls).toHaveLength(2);
  });

  it("drops a page that arrives after a reset", async () => {
    let release: (page: Page) => void = () => {};
    const tree = new Tree(() => new Promise<Page>((resolve) => (release = resolve)));
    const pending = tree.loadMore(ROOT);
    tree.reset();
    release({ items: [prefix("stale")], next: null });
    await pending;
    expect(tree.rows()).toEqual([]);
  });

  it("marks a failed page, which can be read again", async () => {
    let fail = true;
    const tree = new Tree(async () => {
      if (fail) throw new Error("network");
      return { items: [prefix("a")], next: null };
    });
    await tree.loadMore(ROOT);
    expect(tree.rows()).toMatchObject([{ kind: "more", failed: true }]);
    fail = false;
    await tree.loadMore(ROOT);
    expect(tree.rows().map((r) => r.key)).toEqual(["a"]);
  });
});

describe("the keys of the treeview pattern (APG)", () => {
  it("moves through the rows, opens, enters, closes and goes up", async () => {
    const { fetch } = fetcher({ [ROOT]: [prefix("a"), prefix("b")], a: [prefix("a1")] }, 10);
    const tree = new Tree(fetch);
    await tree.loadMore(ROOT);
    let rows = tree.rows();
    expect(moveFocus(rows, 0, "ArrowDown")).toEqual({ to: 1 });
    expect(moveFocus(rows, 1, "ArrowDown")).toEqual({ to: 1 });
    expect(moveFocus(rows, 0, "ArrowUp")).toEqual({ to: 0 });
    expect(moveFocus(rows, 0, "ArrowRight")).toEqual({ open: "a" });
    await tree.expand("a");
    rows = tree.rows();
    expect(moveFocus(rows, 0, "ArrowRight")).toEqual({ to: 1 });
    expect(moveFocus(rows, 1, "ArrowLeft")).toEqual({ to: 0 });
    expect(moveFocus(rows, 0, "ArrowLeft")).toEqual({ close: "a" });
    expect(moveFocus(rows, 1, "Home")).toEqual({ to: 0 });
    expect(moveFocus(rows, 0, "End")).toEqual({ to: 2 });
    expect(moveFocus(rows, 0, "x")).toBeNull();
  });
});
