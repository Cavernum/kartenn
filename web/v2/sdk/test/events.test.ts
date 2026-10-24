// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it, vi } from "vitest";
import { applyEvent } from "../src/events/apply.ts";
import {
  BACKOFF,
  EventStream,
  SEEN_WINDOW,
  identity,
  type ChannelLike,
  type Check,
  type KartennEvent,
  type LocksLike,
  type Relay,
  type SourceLike,
} from "../src/events/stream.ts";
import { createInvalidation } from "../src/data/invalidate.ts";
import { D1, P1, P2, fakeCache, json } from "./helpers.ts";

/** A simulated `EventSource`: the test sends events and closes it. */
class FakeSource implements SourceLike {
  readyState = 0;
  onerror: ((event: unknown) => void) | null = null;
  closed = false;
  private readonly listeners = new Map<
    string,
    ((e: { data: string; lastEventId: string }) => void)[]
  >();
  constructor(readonly url: string) {}
  addEventListener(type: string, listener: (e: { data: string; lastEventId: string }) => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }
  close() {
    this.closed = true;
    this.readyState = 2;
  }
  emit(kind: string, data: unknown, id = "") {
    this.readyState = 1;
    for (const l of this.listeners.get(kind) ?? [])
      l({ data: JSON.stringify(data), lastEventId: id });
  }
  /** A status other than 200: the browser closes the source (WHATWG § 9.2.2). */
  refuse() {
    this.readyState = 2;
    this.onerror?.({});
  }
  /** A network cut: the browser reconnects by itself. */
  cut() {
    this.readyState = 0;
    this.onerror?.({});
  }
}

/** The locks of one browser, shared by its simulated tabs. */
function fakeLocks(): LocksLike {
  const queue: { signal: AbortSignal; run: () => Promise<void> }[] = [];
  let held = false;
  const next = () => {
    if (held) return;
    const entry = queue.shift();
    if (!entry) return;
    if (entry.signal.aborted) return next();
    held = true;
    void entry.run().finally(() => {
      held = false;
      next();
    });
  };
  return {
    request(_name, { signal }, run) {
      return new Promise((done) => {
        queue.push({ signal, run: () => run().then(done) });
        signal.addEventListener("abort", () => done(undefined));
        queueMicrotask(next);
      });
    },
  };
}

/** The channels of one browser: a message reaches every other channel of the name. */
function fakeChannels() {
  const open = new Set<ChannelLike>();
  return (name: string): ChannelLike => {
    const channel: ChannelLike = {
      onmessage: null,
      postMessage(message: Relay) {
        for (const other of open) {
          if (other !== channel)
            queueMicrotask(() => other.onmessage?.({ data: structuredClone(message) }));
        }
      },
      close() {
        open.delete(channel);
      },
    };
    void name;
    open.add(channel);
    return channel;
  };
}

const flush = () => new Promise((done) => setTimeout(done, 0));

function tab(
  browser: { locks: LocksLike | null; channel: ((n: string) => ChannelLike) | null },
  check: () => Promise<Check> = async () => "ok",
) {
  const sources: FakeSource[] = [];
  const applied: KartennEvent[] = [];
  const timers: (() => void)[] = [];
  const stream = new EventStream({
    open: (url) => {
      const source = new FakeSource(url);
      sources.push(source);
      return source;
    },
    locks: browser.locks,
    channel: browser.channel,
    check,
    apply: (event) => applied.push(event),
    setTimeout: (run) => timers.push(run),
    clearTimeout: () => undefined,
    random: () => 1,
  });
  return { stream, sources, applied, timers };
}

