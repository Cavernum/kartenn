// @vitest-environment happy-dom
import { GUARD_WAIT, INTENT_DELAY, keys, queryClient, reads } from "@kartenn/sdk";
import { resetContext, setContext, setFetch } from "@kartenn/sdk/host";
import { TreeGrid } from "@kartenn/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, effectScope, h, nextTick, ref, shallowRef, type App } from "vue";
import { createMemoryHistory, createRouter, type Router, type RouteRecordRaw } from "vue-router";
import { useInfiniteQuery } from "../src/data/query.ts";
import { installPrefetch, queriesOf } from "../src/prefetch/intent.ts";
import KLink from "../src/prefetch/KLink.vue";
import { nearEnd, useNextPage, type PagedList } from "../src/prefetch/nextPage.ts";
import { childrenIntent } from "../src/prefetch/tree.ts";

// Prefetch on intent, the shell's part (REQ-021-22; design « Préchargement »): the reads
// of a route on the intent of a link and in the guard of navigation, the next page of a
// list, the children of a closed node, the modules of a plugin.

const P1 = "0b5e3c1a-0000-4000-8000-000000000001";
const P2 = "0b5e3c1a-0000-4000-8000-000000000002";
const Empty = { render: () => null };

/** The fetch of the SDK: what it was asked, answered by `answer` (after `delay` ms). */
function serve(answer: (url: string) => unknown = () => ({ id: P1 }), delay = 0) {
  const seen: string[] = [];
  setFetch((async (input: RequestInfo | URL) => {
    const url = String(input);
    seen.push(url);
    if (delay > 0) await new Promise((done) => setTimeout(done, delay));
    return new Response(JSON.stringify(answer(url)), {
      headers: { "Content-Type": "application/json", ETag: '"v1"' },
    });
  }) as typeof fetch);
  return seen;
}

const prefixRoute: RouteRecordRaw = {
  path: "/prefixes/:id",
  component: Empty,
  meta: {
    queries: (params, time) => [reads.object("core.prefix", String(params.id), time)],
  },
};

function makeRouter(routes: RouteRecordRaw[] = []): Router {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/", component: Empty }, prefixRoute, ...routes],
  });
  installPrefetch(router);
  return router;
}

let app: App | null = null;

function mount(render: () => ReturnType<typeof h>, router?: Router): HTMLElement {
  const root = document.createElement("div");
  document.body.append(root);
  app = createApp({ render });
  if (router) app.use(router);
  app.mount(root);
  return root;
}

/** Mounts a `KLink` to `to` under `router`; returns its `<a>`. */
async function link(router: Router, to: string): Promise<HTMLAnchorElement> {
  await router.push("/");
  return mount(() => h(KLink, { to }, () => "Préfixe"), router).querySelector("a")!;
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  app?.unmount();
  app = null;
  document.body.innerHTML = "";
  document.head.innerHTML = "";
  vi.useRealTimers();
  setFetch(null);
  resetContext();
  queryClient.clear();
});

