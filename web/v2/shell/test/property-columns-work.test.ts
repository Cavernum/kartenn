// @vitest-environment happy-dom
import { drafts, keys, queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { QueryClient } from "@tanstack/query-core";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, effectScope, h, watch, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { useQueries } from "../src/data/queries.ts";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { keepColumns } from "../src/views/viewSettings.ts";

// The work of a list's columns of properties at each response (task 021-22f-d): a response
// of `/properties` makes its rows whose value changes anew, the others kept, in one pass of
// the grid; a batch of the window (task 11p10-c) the same. Before,
// each response rebuilt the results by row, made every row loaded anew and had the grid
// count its keys again: O(responses × rows loaded) where O(rows of the response) is enough.

const ROWS = 1000;
const id = (i: number) => `0190a000-0000-7000-8000-${String(i).padStart(12, "0")}`;
const prefix = (i: number) => ({
  id: id(i),
  value: `10.${Math.floor(i / 256)}.${i % 256}.0/24`,
  nature: "network",
  version: "v1",
  links: { self: `/v1/prefixes/${id(i)}` },
});
const declarations = [
  { id: "d-zone", key: "custom.zone", type: "text", cardinality: "single", carriers: ["prefix"] },
];
const DRAFT = "0190a000-0000-7000-8000-0000000000d1";
/** The draft active in the freshness test: a zone of an unread row, a nature of another. */
const draft = {
  id: DRAFT,
  title: "Brouillon",
  version: 1,
  operation_count: 2,
  operations: [
    {
      op: "assign_property",
      object: { kind: "prefix", id: id(500) },
      property: { id: "d-zone" },
      values: ["nord"],
    },
    { op: "update_prefix", id: { kind: "prefix", id: id(3) }, nature: "container" },
  ],
};

let app: App | undefined;
/** The reads of properties sent and not answered yet, by row. */
const waiting = new Map<string, (zone: string) => void>();
/** The rows of the list as the server reads them. */
let listed = prefix;
afterEach(() => {
  for (const answer of waiting.values()) answer("-");
  waiting.clear();
  listed = prefix;
  drafts.setActive(null);
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
  setFetch(null);
  document.body.innerHTML = "";
  localStorage.clear();
});

/** Each render of the grid, with the rows it was given. */
const passes: (readonly object[])[] = [];

const json = (body: unknown) =>
  new Response(JSON.stringify(body), {
    headers: { "Content-Type": "application/json", ETag: '"v1"' },
  });

/** The list of 1 000 prefixes mounted, its column of zone chosen, no read answered. */
async function mounted() {
  keepColumns("u-1", "prefixes", { "prop.custom.zone": true });
  setFetch((async (input: RequestInfo | URL) => {
    const url = String(input);
    // A batch (task 021-11p10-c): answered once each of its rows is.
    if (url.startsWith("/v1/prefixes/properties?")) {
      const ids = new URL(url, "http://localhost").searchParams.getAll("id");
      const zones = new Map<string, string>();
      return new Promise<Response>((resolve) => {
        for (const one of ids) {
          waiting.set(one, (zone) => {
            zones.set(one, zone);
            if (zones.size < ids.length) return;
            const items = ids.map((of) => ({
              id: of,
              properties: { "custom.zone": zones.get(of) },
              sources: {},
              masked: [],
            }));
            resolve(json({ items }));
          });
        }
      });
    }
    const read = /^\/v1\/prefixes\/([^/?]+)\/properties/.exec(url);
    if (read) {
      return new Promise<Response>((resolve) =>
        waiting.set(read[1]!, (zone) =>
          resolve(json({ properties: { "custom.zone": zone }, masked: [] })),
        ),
      );
    }
    if (url.startsWith("/v1/labels"))
      return json({ lang: "fr", default_lang: "fr", revision: "1", spaces: {} });
    if (url.startsWith("/v1/property-declarations"))
      return json({ items: declarations, links: {} });
    if (url.startsWith(`/v1/drafts/${DRAFT}`)) return json(draft);
    if (url.startsWith("/v1/prefixes"))
      return json({ items: Array.from({ length: ROWS }, (_, i) => listed(i + 1)), links: {} });
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
  const root = document.body.appendChild(document.createElement("div"));
  app = createApp({ render: () => h(RouterView) });
  app.mixin({
    updated() {
      if ((this.$options.__name ?? this.$options.name) === "DataGrid") {
        passes.push((this.$props as { rows: readonly object[] }).rows);
      }
    },
  });
  app.use(createShellI18n("fr")).use(router);
  app.mount(root);
  await router.isReady();
  await expect.poll(() => waiting.size).toBeGreaterThan(0);
  return root;
}

/** The row `of` as the grid was last given it. */
const rowOf = (of: string) =>
  passes.at(-1)?.find((row) => (row as { id: string }).id === of) as
    Record<string, unknown> | undefined;
/** The zone of the row `of` as the grid was last given it. */
const shown = (of: string) => rowOf(of)?.["prop.custom.zone"];
function answer(of: string, zone: string) {
  waiting.get(of)!(zone);
  waiting.delete(of);
}
/** Every read sent answered, `zone` but for the rows of `apart`. */
function answerAll(zone: string, apart: Record<string, string> = {}) {
  for (const of of [...waiting.keys()]) answer(of, apart[of] ?? zone);
}

/** Settled: no pass of the grid during a few turns of the loop. */
async function settled() {
  let seen = -1;
  while (seen !== passes.length) {
    seen = passes.length;
    for (let turn = 0; turn < 5; turn++) await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

describe("a response of properties makes its rows anew, alone (tasks 021-22f-d, 11p10-c)", () => {
  it("of 1 000 rows loaded, a batch read again where one value changes: one new row object, one pass of the grid", async () => {
    await mounted();
    const first = [...waiting.keys()][0]!;
    answerAll("-");
    await settled();
    // Every row read again, in one batch: the value of one alone changes.
    void queryClient.invalidateQueries({
      predicate: (query) => query.queryKey[3] === "properties",
    });
    await expect.poll(() => waiting.has(first)).toBe(true);
    await settled();
    const before = passes.at(-1)!;
    expect(before).toHaveLength(ROWS);
    passes.length = 0;
    answerAll("-", { [first]: "ouest" });
    await expect.poll(() => shown(first)).toBe("ouest");
    await settled();
    const after = passes.at(-1)!;
    const anew = after.filter((row, i) => row !== before[i]);
    expect({ passes: passes.length, anew: anew.length }).toEqual({ passes: 1, anew: 1 });
  });

  it("the responses of a same turn: one pass, their rows alone anew", async () => {
    await mounted();
    await settled();
    const before = passes.at(-1)!;
    passes.length = 0;
    const sent = [...waiting.keys()];
    expect(sent.length).toBeGreaterThan(1);
    for (const of of sent) answer(of, "est");
    await expect.poll(() => sent.map(shown)).toEqual(sent.map(() => "est"));
    await settled();
    const after = passes.at(-1)!;
    const anew = after.filter((row, i) => row !== before[i]);
    expect({ passes: passes.length, anew: anew.length }).toEqual({ passes: 1, anew: sent.length });
  });
});

describe("the results of several queries set once per turn (task 021-22f-d)", () => {
  it("the notifications of a turn: one array of results, every one in it", async () => {
    const client = new QueryClient();
    const scope = effectScope();
    const sets: number[] = [];
    const results = scope.run(() => {
      const out = useQueries(
        () => ["a", "b", "c"].map((key) => ({ queryKey: [key], enabled: false })),
        client,
      );
      watch(out, (now) => sets.push(now.filter((one) => one.data !== undefined).length), {
        flush: "sync",
      });
      return out;
    })!;
    for (const key of ["a", "b", "c"]) client.setQueryData([key], key);
    await Promise.resolve();
    await Promise.resolve();
    expect(sets).toEqual([3]);
    expect(results.value.map((one) => one.data)).toEqual(["a", "b", "c"]);
    scope.stop();
    client.setQueryData(["a"], "z");
    await Promise.resolve();
    expect(sets).toEqual([3]);
  });
});

/** The rows the grid was last given that are not those of `before`, by identifier. */
const anewSince = (before: readonly object[]) =>
  passes
    .at(-1)!
    .filter((row, i) => row !== before[i])
    .map((row) => (row as { id: string }).id);

describe("a row kept is never stale (task 021-22f-d)", () => {
  it("a value read again: its row anew, with it", async () => {
    await mounted();
    const first = [...waiting.keys()][0]!;
    answerAll("-", { [first]: "ouest" });
    await expect.poll(() => shown(first)).toBe("ouest");
    await settled();
    // Every read answered: the one sent again goes alone, by the read of its object.
    while (waiting.size > 0) {
      for (const of of [...waiting.keys()]) answer(of, "-");
      await settled();
    }
    const before = passes.at(-1)!;
    void queryClient.invalidateQueries({ queryKey: keys.allOf("prefix", first) });
    await expect.poll(() => waiting.has(first)).toBe(true);
    answer(first, "sud");
    await expect.poll(() => shown(first)).toBe("sud");
    await settled();
    expect(anewSince(before)).toEqual([first]);
  });

  it("a draft made active: the rows it changes anew, a property and a field", async () => {
    await mounted();
    await settled();
    const before = passes.at(-1)!;
    drafts.setActive(DRAFT);
    await expect.poll(() => shown(id(500))).toBe("nord");
    await settled();
    expect(rowOf(id(3))?.nature).toBe("container");
    expect(anewSince(before).sort()).toEqual([id(3), id(500)]);
    // The draft no longer active: the rows as read.
    drafts.setActive(null);
    await expect.poll(() => shown(id(500))).toBe("");
    expect(rowOf(id(3))?.nature).toBe("network");
  });

  it("a row read again by the list: that row anew, its value of property kept", async () => {
    await mounted();
    const first = [...waiting.keys()][0]!;
    answerAll("-", { [first]: "ouest" });
    await expect.poll(() => shown(first)).toBe("ouest");
    await settled();
    const before = passes.at(-1)!;
    listed = (i) =>
      id(i) === first ? { ...prefix(i), nature: "container", version: "v2" } : prefix(i);
    void queryClient.invalidateQueries({ queryKey: keys.allOn("/v1/prefixes"), exact: false });
    await expect.poll(() => rowOf(first)?.nature).toBe("container");
    await settled();
    expect(shown(first)).toBe("ouest");
    expect(anewSince(before)).toEqual([first]);
  });
});
