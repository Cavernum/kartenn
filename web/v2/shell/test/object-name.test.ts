// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import AnomalyGrid from "../src/views/anomalies/AnomalyGrid.vue";
import type { Anomaly } from "../src/views/anomalies/anomaly.ts";

// The name of an object the server summarised (task 021-11p10-d1, REQ-006-42): its `label`,
// no read of the object; a summary without one (an object the caller may not read, or gone)
// its identifier in fixed pitch, no read and no link either. An identifier alone still
// reads (`gate.test.ts`).

/** The identifier of the `n`-th object. */
const idOf = (n: number) => `0b5e3c1a-0000-4000-8000-${String(n).padStart(12, "0")}`;

describe("the object of an anomaly, summarised", () => {
  let app: App | undefined;
  /** Every URL asked, the labels of the shell aside. */
  let sent: string[];

  beforeEach(() => {
    sent = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      if (!url.startsWith("/v1/labels")) sent.push(url);
      const body = url.startsWith("/v1/labels")
        ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
        : { items: [], links: {} };
      return new Response(JSON.stringify(body), {
        headers: { "Content-Type": "application/json", ETag: '"v1"' },
      });
    }) as typeof fetch);
    // A layout for the grid, which happy-dom does not compute: a scroller tall enough for
    // every row, so that each one is rendered.
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(2400);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      const top = this.dataset.index === undefined ? 0 : 40 + Number(this.dataset.index) * 32;
      return DOMRect.fromRect({ x: 0, y: top, width: 1280, height: top ? 32 : 2400 });
    });
  });
  afterEach(() => {
    app?.unmount();
    app = undefined;
    vi.restoreAllMocks();
    queryClient.clear();
    resetContext();
    document.body.replaceChildren();
  });

  const anomaly = (n: number, object: Anomaly["object"]): Anomaly =>
    ({
      id: `a${n}`,
      rule: "core.addr.documentation",
      message: `message ${n}`,
      object,
      opened_at: "2026-01-01T00:00:00Z",
      links: { acknowledgement: `/v1/anomalies/a${n}/acknowledgement` },
    }) as Anomaly;

  async function mounted(anomalies: Anomaly[]): Promise<HTMLElement> {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({
      render: () => h(AnomalyGrid, { label: "Anomalies", anomalies, withObject: true }),
    });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    for (let i = 0; i < 10; i++) await new Promise((resolve) => setTimeout(resolve));
    await nextTick();
    return root;
  }

  it("66 rows named by their labels: no read of an object", async () => {
    const anomalies = Array.from({ length: 66 }, (_, n) =>
      anomaly(n, {
        id: idOf(n),
        kind: "prefix",
        label: `192.0.2.${n}/32`,
        href: `/v1/prefixes/${idOf(n)}`,
      }),
    );
    const root = await mounted(anomalies);
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("192.0.2.65/32");
    for (const n of [0, 33, 65]) {
      const link = [...root.querySelectorAll("a")].find((a) => a.textContent === `192.0.2.${n}/32`);
      expect(link?.getAttribute("href")).toBe(`/prefixes/${idOf(n)}`);
      expect(link?.classList.contains("k-mono")).toBe(false);
      expect(link?.querySelector(".k-visually-hidden")).toBeNull();
    }
    expect(sent.filter((url) => url.startsWith("/v1/prefixes"))).toEqual([]);
  }, 10_000);

  it("a summary without label: no read, its identifier in fixed pitch, no link", async () => {
    const root = await mounted([anomaly(0, { id: idOf(0) })]);
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("message 0");
    const name = root.querySelector(".k-object-name")!;
    expect(name.querySelector("a")).toBeNull();
    // Shown its identifier, said unreadable to a screen reader.
    expect(name.querySelector(".k-mono")?.firstChild?.textContent).toBe(idOf(0));
    expect(name.querySelector(".k-visually-hidden")?.textContent).toBe(
      " (objet que vous ne lisez pas, ou absent)",
    );
    expect(sent.filter((url) => url.includes(idOf(0)))).toEqual([]);
  }, 10_000);

  it("a summary without label but with its URL: a link, said unreadable", async () => {
    const root = await mounted([anomaly(0, { id: idOf(0), href: `/v1/prefixes/${idOf(0)}` })]);
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("message 0");
    const link = root.querySelector(".k-object-name a")!;
    expect(link.getAttribute("href")).toBe(`/prefixes/${idOf(0)}`);
    expect(link.classList.contains("k-mono")).toBe(true);
    expect(link.querySelector(".k-visually-hidden")).not.toBeNull();
    expect(sent.filter((url) => url.includes(idOf(0)))).toEqual([]);
  }, 10_000);
});
