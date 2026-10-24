// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { QueryObserver } from "@tanstack/query-core";
import { afterEach, describe, expect, it, vi } from "vitest";
import { FRESHNESS, type Entry } from "../src/data/cache.ts";
import {
  ACTIVE_KEY,
  createDrafts,
  SIMULATION_WAIT,
  type Draft,
  type Gone,
  type StorageLike,
} from "../src/data/drafts.ts";
import { createInvalidation } from "../src/data/invalidate.ts";
import { createWrites } from "../src/data/write.ts";
import { P1, fakeCache, json, problem, type Seen } from "./helpers.ts";

const D1 = "00000000-0000-4000-8000-0000000000a1";
const D2 = "00000000-0000-4000-8000-0000000000a2";
const C1 = "00000000-0000-4000-8000-0000000000f1";

function memory(): StorageLike & { map: Map<string, string> } {
  const map = new Map<string, string>();
  return {
    map,
    getItem: (key) => map.get(key) ?? null,
    setItem: (key, value) => void map.set(key, value),
    removeItem: (key) => void map.delete(key),
  };
}

/** A server holding drafts; `answer` overrides a request when it returns a response. */
function setup(answer: (request: Seen) => Response | undefined = () => undefined) {
  const held = new Map<string, { version: number; operations: unknown[]; title: string }>();
  const body = (id: string): Draft => {
    const d = held.get(id)!;
    return {
      id,
      href: `/v1/drafts/${id}`,
      author: C1,
      title: d.title,
      operation_count: d.operations.length,
      version: d.version,
      created_at: "2026-10-24T00:00:00Z",
      updated_at: "2026-10-24T00:00:00Z",
      reason: {},
      operations: d.operations as Draft["operations"],
    };
  };
  const reply = (id: string, status = 200) =>
    json(body(id), { status, etag: `"${held.get(id)!.version}"` });
  let next = 0;
  const cache = fakeCache((request) => {
    const special = answer(request);
    if (special) return special;
    const [path] = request.url.split("?");
    const match = /^\/v1\/drafts\/([^/]+)(\/\w+)?$/.exec(path!);
    if (path === "/v1/drafts" && request.method === "GET") {
      return json({ items: [...held.keys()].map((id) => body(id)) });
    }
    if (path === "/v1/drafts" && request.method === "POST") {
      const id = [D1, D2][next++]!;
      const { title, operations } = request.body as { title: string; operations?: unknown[] };
      held.set(id, { version: 1, operations: operations ?? [], title });
      return reply(id, 201);
    }
    const id = match?.[1];
    const draft = id ? held.get(id) : undefined;
    if (!id || !draft) return problem(404, "not-found");
    const tag = request.headers.get("If-Match");
    if (tag !== null && tag !== `"${draft.version}"`) return problem(412, "precondition-failed");
    switch (`${request.method} ${match?.[2] ?? ""}`) {
      case "GET ":
        return reply(id);
      case "PATCH ":
        Object.assign(draft, request.body, { version: draft.version + 1 });
        return reply(id);
      case "POST /operations":
        draft.operations.push(...(request.body as { operations: unknown[] }).operations);
        draft.version += 1;
        return reply(id);
      case "DELETE ":
        held.delete(id);
        return new Response(null, { status: 204 });
      case "POST /simulation":
        return json({ valid: true, violations: [], diff: [], sample: [], cascaded: [] });
      case "POST /submission":
        held.delete(id);
        return json(
          { changeset: { id: C1, href: "h" }, state: "applied", warnings: [], diff: [] },
          { status: 201 },
        );
    }
    return problem(405, "method");
  });
  const invalidation = createInvalidation(cache.queryClient, cache.reads);
  const writes = createWrites({ ...cache, invalidation, pause: async () => undefined });
  const storage = memory();
  const drafts = createDrafts({
    ...cache,
    invalidation,
    writes,
    storage: () => storage,
    title: () => "untitled",
  });
  /** Another tab changes the draft. */
  const touch = (id: string) => {
    held.get(id)!.version += 1;
  };
  const stale = (key: readonly unknown[]) =>
    cache.queryClient.getQueryState(key)?.isInvalidated ?? false;
  return { ...cache, held, drafts, storage, touch, stale };
}

const close = { op: "close", target: { id: P1 } } as never;

afterEach(() => {
  vi.useRealTimers();
});

