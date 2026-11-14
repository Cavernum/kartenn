// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { InfiniteQueryObserver, QueryObserver } from "@tanstack/query-core";
import { afterEach, describe, expect, it } from "vitest";
import { resetContext, setContext } from "../src/context.ts";
import {
  CHILDREN_PAGES,
  FRESHNESS,
  IMMUTABLE,
  MAX_PAGES,
  fallback,
  sharePages,
  type Entry,
} from "../src/data/cache.ts";
import { isImmutable, keys, normalize, timeKey } from "../src/data/keys.ts";
import { collectionOf, qualify, registerType, typeOfCollection } from "../src/data/types.ts";
import { D1, P1, fakeCache, json, notModified } from "./helpers.ts";

afterEach(() => resetContext());

const PAST = { validAt: null, asOf: "2026-01-01T00:00:00Z" };
const prefix = (value: string) => ({
  id: P1,
  value,
  domain: { id: D1, href: `/v1/domains/${D1}` },
});

describe("keys (REQ-021-21)", () => {
  it("normalize the filters: names sorted, values as text, empty ones left out", () => {
    const a = keys.list("/v1/prefixes", { within: "10.0.0.0/8", domain: D1, value: "" }, null);
    const b = keys.list("/v1/prefixes", { domain: D1, within: "10.0.0.0/8", nature: null }, null);
    expect(a).toEqual(b);
    expect(Object.keys(a[3])).toEqual(["domain", "within"]);
    expect(normalize({ state: ["b", "a", "b"], exists: true, size: 3, none: [] })).toEqual({
      exists: "true",
      size: "3",
      state: ["a", "b"],
    });
  });

  it("write the present null, and the dates of the context on a dated path", () => {
    expect(timeKey()).toEqual({ valid_at: null, as_of: null });
    expect(keys.object("core.prefix", P1, null)).toEqual([
      "v1",
      "core.prefix",
      P1,
      null,
      { valid_at: null, as_of: null },
    ]);
    setContext({ time: { validAt: "2026-03-01T00:00:00Z", asOf: null } });
    expect(keys.object("core.prefix", P1)[4]).toEqual({
      valid_at: "2026-03-01T00:00:00Z",
      as_of: null,
    });
    // `/v1/me` is not dated: the dates of the context never enter its key.
    expect(keys.list("/v1/me")[4]).toEqual({ valid_at: null, as_of: null });
    // The chain of a workload takes `valid_at` only: `as_of` never enters its key.
    setContext({ time: { validAt: "2026-03-01T00:00:00Z", asOf: "2026-04-01T00:00:00Z" } });
    expect(keys.list("/v1/workloads/{id}/chain")[4]).toEqual({
      valid_at: "2026-03-01T00:00:00Z",
      as_of: null,
    });
    expect(keys.object("core.prefix", P1)[4]).toEqual({
      valid_at: "2026-03-01T00:00:00Z",
      as_of: "2026-04-01T00:00:00Z",
    });
  });

  it("put the sub-resources of an object under its key, and tell the immutable ones", () => {
    const sub = keys.sub("core.prefix", P1, "children", { page_size: 50 }, PAST);
    expect(sub.slice(0, 3)).toEqual(keys.allOf("core.prefix", P1));
    expect(isImmutable(sub)).toBe(true);
    expect(isImmutable(keys.object("core.prefix", P1, null))).toBe(false);
  });

  it("know the collections of the core and of the modules, and take those of plugins", () => {
    expect(qualify("prefix")).toBe("core.prefix");
    expect(collectionOf("prefix")).toBe("/v1/prefixes");
    expect(typeOfCollection("/v1/vlans")).toBe("core.vlan");
    expect(collectionOf("vlan_group")).toBe("/v1/vlan-groups");
    registerType("acme.rack", "/v1/racks");
    expect(keys.object("acme.rack", P1, null)[1]).toBe("acme.rack");
    expect(() => registerType("acme.rack", "/v1/other")).toThrow(/already/);
    expect(() => keys.object("nope.nope", P1)).toThrow(/unknown type/);
  });
});