describe("KLink", () => {
  it("reads its target after 60 ms of hover, nothing for a passing pointer", async () => {
    const seen = serve();
    const a = await link(makeRouter(), `/prefixes/${P1}`);
    expect(a.getAttribute("href")).toBe(`/prefixes/${P1}`);
    a.dispatchEvent(new Event("pointerenter"));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY - 1);
    a.dispatchEvent(new Event("pointerleave"));
    await vi.advanceTimersByTimeAsync(100);
    expect(seen).toEqual([]);
    a.dispatchEvent(new Event("pointerenter"));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY);
    expect(seen).toEqual([`/v1/prefixes/${P1}`]);
    expect(queryClient.getQueryData(keys.object("core.prefix", P1, null))).toBeDefined();
  });

  it("reads its target at once on focus", async () => {
    const seen = serve();
    const a = await link(makeRouter(), `/prefixes/${P1}`);
    a.dispatchEvent(new Event("focusin"));
    await vi.advanceTimersByTimeAsync(0);
    expect(seen).toEqual([`/v1/prefixes/${P1}`]);
  });

  it("reads in the dated context of its target, else in the current one", async () => {
    const seen = serve();
    const a = await link(makeRouter(), `/prefixes/${P1}?as_of=2026-01-01T00:00:00Z`);
    a.dispatchEvent(new Event("focusin"));
    await vi.advanceTimersByTimeAsync(0);
    expect(seen).toEqual([`/v1/prefixes/${P1}?as_of=2026-01-01T00%3A00%3A00Z`]);
    setContext({ time: { validAt: "2026-02-01T00:00:00Z", asOf: null } });
    const router = makeRouter();
    const target = router.resolve(`/prefixes/${P2}`);
    expect(queriesOf(target).map((q) => q.queryKey)).toEqual([
      keys.object("core.prefix", P2, { validAt: "2026-02-01T00:00:00Z", asOf: null }),
    ]);
  });

  it("navigates on click, the current page marked", async () => {
    serve();
    const router = makeRouter();
    const a = await link(router, `/prefixes/${P1}`);
    expect(a.hasAttribute("aria-current")).toBe(false);
    a.click();
    await vi.advanceTimersByTimeAsync(GUARD_WAIT);
    expect(router.currentRoute.value.path).toBe(`/prefixes/${P1}`);
    await nextTick();
    expect(a.getAttribute("aria-current")).toBe("page");
  });

  it("preloads the modules of a plugin with their digests, once each", async () => {
    serve();
    const router = makeRouter([
      {
        path: "/x/:plugin/:id/:rest(.*)*",
        component: Empty,
        meta: {
          modules: (params) => ({
            base: `/plugins/${String(params.plugin)}/1.0.0/`,
            integrity: {
              "index.js": "sha384-entry",
              "view.js": "sha384-view",
              "fr.json": "sha384-catalogue",
            },
          }),
        },
      },
    ]);
    const a = await link(router, "/x/vlans/list");
    const preloads = () =>
      [...document.head.querySelectorAll<HTMLLinkElement>('link[rel="modulepreload"]')].map((l) => [
        l.getAttribute("href"),
        l.integrity,
      ]);
    a.dispatchEvent(new Event("pointerenter"));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY);
    expect(preloads()).toEqual([
      ["/plugins/vlans/1.0.0/index.js", "sha384-entry"],
      ["/plugins/vlans/1.0.0/view.js", "sha384-view"],
    ]);
    a.dispatchEvent(new Event("focusin"));
    await vi.advanceTimersByTimeAsync(0);
    expect(preloads()).toHaveLength(2);
  });

  it("forgets a module that failed, inserted again on the next intent", async () => {
    serve();
    const router = makeRouter([
      {
        path: "/x/:plugin/:id/:rest(.*)*",
        component: Empty,
        meta: {
          modules: () => ({ base: "/plugins/hosts/1.0.0/", integrity: { "view.js": "sha384-v" } }),
        },
      },
    ]);
    const a = await link(router, "/x/hosts/list");
    const preload = () => document.head.querySelector<HTMLLinkElement>('link[rel="modulepreload"]');
    a.dispatchEvent(new Event("focusin"));
    await vi.advanceTimersByTimeAsync(0);
    const first = preload()!;
    first.dispatchEvent(new Event("error"));
    expect(preload()).toBeNull();
    a.dispatchEvent(new Event("focusin"));
    await vi.advanceTimersByTimeAsync(0);
    expect(preload()).not.toBeNull();
    expect(preload()).not.toBe(first);
    expect(preload()!.getAttribute("href")).toBe("/plugins/hosts/1.0.0/view.js");
  });
});

