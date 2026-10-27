// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { InfiniteQueryObserver, QueryObserver } from "@tanstack/query-core";
import { afterEach, describe, expect, it } from "vitest";
import { resetContext, setContext } from "../src/context.ts";
import { FRESHNESS, IMMUTABLE, MAX_PAGES, fallback, type Entry } from "../src/data/cache.ts";
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
