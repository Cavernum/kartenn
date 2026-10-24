// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import type { components } from "../src/api/schema";
import { refsOf, versionOf } from "../src/data/dependents.ts";
import { createInvalidation } from "../src/data/invalidate.ts";
import { keys } from "../src/data/keys.ts";
import { D1, P1, P2, fakeCache, json } from "./helpers.ts";

type Prefix = components["schemas"]["Prefix"];
type MachineCard = components["schemas"]["MachineCard"];

const M1 = "00000000-0000-4000-8000-0000000000a1";
const I1 = "00000000-0000-4000-8000-0000000000b1";
const A1 = "00000000-0000-4000-8000-0000000000c1";
const S1 = "00000000-0000-4000-8000-0000000000e1";
const PAST = { asOf: "2026-01-01T00:00:00Z" };

const prefix = (id: string, version = "c0"): Prefix => ({
  id,
  version,
  value: "10.0.0.0/24",
  nature: "network",
  point_to_point: false,
  anycast: false,
  public: false,
  domain: { id: D1, href: `/v1/domains/${D1}`, slug: "lab" },
  links: { self: `/v1/prefixes/${id}`, history: "h", properties: "p" },
});

const card: MachineCard = {
  id: M1,
  name: "web-1",
  kind: "virtual",
  href: `/v1/machines/${M1}`,
  site: { id: S1 },
  host: { cluster: { id: "not-a-uuid" } },
  interfaces: [
    {
      id: I1,
      name: "eth0",
      enabled: true,
      href: `/v1/interfaces/${I1}`,
      addresses: [{ id: A1, value: "10.0.0.5", primary: true }],
    },
  ],
};

describe("references of a body (REQ-021-23)", () => {
  it("finds the summary of the domain of a prefix", () => {
    expect(refsOf(prefix(P1))).toEqual(new Set([P1, D1]));
  });

  it("finds the site, interfaces and addresses of a machine card, UUIDs only", () => {
    expect(refsOf(card)).toEqual(new Set([M1, S1, I1, A1]));
  });

  it("reads the version an object body states (006/H19), null when it states none", () => {
    expect(versionOf(prefix(P1, "c1"))).toBe("c1");
    expect(versionOf({ id: P1 })).toBeNull();
    expect(versionOf(null)).toBeNull();
  });
});

describe("invalidation by object (REQ-021-23, 24)", () => {
  function setup(bodies: Record<string, unknown>) {
    const cache = fakeCache((request) => json(bodies[request.url.split("?")[0]!] ?? {}));
    const invalidation = createInvalidation(cache.queryClient, cache.reads);
    const stale = (key: readonly unknown[]) =>
      cache.queryClient.getQueryState(key)?.isInvalidated ?? false;
    return { ...cache, invalidation, stale };
  }

  it("invalidates the object, its lists and the queries that show it, never the past", async () => {
    const { queryClient, reads, invalidation, stale } = setup({
      [`/v1/prefixes/${P1}`]: prefix(P1),
      [`/v1/prefixes/${P2}`]: prefix(P2),
      [`/v1/domains/${D1}`]: { id: D1, slug: "lab" },
      "/v1/prefixes": { items: [prefix(P1)], links: {} },
    });
    const p1 = reads.object("core.prefix", P1, null);
    const p2 = reads.object("core.prefix", P2, null);
    const domain = reads.object("core.domain", D1, null);
    const past = reads.object("core.domain", D1, PAST);
    const list = reads.list("/v1/prefixes", {}, null);
    await Promise.all([p1, p2, domain, past].map((o) => queryClient.fetchQuery(o)));
    await queryClient.fetchInfiniteQuery(list);

    // A rename of the domain: its own key, and the prefixes whose body shows it (H19).
    await invalidation.objects([{ type: "core.domain", id: D1 }], "c1");
    expect(stale(domain.queryKey)).toBe(true);
    expect(stale(p1.queryKey)).toBe(true);
    expect(stale(p2.queryKey)).toBe(true);
    expect(stale(list.queryKey)).toBe(true);
    expect(stale(past.queryKey)).toBe(false);
  });

  it("leaves an object whose version is already the revision", async () => {
    const { queryClient, reads, invalidation, stale } = setup({
      [`/v1/prefixes/${P1}`]: prefix(P1, "c1"),
    });
    const p1 = reads.object("core.prefix", P1, null);
    await queryClient.fetchQuery(p1);
    await invalidation.objects([{ type: "prefix", id: P1 }], "c1");
    expect(stale(p1.queryKey)).toBe(false);
    await invalidation.objects([{ type: "prefix", id: P1 }], "c2");
    expect(stale(p1.queryKey)).toBe(true);
  });

  it("reads again an object whose body states no version", async () => {
    const { queryClient, reads, invalidation, stale } = setup({
      [`/v1/prefixes/${P1}`]: { id: P1 },
    });
    const p1 = reads.object("core.prefix", P1, null);
    await queryClient.fetchQuery(p1);
    await invalidation.objects([{ type: "prefix", id: P1 }], "c1");
    expect(stale(p1.queryKey)).toBe(true);
  });

  it("forgets the references of a query removed from the cache", async () => {
    const { queryClient, reads, invalidation } = setup({ [`/v1/prefixes/${P1}`]: prefix(P1) });
    const p1 = reads.object("core.prefix", P1, null);
    await queryClient.fetchQuery(p1);
    expect(invalidation.dependents.showing(D1).size).toBe(1);
    queryClient.removeQueries({ queryKey: keys.allOf("core.prefix", P1) });
    expect(invalidation.dependents.showing(D1).size).toBe(0);
  });

  it("invalidates whole types on a truncated change, read again unconditionally", async () => {
    let n = 0;
    const cache = fakeCache(() => json(prefix(P1), { etag: `"${n++}"` }));
    const invalidation = createInvalidation(cache.queryClient, cache.reads);
    const p1 = cache.reads.object("core.prefix", P1, null);
    const observer = cache.queryClient.getQueryCache().build(cache.queryClient, p1);
    await cache.queryClient.fetchQuery(p1);
    await invalidation.types(["core.prefix"]);
    expect(observer.state.isInvalidated).toBe(true);
    await cache.queryClient.refetchQueries({ queryKey: p1.queryKey });
    expect(cache.seen.map((r) => r.headers.get("If-None-Match"))).toEqual([null, null]);
  });
});
