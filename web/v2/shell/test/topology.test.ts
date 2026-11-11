import {
  ApiProblem,
  CHILDREN_PAGES,
  queryClient,
  reads,
  type Operation,
  type Simulated,
} from "@kartenn/sdk";
import { setFetch } from "@kartenn/sdk/host";
import type { TreeNode } from "@kartenn/ui";
import { InfiniteQueryObserver } from "@tanstack/query-core";
import { afterEach, describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import {
  askNextPage,
  cappedKey,
  cappedParent,
  draftStateOf,
  isCapped,
  levelOfPages,
  nodesOf,
  overlay,
  previewOf,
  restOf,
  type LevelReader,
  type Preview,
  type Rest,
  type Shift,
} from "../src/views/topology.ts";

// The preview of the draft in the tree of the topography (task 021-24b; REQ-021-17; 021/A12,
// precised by 24a): the places of the simulation laid over the tree served, an object the
// draft creates matched with its creation by the index of its operation (A26, task 24c), a
// parent the author may not read at the first level, a parent not read waiting, the
// truncation kept.

const DOMAIN = "d0000000-0000-4000-8000-000000000000";
const OTHER = "d0000000-0000-4000-8000-000000000001";
const ID = (n: number) => `0b5e3c1a-0000-4000-8000-00000000000${n}`;
const P = ID(1); // 198.51.100.0/24, served at the first level
const C1 = ID(2); // 198.51.100.0/26, under P
const C2 = ID(3); // 198.51.100.64/26, under P
const FORBIDDEN = "urn:kartenn:problem:forbidden";
const REQUEST_ID = "01J0000000000000000000000N";
const Q = ID(4); // 203.0.113.0/24, served at the first level
const TEMP = "7e000000-0000-4000-8000-000000000000"; // an identifier of the simulation alone
const TEMP2 = "7e000000-0000-4000-8000-000000000001";
const MACHINE = "7e000000-0000-4000-8000-000000000002";

type Placement = Simulated["placements"][number];
/** A prefix placed, by operation `operation` of the draft, or by effect (`null`). */
const placed = (
  object: string,
  before: string | null,
  after: string | null,
  operation: number | null = null,
): Placement => ({ object, kind: "prefix", before, after, operation });
const simulated = (placements: Placement[], truncated = false) => ({
  placements,
  placements_truncated: truncated,
});
const create = (prefix: string, domain = DOMAIN) =>
  ({
    op: "create_prefix",
    domain: { id: domain },
    prefix,
    nature: "container",
    point_to_point: false,
    anycast: false,
  }) as unknown as Operation;
const update = (id: string, prefix: string) =>
  ({ op: "update_prefix", id: { id }, prefix }) as unknown as Operation;
const within = (operation: Operation) =>
  ({ op: "within", period: { start: 0, end: null }, operation }) as unknown as Operation;
const machine = { op: "machines.create_machine", name: "h1", kind: "physical" } as Operation;

const served: TreeNode[] = [
  {
    id: P,
    label: "198.51.100.0/24",
    children: [
      { id: C1, label: "198.51.100.0/26" },
      { id: C2, label: "198.51.100.64/26" },
    ],
  },
  { id: Q, label: "203.0.113.0/24", hasChildren: true },
];
const known = (id: string) => [P, C1, C2, Q].includes(id);
const LABELS: Record<string, string> = Object.fromEntries(
  served.flatMap((n) => [n, ...(n.children ?? [])]).map((n) => [n.id, n.label]),
);
const nodeOf = (shift: Shift, children: TreeNode[] | undefined): TreeNode => ({
  id: shift.key,
  label: shift.creation?.label ?? LABELS[shift.object!]!,
  ...(children ? { children } : {}),
});
/** The tree as rows « level label », depth first. */
const rows = (nodes: readonly TreeNode[], level = 1): string[] =>
  nodes.flatMap((n) => [`${level} ${n.label}`, ...rows(n.children ?? [], level + 1)]);
const laid = (preview: Preview) => overlay(served, preview, nodeOf);

describe("the preview of the draft in the tree of the prefixes", () => {
  it("shows a prefix inserted with its future children under it, marked where they are", () => {
    const preview = previewOf(
      simulated([placed(TEMP, null, P, 0), placed(C1, P, TEMP), placed(C2, P, TEMP)]),
      [create("198.51.100.0/25")],
      known,
      DOMAIN,
    );
    expect(preview.shifts.map((s) => s.key)).toEqual(["draft:0", `draft:${C1}`, `draft:${C2}`]);
    expect(preview.shifts[0]!.creation).toMatchObject({ label: "198.51.100.0/25", index: 0 });
    // Marked at their present place, which the tree served keeps.
    expect([...preview.moved]).toEqual([C1, C2]);
    const { nodes, shown } = laid(preview);
    expect(shown).toBe(3);
    expect(rows(nodes)).toEqual([
      "1 198.51.100.0/24",
      "2 198.51.100.0/26",
      "2 198.51.100.64/26",
      "2 198.51.100.0/25",
      "3 198.51.100.0/26",
      "3 198.51.100.64/26",
      "1 203.0.113.0/24",
    ]);
    // The nodes served are not written over (no optimism).
    expect(served[0]!.children).toHaveLength(2);
  });

  it("matches a creation by the index of its operation, past those that move nothing", () => {
    // The first update keeps its parent (not placed); the second moves C2 under Q; the
    // creation after them, then the objects moved.
    const preview = previewOf(
      simulated([placed(C2, P, Q, 1), placed(TEMP, null, Q, 2), placed(C1, P, null)]),
      [update(Q, "203.0.113.0/24"), update(C2, "203.0.113.64/26"), create("203.0.113.0/25")],
      known,
      DOMAIN,
    );
    expect(preview.shifts.map((s) => [s.key, s.parent])).toEqual([
      [`draft:${C2}`, Q],
      ["draft:2", Q],
      [`draft:${C1}`, null],
    ]);
    // Q's children are not read yet: its rows of the preview wait for them.
    const { nodes, shown } = laid(preview);
    expect(shown).toBe(1);
    expect(rows(nodes)).toContain("1 198.51.100.0/26");
  });

  it("says each row: moved at its present place, preview at its place after, draft otherwise", () => {
    // A /25 inserted above C1 and C2; Q's value changed in place (not moved).
    const preview = previewOf(
      simulated([placed(TEMP, null, P, 0), placed(C1, P, TEMP), placed(C2, P, TEMP)]),
      [create("198.51.100.0/25"), update(Q, "203.0.113.0/24")],
      known,
      DOMAIN,
    );
    const inDraft = new Set([Q, C1]);
    const states = (list: readonly TreeNode[]): [string, string | undefined][] =>
      list.flatMap((n) => [
        [n.id, draftStateOf(n.id, inDraft, preview)],
        ...states(n.children ?? []),
      ]);
    expect(states(laid(preview).nodes)).toEqual([
      [P, undefined],
      [C1, "moved"],
      [C2, "moved"],
      ["draft:0", "preview"],
      [`draft:${C1}`, "preview"],
      [`draft:${C2}`, "preview"],
      [Q, "draft"],
    ]);
    // Before the simulation answers, a node the draft changes is in the draft only.
    expect(draftStateOf(C1, inDraft, undefined)).toBe("draft");
    expect(draftStateOf(C2, inDraft, undefined)).toBeUndefined();
  });

  it("says moved a node whose parent after is not read, and preview a child taken along", () => {
    // P moved under Q, C1 along with it (not placed), C2 left at the first level.
    const preview = previewOf(
      simulated([placed(P, null, Q, 0), placed(C2, P, null)]),
      [update(P, "203.0.113.0/25")],
      known,
      DOMAIN,
    );
    const inDraft = new Set([P, C1]);
    // Q's children not read: P waits (counted in the notice), moved at its present place.
    expect(laid(preview).shown).toBe(1);
    expect(draftStateOf(P, inDraft, preview)).toBe("moved");
    // C1 keeps its parent: at its present place in the draft only; its copy, a preview row.
    expect(draftStateOf(C1, inDraft, preview)).toBe("draft");
    expect(draftStateOf(`draft:${C1}`, inDraft, preview)).toBe("preview");
  });

  it("keeps the key of a creation from one simulation to the next", () => {
    const ops = [create("198.51.100.0/25")];
    const first = previewOf(simulated([placed(TEMP, null, P, 0)]), ops, known, DOMAIN);
    const second = previewOf(simulated([placed(TEMP2, null, P, 0)]), ops, known, DOMAIN);
    expect(first.shifts[0]!.key).toBe(second.shifts[0]!.key);
  });

  it("places a child moved with a parent that moves under that parent's row after", () => {
    // P inserted under a creation at the first level, C1 leaving P for the creation too.
    const preview = previewOf(
      simulated([placed(TEMP, null, null, 0), placed(P, null, TEMP), placed(C1, P, TEMP)]),
      [create("198.51.0.0/16")],
      known,
      DOMAIN,
    );
    // P takes along the child it keeps (C2); C1 goes under the creation itself.
    expect(rows(laid(preview).nodes).slice(-4)).toEqual([
      "1 198.51.0.0/16",
      "2 198.51.100.0/24",
      "3 198.51.100.64/26",
      "2 198.51.100.0/26",
    ]);
  });

  it("takes along the children a moved prefix keeps, as rows of the preview", () => {
    // P moved under Q (its value changed): C1 and C2 follow it, not placed; C2 leaves it.
    const preview = previewOf(
      simulated([placed(P, null, Q, 0), placed(C2, P, null)]),
      [update(P, "203.0.113.0/25")],
      known,
      DOMAIN,
    );
    const open: TreeNode[] = [served[0]!, { ...served[1]!, hasChildren: false, children: [] }];
    const { nodes } = overlay(open, preview, nodeOf);
    const q = nodes.find((n) => n.id === Q)!;
    expect(q.children!.map((n) => n.id)).toEqual([`draft:${P}`]);
    // C1 along with P; C2, placed elsewhere, not twice.
    expect(q.children![0]!.children!.map((n) => n.id)).toEqual([`draft:${C1}`]);
    expect(nodes.at(-1)!.id).toBe(`draft:${C2}`);
  });

  it("leaves the notice of a capped level under the row at its present place", () => {
    const preview = previewOf(
      simulated([placed(P, null, Q, 0)]),
      [update(P, "203.0.113.0/25")],
      known,
      DOMAIN,
    );
    const notice = { id: cappedKey(P), label: "rest", notice: true };
    const capped = { ...served[0]!, children: [...served[0]!.children!, notice] };
    const open: TreeNode[] = [capped, { ...served[1]!, hasChildren: false, children: [] }];
    const { nodes } = overlay(open, preview, nodeOf);
    expect(nodes[0]!.children!.at(-1)!.id).toBe(cappedKey(P));
    const moved = nodes.find((n) => n.id === Q)!.children![0]!;
    expect(moved.children!.map((n) => n.id)).toEqual([`draft:${C1}`, `draft:${C2}`]);
  });

  it("leaves out what is not of this tree, and keeps the truncation", () => {
    const preview = previewOf(
      simulated(
        [
          placed(TEMP, null, null, 0),
          placed(TEMP2, null, null, 1),
          { object: ID(9), kind: "entity", before: null, after: null, operation: null },
          placed(ID(8), ID(7), ID(6)),
        ],
        true,
      ),
      [create("192.0.2.0/24", OTHER), create("198.51.100.0/23")],
      known,
      DOMAIN,
    );
    // The creation of another domain, an entity, an object not read here: none shown.
    expect(preview.shifts.map((s) => s.key)).toEqual(["draft:1"]);
    expect(preview.truncated).toBe(true);
  });

  it("shows under the first level an object whose parent after the author may not read", () => {
    // `after` `null`: at the root, or a parent hidden — the top of the tree it reads.
    const preview = previewOf(simulated([placed(C1, P, null)]), [], known, DOMAIN);
    expect(rows(laid(preview).nodes).at(-1)).toBe("1 198.51.100.0/26");
  });

  it("labels a creation after a module creation, which the order would mismatch", () => {
    // The machine first, placed from no parent too, then the prefix: each by its index.
    const preview = previewOf(
      simulated([
        { object: MACHINE, kind: "machines.machine", before: null, after: null, operation: 0 },
        placed(TEMP, null, P, 1),
      ]),
      [machine, create("198.51.100.128/25")],
      known,
      DOMAIN,
    );
    expect(preview.shifts.map((s) => s.key)).toEqual(["draft:1"]);
    expect(rows(laid(preview).nodes)).toContain("2 198.51.100.128/25");
  });

  it("labels a creation under `within` by the index of its envelope", () => {
    const preview = previewOf(
      simulated([placed(TEMP, null, P, 0), placed(TEMP2, null, Q, 1)]),
      [within(create("198.51.100.128/25")), create("203.0.113.128/25")],
      known,
      DOMAIN,
    );
    expect(preview.shifts.map((s) => [s.key, s.creation?.label, s.parent])).toEqual([
      ["draft:0", "198.51.100.128/25", P],
      ["draft:1", "203.0.113.128/25", Q],
    ]);
    const open: TreeNode[] = [served[0]!, { ...served[1]!, hasChildren: false, children: [] }];
    expect(rows(overlay(open, preview, nodeOf).nodes)).toEqual([
      "1 198.51.100.0/24",
      "2 198.51.100.0/26",
      "2 198.51.100.64/26",
      "2 198.51.100.128/25",
      "1 203.0.113.0/24",
      "2 203.0.113.128/25",
    ]);
  });
});

// The children of a node up to 10⁴ rows (task 021-24e, 021/A34): the pages of `children`
// kept to `CHILDREN_PAGES` by the cache, the next one read while pages remain within the
// cap (as `TopologyView` asks it, `onLoad`); past the cap, none more and a notice under
// the node, leading to the list « dans ce préfixe ».
describe("the children of a node, page by page up to the cap", () => {
  afterEach(() => {
    setFetch(null);
    queryClient.clear();
  });

  type Child = { id: string; kind: "prefix" };
  type Page = { data: { items: Child[] } };
  /**
   * A container with more children than the cap: a `next` always; 10 children by page (the
   * server gives 100; the cap counts pages, and the test stays light).
   */
  function wide(): string[] {
    const seen: string[] = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      seen.push(url);
      const n = Number(new URL(url, "http://x").searchParams.get("cursor") ?? 0);
      const items = Array.from({ length: 10 }, (_, i) => ({ id: `c${n}-${i}`, kind: "prefix" }));
      const next = `/v1/prefixes/${P}/children?cursor=${n + 1}`;
      return new Response(JSON.stringify({ items, links: { next } }), {
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch);
    return seen;
  }

  it("reads the eleventh page, then stops at the hundredth with a notice under the node", async () => {
    const seen = wide();
    const options = queryClient.defaultQueryOptions(
      reads.subPages<Child>("prefix", P, "children") as never,
    );
    const observer = new InfiniteQueryObserver<Page>(queryClient, options as never);
    const stop = observer.subscribe(() => undefined);
    await observer.refetch();
    const level = () => levelOfPages<Child>(observer.getCurrentResult());
    // The tree asks for the next page as long as its level says more (`onLoad`).
    const ask = async () => {
      if (level()?.more) await observer.fetchNextPage();
    };
    for (let page = 2; page <= 11; page++) await ask();
    expect(seen).toHaveLength(11);
    expect(level()?.items).toHaveLength(110);
    expect(level()?.items[0]).toEqual({ id: "c0-0", kind: "prefix" });
    expect(level()).toMatchObject({ more: true, capped: false });

    for (let page = 12; page <= CHILDREN_PAGES; page++) await ask();
    expect(seen).toHaveLength(CHILDREN_PAGES);
    expect(level()).toMatchObject({ more: false, capped: true });
    // The hundred and first is never asked: the first page stays.
    await ask();
    stop();
    expect(seen).toHaveLength(CHILDREN_PAGES);
    expect(level()?.items).toHaveLength(CHILDREN_PAGES * 10);
    expect(level()?.items[0]).toEqual({ id: "c0-0", kind: "prefix" });

    // The node shows its children read, no row to read more, then the notice.
    const nodes = nodesOf<Child>(
      (parent) => (parent === null ? { items: [{ id: P, kind: "prefix" }], more: false } : level()),
      (item) => ({ id: item.id, label: item.id }),
      () => true,
      (parent) => ({ id: cappedKey(parent), label: `rest of ${parent}` }),
    );
    const node = nodes[0]!;
    expect(node.more).toBeUndefined();
    expect(node.children).toHaveLength(CHILDREN_PAGES * 10 + 1);
    expect(node.children!.at(-1)).toEqual({ id: `capped:${P}`, label: `rest of ${P}` });
    expect(isCapped(node.children!.at(-1)!.id)).toBe(true);
    expect(cappedParent(cappedKey(P))).toBe(P);
    expect(cappedParent(cappedKey(null))).toBeNull();
  });

  it("puts the notice of the first level after its last node", () => {
    const capped = { items: [{ id: P, kind: "prefix" as const }], more: false, capped: true };
    const nodes = nodesOf(
      (parent) => (parent === null ? capped : undefined),
      (item) => ({ id: item.id, label: item.id }),
      () => true,
      (parent) => ({ id: cappedKey(parent), label: "rest" }),
    );
    expect(nodes.map((n) => n.id)).toEqual([P, "capped:"]);
  });
});

// A page the tree asks for, against the reads of an invalidation (task 021-22d): the tree
// asks once per number of children read (`treePages.ts`), so the ask must end with its
// page. `ready` (022) and every change read the trees again (`invalidateQueries`), which
// cancels a read under way: one pass of the bench in seven stopped there, the page asked
// lost, the level left as it was and never asked again.
describe("a page the tree asks for, against the reads of an invalidation", () => {
  afterEach(() => {
    setFetch(null);
    queryClient.clear();
  });

  type Child = { id: string; kind: "prefix" };
  type Page = { data: { items: Child[] } };
  /**
   * 10 children by page, a `next` always, each answer held until `release`; a read aborted
   * rejects. Each child names the read that served it; `fail` answers a problem to that
   * read (1 the first), with its `type` and `Request-Id`.
   */
  function held(fail?: number) {
    const seen: string[] = [];
    const flying = new Map<string, number>();
    let most = 0;
    const waiting: (() => void)[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const read = seen.push(url);
      flying.set(url, (flying.get(url) ?? 0) + 1);
      most = Math.max(most, flying.get(url)!);
      try {
        await new Promise<void>((resolve, reject) => {
          waiting.push(resolve);
          init?.signal?.addEventListener("abort", () => reject(init.signal!.reason));
        });
      } finally {
        flying.set(url, flying.get(url)! - 1);
      }
      if (read === fail)
        return new Response(JSON.stringify({ type: FORBIDDEN, title: "Forbidden", status: 403 }), {
          status: 403,
          headers: { "Content-Type": "application/problem+json", "Request-Id": REQUEST_ID },
        });
      const n = Number(new URL(url, "http://x").searchParams.get("cursor") ?? 0);
      const items = Array.from({ length: 10 }, (_, i) => ({
        id: `c${n}-${i}@${read}`,
        kind: "prefix",
      }));
      const next = `/v1/prefixes/${P}/children?cursor=${n + 1}`;
      return new Response(JSON.stringify({ items, links: { next } }), {
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch);
    /** The reads answered until none waits, the cache's notices between them. */
    async function settle(): Promise<void> {
      for (let i = 0; i < 20; i++) {
        for (const go of waiting.splice(0)) go();
        await new Promise((resolve) => setTimeout(resolve, 0));
      }
    }
    /** The reads of page `n` (0 the first). */
    const of = (n: number) =>
      seen.filter((url) => (new URL(url, "http://x").searchParams.get("cursor") ?? "0") === `${n}`);
    return { seen, of, settle, most: () => most };
  }

  async function opened(fail?: number) {
    const net = held(fail);
    const options = queryClient.defaultQueryOptions(
      reads.subPages<Child>("prefix", P, "children") as never,
    );
    const observer = new InfiniteQueryObserver<Page>(queryClient, options as never);
    const stop = observer.subscribe(() => undefined);
    await net.settle();
    const level = () => levelOfPages<Child>(observer.getCurrentResult());
    return { net, observer, stop, level, reader: observer as unknown as LevelReader<Child> };
  }

  it("asks again, once that read is done, a page an invalidation's read cancelled", async () => {
    const { net, stop, level, reader } = await opened();
    expect(level()?.items).toHaveLength(10);
    askNextPage(reader);
    await Promise.resolve();
    // `ready` while the second page is under way: every read again.
    void queryClient.invalidateQueries();
    await net.settle();
    stop();
    expect(level()?.items).toHaveLength(20);
    expect(level()?.more).toBe(true);
    expect(net.most()).toBe(1);
  });

  it("reads a page asked twice while it is under way once", async () => {
    const { net, stop, level, reader } = await opened();
    askNextPage(reader);
    askNextPage(reader);
    await net.settle();
    stop();
    expect(net.of(1)).toHaveLength(1);
    expect(level()?.items).toHaveLength(20);
  });

  it("asks a page during an invalidation's read once it is done, without cancelling it", async () => {
    const { net, stop, level, reader } = await opened();
    void queryClient.invalidateQueries();
    await Promise.resolve();
    askNextPage(reader);
    await net.settle();
    stop();
    // The first page read again, then the second: the level is the one read again.
    expect(net.of(0)).toHaveLength(2);
    expect(net.of(1)).toHaveLength(1);
    expect(level()?.items[0]?.id).toBe("c0-0@2");
    expect(level()?.items).toHaveLength(20);
  });

  it("reads the first page again when it failed and the row is activated", async () => {
    const { net, stop, level, reader } = await opened(1);
    expect(level()).toBeUndefined();
    askNextPage(reader);
    await net.settle();
    stop();
    expect(net.of(0)).toHaveLength(2);
    expect(level()?.items).toHaveLength(10);
  });

  it("says at the level why its next page was refused, keeps its rows, reads it asked again", async () => {
    const { net, stop, level, reader } = await opened(2);
    askNextPage(reader);
    await net.settle();
    // The page refused: the rows read stay, the rest still to read, the problem at the level
    // with its codes (task 021-13b); not asked again by itself.
    expect(level()?.items).toHaveLength(10);
    expect(level()?.more).toBe(true);
    const failed = level()?.failed;
    expect(failed).toBeInstanceOf(ApiProblem);
    expect(failed).toMatchObject({ status: 403, type: FORBIDDEN, requestId: REQUEST_ID });
    expect(net.of(1)).toHaveLength(1);
    // « Réessayer »: the page read, the problem gone.
    askNextPage(reader);
    await net.settle();
    stop();
    expect(net.of(1)).toHaveLength(2);
    expect(level()?.items).toHaveLength(20);
    expect(level()?.failed).toBeUndefined();
  });
});

// The destination of the notice (task 021-7a): the list of the core « dans ce préfixe »,
// opened through the routes of the frame, which read it filtered; never a list without
// filter when the parent or the domain is not known.
describe("the list the notice of a capped level leads to", () => {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/", component: RouterView, children: frameRoutes }],
  });
  /** The address of the list opened, and the read its screen declares. */
  async function opened(rest: Rest | null) {
    expect(rest).not.toBeNull();
    await router.push(rest!);
    const to = router.currentRoute.value;
    const [read] = to.matched.at(-1)!.meta.queries!(to.params, {}, to.query);
    return { path: to.fullPath, read: read!.queryKey };
  }
  const list = (path: "/v1/prefixes" | "/v1/addresses", filters: Record<string, string>) =>
    reads.list(path, filters as never, null).queryKey;

  it("leads under a container to its prefixes, under a network to its addresses", async () => {
    const container = { kind: "prefix", nature: "container", value: "198.51.100.0/24" };
    expect(await opened(restOf(P, container, DOMAIN))).toEqual({
      path: "/prefixes?within=198.51.100.0/24",
      read: list("/v1/prefixes", { within: "198.51.100.0/24" }),
    });
    const network = { kind: "prefix", nature: "network", value: "2001:db8:3::/64" };
    expect(await opened(restOf(C1, network, DOMAIN))).toEqual({
      path: "/addresses?within=2001:db8:3::/64",
      read: list("/v1/addresses", { within: "2001:db8:3::/64" }),
    });
  });

  it("leads from the first level to the prefixes of the domain", async () => {
    expect(await opened(restOf(null, undefined, DOMAIN))).toEqual({
      path: `/prefixes?domain=${DOMAIN}`,
      read: list("/v1/prefixes", { domain: DOMAIN }),
    });
  });

  it("leads nowhere without the parent or the domain", () => {
    expect(restOf(P, undefined, DOMAIN)).toBeNull();
    expect(restOf(P, { kind: "range" }, DOMAIN)).toBeNull();
    expect(restOf(P, { kind: "prefix", nature: "container", value: "" }, DOMAIN)).toBeNull();
    expect(restOf(null, undefined, undefined)).toBeNull();
    expect(restOf(null, undefined, "")).toBeNull();
  });
});
