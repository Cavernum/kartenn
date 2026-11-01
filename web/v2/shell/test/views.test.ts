// @vitest-environment happy-dom
import { queryClient, reads } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { checkFilter, filtersToParams, type Filter } from "@kartenn/ui";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView, type Router } from "vue-router";
import { crumbsOf } from "../src/frame/breadcrumb.ts";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import {
  anchorOf,
  COLLECTIONS,
  nameOf,
  pageOfHref,
  referenceOf,
} from "../src/views/collections.ts";
import {
  COLUMN_READS,
  limited,
  networkVlanOperation,
  vlanOfProperties,
} from "../src/views/parts/vlan.ts";
import {
  filtersOfQuery,
  listFilterModel,
  queryOfFilters,
  sentFilters,
  type DeclarationOfModel,
  type FilterWords,
  type SuggestRead,
} from "../src/views/filters.ts";
import { keepRecordName } from "../src/views/names.ts";
import { prefetchFilters } from "../src/views/routes.ts";

// The screens of the referential (task 021-11; REQ-021-9, 18): the model of the filters of
// a list drawn from the contract and the declarations — one for the bar and the columns
// (020 decision A7) —, the URL as the request of the API, a link copied opening the same
// list, and the routes of the frame with their screens and reads.

const ENTITY = "0b5e3c1a-0000-4000-8000-0000000000e1";
/** The application of the mounted tests: the route's screen alone. */
const Root = { render: () => h(RouterView) };
const DECLARATIONS: DeclarationOfModel[] = [
  { key: "core.owner", type: "ref", carriers: [], params: { target: "entity" } },
  { key: "core.site", type: "enum", carriers: ["prefix", "address"] },
  { key: "dns.ttl", type: "int", carriers: ["core.address"] },
  { key: "vlan.only", type: "text", carriers: ["vlan"] },
];
const WORDS: FilterWords = {
  field: (name) => ({ domain: "Domaine", value: "Valeur", usage: "Usage" })[name],
  value: (name, value) => (name === "usage" && value === "host" ? "Hôte" : undefined),
  property: (key) => (key === "core.owner" ? "Propriétaire" : undefined),
  propertyValue: () => undefined,
};

/** The reads of the suggestions, answered from a fixture. */
function suggestions(items: Record<string, unknown>[]) {
  const asked: { path: string; filters: unknown }[] = [];
  const read: SuggestRead = async (path, filters) => {
    asked.push({ path, filters });
    return items;
  };
  return { read, asked };
}

