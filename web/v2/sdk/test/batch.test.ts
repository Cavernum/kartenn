// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { QueryObserver } from "@tanstack/query-core";
import { afterEach, describe, expect, it } from "vitest";
import { ApiProblem } from "../src/api/problem.ts";
import { resetContext } from "../src/context.ts";
import { BATCH_MAX } from "../src/data/batch.ts";
import type { Entry } from "../src/data/cache.ts";
import { fakeCache, json, notModified, type Seen } from "./helpers.ts";

// The reads of resolved properties batched by window (task 021-11p10-c, REQ-006-40): one
// request per collection and dates, two hundred identifiers at most, each answer kept under
// the key of the read of its object.

afterEach(() => resetContext());

const id = (i: number) => `0190a000-0000-7000-8000-${String(i).padStart(12, "0")}`;
const PAST = { validAt: null, asOf: "2026-01-01T00:00:00Z" };
const resolved = (zone: string) => ({
  properties: { "custom.zone": zone },
  sources: {},
  masked: [],
});

/** The identifiers a batch asks, `null` for another request. */
const asked = (request: Seen) => {
  const url = new URL(request.url, "http://localhost");
  return /^\/v1\/[a-z-]+\/properties$/.test(url.pathname) ? url.searchParams.getAll("id") : null;
};

/** A server whose batches answer every identifier asked but those of `left`. */
function batches(
  zone: (id: string) => string = () => "ouest",
  left: ReadonlySet<string> = new Set(),
) {
  return fakeCache((request) => {
    const ids = asked(request);
    if (ids === null) return json({ ...resolved("seul"), links: {} }, { etag: '"1.a"' });
    return json({
      items: ids
        .filter((one) => !left.has(one))
        .map((one) => ({ id: one, ...resolved(zone(one)) })),
    });
  });
}

const batchesOf = (seen: readonly Seen[]) => seen.filter((request) => asked(request) !== null);