describe("reads of the drafts (021 « Clés et fraîcheur », 024 § A)", () => {
  it("keys by path, family drafts: 30 s fresh, kept 10 min", () => {
    const s = setup();
    const list = s.drafts.list();
    const one = s.drafts.get(D1);
    expect(list.queryKey).toEqual(["v1", "/v1/drafts", null, {}, { valid_at: null, as_of: null }]);
    expect(one.queryKey).toEqual([
      "v1",
      "/v1/drafts/{id}",
      { id: D1 },
      {},
      { valid_at: null, as_of: null },
    ]);
    expect([one.staleTime, one.gcTime]).toEqual([30_000, 600_000]);
    expect(FRESHNESS.stats).toEqual({ staleTime: 30_000, gcTime: 600_000 });
  });
});

describe("writes of the drafts under If-Match (024 § A)", () => {
  it("creates: the draft and its ETag in the cache, the list read again", async () => {
    const s = setup();
    await s.queryClient.fetchQuery(s.drafts.list());
    const outcome = await s.drafts.create({ title: "t", operations: [close] });
    expect(outcome).toMatchObject({ kind: "saved", draft: { id: D1, version: 1 }, etag: '"1"' });
    expect(s.queryClient.getQueryData<Entry<Draft>>(s.drafts.get(D1).queryKey)?.etag).toBe('"1"');
    expect(s.stale(s.drafts.list().queryKey)).toBe(true);
  });

  it("adds with the ETag read; modified in another tab (412), reads again and adds once more", async () => {
    const s = setup();
    await s.drafts.create({ title: "t" });
    s.touch(D1);
    const outcome = await s.drafts.addOperations(D1, [close]);
    expect(outcome).toMatchObject({ kind: "saved", draft: { version: 3, operation_count: 1 } });
    const adds = s.seen.filter((r) => r.url.endsWith("/operations"));
    expect(adds.map((r) => r.headers.get("If-Match"))).toEqual(['"1"', '"2"']);
  });

  it("a second 412 on an addition, or a 412 on a modification: conflict, read again", async () => {
    let refuse = false;
    const s = setup((r) =>
      refuse && r.method !== "GET" ? problem(412, "precondition-failed") : undefined,
    );
    await s.drafts.create({ title: "t" });
    refuse = true;
    s.touch(D1);
    const added = await s.drafts.addOperations(D1, [close]);
    expect(added).toMatchObject({ kind: "conflict", input: [close], current: { etag: '"2"' } });
    expect(s.seen.filter((r) => r.url.endsWith("/operations"))).toHaveLength(2);
    const updated = await s.drafts.update(D1, { title: "u" });
    expect(updated).toMatchObject({ kind: "conflict", input: { title: "u" } });
  });

  it("modifies; removes under If-Match: out of the cache, no longer active, no signal", async () => {
    const s = setup();
    const gone: Gone[] = [];
    s.drafts.onGone((g) => gone.push(g));
    await s.drafts.create({ title: "t" });
    s.drafts.setActive(D1);
    expect(await s.drafts.update(D1, { title: "u" })).toMatchObject({
      kind: "saved",
      draft: { title: "u", version: 2 },
    });
    expect(await s.drafts.remove(D1)).toEqual({ kind: "removed" });
    const remove = s.seen.find((r) => r.method === "DELETE");
    expect(remove?.headers.get("If-Match")).toBe('"2"');
    expect(s.queryClient.getQueryData(s.drafts.get(D1).queryKey)).toBeUndefined();
    expect(s.drafts.active()).toBeNull();
    await s.drafts.changed({ id: D1, deleted: true });
    expect(gone).toEqual([{ id: D1, active: false }]);
  });
});

describe("the active draft of the tab (sessionStorage) and draft.add", () => {
  it("creates one when there is none, then adds to it; a draft gone, a new one", async () => {
    const s = setup();
    const actives: (string | null)[] = [];
    s.drafts.onActive((id) => actives.push(id));
    expect(await s.drafts.addToActive([close])).toMatchObject({
      kind: "saved",
      draft: { id: D1, title: "untitled", operation_count: 1 },
    });
    expect(s.storage.map.get(ACTIVE_KEY)).toBe(D1);
    await s.drafts.addToActive([close]);
    expect(s.held.get(D1)?.operations).toHaveLength(2);
    s.held.delete(D1);
    expect(await s.drafts.addToActive([close])).toMatchObject({ draft: { id: D2 } });
    expect(actives).toEqual([D1, null, D2]);
  });

  it("without sessionStorage, no active draft kept", async () => {
    const s = setup();
    const drafts = createDrafts({
      ...s,
      invalidation: createInvalidation(s.queryClient, s.reads),
      writes: { submitted: async () => undefined },
      storage: () => {
        throw new DOMException("denied", "SecurityError");
      },
    });
    expect(drafts.active()).toBeNull();
    expect(await drafts.addToActive([close])).toMatchObject({ kind: "saved" });
    expect(drafts.active()).toBeNull();
  });
});