describe("the model of the filters of a list", () => {
  const model = listFilterModel("/v1/addresses", DECLARATIONS, WORDS);
  const property = (key: string) => model.find((p) => p.key === key);

  it("takes the parameters of the route, `within` an operator of the value", () => {
    expect(model.map((p) => p.key)).toEqual([
      "domain",
      "value",
      "usage",
      "prop.core.owner",
      "prop.core.site",
      "prop.dns.ttl",
    ]);
    expect(property("value")).toMatchObject({
      label: "Valeur",
      type: "address",
      operators: ["eq", "within"],
      params: { within: "within" },
    });
    // `q` is the palette's search, not a filter of the bar.
    expect(property("q")).toBeUndefined();
  });

  it("gives a closed list of the contract its values, labels before keys", () => {
    expect(property("usage")).toMatchObject({ type: "enum", operators: ["eq"] });
    expect(property("usage")!.options).toContainEqual({ value: "host", label: "Hôte" });
    expect(property("usage")!.options).toContainEqual({ value: "vip", label: "vip" });
  });

  it("types each declared property its objects carry, never one of another kind", () => {
    expect(property("prop.dns.ttl")).toMatchObject({
      type: "number",
      operators: ["eq", "between", "exists", "absent"],
    });
    // A closed list or a reference is never ordered.
    expect(property("prop.core.site")!.operators).toEqual(["eq", "exists", "absent"]);
    expect(property("prop.core.owner")).toMatchObject({ label: "Propriétaire", type: "enum" });
    expect(model.some((p) => p.key === "prop.vlan.only")).toBe(false);
  });

  it("suggests the slugs of a collection, and the objects a reference targets", async () => {
    const { read, asked } = suggestions([{ id: ENTITY, slug: "exemple" }]);
    const own = listFilterModel("/v1/addresses", DECLARATIONS, WORDS, read);
    const signal = new AbortController().signal;
    const domains = await own.find((p) => p.key === "domain")!.suggest!("prod", signal);
    expect(asked[0]).toEqual({ path: "/v1/domains", filters: { q: "prod" } });
    expect(domains[0]).toMatchObject({ value: "exemple", label: "exemple" });
    const owner = own.find((p) => p.key === "prop.core.owner")!;
    const owners = await owner.suggest!("ex", signal);
    expect(asked[1]).toEqual({ path: "/v1/entities", filters: { q: "ex" } });
    // The value is the identifier; its label, once suggested, is the slug.
    expect(owners[0]).toMatchObject({ value: ENTITY, label: "exemple" });
    expect(owner.valueLabel!(ENTITY)).toBe("exemple");
  });

  it("asks the identifier of a domain or an entity on the anomalies", () => {
    const anomalies = listFilterModel("/v1/anomalies", [], WORDS);
    expect(anomalies.map((p) => p.key)).toEqual(["rule", "domain", "entity"]);
    expect(anomalies.find((p) => p.key === "entity")).toMatchObject({ type: "enum" });
    const gaps = listFilterModel("/v1/property-gaps", DECLARATIONS, WORDS);
    expect(gaps.find((p) => p.key === "property")!.options!.map((o) => o.value)).toContain(
      "dns.ttl",
    );
  });
});

describe("the filters in the URL", () => {
  const model = listFilterModel("/v1/addresses", DECLARATIONS, WORDS);
  const query = {
    domain: "production",
    within: "192.0.2.0/24",
    usage: ["host", "vip"],
    [`prop.core.owner`]: ENTITY,
    "prop.dns.ttl:between": "60..3600",
    valid_at: "2026-01-15T00:00:00Z",
  };

  it("reads a query as the chips of the bar, and writes them back the same", () => {
    const filters = filtersOfQuery(model, query);
    expect(filters).toEqual([
      { key: "domain", operator: "eq", values: ["production"] },
      { key: "value", operator: "within", values: ["192.0.2.0/24"] },
      { key: "usage", operator: "eq", values: ["host", "vip"] },
      { key: "prop.core.owner", operator: "eq", values: [ENTITY] },
      { key: "prop.dns.ttl", operator: "between", values: ["60", "3600"] },
    ]);
    // The dates of the page stay, the order of the filters too.
    expect(queryOfFilters(model, filters, query)).toEqual({
      valid_at: "2026-01-15T00:00:00Z",
      domain: "production",
      within: "192.0.2.0/24",
      usage: ["host", "vip"],
      "prop.core.owner": ENTITY,
      "prop.dns.ttl:between": "60..3600",
    });
  });

  it("sends neither a draft nor a value out of its type, which stays on its chip", () => {
    const filters: Filter[] = [
      { key: "value", operator: "eq", values: ["not-an-address"] },
      { key: "usage", operator: "eq", values: ["host"] },
      { key: "domain", operator: "eq", values: ["essai"], draft: true },
    ];
    expect(checkFilter(model, filters[0]!)).toBeDefined();
    expect(sentFilters("/v1/addresses", model, filters, DECLARATIONS)).toEqual({ usage: "host" });
    expect(filtersToParams(model, filters).toString()).toBe("value=not-an-address&usage=host");
  });

  it("prefetches the very list a copied link opens (one key of the cache)", () => {
    const filters = filtersOfQuery(model, query);
    const shown = sentFilters("/v1/addresses", model, filters, DECLARATIONS);
    queryClient.setQueryData(reads.list("/v1/property-declarations", null, null).queryKey, {
      pages: [{ data: { items: DECLARATIONS, links: {} }, etag: null }],
      pageParams: [null],
    });
    const prefetched = prefetchFilters("/v1/addresses", query);
    expect(reads.list("/v1/addresses", prefetched as never).queryKey).toEqual(
      reads.list("/v1/addresses", shown as never).queryKey,
    );
  });
});

