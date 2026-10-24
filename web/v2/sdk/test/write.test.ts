// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import type { Entry } from "../src/data/cache.ts";
import { createInvalidation } from "../src/data/invalidate.ts";
import {
  createWrites,
  fieldOf,
  fingerprint,
  mergePatch,
  type WriteOutcome,
} from "../src/data/write.ts";
import { D1, P1, P2, fakeCache, json, problem, type Handler, type Seen } from "./helpers.ts";

const C1 = "00000000-0000-4000-8000-0000000000f1";
const prefix = (value: string) => ({
  id: P1,
  value,
  nature: "network",
  domain: { id: D1, href: `/v1/domains/${D1}`, slug: "lab" },
});

/** A server whose reads answer the current prefix and whose `PATCH` answers `write`. */
function setup(write: (request: Seen) => Response | Promise<Response>) {
  let current = prefix("10.0.0.0/24");
  let version = 1;
  const handler: Handler = (request) => {
    if (request.method === "PATCH") return write(request);
    if (request.method === "POST") return write(request);
    return json(current, { etag: `"${version}.x"` });
  };
  const cache = fakeCache(handler);
  const invalidation = createInvalidation(cache.queryClient, cache.reads);
  const writes = createWrites({ ...cache, invalidation, pause: async () => undefined });
  const options = cache.reads.object("core.prefix", P1, null);
  const change = (value: string) => {
    current = prefix(value);
    version += 1;
  };
  return { ...cache, writes, options, change };
}

/** The values the cache held for the object, in order, while `run` ran. */
async function watch(cache: ReturnType<typeof setup>, run: () => Promise<unknown>) {
  const values: string[] = [];
  const stop = cache.queryClient.getQueryCache().subscribe((event) => {
    if (event.type !== "updated" || event.query.queryHash !== cacheHash(cache)) return;
    const data = (event.query.state.data as Entry<{ value: string }> | undefined)?.data;
    if (data && values.at(-1) !== data.value) values.push(data.value);
  });
  const result = await run();
  stop();
  return { values, result };
}

function cacheHash(cache: ReturnType<typeof setup>): string {
  return cache.queryClient.getQueryCache().find({ queryKey: cache.options.queryKey })!.queryHash;
}

const applied = () =>
  json({
    changeset: { id: C1, href: `/v1/changesets/${C1}` },
    state: "applied",
    warnings: [],
    diff: [],
    cascaded: [],
  });

