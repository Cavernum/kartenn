// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";

// The work of a list's grid when its focus moves (tasks 021-22q, 22o-b; budgets « Arrow down
// in a grid », « navigations to a prefetched target »): a step of the arrow, or a press on
// the link of a cell (its `focusin`), renders the row the focus leaves and the row it
// reaches, never the others, and asks the gate of each of their cells (`editability`) a
// bounded number of times. Before the rows were components of their own, each step
// rendered the whole grid: on a page of 50 addresses, the 100 links of its cells and 552
// calls of `editability`, the same at main e86f6af1 and 24622281 (the arrow's work did not
// change between the two passes of the bench, task 22q).

const calls = { editability: 0 };
vi.mock("../src/views/edit/fields.ts", async (original) => {
  const module = await original<typeof import("../src/views/edit/fields.ts")>();
  return {
    ...module,
    editability: (...args: Parameters<typeof module.editability>) => {
      calls.editability += 1;
      return module.editability(...args);
    },
  };
});

const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";
const ROWS = 50;
/** The columns of the list of addresses (`collections.ts`). */
const COLUMNS = 4;
/**
 * The calls of the gate a move of the focus may make: for each cell of its two rows, its
 * editor, then why not; then the foot's, of the cell of the focus (why, what it writes).
 */
const GATE = 2 * 2 * COLUMNS + 2;
const address = (i: number) => ({
  id: `0b5e3c1a-0000-4000-8000-${String(i).padStart(12, "0")}`,
  value: `198.51.100.${i}`,
  usage: "host",
  public: false,
  version: "0b5e3c1a-0000-4000-8000-0000000000f1",
  domain: { id: D1, slug: "essai", href: `/v1/domains/${D1}` },
});

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
  setFetch(null);
  document.body.innerHTML = "";
});

/** The renders of each kind of component since the last `clear`. */
const renders = new Map<string, number>();

/** The list of addresses mounted, one page of 50 rows, editable by its user. */
async function mounted() {
  setFetch((async (input: RequestInfo | URL) => {
    const url = String(input);
    const body = url.startsWith("/v1/labels")
      ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
      : url.startsWith("/v1/addresses")
        ? { items: Array.from({ length: ROWS }, (_, i) => address(i + 1)), links: {} }
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
          global_permissions: ["core.address.read", "core.address.edit"],
          bindings: [],
        } as unknown as Me)
      : null,
  );
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/", component: RouterView, children: frameRoutes }],
  });
  await router.push("/addresses");
  const root = document.body.appendChild(document.createElement("div"));
  app = createApp({ render: () => h(RouterView) });
  app.mixin({
    updated() {
      const name = String(this.$options.__name ?? this.$options.name ?? "?");
      renders.set(name, (renders.get(name) ?? 0) + 1);
    },
  });
  app.use(createShellI18n("fr")).use(router);
  app.mount(root);
  await router.isReady();
  await expect
    .poll(() => root.querySelectorAll("[role='gridcell'][data-column='usage']").length)
    .toBe(ROWS);
  return root;
}

const cell = (root: HTMLElement, n: number, column: string) =>
  root.querySelector<HTMLElement>(
    `[role='gridcell'][data-row="${address(n).id}"][data-column="${column}"]`,
  )!;

/** What a move of the focus rendered and asked: rows, links, calls of the gate. */
function work() {
  return {
    rows: renders.get("DataGridRow") ?? 0,
    links: renders.get("KLink") ?? 0,
    editability: calls.editability,
  };
}
function clear() {
  renders.clear();
  calls.editability = 0;
}

describe("a list's grid renders the rows its focus leaves and reaches (tasks 021-22q, 22o-b)", () => {
  it("a step of the arrow: two rows, their links, their cells' gate", async () => {
    const root = await mounted();
    cell(root, 1, "usage").focus();
    await nextTick();
    for (let step = 0; step < 10; step++) {
      clear();
      document.activeElement!.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
      );
      await nextTick();
      await nextTick();
      expect(document.activeElement).toBe(cell(root, step + 2, "usage"));
      const done = work();
      expect(done.rows, `step ${step}`).toBeLessThanOrEqual(2);
      // Each row's two links (its value, its domain).
      expect(done.links, `step ${step}`).toBeLessThanOrEqual(4);
      expect(done.editability, `step ${step}`).toBeLessThanOrEqual(GATE);
    }
    // The roving stop followed the focus: one tab stop in the grid.
    expect(root.querySelectorAll("[role='grid'] [tabindex='0']")).toHaveLength(1);
    expect(cell(root, 11, "usage").tabIndex).toBe(0);
  });

  it("a press on the link of a cell: its row and the row of the stop", async () => {
    const root = await mounted();
    cell(root, 3, "usage").focus();
    await nextTick();
    clear();
    const link = cell(root, 30, "value").querySelector<HTMLAnchorElement>("a")!;
    link.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    link.focus();
    await nextTick();
    await nextTick();
    const done = work();
    expect(done.rows).toBeLessThanOrEqual(2);
    expect(done.links).toBeLessThanOrEqual(4);
    expect(done.editability).toBeLessThanOrEqual(GATE);
    // The stop on the cell of the link, the link out of the tab order.
    expect(cell(root, 30, "value").tabIndex).toBe(0);
    expect(cell(root, 3, "usage").tabIndex).toBe(-1);
    expect(link.tabIndex).toBe(-1);
  });
});