describe("the reads of properties batched (task 021-11p10-c)", () => {
  it("forty reads of a window: one request, each answer under its own key", async () => {
    const { queryClient, reads, seen } = batches((one) => one.slice(-2));
    const options = Array.from({ length: 40 }, (_, i) =>
      reads.sub("core.prefix", id(i), "properties"),
    );
    await Promise.all(options.map((one) => queryClient.fetchQuery(one)));
    expect(seen).toHaveLength(1);
    expect(asked(seen[0]!)).toEqual(options.map((_, i) => id(i)));
    const entry = queryClient.getQueryData<Entry>(options[7]!.queryKey);
    expect(entry).toEqual({
      data: {
        ...resolved("07"),
        links: { self: `/v1/prefixes/${id(7)}/properties`, object: `/v1/prefixes/${id(7)}` },
      },
      etag: null,
    });
  });

  it("two dates: two requests, each with its dates", async () => {
    const { queryClient, reads, seen } = batches();
    await Promise.all(
      [null, PAST].flatMap((time) =>
        [1, 2, 3].map((i) =>
          queryClient.fetchQuery(reads.sub("core.prefix", id(i), "properties", null, time)),
        ),
      ),
    );
    expect(seen).toHaveLength(2);
    expect(
      seen.map((request) => new URL(request.url, "http://localhost").searchParams.get("as_of")),
    ).toEqual([null, PAST.asOf]);
  });

  it("two collections: two requests", async () => {
    const { queryClient, reads, seen } = batches();
    await Promise.all(
      ["core.prefix", "core.address"].flatMap((type) =>
        [1, 2].map((i) => queryClient.fetchQuery(reads.sub(type, id(i), "properties"))),
      ),
    );
    expect(seen.map((request) => request.url.split("?")[0])).toEqual([
      "/v1/prefixes/properties",
      "/v1/addresses/properties",
    ]);
  });

  it("201 reads: two requests, two hundred identifiers in the first", async () => {
    const { queryClient, reads, seen } = batches();
    await Promise.all(
      Array.from({ length: BATCH_MAX + 1 }, (_, i) =>
        queryClient.fetchQuery(reads.sub("core.prefix", id(i), "properties")),
      ),
    );
    expect(seen).toHaveLength(2);
    expect(asked(seen[0]!)).toHaveLength(BATCH_MAX);
  });

  it("a read alone in its window: the read of its object, with its ETag", async () => {
    const { queryClient, reads, seen } = batches();
    const options = reads.sub("core.prefix", id(1), "properties");
    await queryClient.fetchQuery(options);
    expect(seen.map((request) => request.url)).toEqual([`/v1/prefixes/${id(1)}/properties`]);
    expect(queryClient.getQueryData<Entry>(options.queryKey)?.etag).toBe('"1.a"');
  });

  it("a read with keys of its own is not batched", async () => {
    const { queryClient, reads, seen } = batches();
    await Promise.all(
      [1, 2].map((i) =>
        queryClient.fetchQuery(reads.sub("core.prefix", id(i), "properties", { key: "x" })),
      ),
    );
    expect(batchesOf(seen)).toHaveLength(0);
    expect(seen).toHaveLength(2);
  });

  it("an identifier left out: the refusal of the read alone, never tried again", async () => {
    const { queryClient, reads, seen } = batches(undefined, new Set([id(2)]));
    const [kept, left] = [1, 2].map((i) => reads.sub("core.prefix", id(i), "properties"));
    const results = await Promise.allSettled([
      queryClient.fetchQuery(kept!),
      queryClient.fetchQuery(left!),
    ]);
    expect(results[0]?.status).toBe("fulfilled");
    const refused = (results[1] as PromiseRejectedResult).reason as ApiProblem;
    expect(refused).toBeInstanceOf(ApiProblem);
    expect([refused.status, refused.code]).toEqual([404, "not-found"]);
    expect(seen).toHaveLength(1);
  });

  it("a read given up alone does not give up the batch", async () => {
    let answer: (() => void) | undefined;
    const { queryClient, reads, seen } = fakeCache(async (request) => {
      await new Promise<void>((resolve) => (answer = resolve));
      return json({ items: asked(request)!.map((one) => ({ id: one, ...resolved("est") })) });
    });
    const options = [1, 2, 3].map((i) => reads.sub("core.prefix", id(i), "properties"));
    const stops = options.map((one) => new QueryObserver(queryClient, one).subscribe(() => {}));
    await expect.poll(() => answer).toBeDefined();
    await queryClient.cancelQueries({ queryKey: options[0]!.queryKey });
    answer!();
    await expect.poll(() => queryClient.getQueryData(options[1]!.queryKey)).toBeDefined();
    expect(queryClient.getQueryData(options[2]!.queryKey)).toBeDefined();
    expect(queryClient.getQueryData(options[0]!.queryKey)).toBeUndefined();
    expect(seen).toHaveLength(1);
    stops.forEach((stop) => stop());
  });

  it("every read of a batch given up: its request given up", async () => {
    let aborted: AbortSignal | undefined;
    const { client, queryClient, reads } = fakeCache(() => new Promise<Response>(() => {}));
    const request = client.request.bind(client);
    client.request = (method, url, init) => {
      aborted = init?.signal;
      return request(method, url, init);
    };
    const options = [1, 2].map((i) => reads.sub("core.prefix", id(i), "properties"));
    const stops = options.map((one) => new QueryObserver(queryClient, one).subscribe(() => {}));
    await expect.poll(() => aborted).toBeDefined();
    await queryClient.cancelQueries({ queryKey: options[0]!.queryKey });
    expect(aborted!.aborted).toBe(false);
    await queryClient.cancelQueries({ queryKey: options[1]!.queryKey });
    expect(aborted!.aborted).toBe(true);
    stops.forEach((stop) => stop());
  });

  it("an answer of a batch equal to the entry kept gives it back; a changed one, anew", async () => {
    let zone = "ouest";
    const { queryClient, reads } = batches((one) => (one === id(2) ? zone : "ouest"));
    const options = [1, 2, 3].map((i) => reads.sub("core.prefix", id(i), "properties"));
    await Promise.all(options.map((one) => queryClient.fetchQuery(one)));
    const before = options.map((one) => queryClient.getQueryData(one.queryKey));
    zone = "sud";
    await queryClient.refetchQueries({ queryKey: ["v1", "core.prefix"] });
    const after = options.map((one) => queryClient.getQueryData(one.queryKey));
    expect(after.map((entry, i) => entry === before[i])).toEqual([true, false, true]);
  });

  it("a 304 of a read alone keeps its entry", async () => {
    const { queryClient, reads, seen } = fakeCache((request) =>
      request.headers.get("If-None-Match") === '"1.a"'
        ? notModified('"1.a"')
        : json({ ...resolved("seul"), links: {} }, { etag: '"1.a"' }),
    );
    const options = reads.sub("core.prefix", id(1), "properties");
    await queryClient.fetchQuery(options);
    const first = queryClient.getQueryData(options.queryKey);
    await queryClient.refetchQueries({ queryKey: options.queryKey });
    expect(seen).toHaveLength(2);
    expect(queryClient.getQueryData(options.queryKey)).toBe(first);
  });
});
