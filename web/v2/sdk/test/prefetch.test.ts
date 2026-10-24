// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GUARD_WAIT, INTENT_DELAY, createPrefetch } from "../src/data/prefetch.ts";
import { D1, P1, fakeCache, json } from "./helpers.ts";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function setup(answer: () => Promise<Response> | Response = () => json({ id: P1 })) {
  const cache = fakeCache(answer);
  const prefetch = createPrefetch(cache);
  const queries = () => [
    cache.reads.object("core.prefix", P1, null),
    cache.reads.list("/v1/prefixes", { domain: D1 }, null),
  ];
  return { ...cache, prefetch, queries };
}

describe("prefetch on intent (REQ-021-22)", () => {
  it("reads the target's queries after 60 ms of hover, not on a passing pointer", async () => {
    const s = setup((): Response => json({ id: P1, items: [], links: {} }));
    const link = new EventTarget();
    const stop = s.prefetch.intent(link, s.queries);
    link.dispatchEvent(new Event("pointerenter"));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY - 10);
    link.dispatchEvent(new Event("pointerleave"));
    await vi.advanceTimersByTimeAsync(100);
    expect(s.seen).toHaveLength(0);
    link.dispatchEvent(new Event("pointerenter"));
    await vi.advanceTimersByTimeAsync(INTENT_DELAY);
    expect(s.seen.map((r) => r.url)).toEqual([`/v1/prefixes/${P1}`, `/v1/prefixes?domain=${D1}`]);
    // Fresh in the cache: a second intent reads nothing.
    link.dispatchEvent(new Event("focusin"));
    await vi.advanceTimersByTimeAsync(0);
    expect(s.seen).toHaveLength(2);
    stop();
  });

  it("reads at once on focus", async () => {
    const s = setup((): Response => json({ id: P1, items: [], links: {} }));
    const link = new EventTarget();
    s.prefetch.intent(link, s.queries);
    link.dispatchEvent(new Event("focusin"));
    await vi.advanceTimersByTimeAsync(0);
    expect(s.seen).toHaveLength(2);
  });

  it("the guard waits 100 ms at most", async () => {
    const slow = setup(
      () =>
        new Promise<Response>((done) =>
          setTimeout(() => done(json({ items: [], links: {} })), 500),
        ),
    );
    const outcome = slow.prefetch.guard(slow.queries());
    await vi.advanceTimersByTimeAsync(GUARD_WAIT);
    await expect(outcome).resolves.toBe("timeout");
    const fast = setup((): Response => json({ id: P1, items: [], links: {} }));
    const ready = fast.prefetch.guard(fast.queries());
    await vi.advanceTimersByTimeAsync(0);
    await expect(ready).resolves.toBe("ready");
  });
});