describe("reads (REQ-021-21, ADR 0037)", () => {
  it("send If-None-Match with the ETag kept, and keep the same entry on a 304", async () => {
    let version = 0;
    const { queryClient, reads, seen } = fakeCache((request) =>
      request.headers.get("If-None-Match") === '"1.a"'
        ? notModified('"1.a"')
        : json(prefix(`10.0.${version++}.0/24`), { etag: '"1.a"' }),
    );
    const options = reads.object("core.prefix", P1, null);
    const observer = new QueryObserver(queryClient, options);
    const renders: unknown[] = [];
    const stop = observer.subscribe((result) => renders.push(result.data));
    await queryClient.fetchQuery(options);
    const first = queryClient.getQueryData<Entry>(options.queryKey);
    const before = renders.filter((data) => data !== undefined).length;
    await queryClient.refetchQueries({ queryKey: options.queryKey });
    stop();
    expect(seen.map((r) => r.url)).toEqual([`/v1/prefixes/${P1}`, `/v1/prefixes/${P1}`]);
    expect(seen[1]?.headers.get("If-None-Match")).toBe('"1.a"');
    expect(queryClient.getQueryData(options.queryKey)).toBe(first);
    // Notifications may come for the fetching state, never with other data.
    expect(new Set(renders.filter((data) => data !== undefined))).toEqual(new Set([first]));
    expect(before).toBeGreaterThan(0);
  });

  it("read the dates of the key, never those of the context at the time of the fetch", async () => {
    const { queryClient, reads, seen } = fakeCache(() => json(prefix("10.0.0.0/24")));
    const options = reads.object("core.prefix", P1, { validAt: "2026-02-01T00:00:00Z" });
    setContext({ time: { validAt: "2026-05-01T00:00:00Z", asOf: null } });
    await queryClient.fetchQuery(options);
    expect(seen[0]?.url).toBe(`/v1/prefixes/${P1}?valid_at=2026-02-01T00%3A00%3A00Z`);
  });

  it("never read the past as recorded twice", async () => {
    const { queryClient, reads, seen } = fakeCache(() => json(prefix("10.0.0.0/24")));
    const options = reads.object("core.prefix", P1, PAST);
    expect(options.staleTime).toBe(IMMUTABLE.staleTime);
    await queryClient.fetchQuery(options);
    await queryClient.fetchQuery(options);
    const query = queryClient.getQueryCache().find({ queryKey: options.queryKey })!;
    const focus = queryClient.getDefaultOptions().queries!.refetchOnWindowFocus as (
      q: typeof query,
    ) => boolean;
    expect(seen).toHaveLength(1);
    expect(fallback.active && focus(query)).toBe(false);
  });

  it("read the history again at each use, even at an as_of of the past (Q-021-29)", async () => {
    // Its authors are named as the identities read now: a pseudonymisation reaches it.
    const { queryClient, reads, seen } = fakeCache((request) =>
      request.headers.get("If-None-Match") === 'W/"h1"'
        ? notModified('W/"h1"')
        : json({ items: [], links: {} }, { etag: 'W/"h1"' }),
    );
    const history = reads.subPages("core.prefix", P1, "history", null, PAST);
    expect(isImmutable(history.queryKey)).toBe(false);
    expect(history.staleTime).toBe(0);
    expect(reads.subPages("core.prefix", P1, "history", null, null).staleTime).toBe(0);
    // The other sub-resources of the past stay frozen.
    expect(isImmutable(keys.sub("core.prefix", P1, "children", null, PAST))).toBe(true);
    await queryClient.fetchInfiniteQuery(history);
    await queryClient.fetchInfiniteQuery(history);
    expect(seen).toHaveLength(2);
    expect(seen[1]?.headers.get("If-None-Match")).toBe('W/"h1"');
  });

  it("keep each family's freshness", () => {
    const { reads } = fakeCache(() => json({}));
    expect(reads.object("core.prefix", P1, null).staleTime).toBe(FRESHNESS.object.staleTime);
    expect(reads.read("/v1/me", { family: "session" }).staleTime).toBe(5 * 60_000);
    expect(reads.list("/v1/prefixes", { domain: D1 }, null).gcTime).toBe(FRESHNESS.list.gcTime);
    expect(reads.subPages("core.prefix", P1, "children", null, null).gcTime).toBe(
      FRESHNESS.children.gcTime,
    );
  });

  it("read a list page by page, keeping at most the last pages, each with its ETag", async () => {
    const { queryClient, reads, seen } = fakeCache((request) => {
      const cursor = new URL(request.url, "http://x").searchParams.get("cursor");
      const n = Number(cursor ?? 0);
      if (request.headers.get("If-None-Match") === `W/"p${n}"`) return notModified(`W/"p${n}"`);
      return json(
        { items: [{ id: `i${n}` }], links: { next: `/v1/prefixes?cursor=${n + 1}` } },
        { etag: `W/"p${n}"` },
      );
    });
    const options = reads.list("/v1/prefixes", { domain: D1 }, null);
    expect(options.maxPages).toBe(MAX_PAGES);
    const observer = new InfiniteQueryObserver(queryClient, options);
    const stop = observer.subscribe(() => undefined);
    await observer.refetch();
    await observer.fetchNextPage();
    expect(seen.map((r) => r.url)).toEqual([`/v1/prefixes?domain=${D1}`, "/v1/prefixes?cursor=1"]);
    await observer.refetch();
    stop();
    // The first page goes back with its ETag; the second follows the `next` of the first.
    const again = seen.slice(-2);
    expect(again[0]?.headers.get("If-None-Match")).toBe('W/"p0"');
    expect(again[1]?.headers.get("If-None-Match")).toBe('W/"p1"');
  });

  it("keep up to 100 pages of the children of a node, the other lists 10 (021/A34)", async () => {
    const { queryClient, reads, seen } = fakeCache((request) => {
      const n = Number(new URL(request.url, "http://x").searchParams.get("cursor") ?? 0);
      return json({
        items: [{ id: `c${n}` }],
        links: { next: `/v1/prefixes/${P1}/children?cursor=${n + 1}` },
      });
    });
    const children = reads.subPages("core.prefix", P1, "children", null, null);
    expect(children.maxPages).toBe(CHILDREN_PAGES);
    expect(reads.subPages("core.domain", D1, "roots", null, null).maxPages).toBe(CHILDREN_PAGES);
    expect(reads.subPages("core.prefix", P1, "history", null, null).maxPages).toBe(MAX_PAGES);
    expect(reads.list("/v1/prefixes", null, null).maxPages).toBe(MAX_PAGES);
    const observer = new InfiniteQueryObserver(queryClient, children);
    const stop = observer.subscribe(() => undefined);
    await observer.refetch();
    for (let page = 1; page <= MAX_PAGES; page++) await observer.fetchNextPage();
    stop();
    // The eleventh page read, the first kept: 11 children, the first one still there.
    const data = observer.getCurrentResult().data!;
    expect(data.pages).toHaveLength(MAX_PAGES + 1);
    expect(data.pages[0]!.data.items).toEqual([{ id: "c0" }]);
    expect(seen.at(-1)!.url).toBe(`/v1/prefixes/${P1}/children?cursor=10`);
  });

  it("keep the objects of the pages kept once the first is dropped, matched by cursor (task 021-22f)", async () => {
    const { queryClient, reads } = fakeCache((request) => {
      const n = Number(new URL(request.url, "http://x").searchParams.get("cursor") ?? 0);
      return json(
        {
          items: [{ id: `i${n}`, links: { self: `/v1/prefixes/i${n}` } }],
          links: { next: `/v1/prefixes?cursor=${n + 1}` },
        },
        { etag: `W/"p${n}"` },
      );
    });
    const observer = new InfiniteQueryObserver(queryClient, reads.list("/v1/prefixes", null, null));
    const stop = observer.subscribe(() => undefined);
    await observer.refetch();
    for (let page = 1; page < MAX_PAGES; page++) await observer.fetchNextPage();
    const before = observer.getCurrentResult().data!;
    expect(before.pages).toHaveLength(MAX_PAGES);
    // The eleventh page: the first dropped, each other one a place up, the same object.
    await observer.fetchNextPage();
    const after = observer.getCurrentResult().data!;
    expect(after.pages).toHaveLength(MAX_PAGES);
    expect(after.pages[0]!.data.items[0]!.id).toBe("i1");
    after.pages.slice(0, -1).forEach((page, i) => {
      expect(page).toBe(before.pages[i + 1]);
      expect(page.data.items[0]).toBe(before.pages[i + 1]!.data.items[0]);
    });
    // Read again unchanged: the same data, page by page.
    await observer.refetch();
    stop();
    const again = observer.getCurrentResult().data!;
    expect(again.pages[3]!.data.items[0]).toBe(after.pages[3]!.data.items[0]);
  });

  it("keep the unchanged rows of a list read again under new cursors, never a changed one (task 021-22f)", async () => {
    // Each read of the first page gives cursors of its own (`as_of` frozen at the read):
    // the list read again comes back page by page under cursors the cache never saw.
    let reading = 0;
    let renamed = false;
    const { queryClient, reads } = fakeCache((request) => {
      const cursor = new URL(request.url, "http://x").searchParams.get("cursor");
      if (cursor === null) reading++;
      const n = Number((cursor ?? "0.0").split(".")[0]);
      const items = [0, 1].map((j) => ({
        id: `i${n}-${j}`,
        name: renamed && (n === 0 || n === 3) && j === 1 ? "renommée" : `ligne ${n}-${j}`,
        links: { self: `/v1/prefixes/i${n}-${j}` },
      }));
      return json({ items, links: { next: `/v1/prefixes?cursor=${n + 1}.${reading}` } });
    });
    const observer = new InfiniteQueryObserver(queryClient, reads.list("/v1/prefixes", null, null));
    const stop = observer.subscribe(() => undefined);
    await observer.refetch();
    for (let page = 1; page < MAX_PAGES; page++) await observer.fetchNextPage();
    const before = observer.getCurrentResult().data!;
    renamed = true;
    await observer.refetch();
    stop();
    const after = observer.getCurrentResult().data!;
    expect(after.pageParams[1]).not.toBe(before.pageParams[1]);
    // The first page changed, its renamed row a new object, its other row kept.
    expect(after.pages[0]).not.toBe(before.pages[0]);
    expect(after.pages[0]!.data.items[0]).toBe(before.pages[0]!.data.items[0]);
    expect(after.pages[0]!.data.items[1]).not.toBe(before.pages[0]!.data.items[1]);
    expect(after.pages[0]!.data.items[1]).toMatchObject({ name: "renommée" });
    // Every unchanged row of the other pages kept, whatever their new cursors; the renamed
    // one of the fourth page new, though its id is the same.
    for (let i = 1; i < MAX_PAGES; i++)
      after.pages[i]!.data.items.forEach((item, j) => {
        if (i === 3 && j === 1) expect(item).not.toBe(before.pages[i]!.data.items[j]);
        else expect(item, `${i}-${j}`).toBe(before.pages[i]!.data.items[j]);
      });
    expect(after.pages[3]!.data.items[1]).toMatchObject({ name: "renommée" });
  });

  it("share the pages by cursor: an unchanged page kept, another one new, plain data by value", () => {
    const page = (id: string) => ({ data: { items: [{ id }] }, etag: null });
    const before = { pages: [page("a"), page("b")], pageParams: [null, "c1"] };
    expect(sharePages(before, structuredClone(before))).toBe(before);
    const moved = sharePages(before, {
      pages: [page("b"), page("c")],
      pageParams: ["c1", "c2"],
    }) as typeof before;
    expect(moved.pages[0]).toBe(before.pages[1]);
    expect(moved.pages[1]!.data.items[0]!.id).toBe("c");
    // Changed under its cursor: its unchanged items kept, the changed one new.
    const two = {
      pages: [{ data: { items: [{ id: "a" }, { id: "b" }] }, etag: "1" }],
      pageParams: [null],
    };
    const changed = sharePages(two, {
      pages: [{ data: { items: [{ id: "a" }, { id: "b2" }] }, etag: "2" }],
      pageParams: [null],
    }) as typeof two;
    expect(changed.pages[0]!.data.items[0]).toBe(two.pages[0]!.data.items[0]);
    expect(changed.pages[0]!.data.items[1]!.id).toBe("b2");
    const plain = { id: "x", n: 1 };
    expect(sharePages(plain, { id: "x", n: 1 })).toBe(plain);
  });

  it("read again without If-None-Match the queries marked unconditional", async () => {
    const { queryClient, reads, seen } = fakeCache(() =>
      json(prefix("10.0.0.0/24"), { etag: '"1.a"' }),
    );
    const options = reads.object("core.prefix", P1, null);
    await queryClient.fetchQuery(options);
    reads.unconditional((query) => query.queryKey[1] === "core.prefix");
    await queryClient.refetchQueries({ queryKey: options.queryKey });
    await queryClient.refetchQueries({ queryKey: options.queryKey });
    expect(seen.map((r) => r.headers.get("If-None-Match"))).toEqual([null, null, '"1.a"']);
  });
});
