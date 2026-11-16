// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, h, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";

// The objects the VLAN name, by the summaries the server gives (task 021-11p10-d2,
// REQ-006-42): the group of a VLAN and the anchor of a group, in their lists, and the VLAN of
// a network in the column of the prefixes, by their `label`, nothing read; a summary
// without one (an object the reader may not read, or one gone), its identifier in fixed
// pitch, said unreadable. Before, one read of the record per cell (`ObjectName`).

const ROWS = 30;
const idOf = (n: number, tail = "0") =>
  `0190a000-0000-7000-8000-${tail}${String(n).padStart(11, "0")}`;
const G1 = idOf(1, "9");
const S1 = idOf(2, "9");
const D1 = idOf(3, "9");
/** An object the reader may not read: its identifier alone. */
const HIDDEN = idOf(4, "9");

const vlan = (i: number) => ({
  id: idOf(i),
  vid: 100 + i,
  name: `vlan-${i}`,
  status: "active",
  version: "v1",
  group:
    i === ROWS
      ? { id: HIDDEN }
      : { id: G1, slug: "campus", label: "campus", href: `/v1/vlan-groups/${G1}` },
  links: { self: `/v1/vlans/${idOf(i)}` },
});
const group = (i: number) => ({
  id: idOf(i),
  slug: `groupe-${i}`,
  vid_min: 100,
  vid_max: 199,
  version: "v1",
  anchor:
    i === ROWS
      ? { entity: { id: HIDDEN } }
      : { site: { id: S1, slug: "nord", label: "nord", href: `/v1/sites/${S1}` } },
  links: { self: `/v1/vlan-groups/${idOf(i)}` },
});
const prefix = (i: number) => ({
  id: idOf(i),
  value: `10.0.${i}.0/24`,
  nature: "network",
  public: false,
  version: "v1",
  domain: { id: D1, slug: "essai", label: "essai", href: `/v1/domains/${D1}` },
  links: { self: `/v1/prefixes/${idOf(i)}` },
});
/** The VLAN the property `core.vlan` of the `i`-th network names (the last, unreadable). */
const vlanValue = (i: number) =>
  i === ROWS
    ? { id: HIDDEN }
    : { id: idOf(i, "5"), label: `${100 + i} · hotes`, href: `/v1/vlans/${idOf(i, "5")}` };

/** A read of the record of one object (`/v1/<collection>/<uuid>`). */
const OBJECT_READ = /^\/v1\/[a-z-]+\/[0-9a-f]{8}-[0-9a-f-]{27}(?:[?/]|$)/;

/** Every row laid out in a tall window (happy-dom lays nothing out). */
function everyRowInView() {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement,
  ) {
    const index = this.getAttribute("role") === "row" ? Number(this.dataset.index) : NaN;
    return Number.isFinite(index)
      ? DOMRect.fromRect({ x: 0, y: index * 20, width: 1280, height: 20 })
      : DOMRect.fromRect({ x: 0, y: 0, width: 1280, height: 0 });
  });
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(ROWS * 40);
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