describe("simulation and submission (024 § A, B)", () => {
  it("simulates the last version of a burst only, after 400 ms", async () => {
    const s = setup();
    await s.drafts.create({ title: "t" });
    vi.useFakeTimers();
    const observer = new QueryObserver(s.queryClient, s.drafts.simulate({ id: D1, version: 1 }));
    const stop = observer.subscribe(() => undefined);
    await vi.advanceTimersByTimeAsync(100);
    observer.setOptions(s.drafts.simulate({ id: D1, version: 2 }));
    await vi.advanceTimersByTimeAsync(SIMULATION_WAIT - 10);
    observer.setOptions(s.drafts.simulate({ id: D1, version: 3 }));
    await vi.advanceTimersByTimeAsync(SIMULATION_WAIT - 10);
    expect(s.seen.filter((r) => r.url.endsWith("/simulation"))).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(20);
    expect(s.seen.filter((r) => r.url.endsWith("/simulation"))).toHaveLength(1);
    expect(observer.getCurrentResult().data?.valid).toBe(true);
    stop();
  });

  it("submits with ?apply=own: applied, the draft gone from the tab, what it named read again", async () => {
    const s = setup();
    await s.drafts.create({ title: "t", operations: [close] });
    s.drafts.setActive(D1);
    const prefix = s.reads.object("core.prefix", P1, null);
    s.queryClient.setQueryData(prefix.queryKey, { data: { id: P1 }, etag: '"p"' });
    const outcome = await s.drafts.submit(D1, { applyOwn: true, idempotencyKey: "k" });
    expect(outcome).toEqual({ kind: "applied", changeset: C1, warnings: [] });
    const post = s.seen.at(-1)!;
    expect(post.url).toBe(`/v1/drafts/${D1}/submission?apply=own`);
    expect(post.headers.get("If-Match")).toBe('"1"');
    expect(post.headers.get("Idempotency-Key")).toBe("k");
    expect(s.drafts.active()).toBeNull();
    expect(s.stale(prefix.queryKey)).toBe(true);
  });

  it("202: proposed with the code of its reason; without applyOwn, no parameter", async () => {
    const reason = { code: "changeset.apply_own.missing", message: "lacks", objects: [P1] };
    const s = setup((r) =>
      r.url.includes("/submission")
        ? json(
            {
              changeset: { id: C1, href: "h" },
              state: "pending",
              kind: "approval",
              diff: [],
              reason,
            },
            { status: 202 },
          )
        : undefined,
    );
    await s.drafts.create({ title: "t" });
    expect(await s.drafts.submit(D1, { applyOwn: false })).toEqual({
      kind: "proposed",
      changeset: C1,
      review: "approval",
      reason,
    });
    expect(s.seen.at(-1)?.url).toBe(`/v1/drafts/${D1}/submission`);
  });

  it("422: invalid, the draft stays active", async () => {
    const s = setup((r) =>
      r.url.includes("/submission")
        ? problem(422, "validation", {
            violations: [{ rule: "core.x", severity: "error", detail: "no", subjects: [] }],
          })
        : undefined,
    );
    await s.drafts.create({ title: "t" });
    s.drafts.setActive(D1);
    const outcome = await s.drafts.submit(D1, { applyOwn: true });
    expect(outcome.kind).toBe("invalid");
    expect(s.drafts.active()).toBe(D1);
    expect(s.queryClient.getQueryData(s.drafts.get(D1).queryKey)).toBeDefined();
  });
});

describe("the event draft (021 « Flux d'événements », 024 § A)", () => {
  it("another version: the draft and the list read again; the same: left as is", async () => {
    const s = setup();
    await s.drafts.create({ title: "t" });
    await s.queryClient.fetchQuery(s.drafts.list());
    await s.drafts.changed({ id: D1, version: 1 });
    expect(s.stale(s.drafts.get(D1).queryKey)).toBe(false);
    expect(s.stale(s.drafts.list().queryKey)).toBe(true);
    await s.drafts.changed({ id: D1, version: 2 });
    expect(s.stale(s.drafts.get(D1).queryKey)).toBe(true);
  });

  it("deleted or taken over elsewhere: leaves the tab with a signal", async () => {
    const s = setup();
    const gone: Gone[] = [];
    s.drafts.onGone((g) => gone.push(g));
    await s.drafts.create({ title: "t" });
    s.drafts.setActive(D1);
    await s.drafts.changed({ id: D1, deleted: true });
    expect(gone).toEqual([{ id: D1, active: true }]);
    expect(s.drafts.active()).toBeNull();
    expect(s.queryClient.getQueryData(s.drafts.get(D1).queryKey)).toBeUndefined();
  });
});
