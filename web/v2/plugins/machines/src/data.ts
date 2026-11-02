// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The reads of a first-party plugin in Vue (ADR 0038 « Résolution de D-1 », amended by A23;
// task 18ca): small composables over the observers the SDK re-exports (`QueryObserver`,
// `InfiniteQueryObserver`) on its one `QueryClient` — not `VueQueryPlugin`, which is no
// shared module and would bring a second copy, and a second cache. Each read is observed
// while its effect scope lives, read again once invalidated (a write, an event of the
// stream), and moved to the dates of the context when they change; the state is shallow
// (the data keeps its identity). Within the application of `host.ts`; nothing here names a
// plugin: to move as is into @kartenn/ui (A23).

import {
  InfiniteQueryObserver,
  QueryObserver,
  type Entry,
  type InfiniteQueryObserverResult,
  type Key,
  type ListOptions,
  type QueryObserverResult,
  type ReadOptions,
} from "@kartenn/sdk";
import { onScopeDispose, shallowRef, watch, type ShallowRef } from "vue";
import { useHosted } from "./host.ts";

/** A read: its value once read, its failure (`null` if none), whether it is still awaited. */
export interface Read<T> {
  readonly data: T | undefined;
  readonly error: unknown;
  readonly pending: boolean;
}

/** A list read page by page: the items of the pages read (`undefined` before the first). */
export interface List<T> {
  readonly items: T[] | undefined;
  readonly error: unknown;
  readonly pending: boolean;
  /**
   * The items are those of the former options, kept while the new ones are read (the lists
   * of the SDK keep their previous data: a filter changed does not blank the list).
   */
  readonly previous: boolean;
  /** A next page is there to read. */
  readonly more: boolean;
  /** The next page is being read. */
  readonly fetching: boolean;
  /** The pages held (at most `MAX_PAGES` of the SDK: a later one drops the first). */
  readonly pages: number;
}

const IDLE = Object.freeze({
  data: undefined,
  items: undefined,
  error: null,
  pending: false,
  previous: false,
  more: false,
  fetching: false,
  pages: 0,
});

/** The members of an observer this module uses, whatever its kind. */
interface Observer<R> {
  setOptions(options: never): void;
  getOptimisticResult(options: never): R;
  subscribe(listener: (result: R) => void): () => void;
}

/**
 * Follows the options of `options()` (reactive, at the dates of the context; `null`: no
 * read) with the observer `create` makes, while the scope lives.
 */
function observe<R, S>(
  options: () => unknown,
  create: (options: never) => Observer<R>,
  shape: (result: R) => S,
  idle: S,
): { state: ShallowRef<S>; current: () => Observer<R> | null } {
  const { context, queryClient } = useHosted();
  const state = shallowRef(idle);
  let observer: Observer<R> | null = null;
  let stop: (() => void) | null = null;
  const set = (result: R) => (state.value = shape(result));
  watch(
    () => {
      // The options of `reads` are those of the dates of now: read again when they move.
      void context.value.time;
      return options();
    },
    (next) => {
      if (next === null || next === undefined) {
        stop?.();
        stop = null;
        observer = null;
        state.value = idle;
        return;
      }
      const defaulted = queryClient.defaultQueryOptions(next as never) as never;
      if (observer) observer.setOptions(defaulted);
      else {
        observer = create(defaulted);
        stop = observer.subscribe(set);
      }
      set(observer.getOptimisticResult(defaulted));
    },
    { immediate: true, flush: "sync" },
  );
  onScopeDispose(() => stop?.());
  return { state, current: () => observer };
}

/** The read of `options()` (`reads.object`, `reads.sub`…; `null`: none), followed. */
export function useRead<T, K extends Key>(
  options: () => ReadOptions<T, K> | null,
): Readonly<ShallowRef<Read<T>>> {
  const { queryClient } = useHosted();
  return observe(
    options,
    (o) => new QueryObserver<Entry<T>, Error>(queryClient, o),
    (result: QueryObserverResult<Entry<T>, Error>): Read<T> => ({
      data: result.data?.data,
      error: result.isError ? result.error : null,
      pending: result.data === undefined && !result.isError,
    }),
    IDLE as Read<T>,
  ).state;
}

type Paged<T> = { items: T[]; links: { next?: string | null } };

/** The list of `options()` (`reads.list`; `null`: none), followed page by page. */
export function useList<T, K extends Key>(
  options: () => ListOptions<T, K> | null,
): { state: Readonly<ShallowRef<List<T>>>; next(): void; retry(): void } {
  const { queryClient } = useHosted();
  const { state, current } = observe(
    options,
    (o) => new InfiniteQueryObserver<Entry<Paged<T>>, Error>(queryClient, o),
    (result: InfiniteQueryObserverResult<{ pages: Entry<Paged<T>>[] }, Error>): List<T> => ({
      items: result.data?.pages.flatMap((page) => page.data.items),
      error: result.isError ? result.error : null,
      pending: result.data === undefined && !result.isError,
      previous: result.isPlaceholderData,
      more: result.hasNextPage,
      fetching: result.isFetchingNextPage,
      pages: result.data?.pages.length ?? 0,
    }),
    IDLE as List<T>,
  );
  return {
    state,
    next() {
      const observer = current() as { fetchNextPage?: () => Promise<unknown> } | null;
      void observer?.fetchNextPage?.();
    },
    /** After a failure: the first page read again, or the next page that failed. */
    retry() {
      const observer = current() as {
        refetch?: () => Promise<unknown>;
        fetchNextPage?: () => Promise<unknown>;
      } | null;
      if (state.value.items === undefined) void observer?.refetch?.();
      else void observer?.fetchNextPage?.();
    },
  };
}

/**
 * A store of the plugin that says when what it holds changes (the names of the objects an
 * element cites, read a few at a time): `make` receives the function to call then, and
 * whether the scope still lives; what reads the store through the returned function draws
 * again at each change.
 */
export function useNotified<S>(make: (changed: () => void, live: () => boolean) => S): () => S {
  const version = shallowRef(0);
  let live = true;
  onScopeDispose(() => (live = false));
  const store = make(
    () => {
      if (live) version.value++;
    },
    () => live,
  );
  return () => {
    void version.value;
    return store;
  };
}
