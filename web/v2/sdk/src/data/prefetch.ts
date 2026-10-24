// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Prefetch on intent (ADR 0037 decision 4, REQ-021-22; design « Préchargement »): a link
// reads the queries of its target after 60 ms of hover, or at once on focus; navigation
// waits for them 100 ms at most, then shows the cache or a skeleton. The link (`KLink`) and
// the guard of the router are the shell's; this is what they call.

import type { QueryClient } from "@tanstack/query-core";
import type { ListOptions, ReadOptions } from "./cache.ts";

/** Hover before the queries of a target are read. */
export const INTENT_DELAY = 60;
/** What navigation waits for its queries, at most. */
export const GUARD_WAIT = 100;

/** A read or a list, as the factories of `reads` return them. */
// Structural: the factories differ by their key and data, which the prefetch never reads.
export type Prefetchable =
  | (Pick<ReadOptions<unknown>, "staleTime" | "gcTime"> & {
      readonly queryKey: readonly unknown[];
      readonly queryFn: unknown;
    })
  | (Pick<ListOptions<unknown>, "initialPageParam" | "maxPages"> & {
      readonly queryKey: readonly unknown[];
      readonly queryFn: unknown;
      readonly getNextPageParam: unknown;
    });

export interface PrefetchDeps {
  queryClient: QueryClient;
  setTimeout?: (run: () => void, ms: number) => unknown;
  clearTimeout?: (timer: unknown) => void;
}

export function createPrefetch({ queryClient, ...timers }: PrefetchDeps) {
  const later = timers.setTimeout ?? ((run: () => void, ms: number) => setTimeout(run, ms));
  const cancel = timers.clearTimeout ?? ((timer: unknown) => clearTimeout(timer as number));

  /** Reads what is missing or stale; never throws (a failure shows on the screen). */
  function prefetch(queries: readonly Prefetchable[]): Promise<void> {
    return Promise.all(
      queries.map((options) =>
        "initialPageParam" in options
          ? queryClient.prefetchInfiniteQuery(options as never)
          : queryClient.prefetchQuery(options as never),
      ),
    ).then(() => undefined);
  }

  return {
    prefetch,

    /**
     * Prefetches the queries of a link's target on intent: hover held `delay` ms, or
     * focus. Returns the function that stops listening.
     */
    intent(
      target: EventTarget,
      queries: () => readonly Prefetchable[],
      delay: number = INTENT_DELAY,
    ): () => void {
      let timer: unknown = null;
      const stop = () => {
        if (timer !== null) cancel(timer);
        timer = null;
      };
      const enter = () => {
        stop();
        timer = later(() => {
          timer = null;
          void prefetch(queries());
        }, delay);
      };
      const focus = () => {
        stop();
        void prefetch(queries());
      };
      target.addEventListener("pointerenter", enter);
      target.addEventListener("pointerleave", stop);
      target.addEventListener("focusin", focus);
      return () => {
        stop();
        target.removeEventListener("pointerenter", enter);
        target.removeEventListener("pointerleave", stop);
        target.removeEventListener("focusin", focus);
      };
    },

    /** The guard of a navigation: the queries read, or `wait` ms passed. */
    guard(
      queries: readonly Prefetchable[],
      wait: number = GUARD_WAIT,
    ): Promise<"ready" | "timeout"> {
      return new Promise((done) => {
        const timer = later(() => done("timeout"), wait);
        void prefetch(queries).then(() => {
          cancel(timer);
          done("ready");
        });
      });
    },
  };
}

export type Prefetch = ReturnType<typeof createPrefetch>;
