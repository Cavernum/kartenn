// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { QueryObserver } from "@tanstack/query-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createClient } from "../src/api/client.ts";
import { ApiProblem } from "../src/api/problem.ts";
import { resetContext } from "../src/context.ts";
import { BATCH_FLIGHTS, BATCH_MAX, createBatcher } from "../src/data/batch.ts";
import type { Entry } from "../src/data/cache.ts";
import { timeKey } from "../src/data/keys.ts";
import { fakeCache, fakeServer, json, notModified, problem, type Seen } from "./helpers.ts";

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

/** A batch answered, every identifier asked in it. */
const answered = (ids: readonly string[]) =>
  json({ items: ids.map((one) => ({ id: one, ...resolved("est") })) });

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

describe("a batch refused goes again as one batch (task 021-11p10-c)", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  const refusal = (status: number) =>
    new Response(JSON.stringify({ type: "urn:kartenn:problem:x", title: "x", status }), {
      status,
      headers: { "Content-Type": "application/problem+json", "Retry-After": "1" },
    });
  type Answer = (ids: readonly string[]) => Response;

  /** A hundred rows observed over a server giving `answers` to its batches in turn. */
  function hundred(...answers: Answer[]) {
    const cache = fakeCache((request) => (answers.shift() ?? answered)(asked(request)!));
    const options = Array.from({ length: 100 }, (_, i) =>
      cache.reads.sub("core.prefix", id(i), "properties"),
    );
    const observers = options.map((one) => new QueryObserver(cache.queryClient, one));
    const stops = observers.map((observer) => observer.subscribe(() => {}));
    const sizes = () => batchesOf(cache.seen).map((request) => asked(request)!.length);
    return { ...cache, options, observers, sizes, stop: () => stops.forEach((s) => s()) };
  }

  it("a 429 Retry-After: 1 every time: three batches of 100, then the refusal on each row", async () => {
    const { observers, seen, sizes, stop } = hundred(
      () => refusal(429),
      () => refusal(429),
      () => refusal(429),
    );
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sizes()).toEqual([100, 100, 100]);
    expect(seen).toHaveLength(3);
    const results = observers.map((observer) => observer.getCurrentResult());
    expect(results.every((result) => (result.error as ApiProblem | null)?.status === 429)).toBe(
      true,
    );
    stop();
  });

  it.each([
    ["a 429", () => refusal(429)],
    ["a 503", () => refusal(503)],
    ["a 500", () => problem(500, "internal")],
  ] as [string, () => Response][])(
    "%s, then the answer: one batch read again, every row shown",
    async (_, first) => {
      const { observers, sizes, stop } = hundred(first);
      await vi.advanceTimersByTimeAsync(10_000);
      expect(sizes()).toEqual([100, 100]);
      expect(observers.every((o) => o.getCurrentResult().status === "success")).toBe(true);
      stop();
    },
  );

  it("a failure of the network, then the answer: one batch read again", async () => {
    let failed = false;
    const { observers, sizes, stop } = hundred((ids) => {
      if (failed) return answered(ids);
      failed = true;
      throw new TypeError("network");
    });
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sizes()).toEqual([100, 100]);
    expect(observers.every((o) => o.getCurrentResult().status === "success")).toBe(true);
    stop();
  });

  it("a read given up while the batch waits its Retry-After: the batch again without it", async () => {
    const { queryClient, options, sizes, stop } = hundred(() => refusal(429));
    await vi.advanceTimersByTimeAsync(500);
    await queryClient.cancelQueries({ queryKey: options[42]!.queryKey });
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sizes()).toEqual([100, 99]);
    stop();
  });
});

describe("the batches in flight, bounded (task 021-11p10-c)", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("thirty windows under a latency of 500 ms: four batches in flight at most, rows given up never asked", async () => {
    let flying = 0;
    let most = 0;
    const abandoned = new Set<string>();
    const askedAfterAbandon: string[] = [];
    const server = fakeServer(async (request) => {
      const ids = asked(request)!;
      askedAfterAbandon.push(...ids.filter((one) => abandoned.has(one)));
      most = Math.max(most, ++flying);
      await new Promise((resolve) => setTimeout(resolve, 500));
      flying--;
      return answered(ids);
    });
    const batcher = createBatcher(createClient({ fetch: server.fetch }));
    const alone = () => Promise.reject(new Error("never alone here"));
    const frames: AbortController[][] = [];
    const reads: Promise<unknown>[] = [];
    for (let frame = 0; frame < 30; frame++) {
      // A frame of a fast scroll: twenty rows come in view, those of two frames ago leave.
      const controllers = Array.from({ length: 20 }, (_, i) => {
        const controller = new AbortController();
        const row = id(frame * 20 + i);
        reads.push(
          batcher
            .load("/v1/prefixes", row, timeKey(null), controller.signal, alone)
            .catch(() => {}),
        );
        controller.signal.addEventListener("abort", () => abandoned.add(row));
        return controller;
      });
      frames.push(controllers);
      for (const controller of frames[frame - 2] ?? []) controller.abort();
      await vi.advanceTimersByTimeAsync(16);
    }
    await vi.advanceTimersByTimeAsync(10_000);
    await Promise.all(reads);
    expect(most).toBeLessThanOrEqual(BATCH_FLIGHTS);
    expect(askedAfterAbandon).toEqual([]);
    // The rows of the last frames, never given up, all read.
    const sent = server.seen.flatMap((request) => asked(request)!);
    expect(sent).toEqual(expect.arrayContaining(frames.at(-1)!.map((_, i) => id(29 * 20 + i))));
  });
});