describe("the collections", () => {
  it("name an object by its value, its bounds or its slug", () => {
    expect(nameOf({ value: "192.0.2.0/24" })).toBe("192.0.2.0/24");
    expect(nameOf({ first: "192.0.2.10", last: "192.0.2.20" })).toBe("192.0.2.10 – 192.0.2.20");
    expect(nameOf({ slug: "production" })).toBe("production");
    // A VLAN by its VID and name (026).
    expect(nameOf({ vid: 100, name: "serveurs" })).toBe("100 · serveurs");
  });

  it("read the objects a VLAN names by their identifier only (026)", () => {
    const columns = Object.fromEntries(
      COLLECTIONS.flatMap((c) => c.columns.map((column) => [`${c.id}.${column.id}`, column])),
    );
    expect(referenceOf(columns["vlans.group"]!, { id: ENTITY })).toEqual({
      kind: "vlan_group",
      id: ENTITY,
    });
    expect(referenceOf(columns["vlan-groups.anchor"]!, { site: { id: ENTITY } })).toEqual({
      kind: "site",
      id: ENTITY,
    });
    expect(anchorOf({ site: { id: ENTITY }, domain: { id: ENTITY } })).toBeNull();
    expect(referenceOf(columns["prefixes.domain"]!, { id: ENTITY })).toBeNull();
  });

  it("read and write the VLAN of a network by its property `core.vlan`", () => {
    expect(vlanOfProperties({ properties: { "core.vlan": ENTITY }, masked: [] } as never)).toBe(
      ENTITY,
    );
    expect(
      vlanOfProperties({ properties: { "core.vlan": { id: ENTITY } }, masked: [] } as never),
    ).toBe(ENTITY);
    expect(vlanOfProperties({ properties: {}, masked: ["core.vlan"] } as never)).toBeNull();
    expect(networkVlanOperation("p", "d", ENTITY)).toEqual({
      op: "assign_property",
      kind: "prefix",
      object: { id: "p" },
      property: { id: "d" },
      values: [ENTITY],
    });
    expect(networkVlanOperation("p", "d", null)).toEqual({
      op: "unassign_property",
      kind: "prefix",
      object: { id: "p" },
      property: { id: "d" },
    });
  });

  it("read the VLAN of the rows a few at a time, never a row gone meanwhile", async () => {
    let running = 0;
    let most = 0;
    const releases: (() => void)[] = [];
    const read = () =>
      new Promise<string>((resolve) => {
        running++;
        most = Math.max(most, running);
        releases.push(() => {
          running--;
          resolve("read");
        });
      });
    const gone = new AbortController();
    const outcome = (signal?: AbortSignal) =>
      limited(signal, read).then(
        () => "read",
        () => "dropped",
      );
    const reads = [
      ...Array.from({ length: COLUMN_READS + 2 }, () => outcome()),
      outcome(gone.signal),
    ];
    gone.abort();
    for (let i = 0; i < 20 && releases.length > 0; i++) {
      releases.shift()!();
      await new Promise((resolve) => setTimeout(resolve));
    }
    const settled = await Promise.all(reads);
    expect(most).toBe(COLUMN_READS);
    expect(settled.filter((s) => s === "read")).toHaveLength(COLUMN_READS + 2);
    expect(settled.at(-1)).toBe("dropped");
  });

  it("open the record a URL of the API names, of a collection of the shell only", () => {
    expect(pageOfHref("/v1/prefixes/abc")).toBe("/prefixes/abc");
    expect(pageOfHref("https://kartenn.example/v1/addresses/abc")).toBe("/addresses/abc");
    expect(pageOfHref("/v1/flows/abc")).toBeNull();
  });

  it("offer the filter of a column only when its list serves that parameter (A7)", () => {
    for (const collection of COLLECTIONS) {
      const served = new Set(listFilterModel(collection.path, [], WORDS).map((p) => p.key));
      for (const column of collection.columns) {
        if (column.filter) expect(served, `${collection.id}.${column.id}`).toContain(column.filter);
      }
    }
  });
});

