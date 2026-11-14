// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { collectionById } from "../src/views/collections.ts";
import { valueOriginOf } from "../src/views/record/provenance.ts";

// The column « Source » of the properties and the notice of the gaps above them (task
// 021-11p4, Q-021-23; REQ-021-9, REQ-021-20; maquette E2): each sort of source read from
// `sources` of `Resolved` — direct, inherited from an origin named, inherited from an
// object the caller may not read, the default of a source, derived by a module, masked —
// then the gaps of the object read by `GET /v1/property-gaps?object=`, the notice absent
// without any, filled with one, an error said with its code.

const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const S1 = "0b5e3c1a-0000-4000-8000-0000000000b1";
const G1 = "0b5e3c1a-0000-4000-8000-0000000000f1";
const node = (id: string, kind: "prefix" | "site") => ({ id, kind, href: `/v1/${kind}s/${id}` });

describe("the source of a value", () => {
  it("is read from its entry of `sources`", () => {
    const self = node(P1, "prefix");
    expect(valueOriginOf(undefined)).toBeNull();
    expect(valueOriginOf({ origin: self, inherited: false, provenance: "input" })).toEqual({
      kind: "direct",
    });
    expect(valueOriginOf({ origin: self, inherited: false, provenance: null })).toEqual({
      kind: "direct",
    });
    const site = node(S1, "site");
    expect(valueOriginOf({ origin: site, inherited: true, provenance: "input" })).toEqual({
      kind: "inherited",
      origin: site,
    });
    // An origin the caller may not read: inherited, never named, its provenance not said.
    expect(valueOriginOf({ origin: null, inherited: true, provenance: "source:cmdb@r12" })).toEqual(
      { kind: "unreadable" },
    );
    expect(
      valueOriginOf({ origin: self, inherited: false, provenance: "source:cmdb@r12" }),
    ).toEqual({
      kind: "derived",
      provenance: { kind: "source", code: "source:cmdb@r12", source: "cmdb", revision: "r12" },
    });
    expect(
      valueOriginOf({ origin: self, inherited: false, provenance: "module:dhcp#lease" }),
    ).toEqual({
      kind: "derived",
      provenance: { kind: "module", module: "dhcp", motif: "lease" },
    });
  });
});