describe("quick edit (REQ-021-16, 24; D-7)", () => {
  it("200: shows the value at once, sends If-Match and the motive, then reads the object again", async () => {
    const cache = setup((request) => {
      cache.change((request.body as { value: string }).value);
      return applied();
    });
    await cache.queryClient.fetchQuery(cache.options);
    const { values, result } = await watch(cache, () =>
      cache.writes.quick({
        key: cache.options.queryKey,
        patch: { value: "10.0.1.0/24" },
        motive: "resize",
      }),
    );
    expect(result).toEqual({ kind: "applied", changeset: C1, warnings: [] });
    const patch = cache.seen.find((r) => r.method === "PATCH")!;
    // Applied at once when its author holds `apply_own` (024, REQ-024-13).
    expect(patch.url).toBe(`/v1/prefixes/${P1}?apply=own`);
    expect(patch.headers.get("If-Match")).toBe('"1.x"');
    expect(patch.headers.get("Content-Type")).toBe("application/merge-patch+json");
    expect(patch.body).toEqual({ value: "10.0.1.0/24", motive: "resize" });
    expect(values[0]).toBe("10.0.1.0/24");
    // The object is invalidated by its own write (REQ-021-24), not left to the stream.
    expect(cache.queryClient.getQueryState(cache.options.queryKey)?.isInvalidated).toBe(true);
  });

  it("202: proposed, back to the value read, never shown applied", async () => {
    const cache = setup(() =>
      json(
        { changeset: { id: C1, href: "h" }, state: "pending", kind: "approval", diff: [] },
        { status: 202 },
      ),
    );
    await cache.queryClient.fetchQuery(cache.options);
    const { values, result } = await watch(cache, () =>
      cache.writes.quick({
        key: cache.options.queryKey,
        patch: { value: "10.0.9.0/24" },
        motive: "m",
      }),
    );
    expect(result).toEqual({ kind: "proposed", changeset: C1, review: "approval", reason: null });
    expect(values).toEqual(["10.0.9.0/24", "10.0.0.0/24"]);
  });

  it("202 to ?apply=own: proposed with the code of its reason", async () => {
    for (const code of ["changeset.apply_own.missing", "changeset.approval.self"]) {
      const reason = { code, message: "held", objects: [P1] };
      const cache = setup(() =>
        json(
          {
            changeset: { id: C1, href: "h" },
            state: "pending",
            kind: "approval",
            diff: [],
            reason,
          },
          { status: 202 },
        ),
      );
      await cache.queryClient.fetchQuery(cache.options);
      const result = await cache.writes.quick({
        key: cache.options.queryKey,
        patch: { value: "10.0.9.0/24" },
        motive: "m",
      });
      expect(result).toMatchObject({ kind: "proposed", reason: { code } });
    }
  });

  it("412: reads the object again and gives the input back", async () => {
    const cache = setup(() => problem(412, "precondition-failed"));
    await cache.queryClient.fetchQuery(cache.options);
    cache.change("10.0.7.0/24");
    const result = await cache.writes.quick({
      key: cache.options.queryKey,
      patch: { value: "10.0.1.0/24" },
      motive: "m",
    });
    expect(result.kind).toBe("conflict");
    if (result.kind !== "conflict") return;
    expect((result.current?.data as { value: string }).value).toBe("10.0.7.0/24");
    expect(result.current?.etag).toBe('"2.x"');
    expect(result.input).toEqual({ value: "10.0.1.0/24" });
  });

  it("422: back to the value read, violations by member with their codes", async () => {
    const cache = setup(() =>
      problem(422, "validation", {
        violations: [
          {
            rule: "core.prefix.overlap",
            severity: "error",
            detail: "overlaps",
            pointer: "/value",
            subjects: [],
          },
          { rule: "core.final", severity: "error", detail: "final", pointer: null, subjects: [] },
        ],
      }),
    );
    await cache.queryClient.fetchQuery(cache.options);
    const { values, result } = await watch(cache, () =>
      cache.writes.quick({
        key: cache.options.queryKey,
        patch: { value: "10.0.1.0/24" },
        motive: "m",
      }),
    );
    expect(values).toEqual(["10.0.1.0/24", "10.0.0.0/24"]);
    expect(result).toMatchObject({ kind: "invalid", input: { value: "10.0.1.0/24" } });
    const invalid = result as Extract<WriteOutcome, { kind: "invalid" }>;
    expect(invalid.byField["value"]?.map((v) => v.rule)).toEqual(["core.prefix.overlap"]);
    expect(invalid.general.map((v) => v.rule)).toEqual(["core.final"]);
    expect(invalid.problem.code).toBe("validation");
  });

  it("403, or a rule refusing for rights: forbidden, back to the value read", async () => {
    for (const answer of [
      () => problem(403, "forbidden"),
      () =>
        problem(422, "validation", {
          violations: [
            { rule: "core.object.forbidden", severity: "error", detail: "no", subjects: [] },
          ],
        }),
    ]) {
      const cache = setup(answer);
      await cache.queryClient.fetchQuery(cache.options);
      const result = await cache.writes.quick({
        key: cache.options.queryKey,
        patch: { value: "10.0.1.0/24" },
        motive: "m",
      });
      expect(result.kind).toBe("forbidden");
      const entry = cache.queryClient.getQueryData<Entry<{ value: string }>>(
        cache.options.queryKey,
      );
      expect(entry?.data.value).toBe("10.0.0.0/24");
    }
  });

  it("no optimism under valid_at, no write at all under as_of", async () => {
    const cache = setup(() => applied());
    const dated = cache.reads.object("core.prefix", P1, { validAt: "2027-01-01T00:00:00Z" });
    await cache.queryClient.fetchQuery(dated);
    const { values } = await watch({ ...cache, options: dated }, () =>
      cache.writes.quick({ key: dated.queryKey, patch: { value: "10.0.1.0/24" }, motive: "m" }),
    );
    expect(values).not.toContain("10.0.1.0/24");
    const past = cache.reads.object("core.prefix", P2, { asOf: "2026-01-01T00:00:00Z" });
    await expect(
      cache.writes.quick({ key: past.queryKey, patch: {}, motive: "m", ifMatch: '"x"' }),
    ).rejects.toThrow(/as_of/);
  });

  it("the network failing: back to the value read, the input kept", async () => {
    const cache = setup(() => Promise.reject(new TypeError("offline")));
    await cache.queryClient.fetchQuery(cache.options);
    const result = await cache.writes.quick({
      key: cache.options.queryKey,
      patch: { value: "10.0.1.0/24" },
      motive: "m",
    });
    expect(result).toEqual({ kind: "failed", input: { value: "10.0.1.0/24" }, problem: null });
    const entry = cache.queryClient.getQueryData<Entry<{ value: string }>>(cache.options.queryKey);
    expect(entry?.data.value).toBe("10.0.0.0/24");
  });
});

describe("draft submission and simulation (W10, ADR 0037)", () => {
  const submission = {
    motive: "m",
    operations: [{ op: "close", target: { id: P1 } }] as never,
  };

  it("simulates under a key by digest, a 422 being a report", async () => {
    const cache = setup(() =>
      problem(422, "validation", {
        violations: [
          { rule: "core.x", severity: "error", detail: "no", operation: 0, subjects: [] },
        ],
      }),
    );
    const options = cache.writes.simulate(submission);
    expect(options.queryKey).toEqual(["simulate", fingerprint(submission)]);
    const report = await cache.queryClient.fetchQuery(options);
    expect(report.valid).toBe(false);
    expect(report.violations[0]?.rule).toBe("core.x");
    expect(cache.seen[0]?.url).toBe("/v1/changesets?simulate=true");
  });

  it("submits under the key given, tries a busy key again, and invalidates what it touched", async () => {
    let calls = 0;
    const cache = setup(() =>
      calls++ === 0
        ? problem(409, "in-progress")
        : json(
            {
              changeset: { id: C1, href: "h" },
              state: "applied",
              warnings: [],
              diff: [],
              cascaded: [],
            },
            { status: 201 },
          ),
    );
    await cache.queryClient.fetchQuery(cache.options);
    const result = await cache.writes.submit(submission, "key-1");
    expect(result.kind).toBe("applied");
    const posts = cache.seen.filter((r) => r.method === "POST");
    expect(posts.map((r) => r.headers.get("Idempotency-Key"))).toEqual(["key-1", "key-1"]);
    expect(cache.queryClient.getQueryState(cache.options.queryKey)?.isInvalidated).toBe(true);
  });
});

describe("helpers", () => {
  it("apply a merge patch (RFC 7396)", () => {
    expect(mergePatch({ a: 1, b: { c: 2, d: 3 } }, { a: null, b: { c: 4 }, e: [1] })).toEqual({
      b: { c: 4, d: 3 },
      e: [1],
    });
  });

  it("name the member of a pointer", () => {
    expect(fieldOf("/value")).toBe("value");
    expect(fieldOf("/properties/core.owner")).toBe("properties");
    expect(fieldOf(null)).toBeNull();
  });
});