describe("guard of navigation", () => {
  it("waits for the reads of the target", async () => {
    serve(() => ({ id: P1 }), 30);
    const router = makeRouter();
    await router.push("/");
    let arrived = false;
    void router.push(`/prefixes/${P1}`).then(() => {
      arrived = true;
    });
    await vi.advanceTimersByTimeAsync(20);
    expect(arrived).toBe(false);
    await vi.advanceTimersByTimeAsync(10);
    expect(arrived).toBe(true);
    expect(queryClient.getQueryData(keys.object("core.prefix", P1, null))).toBeDefined();
  });

  it("waits 100 ms at most, then the screen shows its skeleton", async () => {
    serve(() => ({ id: P1 }), 500);
    const router = makeRouter();
    await router.push("/");
    let arrived = false;
    void router.push(`/prefixes/${P1}`).then(() => {
      arrived = true;
    });
    await vi.advanceTimersByTimeAsync(GUARD_WAIT - 1);
    expect(arrived).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(arrived).toBe(true);
    expect(router.currentRoute.value.path).toBe(`/prefixes/${P1}`);
    expect(queryClient.getQueryData(keys.object("core.prefix", P1, null))).toBeUndefined();
  });

  it("goes on at once without reads, and past a refused read", async () => {
    const seen = serve();
    const router = makeRouter([{ path: "/about", component: Empty }]);
    await router.push("/about");
    expect(router.currentRoute.value.path).toBe("/about");
    expect(seen).toEqual([]);
    setFetch(
      (async () =>
        new Response(JSON.stringify({ status: 403, code: "core.forbidden" }), {
          status: 403,
          headers: { "Content-Type": "application/problem+json" },
        })) as typeof fetch,
    );
    await router.push(`/prefixes/${P1}`);
    expect(router.currentRoute.value.path).toBe(`/prefixes/${P1}`);
  });

  it("reads at the present a target left without dates: the return to the present", async () => {
    const seen = serve();
    setContext({ time: { validAt: "2026-02-01T00:00:00Z", asOf: null } });
    const router = makeRouter();
    await router.push(`/prefixes/${P1}`);
    expect(seen).toEqual([`/v1/prefixes/${P1}`]);
  });

  it("goes on when a route's reads cannot be declared", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const router = makeRouter([
      {
        path: "/broken",
        component: Empty,
        meta: {
          queries: () => {
            throw new TypeError("no such type");
          },
        },
      },
    ]);
    await router.push("/broken");
    expect(router.currentRoute.value.path).toBe("/broken");
    expect(warn).toHaveBeenCalledWith(
      "kartenn: the reads of /broken could not be declared",
      expect.any(TypeError),
    );
    warn.mockRestore();
  });
});

describe("next page of a list", () => {
  it("is near the end within two screens of the loaded rows", () => {
    expect(nearEnd(100, { end: 60, size: 20 })).toBe(false);
    expect(nearEnd(100, { end: 61, size: 20 })).toBe(true);
    expect(nearEnd(0, { end: 0, size: 0 })).toBe(false);
  });

  it("reads the next page when the window nears the end, one at a time", async () => {
    const page = (n: number) => ({
      data: { items: Array.from({ length: n }), links: {} },
      etag: null,
    });
    const fetchNextPage = vi.fn(async () => {});
    const hasNextPage = ref(true);
    const isFetching = ref(false);
    const list: PagedList = {
      data: shallowRef({ pages: [page(50)], pageParams: [null] }),
      hasNextPage,
      isFetching,
      isFetchNextPageError: ref(false),
      fetchNextPage,
    };
    const view = ref({ end: 10, size: 20 });
    const scope = effectScope();
    scope.run(() => useNextPage(list, view));
    expect(fetchNextPage).not.toHaveBeenCalled();
    isFetching.value = true;
    view.value = { end: 30, size: 20 };
    await nextTick();
    expect(fetchNextPage).not.toHaveBeenCalled();
    isFetching.value = false;
    await nextTick();
    expect(fetchNextPage).toHaveBeenCalledTimes(1);
    hasNextPage.value = false;
    view.value = { end: 45, size: 20 };
    await nextTick();
    expect(fetchNextPage).toHaveBeenCalledTimes(1);
    scope.stop();
  });

  it("follows the link to the next page of a list of the cache", async () => {
    const seen = serve((url) =>
      url.includes("cursor=")
        ? { items: [{ id: P2 }], links: {} }
        : { items: [{ id: P1 }], links: { next: "/v1/prefixes?cursor=c2" } },
    );
    const scope = effectScope();
    const view = ref({ end: 0, size: 20 });
    const list = scope.run(() => {
      const list = useInfiniteQuery(reads.list("/v1/prefixes", {}));
      useNextPage(list, view);
      return list;
    })!;
    await vi.advanceTimersByTimeAsync(0);
    await vi.waitFor(() => expect(list.data.value?.pages).toHaveLength(2));
    expect(seen).toEqual(["/v1/prefixes", "/v1/prefixes?cursor=c2"]);
    scope.stop();
  });

  it.each([
    [403, 1],
    [503, 3],
  ])("stops after a next page refused with %i (%i reads)", async (status, count) => {
    const seen: string[] = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      seen.push(url);
      return url.includes("cursor=")
        ? new Response(JSON.stringify({ status, code: "core.failed" }), {
            status,
            headers: { "Content-Type": "application/problem+json" },
          })
        : new Response(
            JSON.stringify({ items: [{ id: P1 }], links: { next: "/v1/prefixes?cursor=c2" } }),
            { headers: { "Content-Type": "application/json", ETag: '"v1"' } },
          );
    }) as typeof fetch);
    const scope = effectScope();
    const view = ref({ end: 0, size: 20 });
    const list = scope.run(() => {
      const list = useInfiniteQuery(reads.list("/v1/prefixes", {}));
      useNextPage(list, view);
      return list;
    })!;
    // The retries of a 5xx (two, 1 s then 2 s), none for a 4xx.
    await vi.advanceTimersByTimeAsync(5_000);
    expect(list.isFetchNextPageError.value).toBe(true);
    // The window moves, still near the end: nothing read again.
    view.value = { end: 1, size: 20 };
    await vi.advanceTimersByTimeAsync(5_000);
    expect(seen.filter((url) => url.includes("cursor="))).toHaveLength(count);
    expect(list.data.value?.pages).toHaveLength(1);
    scope.stop();
  });
});

