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
import { COLLECTIONS, nameOf, pageOfHref } from "../src/views/collections.ts";
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
    app = createApp({ render: () => h(RouterView) });
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
});
