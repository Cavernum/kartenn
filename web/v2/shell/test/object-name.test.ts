// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, type App, type VNode } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import AnomalyGrid from "../src/views/anomalies/AnomalyGrid.vue";
import type { Anomaly } from "../src/views/anomalies/anomaly.ts";
import PropertyValue from "../src/views/record/PropertyValue.vue";

// The name of an object the server summarised (task 021-11p10-d1, REQ-006-42): its `label`,
// nothing read, no read even set up (no query, no watch of the view); a summary without
// `label` but with its `href` (a type of module that tells no key), a link named by its
// identifier; with neither (an object the caller may not read, or one gone: `label`, `slug`
// and `href` left out together), its identifier in fixed pitch, no link, said unreadable to
// a screen reader. An external reference (`{system, id}`) is not a summary. An identifier
// alone still reads (`gate.test.ts`).

/** The identifier of the `n`-th object. */
const idOf = (n: number) => `0b5e3c1a-0000-4000-8000-${String(n).padStart(12, "0")}`;

describe("the name of an object summarised", () => {
  let app: App | undefined;
  /** Every URL asked, the labels of the shell aside. */
  let sent: string[];
  /** The watches of the view set up. */
  let watches: number;

  beforeEach(() => {
    sent = [];
    watches = 0;
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
    vi.stubGlobal(
      "IntersectionObserver",
      class {
        constructor() {
          watches++;
        }
        observe(): void {}
        unobserve(): void {}
        disconnect(): void {}
      },
    );
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
    vi.unstubAllGlobals();
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
  const labelled = (n: number) => ({
    id: idOf(n),
    kind: "prefix",
    label: `192.0.2.${n}/32`,
    href: `/v1/prefixes/${idOf(n)}`,
  });

  async function mounted(render: () => VNode | VNode[]): Promise<HTMLElement> {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    for (let i = 0; i < 10; i++) await new Promise((resolve) => setTimeout(resolve));
    await nextTick();
    return root;
  }
  const grid = (anomalies: Anomaly[]) => () =>
    h(AnomalyGrid, { label: "Anomalies", anomalies, withObject: true });
  /** The queries in the cache, those of the labels of the shell aside. */
  const queries = () =>
    queryClient
      .getQueryCache()
      .getAll()
      .filter((query) => !JSON.stringify(query.queryKey).includes("labels"));

  it("66 rows of anomalies named by their labels: no read of an object", async () => {
    const root = await mounted(grid(Array.from({ length: 66 }, (_, n) => anomaly(n, labelled(n)))));
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("192.0.2.65/32");
    for (const n of [0, 33, 65]) {
      const link = [...root.querySelectorAll("a")].find((a) => a.textContent === `192.0.2.${n}/32`);
      expect(link?.getAttribute("href")).toBe(`/prefixes/${idOf(n)}`);
      expect(link?.classList.contains("k-mono")).toBe(false);
    }
    expect(sent.filter((url) => url.startsWith("/v1/prefixes"))).toEqual([]);
  }, 10_000);

  it("66 summaries: no query and no watch of the view set up", async () => {
    const root = await mounted(() =>
      Array.from({ length: 66 }, (_, n) =>
        h(PropertyValue, { property: "core.owner", value: labelled(n), type: "ref" }),
      ),
    );
    expect(root.querySelectorAll(".k-object-name")).toHaveLength(66);
    expect(root.textContent).toContain("192.0.2.65/32");
    expect(queries()).toEqual([]);
    expect(watches).toBe(0);
    expect(sent).toEqual([]);
  }, 10_000);

  it("neither label nor URL: no read, its identifier in fixed pitch, said unreadable", async () => {
    const root = await mounted(grid([anomaly(0, { id: idOf(0) })]));
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("message 0");
    const name = root.querySelector(".k-object-name")!;
    expect(name.querySelector("a")).toBeNull();
    expect(name.querySelector(".k-mono")?.firstChild?.textContent).toBe(idOf(0));
    expect(name.querySelectorAll(".k-visually-hidden")).toHaveLength(1);
    expect(name.querySelector(".k-visually-hidden")?.textContent).toBe(
      " (illisible pour vous, ou absent)",
    );
    expect(sent.filter((url) => url.includes(idOf(0)))).toEqual([]);
  }, 10_000);

  it("its URL without label (a type of module that tells no key): a link by its identifier", async () => {
    const root = await mounted(
      grid([anomaly(0, { id: idOf(0), kind: "prefix", href: `/v1/prefixes/${idOf(0)}` })]),
    );
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("message 0");
    const link = root.querySelector(".k-object-name a")!;
    expect(link.getAttribute("href")).toBe(`/prefixes/${idOf(0)}`);
    expect(link.textContent).toBe(idOf(0));
    expect(link.classList.contains("k-mono")).toBe(true);
    expect(root.querySelector(".k-object-name .k-visually-hidden")).toBeNull();
    expect(sent.filter((url) => url.includes(idOf(0)))).toEqual([]);
  }, 10_000);

  it("an external reference: `<system>:<identifier>`, not a summary", async () => {
    const root = await mounted(() =>
      h(PropertyValue, {
        property: "app.ticket",
        value: { system: "tracker", id: "T-12", label: "Ticket T-12" },
        type: "external",
      }),
    );
    expect(root.textContent).toBe("tracker:T-12");
    expect(root.querySelector(".k-object-name")).toBeNull();
  }, 10_000);
});