describe("one stream per browser (REQ-021-23, Q-021-3)", () => {
  it("three tabs: one connection, every tab gets the events", async () => {
    const browser = { locks: fakeLocks(), channel: fakeChannels() };
    const tabs = [tab(browser), tab(browser), tab(browser)];
    for (const t of tabs) t.stream.start();
    await flush();
    expect(tabs.map((t) => t.sources.length)).toEqual([1, 0, 0]);
    tabs[0]!.sources[0]!.emit("ready", { instance: "i" }, "k1.a");
    tabs[0]!.sources[0]!.emit("changes", { changeset: "c1", objects: [], domains: [] }, "k1.b");
    await flush();
    for (const t of tabs) expect(t.applied.map((e) => e.kind)).toEqual(["ready", "changes"]);
    expect(tabs.map((t) => t.stream.state())).toEqual(["open", "open", "open"]);
  });

  it("the tab holding the stream closes: another opens it after the last cursor", async () => {
    const browser = { locks: fakeLocks(), channel: fakeChannels() };
    const [a, b] = [tab(browser), tab(browser)];
    a.stream.start();
    b.stream.start();
    await flush();
    a.sources[0]!.emit("ready", {}, "k1.a");
    a.sources[0]!.emit("changes", { changeset: "c1", domains: [] }, "k1.b");
    await flush();
    a.stream.stop();
    await flush();
    expect(a.sources[0]!.closed).toBe(true);
    expect(b.stream.leader).toBe(true);
    expect(b.sources.map((s) => s.url)).toEqual(["/v1/events?last_event_id=k1.b"]);
  });

  it("without navigator.locks, a stream per tab", async () => {
    const browser = { locks: null, channel: fakeChannels() };
    const [a, b] = [tab(browser), tab(browser)];
    a.stream.start();
    b.stream.start();
    expect([a.sources.length, b.sources.length]).toEqual([1, 1]);
  });
});

describe("contract of the 022 (duplicates, closure)", () => {
  it("ignores an event already seen, by identity, within a window", async () => {
    const t = tab({ locks: null, channel: null });
    t.stream.start();
    const source = t.sources[0]!;
    source.emit("ready", {});
    source.emit("changes", { changeset: "c1", domains: [] }, "k1.1");
    source.emit("changes", { changeset: "c1", domains: [] }, "k1.1");
    source.emit("changes", { task: "m:t", event: "e1", domains: [] });
    source.emit("changes", { task: "m:t", event: "e1", domains: [] });
    source.emit("changeset", { changeset: "c1", state: "pending" });
    source.emit("changeset", { changeset: "c1", state: "applied" });
    source.emit("ready", {});
    expect(t.applied.map((e) => e.kind)).toEqual([
      "ready",
      "changes",
      "changes",
      "changeset",
      "changeset",
      "ready",
    ]);
    for (let i = 0; i < SEEN_WINDOW; i += 1)
      source.emit("changes", { changeset: `x${i}`, domains: [] });
    source.emit("changes", { changeset: "c1", domains: [] });
    expect(t.applied.at(-1)?.data["changeset"]).toBe("c1");
    expect(identity({ kind: "reset", data: {}, id: null })).toBeNull();
  });

  it("a cut is the browser's; a closure (401) checks the session, then reopens later", async () => {
    let check: Check = "unauthorized";
    const t = tab({ locks: null, channel: null }, async () => check);
    t.stream.start();
    t.sources[0]!.emit("ready", {}, "k1.a");
    t.sources[0]!.cut();
    expect(t.stream.state()).toBe("connecting");
    expect(t.sources).toHaveLength(1);
    t.sources[0]!.refuse();
    await flush();
    expect(t.stream.state()).toBe("closed");
    expect(t.timers).toHaveLength(0);
    // Logged in again: the shell restarts the stream, after the cursor.
    check = "ok";
    t.stream.restart();
    expect(t.sources[1]?.url).toBe("/v1/events?last_event_id=k1.a");
    t.sources[1]!.refuse();
    await flush();
    expect(t.timers).toHaveLength(1);
    t.timers[0]!();
    expect(t.sources).toHaveLength(3);
  });

  it("waits longer after each closure, capped", async () => {
    const delays: number[] = [];
    const sources: FakeSource[] = [];
    const stream = new EventStream({
      open: (url) => {
        const s = new FakeSource(url);
        sources.push(s);
        return s;
      },
      locks: null,
      channel: null,
      check: async () => "ok",
      apply: () => undefined,
      setTimeout: (run, ms) => {
        delays.push(ms);
        queueMicrotask(run);
      },
      random: () => 1,
    });
    stream.start();
    for (let i = 0; i < 8; i += 1) {
      sources.at(-1)!.refuse();
      await flush();
    }
    expect(delays.slice(0, 3)).toEqual([BACKOFF.base, 2 * BACKOFF.base, 4 * BACKOFF.base]);
    expect(Math.max(...delays)).toBe(BACKOFF.cap);
  });
});

