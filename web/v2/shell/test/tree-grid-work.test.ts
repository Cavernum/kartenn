// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, defineComponent, h, nextTick, type App, type Component } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";

// The work of the tree of the topography as it scrolls (task 021-22e-c; budget « dropped
// frames over 10⁴ rows in 10 s »): the real view, a container open with its children, the
// tree virtualised, scrolled by the rows a frame of the bench brings (≈ 17 at 1 000 rows/s).
// Each step renders the rows that come (recycled: their props change) and asks the host's
// functions for them, never for the rows that stay; a step of the focus renders the row it
// leaves and the row it reaches.

/** The calls of each function the view gives the tree, by the name of its prop. */
const calls = new Map<string, number>();
vi.mock("@kartenn/ui", async (original) => {
  const module = await original<typeof import("@kartenn/ui")>();
  const counted = new WeakMap<object, unknown>();
  const count = (name: string, fn: (...args: unknown[]) => unknown) => {
    if (!counted.has(fn))
      counted.set(fn, (...args: unknown[]) => {
        calls.set(name, (calls.get(name) ?? 0) + 1);
        return fn(...args);
      });
    return counted.get(fn);
  };
  // The tree given the view's functions counted, each the same from one render to the next.
  const TreeGrid = defineComponent({
    name: "CountedTreeGrid",
    inheritAttrs: false,
    setup(_, { attrs, slots }) {
      return () =>
        h(
          module.TreeGrid as Component,
          Object.fromEntries(
            Object.entries(attrs).map(([key, value]) => [
              key,
              typeof value === "function" && !key.startsWith("on")
                ? count(key, value as (...args: unknown[]) => unknown)
                : value,
            ]),
          ),
          slots,
        );
    },
  });
  return { ...module, TreeGrid };
});

const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";
const C1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
const VERSION = "0b5e3c1a-0000-4000-8000-0000000000f1";
/** The children of the container: past `VIRTUAL_ROWS`, the tree virtualised. */
const CHILDREN = 1000;
/** The rows a frame brings at 1 000 rows/s. */
const STEP = 17;
const ROW = 28;
const prefixRef = (id: string) => ({ id, href: `/v1/prefixes/${id}` });
const prefix = (id: string, value: string, nature: "container" | "network") => ({
  kind: "prefix",
  id,
  href: `/v1/prefixes/${id}`,
  value,
  nature,
  version: VERSION,
  occupancy: { nature, prefix: prefixRef(id), total: "256", used: "12" },
});
const child = (i: number) =>
  prefix(
    `0b5e3c1a-0000-4000-8000-${String(i).padStart(12, "0")}`,
    `10.${Math.floor(i / 256)}.${i % 256}.0/24`,
    "network",
  );

// The tree scrolls in a box of 600 px, its rows 28 px high; the document lays nothing out.
const offsets = new WeakMap<Element, number>();
const isTree = (el: HTMLElement) => el.classList.contains("k-tree-grid");
beforeEach(() => {
  Object.defineProperty(HTMLElement.prototype, "clientHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return isTree(this) ? 600 : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return this.classList.contains("k-tree-grid__row") ? ROW : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
    configurable: true,
    get(this: HTMLElement) {
      return isTree(this) ? (CHILDREN + 2) * ROW : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, "scrollTop", {
    configurable: true,
    get(this: HTMLElement) {
      return offsets.get(this) ?? 0;
    },
    set(this: HTMLElement, top: number) {
      offsets.set(this, Math.max(0, top));
    },
  });
});

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
  setFetch(null);
  document.body.innerHTML = "";
  for (const name of ["clientHeight", "offsetHeight", "scrollHeight", "scrollTop"])
    delete (HTMLElement.prototype as unknown as Record<string, unknown>)[name];
});

/** The renders of each kind of component since the last `clear`, and their mounts. */
const renders = new Map<string, number>();
const mounts = new Map<string, number>();
const nameOf = (self: { $options: { __name?: string; name?: string } }) =>
  String(self.$options.__name ?? self.$options.name ?? "?");

