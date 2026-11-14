// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import type { App } from "vue";
import type { RouteRecordRaw } from "vue-router";

// The words of the screens read with them (task 22n; `views/words.ts`): each screen of the
// frame and each panel it opens awaits them, so that none renders a key in place of its
// words. `test/setup.ts` reads them before every file of tests: here they are not, the
// modules read again once the registry is reset, the catalog's read held.

/** The routes of `records` whose screen is a chunk read with its route. */
function lazy(records: readonly RouteRecordRaw[]): RouteRecordRaw[] {
  return records.flatMap((record) => [
    ...(typeof record.component === "function" ? [record] : []),
    ...lazy(record.children ?? []),
  ]);
}

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  document.body.innerHTML = "";
  vi.doUnmock("../src/views/i18n/index.ts");
  vi.resetModules();
});

describe("the words of the screens", () => {
  it("are awaited by every screen of the frame and every panel it opens", async () => {
    const { frameRoutes } = await import("../src/frame/routes.ts");
    const { PANEL_LOADERS } = await import("../src/frame/panels.ts");
    const { isWorded } = await import("../src/views/words.ts");
    // The views of the plugins excepted: their words are their plugins' (`plugin.<name>`).
    const screens = lazy(frameRoutes).filter((record) => record.name !== "plugin-view");
    // The lists, the records, the changesets, the drafts and the administration.
    expect(screens.length).toBeGreaterThan(30);
    for (const record of screens)
      expect(isWorded(record.component), String(record.name)).toBe(true);
    for (const [name, load] of Object.entries(PANEL_LOADERS))
      expect(isWorded(load), name).toBe(true);
  });

  it("show a screen and a panel only once read, a lost read tried again at the next navigation", async () => {
    vi.resetModules();
    let release!: () => void;
    const held = new Promise<void>((resolve) => (release = resolve));
    let reads = 0;
    // The catalog of the screens: lost at its first read, held at the second.
    vi.doMock("../src/views/i18n/index.ts", async () => {
      reads += 1;
      if (reads === 1) throw new Error("catalog lost");
      await held;
      return vi.importActual("../src/views/i18n/index.ts");
    });
    const { setFetch } = await import("@kartenn/sdk/host");
    // Refused at once, never tried again: the problem is said without waiting.
    const { queryClient } = await import("@kartenn/sdk");
    queryClient.setDefaultOptions({ queries: { retry: false } });
    // Every read refused: the screen says its problem in its own words.
    setFetch(
      (async () =>
        new Response(JSON.stringify({ type: "about:blank", status: 503, title: "Unavailable" }), {
          status: 503,
          headers: { "Content-Type": "application/problem+json" },
        })) as typeof fetch,
    );
    const { createApp, h, shallowRef } = await import("vue");
    const { createMemoryHistory, createRouter, RouterView } = await import("vue-router");
    const { frameRoutes } = await import("../src/frame/routes.ts");
    const { ClosePanel } = await import("../src/frame/panels.ts");
    const { createShellI18n } = await import("../src/i18n/index.ts");
    const screens = (
      await vi.importActual<typeof import("../src/views/i18n/index.ts")>(
        "../src/views/i18n/index.ts",
      )
    ).default.en;

    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
    const root = document.createElement("div");
    document.body.append(root);
    const request = {
      mode: "close" as const,
      collection: "prefixes" as const,
      id: "0b5e3c1a-0000-4000-8000-000000000003",
      serial: 1,
      opener: null,
    };
    // The panel asked for once the first read is lost, as the frame opens it on demand.
    const panel = shallowRef(false);
    app = createApp({
      render: () => [
        h("main", h(RouterView)),
        h("aside", panel.value ? [h(ClosePanel, { request })] : []),
      ],
    });
    app.use(createShellI18n("en")).use(router);
    app.mount(root);

    // The first navigation fails with the read of its words, nothing shown.
    await expect(router.push("/")).rejects.toThrow();
    expect(reads).toBe(1);
    // The next one reads them again; until they come, neither the screen nor the panel.
    const next = router.push("/");
    panel.value = true;
    await expect.poll(() => reads).toBe(2);
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(root.querySelector("main")!.textContent).toBe("");
    expect(root.querySelector("aside")!.textContent).toBe("");

    release();
    await next;
    // Their words, never a key: the dashboard's problem, the closure's title or read error.
    await expect
      .poll(() => root.querySelector("main")!.textContent)
      .toContain(screens.views.home.error);
    await expect
      .poll(() => root.querySelector("aside")!.textContent)
      .toMatch(/^Close: |could not be read/);
    expect(root.textContent).not.toMatch(/\b(views|edit)\.[a-z]+\.[a-zA-Z]/);
  }, 20_000);
});