describe("events applied to the cache (022, amended by the 021)", () => {
  async function setup() {
    const bodies: Record<string, unknown> = {
      [`/v1/prefixes/${P1}`]: { id: P1, domain: { id: D1, href: "d" } },
      [`/v1/prefixes/${P2}`]: { id: P2, version: "c2" },
      [`/v1/domains/${D1}`]: { id: D1 },
      "/v1/proposals": { items: [], links: {} },
    };
    const cache = fakeCache((r) => json(bodies[r.url.split("?")[0]!] ?? {}));
    const invalidation = createInvalidation(cache.queryClient, cache.reads);
    const labels = { changed: vi.fn() };
    const apply = (kind: KartennEvent["kind"], data: Record<string, unknown>) =>
      applyEvent({ kind, data, id: null }, { invalidation, labels });
    const p1 = cache.reads.object("core.prefix", P1, null);
    const p2 = cache.reads.object("core.prefix", P2, null);
    const past = cache.reads.object("core.prefix", P1, { asOf: "2026-01-01T00:00:00Z" });
    const proposals = cache.reads.list("/v1/proposals", {}, null);
    await Promise.all([p1, p2, past].map((o) => cache.queryClient.fetchQuery(o)));
    await cache.queryClient.fetchInfiniteQuery(proposals);
    const stale = (key: readonly unknown[]) =>
      cache.queryClient.getQueryState(key)?.isInvalidated ?? false;
    return { ...cache, apply, labels, p1, p2, past, proposals, stale };
  }

  it("ready and reset invalidate everything but the past as recorded", async () => {
    for (const kind of ["ready", "reset"] as const) {
      const s = await setup();
      await s.apply(kind, {});
      expect([s.stale(s.p1.queryKey), s.stale(s.p2.queryKey), s.stale(s.past.queryKey)]).toEqual([
        true,
        true,
        false,
      ]);
    }
  });

  it("changes: the objects whose version differs, and those that show them", async () => {
    const s = await setup();
    await s.apply("changes", {
      changeset: "c2",
      objects: [{ type: "core.prefix", id: P2 }],
      domains: [],
    });
    expect(s.stale(s.p2.queryKey)).toBe(false);
    await s.apply("changes", {
      changeset: "c3",
      objects: [{ type: "core.domain", id: D1 }],
      domains: [],
    });
    expect(s.stale(s.p1.queryKey)).toBe(true);
    expect(s.stale(s.p2.queryKey)).toBe(false);
  });

  it("truncated: every query of the types named", async () => {
    const s = await setup();
    await s.apply("changes", {
      changeset: "c4",
      truncated: true,
      types: ["core.prefix"],
      domains: [],
    });
    expect([s.stale(s.p1.queryKey), s.stale(s.p2.queryKey), s.stale(s.past.queryKey)]).toEqual([
      true,
      true,
      false,
    ]);
  });

  it("changeset: the changeset and the proposals; labels: the dictionary", async () => {
    const s = await setup();
    await s.apply("changeset", { changeset: "c5", state: "applied" });
    expect(s.stale(s.proposals.queryKey)).toBe(true);
    expect(s.stale(s.p1.queryKey)).toBe(false);
    await s.apply("labels", { revision: "r2" });
    expect(s.labels.changed).toHaveBeenCalledWith("r2");
  });
});

describe("the data layer of the page (bfcache, fallback)", () => {
  it("opens the stream, gives it up on pagehide, reopens and revalidates on pageshow", async () => {
    const opened: FakeSource[] = [];
    vi.stubGlobal(
      "EventSource",
      class extends FakeSource {
        constructor(url: string) {
          super(url);
          opened.push(this);
        }
      },
    );
    const { startData, stream } = await import("../src/instance.ts");
    const { fallback } = await import("../src/data/cache.ts");
    const page = new EventTarget();
    const persisted = (type: string) => Object.assign(new Event(type), { persisted: true });
    const stop = startData(page);
    await vi.waitFor(() => expect(opened).toHaveLength(1));
    opened[0]!.emit("ready", {}, "k1.a");
    expect(stream.state()).toBe("open");
    expect(fallback.active).toBe(false);
    page.dispatchEvent(persisted("pagehide"));
    expect(opened[0]!.closed).toBe(true);
    expect(fallback.active).toBe(true);
    page.dispatchEvent(persisted("pageshow"));
    await vi.waitFor(() => expect(opened).toHaveLength(2));
    expect(opened[1]!.url).toBe("/v1/events?last_event_id=k1.a");
    stop();
    vi.unstubAllGlobals();
  });
});
