// @vitest-environment happy-dom
import { drafts, queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import type { TreeNode } from "@kartenn/ui";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, getCurrentInstance, h, type App, type ComponentPublicInstance } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { changeLocale, createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";

// The nodes of the tree built once per object read (task 021-22e-k): a level notified again
// keeps the nodes of the objects it held, and every cause that changes what a node says
// builds them again — the language, the dictionary read again, the values of the draft, an
// object read again.

const DOM = "0b5e3c1a-0000-4000-8000-0000000000e1";
const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const A1 = "0b5e3c1a-0000-4000-8000-0000000000a9";
const V1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
const V2 = "0b5e3c1a-0000-4000-8000-0000000000c2";
const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";
const N1 = "0b5e3c1a-0000-4000-8000-000000000101";
const network = (nature = "network", version = V1) => ({
  kind: "prefix",
  id: N1,
  value: "192.0.2.4/30",
  nature,
  version,
  href: "",
});
const address = { kind: "address", id: A1, value: "192.0.2.9", version: V1, href: "" };
const CHILDREN = {
  predicate: (q: { queryKey: readonly unknown[] }) =>
    q.queryKey.includes(P1) && q.queryKey.includes("children"),
};

describe("the nodes of the topography", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    drafts.setActive(null);
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    document.body.innerHTML = "";
  });

  /** The topography of a domain, its prefix opened: a network and an address under it. */
  async function topography() {
    let children: unknown[] = [network(), address];
    let dictionary: unknown = { revision: "r1", spaces: {} };
    const operations: unknown[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const json = (body: unknown) =>
        new Response(JSON.stringify(body), { headers: { "Content-Type": "application/json" } });
      if (url.startsWith(`/v1/domains/${DOM}/roots`)) {
        return json({
          items: [
            { kind: "prefix", id: P1, value: "192.0.2.0/24", nature: "container", version: V1 },
          ].map((r) => ({ ...r, href: "" })),
          links: {},
        });
      }
      if (url.startsWith(`/v1/prefixes/${P1}/children`))
        return json({ items: children, links: {} });
      if (url.startsWith("/v1/labels")) return json(dictionary);
      if ((init?.method ?? "GET") === "GET" && url.startsWith(`/v1/drafts/${D1}`)) {
        return json({ id: D1, title: "b", version: 1, operations });
      }
      if (url.startsWith("/v1/domains"))
        return json({ items: [{ id: DOM, slug: "lab" }], links: {} });
      return json({ items: [], links: {} });
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
    let tree: ComponentPublicInstance | undefined;
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", component: TopologyView },
        { path: "/:rest(.*)*", component: { render: () => null } },
      ],
    });
    await router.push(`/?domain=${DOM}&node=${P1}`);
    const root = document.createElement("div");
    document.body.append(root);
    const i18n = createShellI18n("fr");
    app = createApp({ render: () => h(RouterView) });
    app.mixin({
      mounted(this: ComponentPublicInstance) {
        if (this.$options.__name === "TreeGrid") tree = getCurrentInstance()!.proxy!;
      },
    });
    app.use(i18n).use(router);
    app.mount(root);
    await router.isReady();
    const cell = (row: string) =>
      root.querySelector<HTMLElement>(
        `[data-k-tree] [data-row="${row}"][data-column=""]:not([data-k-skeleton])`,
      );
    await expect.poll(() => cell(P1)).not.toBeNull();
    cell(P1)!.focus();
    cell(P1)!.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }),
    );
    /** The node `id` the tree holds now. */
    const node = (id: string): TreeNode | undefined => {
      const find = (list: readonly TreeNode[]): TreeNode | undefined => {
        for (const n of list) {
          if (n.id === id) return n;
          const below = find(n.children ?? []);
          if (below) return below;
        }
        return undefined;
      };
      return find((tree!.$props as { nodes: TreeNode[] }).nodes);
    };
    await expect.poll(() => node(A1)).toBeDefined();
    return {
      node,
      serve: (next: unknown[]) => (children = next),
      dictionary: (next: unknown) => (dictionary = next),
      draft: (next: unknown[]) => operations.splice(0, operations.length, ...next),
      locale: (to: "fr" | "en") => changeLocale(i18n.global, to),
    };
  }

  it("keeps the nodes of a level read again unchanged, builds an object read again", async () => {
    const t = await topography();
    const kept = t.node(A1)!;
    expect(t.node(N1)!.cells).toMatchObject({ detail: "Réseau" });

    // Read again as it was: the node of the address is the one built before.
    await queryClient.invalidateQueries(CHILDREN);
    await expect.poll(() => queryClient.isFetching(CHILDREN)).toBe(0);
    expect(t.node(A1)).toBe(kept);

    // The network read again at another version, another nature: its node says it.
    t.serve([network("container", V2), address]);
    await queryClient.invalidateQueries(CHILDREN);
    await expect.poll(() => t.node(N1)?.cells?.detail).toBe("Conteneur");
    expect(t.node(A1)).toBe(kept);
  });

  it("builds its nodes again in another language", async () => {
    const t = await topography();
    expect(t.node(N1)).toMatchObject({ detail: "Préfixe", cells: { detail: "Réseau" } });
    t.locale("en");
    await expect.poll(() => t.node(N1)?.cells?.detail).toBe("Network");
    expect(t.node(N1)!.detail).toBe("Prefix");
    expect(t.node(A1)!.detail).toBe("Address");
  });

  it("builds its nodes again when the dictionary is read again", async () => {
    const t = await topography();
    t.dictionary({ revision: "r2", spaces: { kind: { "core.prefix": "Bloc" } } });
    await queryClient.invalidateQueries({ queryKey: ["v1", "labels"] });
    await expect.poll(() => t.node(N1)?.detail).toBe("Bloc");
    // The nature and the address keep their words.
    expect(t.node(N1)!.cells).toMatchObject({ detail: "Réseau" });
    expect(t.node(A1)!.detail).toBe("Adresse");
  });

  it("builds its nodes again with the values of the draft", async () => {
    const t = await topography();
    t.draft([{ op: "update_prefix", id: { id: N1 }, nature: "container" }]);
    drafts.setActive(D1);
    await expect.poll(() => t.node(N1)?.cells?.detail).toBe("Conteneur");
    // The draft left: the value read again.
    drafts.setActive(null);
    await expect.poll(() => t.node(N1)?.cells?.detail).toBe("Réseau");
  });
});