describe("the routes of the referential", () => {
  let router: Router;
  let app: App | undefined;
  const t = (key: string) => key;

  beforeEach(async () => {
    router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
  });
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
  });

  it("give each list, record and screen its reads", async () => {
    await router.push("/addresses?usage=host");
    const list = router.currentRoute.value;
    const reads_ = list.matched.at(-1)!.meta.queries!(list.params, {}, list.query);
    expect(reads_[0]!.queryKey).toEqual(reads.list("/v1/addresses", { usage: "host" }).queryKey);
    for (const path of ["/", "/topology", "/anomalies", "/property-declarations"]) {
      await router.push(path);
      expect(router.currentRoute.value.matched.at(-1)!.meta.queries, path).toBeDefined();
    }
    const range = "0b5e3c1a-0000-4000-8000-000000000001";
    await router.push(`/ranges/${range}`);
    const record = router.currentRoute.value;
    expect(record.name).toBe("ranges-record");
    // The record's properties come from the route (vue-router gives the function the route).
    const props = record.matched.at(-1)!.props.default;
    expect(typeof props === "function" ? props(record) : props).toMatchObject({ id: range });
  });

  it("name a record in its breadcrumb once read, by its identifier before", async () => {
    const id = "0b5e3c1a-0000-4000-8000-000000000002";
    await router.push(`/prefixes/${id}`);
    expect(crumbsOf(router.currentRoute.value, t).at(-1)).toEqual({ label: id, mono: true });
    keepRecordName(id, "192.0.2.0/24");
    expect(crumbsOf(router.currentRoute.value, t).at(-1)!.label).toBe("192.0.2.0/24");
  });

  it("read the list of the URL with its filters, a link copied opening the same", async () => {
    const seen: string[] = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      seen.push(url);
      const body = url.startsWith("/v1/labels")
        ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
        : { items: url.startsWith("/v1/property-declarations") ? DECLARATIONS : [], links: {} };
      return new Response(JSON.stringify(body), {
        headers: { "Content-Type": "application/json", ETag: '"v1"' },
      });
    }) as typeof fetch);
    const link = `/addresses?within=192.0.2.0%2F24&prop.core.owner=${ENTITY}`;
    await router.push(link);
    app = createApp(Root);
    app.use(createShellI18n("fr")).use(router);
    app.mount(document.createElement("div"));
    await router.isReady();
    await expect.poll(() => seen.find((url) => url.startsWith("/v1/addresses"))).toBeDefined();
    const read = new URL(
      seen.find((url) => url.startsWith("/v1/addresses"))!,
      "https://k",
    );
    expect(read.searchParams.get("within")).toBe("192.0.2.0/24");
    expect(read.searchParams.get("prop.core.owner")).toBe(ENTITY);
    // The property filter waited for the declarations: no read without it.
    expect(seen.filter((url) => url.startsWith("/v1/addresses"))).toHaveLength(1);
    await nextTick();
    expect(router.currentRoute.value.fullPath).toBe(link);
  });

  /** The shell mounted at `link`, the API answered by `answer` (a body, or a status). */
  async function mounted(link: string, answer: (url: string) => object | number) {
    const seen: string[] = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      seen.push(url);
      const body = url.startsWith("/v1/labels")
        ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
        : answer(url);
      if (typeof body === "number") {
        return new Response(
          JSON.stringify({ type: "urn:kartenn:problem:forbidden", title: "", status: body }),
          { status: body, headers: { "Content-Type": "application/problem+json" } },
        );
      }
      return new Response(JSON.stringify(body), {
        headers: { "Content-Type": "application/json", ETag: '"v1"' },
      });
    }) as typeof fetch);
    await router.push(link);
    const root = document.createElement("div");
    app = createApp(Root);
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    return { seen, root };
  }
  const page = (items: object[], next?: string) => ({ items, links: next ? { next } : {} });
  const OTHER = (i: number): DeclarationOfModel => ({
    key: `custom.p${i}`,
    type: "text",
    carriers: [],
  });
  const addresses = (seen: string[]) => seen.filter((url) => url.startsWith("/v1/addresses"));

  it("reads every page of the declarations a property filter needs before the list", async () => {
    const { seen } = await mounted(`/addresses?prop.core.owner=${ENTITY}`, (url) => {
      if (url === "/v1/property-declarations") {
        return page([OTHER(1)], "/v1/property-declarations?cursor=2");
      }
      if (url.startsWith("/v1/property-declarations?cursor=2")) return page([DECLARATIONS[0]!]);
      return page([]);
    });
    await expect.poll(() => addresses(seen).length).toBe(1);
    // Read once, after the second page, with its filter.
    expect(new URL(addresses(seen)[0]!, "https://k").searchParams.get("prop.core.owner")).toBe(
      ENTITY,
    );
    const second = seen.findIndex((url) => url.includes("cursor=2"));
    expect(seen.indexOf(addresses(seen)[0]!)).toBeGreaterThan(second);
  });

  it("past the cap of pages, a key still unknown is a chip not recognised, no list read", async () => {
    const { seen, root } = await mounted("/addresses?prop.custom.absent=x", (url) => {
      if (url.startsWith("/v1/property-declarations")) {
        const at = Number(new URL(url, "https://k").searchParams.get("cursor") ?? "1");
        return page([OTHER(at)], `/v1/property-declarations?cursor=${at + 1}`);
      }
      return page([]);
    });
    await expect.poll(() => root.textContent).toContain("Filtre non reconnu : prop.custom.absent");
    expect(seen.filter((url) => url.startsWith("/v1/property-declarations"))).toHaveLength(10);
    expect(root.querySelector(".k-filter-chip__pill[data-state='invalid']")).not.toBeNull();
    expect(addresses(seen)).toEqual([]);
    expect(router.currentRoute.value.query["prop.custom.absent"]).toBe("x");
  });

  for (const status of [403, 500]) {
    it(`the declarations refused (${status}): an error state, no list read`, async () => {
      const { seen, root } = await mounted(`/addresses?prop.core.owner=${ENTITY}`, (url) =>
        url.startsWith("/v1/property-declarations") ? status : page([]),
      );
      await expect
        .poll(() => root.textContent, { timeout: 8_000 })
        .toContain("Les propriétés déclarées n'ont pas pu être lues");
      // Why, in plain words (the type translated: the dictionary has none, the catalog
      // has), then its code as served (REQ-021-20).
      expect(root.textContent).toContain("Vous n'avez pas le droit de faire cela.");
      expect(root.textContent).toContain("urn:kartenn:problem:forbidden");
      expect(addresses(seen)).toEqual([]);
    }, 10_000);
  }

  it("a next page refused: the rows kept, the refusal said above them by problems/", async () => {
    const { root } = await mounted("/addresses", (url) => {
      if (url.includes("cursor=c2")) return 500;
      if (url.startsWith("/v1/addresses")) {
        return page(
          [{ id: ENTITY, value: "192.0.2.10", links: { self: `/v1/addresses/${ENTITY}` } }],
          "/v1/addresses?cursor=c2",
        );
      }
      return page([]);
    });
    await expect.poll(() => root.textContent).toContain("192.0.2.10");
    // The next page, as the grid asks for it once scrolled to its end (no layout here).
    const query = queryClient
      .getQueryCache()
      .findAll({ queryKey: ["v1", "/v1/addresses"] })
      .find((q) => q.state.data !== undefined)!;
    await query.fetch(undefined, { meta: { fetchMore: { direction: "forward" } } }).catch(() => {});
    await expect
      .poll(() => root.textContent, { timeout: 8_000 })
      .toContain("La suite de la liste n'a pas pu être lue");
    expect(root.textContent).toContain("192.0.2.10");
    const alert = [...root.querySelectorAll("[role='alert']")].find((el) =>
      el.textContent?.includes("La suite de la liste"),
    )!;
    expect(alert.textContent).toContain("urn:kartenn:problem:forbidden");
    expect(alert.textContent).toContain("Réessayer");
  }, 10_000);

  it("shows the type of a declared property in words, its key at hand (REQ-021-19)", async () => {
    const { root } = await mounted("/property-declarations", (url) =>
      url.startsWith("/v1/property-declarations")
        ? page([
            { id: "d1", key: "core.site", type: "enum", carriers: [] },
            { id: "d2", key: "custom.odd", type: "novel", carriers: [] },
          ])
        : page([]),
    );
    await expect.poll(() => root.textContent).toContain("Liste fermée");
    // The word shown; the key in the description of its pastille, copied by it.
    const typed = [...root.querySelectorAll(".k-key-label")].find((el) =>
      el.textContent?.includes("Liste fermée"),
    )!;
    expect(typed.textContent).toContain("enum");
    expect(typed.querySelector("button")).not.toBeNull();
    // A type the catalogs lack: its key alone, never an empty cell.
    const alone = [...root.querySelectorAll("[data-state='key-only']")].map((el) => el.textContent);
    expect(alone.some((text) => text?.includes("novel"))).toBe(true);
  });

  it("says why the anomalies of a site cannot be read, not that it has none", async () => {
    const site = "0b5e3c1a-0000-4000-8000-0000000000f1";
    const { seen, root } = await mounted(`/sites/${site}?tab=anomalies`, (url) =>
      url.startsWith(`/v1/sites/${site}`) && !url.includes("/properties")
        ? { id: site, slug: "nord", links: { self: `/v1/sites/${site}` } }
        : page([]),
    );
    await expect.poll(() => root.textContent).toContain("Lecture non disponible");
    expect(root.textContent).not.toContain("Aucune anomalie ouverte");
    expect(seen.some((url) => url.startsWith("/v1/anomalies"))).toBe(false);
  });
});

describe("prefetch and suggestions without what they need", () => {
  afterEach(() => queryClient.clear());

  it("prefetches no list whose property filter the cache cannot type", () => {
    expect(prefetchFilters("/v1/addresses", { "prop.core.owner": ENTITY })).toBeNull();
    expect(prefetchFilters("/v1/addresses", { usage: "host" })).toEqual({ usage: "host" });
  });

  it("sends `q` only to the lists that take it, else keeps what the text matches", async () => {
    const { read, asked } = suggestions([
      { id: "r1", first: "192.0.2.10", last: "192.0.2.20", value: "192.0.2.10" },
      { id: "r2", first: "198.51.100.1", last: "198.51.100.9", value: "198.51.100.1" },
    ]);
    const ranged: DeclarationOfModel[] = [
      { key: "custom.plage", type: "ref", carriers: [], params: { target: "range" } },
    ];
    const model = listFilterModel("/v1/addresses", ranged, WORDS, read);
    const found = await model.find((p) => p.key === "prop.custom.plage")!.suggest!(
      "198",
      new AbortController().signal,
    );
    expect(asked[0]).toEqual({ path: "/v1/ranges", filters: {} });
    expect(found.map((o) => o.value)).toEqual(["r2"]);
  });
});
