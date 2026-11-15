// @vitest-environment happy-dom
import { FRESHNESS, queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, getCurrentInstance, h, type App, type ComponentPublicInstance } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { staleOnly } from "../src/views/topology.ts";

// A level of the tree turned stale (task 021-22e-j): 30 s after its last read
// (`FRESHNESS.children`), the observer of a level says so, and nothing else changes; the
// topography keeps what it holds, its nodes are not built again and nothing renders (the
// freeze of ≈ 270 ms on a level of 10⁴, task 22e-i). A real change still passes: a page
// read, the level read again after an invalidation, an error; and a stale level is read
// again when the view comes back.

const DOM = "0b5e3c1a-0000-4000-8000-0000000000e1";
const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const V1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
const child = (n: number) => ({
  kind: "prefix",
  id: `0b5e3c1a-0000-4000-8000-0000000001${String(n).padStart(2, "0")}`,
  value: `192.0.2.${n * 4}/30`,
  nature: "network",
  version: V1,
  href: "",
});
/** The level of the children of `P1` in the cache. */
const CHILDREN = {
  predicate: (q: { queryKey: readonly unknown[] }) =>
    q.queryKey.includes(P1) && q.queryKey.includes("children"),
};

describe("whether a notification of an observer changes what the view reads", () => {
  const read = { data: { pages: [] }, isFetching: false, isStale: false, error: null };
  it("is stale only when `isStale` alone differs", () => {
    expect(staleOnly(read, { ...read, isStale: true })).toBe(true);
    expect(staleOnly(read, { ...read })).toBe(true);
  });
  it("is a change for anything else: a page, a read under way, an error, the first result", () => {
    expect(staleOnly(read, { ...read, isStale: true, data: { pages: [] } })).toBe(false);
    expect(staleOnly(read, { ...read, isStale: true, isFetching: true })).toBe(false);
    expect(staleOnly(read, { ...read, error: new Error("refused") })).toBe(false);
    expect(staleOnly(read, { ...read, isStale: true, extra: 1 })).toBe(false);
    expect(staleOnly(undefined, read)).toBe(false);
  });
});

describe("a level of the tree turned stale", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    vi.useRealTimers();
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    document.body.innerHTML = "";
  });

  /** The topography of a domain, its prefix opened; `children` what its level answers. */
  async function topography() {
    const reads: string[] = [];
    let children = [child(1), child(2)];
    let refuse = false;
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      reads.push(url);
      const json = (status: number, body: unknown) =>
        new Response(JSON.stringify(body), {
          status,
          headers: {
            "Content-Type": status >= 400 ? "application/problem+json" : "application/json",
          },
        });
      if (url.startsWith(`/v1/domains/${DOM}/roots`)) {
        if (refuse)
          return json(403, {
            type: "urn:kartenn:problem:forbidden",
            title: "Forbidden",
            status: 403,
          });
        const roots = [{ kind: "prefix", id: P1, value: "192.0.2.0/24", nature: "container" }];
        return json(200, { items: roots.map((r) => ({ ...r, version: V1, href: "" })), links: {} });
      }
      if (url.startsWith(`/v1/prefixes/${P1}/children`)) {
        return json(200, { items: children, links: {} });
      }
      if (url.startsWith("/v1/domains")) {
        return json(200, { items: [{ id: DOM, slug: "lab" }], links: {} });
      }
      return json(200, { items: [], links: {} });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "alice",
            kind: "person",
            global_permissions: [],
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const { default: TopologyView } = await import("../src/views/TopologyView.vue");
    /** The renders of every component, and the tree grid itself. */
    const renders: string[] = [];
    let tree: ComponentPublicInstance | undefined;
    async function mount(): Promise<HTMLElement> {
      const router = createRouter({
        history: createMemoryHistory(),
        // The links of the rows lead somewhere: anywhere but the tree, nothing.
        routes: [
          { path: "/", component: TopologyView },
          { path: "/:rest(.*)*", component: { render: () => null } },
        ],
      });
      await router.push(`/?domain=${DOM}&node=${P1}`);
      const root = document.createElement("div");
      document.body.append(root);
      app = createApp({ render: () => h(RouterView) });
      app.mixin({
        mounted(this: ComponentPublicInstance) {
          if (this.$options.__name === "TreeGrid") tree = getCurrentInstance()!.proxy!;
        },
        updated(this: ComponentPublicInstance) {
          renders.push(this.$options.__name ?? this.$options.name ?? "?");
        },
      });
      app.use(createShellI18n("fr")).use(router);
      app.mount(root);
      await router.isReady();
      return root;
    }
    const root = await mount();
    const cell = (row: string) =>
      root.querySelector<HTMLElement>(
        `[data-k-tree] [data-row="${row}"][data-column=""]:not([data-k-skeleton])`,
      );
    await expect.poll(() => cell(P1)).not.toBeNull();
    cell(P1)!.focus();
    cell(P1)!.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }),
    );
    await expect.poll(() => cell(child(2).id)).not.toBeNull();
    return {
      root,
      reads,
      renders,
      cell,
      mount,
      nodes: () => tree!.$props as { nodes: unknown },
      serve: (next: ReturnType<typeof child>[]) => (children = next),
      refuse: () => (refuse = true),
    };
  }

  /** Every timer of the next `ms` run, then the renders they ask for. */
  async function after(ms: number): Promise<void> {
    await vi.advanceTimersByTimeAsync(ms);
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  it("keeps its nodes and renders nothing once its reads turn stale", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const t = await topography();
    await after(1_000);
    const nodes = t.nodes().nodes;
    t.renders.length = 0;
    const read = t.reads.length;

    await after(FRESHNESS.children.staleTime + 1_000);
    // The level is stale in the cache, as its observer said…
    expect(queryClient.getQueryCache().findAll(CHILDREN)[0]?.isStale()).toBe(true);
    // …and the view kept what it holds: no node built again, no render, no read.
    expect(t.nodes().nodes).toBe(nodes);
    expect(t.renders).toEqual([]);
    expect(t.reads.slice(read).filter((url) => url.includes(P1) || url.includes(DOM))).toEqual([]);
  });

  it("still shows a level read again, refused, and reads a stale level when the view is back", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const t = await topography();
    await after(FRESHNESS.children.staleTime + 1_000);

    // The level invalidated (as the event stream does): read again, its new node shown.
    t.serve([child(1), child(2), child(3)]);
    await queryClient.invalidateQueries(CHILDREN);
    await expect.poll(() => t.cell(child(3).id)).not.toBeNull();

    // The first level, stale, read again and refused: the tree says so.
    t.refuse();
    await queryClient.invalidateQueries({
      predicate: (q) => q.queryKey.includes(DOM) && q.queryKey.includes("roots"),
    });
    await expect
      .poll(() => t.root.textContent)
      .toContain("L'arbre des préfixes n'a pas pu être lu.");

    // The view gone, then back once its levels are stale: the first level read again.
    app!.unmount();
    app = undefined;
    await after(FRESHNESS.children.staleTime + 1_000);
    const roots = () => t.reads.filter((url) => url.startsWith(`/v1/domains/${DOM}/roots`)).length;
    const before = roots();
    await t.mount();
    await expect.poll(roots).toBe(before + 1);
  });
});