describe("the tab « Propriétés », its sources and its gaps", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    document.body.innerHTML = "";
  });

  /** The tab of P1 against a fake API whose gaps answer `gaps`; the URLs read. */
  async function mount(
    gaps: (url: string) => Response,
    declarations?: (url: string) => Response,
  ): Promise<{
    root: HTMLElement;
    seen: string[];
  }> {
    const seen: string[] = [];
    const json = (status: number, payload: unknown) =>
      new Response(JSON.stringify(payload), {
        status,
        headers: { "Content-Type": "application/json" },
      });
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      seen.push(url);
      if (url.startsWith("/v1/property-gaps")) return gaps(url);
      if (declarations && url.startsWith("/v1/property-declarations")) return declarations(url);
      if (url.startsWith(`/v1/prefixes/${P1}/properties`)) {
        const self = node(P1, "prefix");
        return json(200, {
          properties: {
            "core.site": "site-ouest",
            "core.owner": "atelier",
            "core.contact": "équipe",
            "dns.ttl": 3600,
            "dhcp.pool": "pool-a",
            "app.tier": "front",
            "app.zone": "z1",
          },
          sources: {
            "core.site": { origin: self, inherited: false, provenance: "input" },
            "core.owner": { origin: node(S1, "site"), inherited: true, provenance: "input" },
            "core.contact": { origin: null, inherited: true, provenance: null },
            "dns.ttl": { origin: self, inherited: false, provenance: "source:cmdb@r12" },
            "dhcp.pool": { origin: self, inherited: false, provenance: "module:dhcp#lease" },
            "app.tier": {
              origin: self,
              inherited: false,
              provenance: "module:kube#label:app.example/tier",
            },
            // A provenance the shell does not know: shown as stored.
            "app.zone": { origin: self, inherited: false, provenance: "import:legacy" },
          },
          masked: ["core.secret"],
          links: {},
        });
      }
      if (url.startsWith(`/v1/sites/${S1}`))
        return json(200, { id: S1, slug: "site-a", links: {} });
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
    const { default: PropertiesTab } = await import("../src/views/record/PropertiesTab.vue");
    const Host = defineComponent({
      setup: () => () =>
        h(PropertiesTab, {
          collection: collectionById("prefixes")!,
          id: P1,
          name: "192.0.2.0/24",
          version: null,
        }),
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Host }],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp(RouterView);
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    return { root, seen };
  }
  const json = (status: number, payload: unknown, headers: Record<string, string> = {}) =>
    new Response(JSON.stringify(payload), {
      status,
      headers: { "Content-Type": "application/json", ...headers },
    });
  /** What an element shows, its spaces folded: hidden descriptions and words left out. */
  const shown = (element: Element | null) => {
    if (!element) return undefined;
    const copy = element.cloneNode(true) as Element;
    copy.querySelectorAll(".k-visually-hidden, [hidden]").forEach((hidden) => hidden.remove());
    return copy.textContent?.replace(/\s+/g, " ").trim();
  };
  /** The text a cell shows. */
  const text = (root: HTMLElement, key: string, column: string) =>
    shown(root.querySelector(`[data-row="${key}"][data-column="${column}"]`));

  it("says the source of each value, and no notice without a gap", async () => {
    const { root, seen } = await mount(() => json(200, { items: [], links: {} }));
    await expect.poll(() => text(root, "core.site", "source")).toBe("Directe");
    // Inherited: its origin named once read, a link to its record.
    await expect.poll(() => text(root, "core.owner", "source")).toBe("Héritée de Site site-a");
    expect(
      root.querySelector('[data-row="core.owner"][data-column="source"] a')?.getAttribute("href"),
    ).toBe(`/sites/${S1}`);
    expect(text(root, "core.contact", "source")).toBe("Héritée d'un objet que vous ne lisez pas");
    expect(text(root, "dns.ttl", "source")).toContain("Inférée de la source cmdb révision r12");
    expect(text(root, "dhcp.pool", "source")).toContain("Dérivée par le module dhcp : lease");
    expect(text(root, "app.tier", "source")).toContain(
      "Tirée par le module kube du label app.example/tier",
    );
    expect(text(root, "app.zone", "source")).toContain("Provenance");
    expect(
      root.querySelector('[data-row="app.zone"][data-column="source"] code')?.textContent,
    ).toBe("import:legacy");
    // Masked: no source, what it lacks said; nor any explanation to open.
    expect(text(root, "core.secret", "source")).toBe("Droit de lecture manquant");
    // The gaps of this object alone, read once; none, no notice.
    await expect
      .poll(() => seen.filter((url) => url.startsWith("/v1/property-gaps")))
      .toEqual([`/v1/property-gaps?object=${P1}`]);
    expect(root.querySelector(".k-inline-alert")).toBeNull();
  });

  it("shows the gaps of the object above its properties, each from its members", async () => {
    const { root } = await mount(() =>
      json(200, {
        items: [
          {
            id: G1,
            object: node(P1, "prefix"),
            property: "dns.ttl",
            resolved: null,
            expected: 7200,
            source: "cmdb",
            revision: "r12",
            rule: "prop.resolution.expected_mismatch",
            message: "a sentence of the server, not shown",
            opened_at: "2026-11-13T08:00:00Z",
          },
        ],
        links: {},
      }),
    );
    await expect.poll(() => root.querySelector(".k-inline-alert")).not.toBeNull();
    const notice = root.querySelector<HTMLElement>(".k-inline-alert")!;
    expect(notice.getAttribute("role")).toBe("status");
    const said = shown(notice)!;
    expect(said).toContain("Une propriété attendue manque ou diffère");
    expect(said).toContain("dns.ttl : aucune valeur, attendue 7200, selon la source cmdb");
    expect(said).toContain("révision r12");
    expect(said).toContain("prop.resolution.expected_mismatch");
    expect(said).not.toContain("a sentence of the server");
    // Above the grid: the grid follows it in the document.
    await expect.poll(() => root.querySelector("[data-k-properties]")).not.toBeNull();
    const grid = root.querySelector("[data-k-properties]")!;
    expect(notice.compareDocumentPosition(grid) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it("says a refused read of the gaps with its code, the properties shown, and retries", async () => {
    let refused = true;
    const { root } = await mount(() =>
      refused
        ? json(
            400,
            { type: "urn:kartenn:problem:validation", title: "Invalid", status: 400 },
            { "Content-Type": "application/problem+json", "Request-Id": "req-1" },
          )
        : json(200, { items: [], links: {} }),
    );
    await expect.poll(() => root.querySelector(".k-inline-alert[role='alert']")).not.toBeNull();
    const alert = root.querySelector<HTMLElement>(".k-inline-alert")!;
    expect(alert.textContent).toContain("Les lacunes n'ont pas pu être lues.");
    expect(alert.textContent).toContain("urn:kartenn:problem:validation");
    await expect.poll(() => text(root, "core.site", "source")).toBe("Directe");
    // « Réessayer » reads them again: none now, the alert gone.
    refused = false;
    [...alert.querySelectorAll("button")]
      .find((b) => b.textContent?.includes("Réessayer"))!
      .click();
    await expect.poll(() => root.querySelector(".k-inline-alert")).toBeNull();
  });

  it("reads by key the declarations its kind lacks, says their refusal, and retries", async () => {
    let refused = true;
    const { root, seen } = await mount(
      () => json(200, { items: [], links: {} }),
      (url) => {
        const params = new URL(url, "https://k").searchParams;
        // The kind's: `core.site` alone; every other key is read by its own.
        if (params.get("carrier") === "prefix") {
          const site = { id: "d1", key: "core.site", type: "text", carriers: ["prefix"] };
          return json(200, { items: [site], links: {} });
        }
        return refused
          ? json(
              403,
              { type: "urn:kartenn:problem:forbidden", title: "Forbidden", status: 403 },
              { "Content-Type": "application/problem+json" },
            )
          : json(200, { items: [], links: {} });
      },
    );
    await expect.poll(() => root.querySelector(".k-inline-alert[role='alert']")).not.toBeNull();
    const alert = root.querySelector<HTMLElement>(".k-inline-alert")!;
    expect(alert.textContent).toContain("Les propriétés déclarées n'ont pas pu être lues.");
    expect(alert.textContent).toContain("urn:kartenn:problem:forbidden");
    // The values stay shown.
    await expect.poll(() => text(root, "core.site", "source")).toBe("Directe");
    // Neither the kind's key nor the masked one asked by its key.
    const byKey = seen
      .filter((url) => url.startsWith("/v1/property-declarations?"))
      .map((url) => new URL(url, "https://k").searchParams.get("q"))
      .filter((q) => q !== null);
    expect(byKey).toContain("core.owner");
    expect(byKey).not.toContain("core.site");
    expect(byKey).not.toContain("core.secret");
    refused = false;
    [...alert.querySelectorAll("button")]
      .find((b) => b.textContent?.includes("Réessayer"))!
      .click();
    await expect.poll(() => root.querySelector(".k-inline-alert")).toBeNull();
  });
});