describe("children of a closed node", () => {
  const nodes = [
    { id: P1, label: "198.51.100.0/24", hasChildren: true },
    { id: P2, label: "203.0.113.0/24", children: [{ id: "leaf", label: "203.0.113.0/25" }] },
  ];

  const tree = () =>
    mount(() => h(TreeGrid, { label: "Préfixes", treeLabel: "Préfixe", nodes, expanded: [P2] }));
  const rowOf = (root: HTMLElement, id: string) =>
    root.querySelector(`[role="row"][data-node="${id}"] .k-tree-grid__node`)!;
  const over = (target: Element) =>
    target.dispatchEvent(new Event("pointerover", { bubbles: true }));

  it("reads the children of a closed node held 60 ms under the pointer", async () => {
    const seen = serve(() => ({ items: [], links: {} }));
    const root = tree();
    const stop = childrenIntent(root, (id) => [reads.subPages("core.prefix", id, "children")]);
    over(rowOf(root, P1));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY - 1);
    root.dispatchEvent(new Event("pointerleave"));
    await vi.advanceTimersByTimeAsync(100);
    expect(seen).toEqual([]);
    over(rowOf(root, P1));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY);
    expect(seen).toEqual([`/v1/prefixes/${P1}/children`]);
    stop();
  });

  it("reads the children of a closed node where the focus rests 60 ms", async () => {
    const seen = serve(() => ({ items: [], links: {} }));
    const root = tree();
    childrenIntent(root, (id) => [reads.subPages("core.prefix", id, "children")]);
    rowOf(root, P1).dispatchEvent(new Event("focusin", { bubbles: true }));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY / 2);
    // The pointer leaving the tree leaves the focus be.
    root.dispatchEvent(new Event("pointerleave"));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY / 2);
    expect(seen).toEqual([`/v1/prefixes/${P1}/children`]);
  });

  it("reads nothing for an open node or a row passed over", async () => {
    const seen = serve(() => ({ items: [], links: {} }));
    const root = tree();
    childrenIntent(root, (id) => [reads.subPages("core.prefix", id, "children")]);
    over(rowOf(root, P2));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY);
    over(rowOf(root, P1));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY / 2);
    over(rowOf(root, "leaf"));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY);
    expect(seen).toEqual([]);
  });
});