/** The topography mounted, its container open, editable by its user. */
async function mounted() {
  setFetch((async (input: RequestInfo | URL) => {
    const url = new URL(String(input), "http://x");
    const path = url.pathname;
    const body = path.startsWith("/v1/labels")
      ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
      : path === "/v1/domains"
        ? { items: [{ id: D1, slug: "essai", href: `/v1/domains/${D1}` }], links: {} }
        : path === `/v1/domains/${D1}/roots`
          ? { items: [prefix(C1, "10.0.0.0/8", "container")], links: {} }
          : path === `/v1/prefixes/${C1}/children`
            ? { items: Array.from({ length: CHILDREN }, (_, i) => child(i + 1)), links: {} }
            : path.endsWith("/children")
              ? { items: [], links: {} }
              : { counts: [], items: [], links: {}, sections: [] };
    return new Response(JSON.stringify(body), {
      headers: { "Content-Type": "application/json", ETag: '"v1"' },
    });
  }) as typeof fetch);
  await loadSession(async (url) =>
    url === "/v1/me"
      ? ({
          identity: "alice",
          kind: "person",
          global_permissions: [
            "core.domain.read",
            "core.prefix.read",
            "core.prefix.edit",
            "core.range.edit",
            "core.address.edit",
          ],
          bindings: [],
        } as unknown as Me)
      : null,
  );
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/", component: RouterView, children: frameRoutes }],
  });
  await router.push(`/topology?domain=${D1}`);
  const root = document.body.appendChild(document.createElement("div"));
  app = createApp({ render: () => h(RouterView) });
  app.mixin({
    mounted() {
      const name = nameOf(this);
      mounts.set(name, (mounts.get(name) ?? 0) + 1);
    },
    updated() {
      const name = nameOf(this);
      renders.set(name, (renders.get(name) ?? 0) + 1);
    },
  });
  app.use(createShellI18n("fr")).use(router);
  app.mount(root);
  await router.isReady();
  const container = () =>
    root.querySelector<HTMLElement>(`[role='row'][data-node="${C1}"] [data-column='']`);
  await expect.poll(() => container()).not.toBeNull();
  container()!.focus();
  container()!.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
  await expect
    .poll(() => root.querySelectorAll("[role='row'][aria-level='2']").length)
    .toBeGreaterThan(0);
  const tree = root.querySelector<HTMLElement>("[role='treegrid']")!;
  expect(tree.dataset.virtual).toBe("true");
  await settle();
  return { root, tree };
}

async function settle(): Promise<void> {
  for (let i = 0; i < 4; i++) await nextTick();
}

/** The tree scrolled to `top`, as a frame of the bench does. */
async function scrollTo(tree: HTMLElement, top: number): Promise<void> {
  tree.scrollTop = top;
  tree.dispatchEvent(new Event("scroll"));
  await settle();
}

function clear() {
  renders.clear();
  mounts.clear();
  calls.clear();
}
/** What a step rendered and asked: rows, the host's functions by the name of their prop. */
function work() {
  return {
    rows: renders.get("TreeGridRow") ?? 0,
    mounted: mounts.get("TreeGridRow") ?? 0,
    calls: Object.fromEntries(calls),
  };
}
/** The columns of the tree: the prefix, its occupancy, its nature (no plugin's column). */
const COLUMNS = 3;
/** What the host may be asked for each row that comes: once per cell, the row's twice. */
function bounded(done: ReturnType<typeof work>, rows: number, what: string): void {
  for (const name of ["cell-state", "editable", "readonly-reason"])
    expect(done.calls[name] ?? 0, `${what}: ${name}`).toBeLessThanOrEqual(rows * COLUMNS);
  // Its state, and its words.
  expect(done.calls["node-state"] ?? 0, `${what}: node-state`).toBeLessThanOrEqual(rows * 2);
  for (const name of ["add-label", "add-blocked"])
    expect(done.calls[name] ?? 0, `${what}: ${name}`).toBeLessThanOrEqual(rows);
}

describe("the tree of the topography renders the rows that come, no other (task 021-22e-c)", () => {
  // Before: each step asked the host of every cell of the 43 rows rendered (the table of
  // the hidden reasons) and of the rows that came again for each attribute: 268 calls of
  // `cellState`, 234 of `editable`, 122 of `readonlyReason` for 17 rows.
  it("a step of the scroll: the rows that come, the host asked once for their cells", async () => {
    const { tree } = await mounted();
    for (let step = 1; step <= 20; step++) {
      clear();
      await scrollTo(tree, step * STEP * ROW);
      const done = work();
      // A row may straddle the edge of the window: one more.
      expect(done.rows + done.mounted, `step ${step}`).toBeLessThanOrEqual(STEP + 1);
      bounded(done, STEP + 1, `step ${step}`);
    }
    // Back over rows already seen: nothing asked again of the host.
    clear();
    await scrollTo(tree, 19 * STEP * ROW);
    const back = work();
    expect(back.rows).toBeLessThanOrEqual(STEP + 1);
    expect(back.calls["cell-state"] ?? 0).toBe(0);
    expect(back.calls["editable"] ?? 0).toBe(0);
  });

  // Before: each row read the roving stop and rendered again at each step (43 rows).
  it("a step of the focus: the row it leaves and the row it reaches", async () => {
    const { root, tree } = await mounted();
    const cell = (n: number) =>
      tree.querySelector<HTMLElement>(
        `[role='row'][aria-level='2'][aria-posinset="${n}"] [data-column='']`,
      )!;
    cell(1).focus();
    await settle();
    for (let step = 1; step <= 5; step++) {
      clear();
      document.activeElement!.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
      );
      await settle();
      expect(document.activeElement).toBe(cell(step + 1));
      const done = work();
      expect(done.rows + done.mounted, `step ${step}`).toBeLessThanOrEqual(2);
      // The « + » of the two rows asked again with them; nothing of their cells.
      bounded(done, 2, `step ${step}`);
      expect(done.calls["cell-state"] ?? 0, `step ${step}`).toBe(0);
    }
    // One tab stop in the tree, on the cell of the focus.
    expect(root.querySelectorAll("[role='treegrid'] [tabindex='0']")).toHaveLength(1);
    expect(cell(6).tabIndex).toBe(0);
  });
});
