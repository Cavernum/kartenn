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
  MAX_PAGES,
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
  getCurrentResult(): R;
  subscribe(listener: (result: R) => void): () => void;
}

const readOf = <T>(result: QueryObserverResult<Entry<T>, Error>): Read<T> => ({
  data: result.data?.data,
  error: result.isError ? result.error : null,
  pending: result.data === undefined && !result.isError,
});

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
    readOf<T>,
    IDLE as Read<T>,
  ).state;
}

type Paged<T> = { items: T[]; links: { next?: string | null } };

const listOf = <T>(
  result: InfiniteQueryObserverResult<{ pages: Entry<Paged<T>>[] }, Error>,
): List<T> => ({
  items: result.data?.pages.flatMap((page) => page.data.items),
  error: result.isError ? result.error : null,
  pending: result.data === undefined && !result.isError,
  previous: result.isPlaceholderData,
  more: result.hasNextPage,
  fetching: result.isFetchingNextPage,
  pages: result.data?.pages.length ?? 0,
});

/** The members of an infinite observer that read a page again or the next one. */
type Pager = { refetch?: () => Promise<unknown>; fetchNextPage?: () => Promise<unknown> };

/** After a failure: the first page read again, or the next page that failed. */
function again(observer: Pager | null, list: List<unknown>): void {
  if (list.items === undefined) void observer?.refetch?.();
  else void observer?.fetchNextPage?.();
}

/** The list of `options()` (`reads.list`; `null`: none), followed page by page. */
export function useList<T, K extends Key>(
  options: () => ListOptions<T, K> | null,
): { state: Readonly<ShallowRef<List<T>>>; next(): void; retry(): void } {
  const { queryClient } = useHosted();
  const { state, current } = observe(
    options,
    (o) => new InfiniteQueryObserver<Entry<Paged<T>>, Error>(queryClient, o),
    listOf<T>,
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
      again(current() as Pager | null, state.value);
    },
  };
}

/**
 * Follows each of the options of `options()` (reactive, at the dates of the context) with an
 * observer `create` makes, while the scope lives: the observer of an option kept from one
 * set to the next is kept (by the hash of its key) and given its new options (`enabled`…),
 * those of the options gone let go. The states and the observers are aligned on the options
 * (the same options twice: the same observer at both places).
 */
function observeAll<R, S>(
  options: () => readonly unknown[],
  create: (options: never) => Observer<R>,
  shape: (result: R) => S,
): { states: ShallowRef<readonly S[]>; observers: () => readonly Observer<R>[] } {
  const { context, queryClient } = useHosted();
  const states = shallowRef<readonly S[]>([]);
  const held = new Map<string, { observer: Observer<R>; stop: () => void }>();
  // The hash of each option, in their order; an observer subscribing calls `draw` with the
  // former order, whose observers are all still held.
  let order: string[] = [];
  const draw = () => {
    const shaped = new Map<string, S>();
    states.value = order.map((hash) => {
      if (!shaped.has(hash)) shaped.set(hash, shape(held.get(hash)!.observer.getCurrentResult()));
      return shaped.get(hash)!;
    });
  };
  watch(
    () => {
      void context.value.time;
      return options();
    },
    (next) => {
      const hashes: string[] = [];
      const seen = new Set<string>();
      for (const option of next) {
        const defaulted = queryClient.defaultQueryOptions(option as never) as {
          queryHash: string;
        };
        const hash = defaulted.queryHash;
        hashes.push(hash);
        if (seen.has(hash)) continue;
        seen.add(hash);
        const kept = held.get(hash);
        if (kept) {
          kept.observer.setOptions(defaulted as never);
          continue;
        }
        const observer = create(defaulted as never);
        // Held before it subscribes: `draw` may run within the subscription.
        const entry = { observer, stop: () => {} };
        held.set(hash, entry);
        entry.stop = observer.subscribe(draw);
      }
      for (const [hash, { stop }] of held) {
        if (seen.has(hash)) continue;
        stop();
        held.delete(hash);
      }
      order = hashes;
      draw();
    },
    { immediate: true, flush: "sync" },
  );
  onScopeDispose(() => {
    for (const { stop } of held.values()) stop();
    held.clear();
  });
  return { states, observers: () => order.map((hash) => held.get(hash)!.observer) };
}

/** The reads of `options()` (`reads.object`, `reads.sub`…), each followed, in their order. */
export function useReads<T, K extends Key>(
  options: () => readonly ReadOptions<T, K>[],
): Readonly<ShallowRef<readonly Read<T>[]>> {
  const { queryClient } = useHosted();
  return observeAll(options, (o) => new QueryObserver<Entry<T>, Error>(queryClient, o), readOf<T>)
    .states;
}

/**
 * The lists of `options()` (`reads.list`), each followed page by page, aligned on the
 * options (the same options twice: one list, at both places).
 */
export function useLists<T, K extends Key>(
  options: () => readonly ListOptions<T, K>[],
): {
  states: Readonly<ShallowRef<readonly List<T>[]>>;
  next(index?: number): void;
  retry(index?: number): void;
} {
  const { queryClient } = useHosted();
  const { states, observers } = observeAll(
    options,
    (o) => new InfiniteQueryObserver<Entry<Paged<T>>, Error>(queryClient, o),
    listOf<T>,
  );
  /** Each observer once, with its list; only the one at `index` when given. */
  const each = (index: number | undefined, act: (observer: unknown, list: List<T>) => void) => {
    const done = new Set<unknown>();
    observers().forEach((observer, i) => {
      const list = states.value[i];
      if ((index !== undefined && i !== index) || done.has(observer) || !list) return;
      done.add(observer);
      act(observer, list);
    });
  };
  return {
    states,
    /** The next page of each list (or of the one at `index`) that has one, not read nor refused yet. */
    next(index?: number) {
      each(index, (observer, list) => {
        if (list.more && !list.fetching && !list.error && list.pages < MAX_PAGES) {
          void (observer as Pager).fetchNextPage?.();
        }
      });
    },
    /** After a failure: each list (or the one at `index`) that failed read again, as `useList().retry`. */
    retry(index?: number) {
      each(index, (observer, list) => {
        if (list.error) again(observer as Pager, list);
      });
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
