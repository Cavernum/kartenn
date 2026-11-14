// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, h, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { keepColumns } from "../src/views/viewSettings.ts";

// The properties of the rows of a list read by window (task 021-11p10-c): a page of a
// hundred prefixes, sixty rows in view and a column of a property chosen, reads them all in
// one request `GET /v1/prefixes/properties?id=…`. Before, one request per row (a hundred),
// four at a time through the gate of the shell.

const ROWS = 100;
const id = (i: number) => `0190a000-0000-7000-8000-${String(i).padStart(12, "0")}`;
const prefix = (i: number) => ({
  id: id(i),
  value: `10.0.${i}.0/24`,
  nature: "network",
  version: "v1",
  links: { self: `/v1/prefixes/${id(i)}` },
});
const declarations = [
  { id: "d-zone", key: "custom.zone", type: "text", cardinality: "single", carriers: ["prefix"] },
];

/**
 * A layout where sixty rows fill the window (happy-dom lays nothing out, and its grid says
 * no row in view): each row of the body placed by its index, the window 768 px high.
 */
function sixtyRowsInView() {
  const height = window.innerHeight / 60;
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement,
  ) {
    const index = this.getAttribute("role") === "row" ? Number(this.dataset.index) : NaN;
    return Number.isFinite(index)
      ? DOMRect.fromRect({ x: 0, y: index * height, width: 800, height })
      : DOMRect.fromRect({ x: 0, y: 0, width: 800, height: 0 });
  });
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(window.innerHeight);
}

let app: App | undefined;
afterEach(() => {
  vi.restoreAllMocks();
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
  setFetch(null);
  document.body.innerHTML = "";
  localStorage.clear();
});

const json = (body: unknown) =>
  new Response(JSON.stringify(body), { headers: { "Content-Type": "application/json" } });

describe("the properties of a list read by window (task 021-11p10-c)", () => {
  it("a page of 100 rows, 60 in view: one request of properties", async () => {
    const sent: string[] = [];
    sixtyRowsInView();
    keepColumns("u-1", "prefixes", { "prop.custom.zone": true });
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      sent.push(url);
      if (url.startsWith("/v1/prefixes/properties?")) {
        const ids = new URL(url, "http://localhost").searchParams.getAll("id");
        const items = ids.map((of) => ({
          id: of,
          properties: { "custom.zone": `zone ${of.slice(-3)}` },
          sources: {},
          masked: [],
        }));
        return json({ items });
      }
      if (/^\/v1\/prefixes\/[^/?]+\/properties/.test(url)) {
        return json({ properties: { "custom.zone": "seule" }, sources: {}, masked: [] });
      }
      if (url.startsWith("/v1/labels"))
        return json({ lang: "fr", default_lang: "fr", revision: "1", spaces: {} });
      if (url.startsWith("/v1/property-declarations"))
        return json({ items: declarations, links: {} });
      if (url.startsWith("/v1/prefixes"))
        return json({ items: Array.from({ length: ROWS }, (_, i) => prefix(i + 1)), links: {} });
      return json({ counts: [], items: [], links: {}, sections: [] });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "u-1",
            kind: "person",
            global_permissions: ["core.prefix.read"],
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
    await router.push("/prefixes");
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(document.body.appendChild(document.createElement("div")));
    await router.isReady();
    const cell = (i: number) =>
      document.querySelector(`[data-row="${id(i)}"][data-column="prop.custom.zone"]`);
    await expect.poll(() => cell(60)?.textContent).toContain(`zone ${id(60).slice(-3)}`);
    await expect.poll(() => cell(1)?.textContent).toContain(`zone ${id(1).slice(-3)}`);
    const properties = sent.filter((url) => url.includes("/properties"));
    expect(properties).toHaveLength(1);
    expect(new URL(properties[0]!, "http://localhost").searchParams.getAll("id")).toHaveLength(
      ROWS,
    );
  });
});
