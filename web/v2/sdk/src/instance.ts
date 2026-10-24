// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The single instance of the data layer (ADR 0037): one client, one cache, one index of
// dependents, shared by the shell and every plugin through the import map. `index.ts`
// exports its public part, `host.ts` what the shell plugs in.

import { createClient } from "./api/client.ts";
import { ApiProblem } from "./api/problem.ts";
import { context } from "./context.ts";
import { createQueries, createQueryClient, fallback } from "./data/cache.ts";
import { createDrafts } from "./data/drafts.ts";
import { createInvalidation } from "./data/invalidate.ts";
import { createLabels } from "./data/labels.ts";
import { createWrites } from "./data/write.ts";
import { applyEvent } from "./events/apply.ts";
import {
  EventStream,
  type ChannelLike,
  type Check,
  type LocksLike,
  type SourceLike,
} from "./events/stream.ts";
import { hooks } from "./hooks.ts";

/** The client of the API, on the same origin, with the dated context of the shell. */
export const api = createClient({
  fetch: (input, init) => (hooks.fetch ?? globalThis.fetch)(input, init),
  time: () => context.get().time,
  onUnauthorized: () => hooks.onUnauthorized?.(),
});

/** The cache shared by the shell and every plugin. */
export const queryClient = createQueryClient();

/** The reads of the cache: their keys, query functions and freshness (REQ-021-21). */
export const reads = createQueries({ client: api, queryClient });

/** What the writes and the events invalidate. */
export const invalidation = createInvalidation(queryClient, reads);

export const writes = createWrites({ client: api, queryClient, invalidation });

/** The drafts of the server (spec 024 § A); the active one of the tab in `sessionStorage`. */
export const drafts = createDrafts({
  client: api,
  queryClient,
  reads,
  invalidation,
  writes,
  storage: () => (typeof sessionStorage === "undefined" ? null : sessionStorage),
});

/** The counters of the home page (`GET /v1/stats`, spec 024 § C), read again on `stats`. */
export const stats = {
  query: () => reads.read("/v1/stats", { family: "stats" }),
};

/** The label dictionary of the language of the context (spec 018, REQ-021-19). */
export const labels = createLabels({ queryClient, reads, locale: () => context.get().locale });

/** After a closure of the stream: the session, and whether the server serves a stream. */
async function check(): Promise<Check> {
  try {
    const { data } = await api.get("/v1/capabilities");
    return data.events ? "ok" : "disabled";
  } catch (error) {
    return error instanceof ApiProblem && error.status === 401 ? "unauthorized" : "ok";
  }
}

/** The event stream of the browser (REQ-021-23), opened by the shell before its reads. */
export const stream = new EventStream({
  open: (url) => new EventSource(url) as unknown as SourceLike,
  // Read when the stream starts: a browser without them gets a stream per tab.
  get locks() {
    const locks = typeof navigator === "undefined" ? undefined : navigator.locks;
    return locks ? (locks as unknown as LocksLike) : null;
  },
  get channel() {
    return typeof BroadcastChannel === "function"
      ? (name: string) => new BroadcastChannel(name) as unknown as ChannelLike
      : null;
  },
  check,
  apply: (event) => applyEvent(event, { invalidation, labels, drafts }),
});

// Without the stream, the cache revalidates on focus and reconnection (design « Repli »).
stream.onState((state) => {
  fallback.active = state !== "open";
});

/**
 * Starts the data layer of the page: the cache follows focus and network, the stream opens
 * (before the first reads, 022), and the bfcache is handled — the stream given to another
 * tab on `pagehide`, everything revalidated on a persisted `pageshow`. Returns the
 * function that stops it all.
 */
export function startData(target: EventTarget | null = globalThis.window ?? null): () => void {
  queryClient.mount();
  stream.start();
  const hide = (event: Event) => {
    if ((event as PageTransitionEvent).persisted) stream.stop();
  };
  const show = (event: Event) => {
    if (!(event as PageTransitionEvent).persisted) return;
    void invalidation.all();
    stream.start();
  };
  target?.addEventListener("pagehide", hide);
  target?.addEventListener("pageshow", show);
  return () => {
    target?.removeEventListener("pagehide", hide);
    target?.removeEventListener("pageshow", show);
    stream.stop();
    queryClient.unmount();
  };
}