/** The list at `path` mounted against a fake API; every URL it asked. */
async function mounted(path: string): Promise<string[]> {
  const sent: string[] = [];
  everyRowInView();
  setFetch((async (input: RequestInfo | URL) => {
    const url = String(input);
    sent.push(url);
    const rows = (of: (i: number) => object) =>
      json({ items: Array.from({ length: ROWS }, (_, i) => of(i + 1)), links: {} });
    if (url.startsWith("/v1/prefixes/properties?")) {
      const ids = new URL(url, "http://localhost").searchParams.getAll("id");
      const items = ids.map((of) => ({
        id: of,
        properties: { "core.vlan": vlanValue(Number(of.slice(-11))) },
        sources: {},
        masked: [],
      }));
      return json({ items });
    }
    if (url.startsWith("/v1/labels"))
      return json({ lang: "fr", default_lang: "fr", revision: "1", spaces: {} });
    if (url.startsWith("/v1/vlans?") || url === "/v1/vlans") return rows(vlan);
    if (url.startsWith("/v1/vlan-groups?") || url === "/v1/vlan-groups") return rows(group);
    if (url.startsWith("/v1/prefixes?") || url === "/v1/prefixes") return rows(prefix);
    return json({ counts: [], items: [], links: {}, sections: [] });
  }) as typeof fetch);
  await loadSession(async (url) =>
    url === "/v1/me"
      ? ({
          identity: "u-1",
          kind: "person",
          global_permissions: ["core.prefix.read", "core.vlan.read", "core.vlan_group.read"],
          bindings: [],
        } as unknown as Me)
      : null,
  );
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/", component: RouterView, children: frameRoutes }],
  });
  await router.push(path);
  app = createApp({ render: () => h(RouterView) });
  app.use(createShellI18n("fr")).use(router);
  app.mount(document.body.appendChild(document.createElement("div")));
  await router.isReady();
  return sent;
}

const cell = (i: number, column: string) =>
  document.querySelector<HTMLElement>(`[data-row="${idOf(i)}"][data-column="${column}"]`);
/** The whole shell mounted: slow when every file of the suite runs at once. */
const shown = { timeout: 10_000 };

/** An unreadable object: its identifier in fixed pitch, no link, said so in words. */
function expectUnreadable(at: HTMLElement | null) {
  expect(at?.querySelector("a")).toBeNull();
  expect(at?.querySelector(".k-mono")?.textContent).toContain(HIDDEN);
  expect(at?.querySelector(".k-visually-hidden")?.textContent).toContain("illisible");
}

describe("the objects the VLAN name, by their summaries (task 021-11p10-d2)", () => {
  it("the list of VLAN: each group by its label, no object read", async () => {
    const sent = await mounted("/vlans");
    await expect.poll(() => cell(1, "group")?.textContent ?? "", shown).toContain("campus");
    await expect.poll(() => cell(ROWS, "group")?.textContent ?? "", shown).toContain(HIDDEN);
    expect(cell(1, "group")?.querySelector("a")?.getAttribute("href")).toBe(`/vlan-groups/${G1}`);
    expectUnreadable(cell(ROWS, "group"));
    expect(sent.filter((url) => OBJECT_READ.test(url))).toEqual([]);
  });

  it("the list of groups: each anchor by its kind and label, no object read", async () => {
    const sent = await mounted("/vlan-groups");
    await expect.poll(() => cell(1, "anchor")?.textContent ?? "", shown).toContain("nord");
    await expect.poll(() => cell(ROWS, "anchor")?.textContent ?? "", shown).toContain(HIDDEN);
    expect(cell(1, "anchor")?.querySelector("a")?.getAttribute("href")).toBe(`/sites/${S1}`);
    expectUnreadable(cell(ROWS, "anchor"));
    expect(sent.filter((url) => OBJECT_READ.test(url))).toEqual([]);
  });

  it("the column VLAN of the prefixes: one request of properties, no VLAN read", async () => {
    const sent = await mounted("/prefixes");
    await expect.poll(() => cell(1, "vlan")?.textContent ?? "", shown).toContain("101 · hotes");
    await expect.poll(() => cell(ROWS, "vlan")?.textContent ?? "", shown).toContain(HIDDEN);
    expect(cell(1, "vlan")?.querySelector("a")?.getAttribute("href")).toBe(
      `/vlans/${idOf(1, "5")}`,
    );
    expectUnreadable(cell(ROWS, "vlan"));
    // The domain of each prefix by its label too.
    expect(cell(1, "domain")?.textContent).toContain("essai");
    expect(sent.filter((url) => url.includes("/properties"))).toHaveLength(1);
    expect(sent.filter((url) => OBJECT_READ.test(url))).toEqual([]);
  });
});
