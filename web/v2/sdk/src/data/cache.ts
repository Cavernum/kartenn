// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The shared cache (ADR 0037, REQ-021-21): one `QueryClient` of `@tanstack/query-core`,
// whose query functions are the client of the API. Each entry keeps the body and its
// `ETag`, opaque (006/H19); a read goes with `If-None-Match` and a 304 gives back the entry
// kept, the same object, so that nothing renders again.

import {
  keepPreviousData,
  QueryClient,
  type InfiniteData,
  type Query,
  type QueryFunctionContext,
  type QueryKey,
} from "@tanstack/query-core";
import type { Client, Page, PathsOf, Success } from "../api/client.ts";
import { search } from "../api/client.ts";
import { FILTERS } from "../api/filters.ts";
import { ApiProblem } from "../api/problem.ts";
import type { paths } from "../api/schema";
import {
  isImmutable,
  keys,
  type Filters,
  type FilterValue,
  type ObjectKey,
  type PathKey,
  type PathParams,
  type SubKey,
  type TimeArg,
  type TimeKey,
} from "./keys.ts";
import { collectionOf } from "./types.ts";

/** What the cache keeps of an answer: its body and its `ETag`, never parsed. */
export interface Entry<T = unknown> {
  readonly data: T;
  readonly etag: string | null;
}

/** The families of freshness (design « Clés et fraîcheur »). */
export type Family =
  "object" | "list" | "children" | "session" | "labels" | "simulation" | "drafts" | "stats";

const SECOND = 1000;
const MINUTE = 60 * SECOND;

/** `staleTime` and `gcTime` of each family, at the present or `valid_at`. */
export const FRESHNESS: Readonly<Record<Family, { staleTime: number; gcTime: number }>> = {
  object: { staleTime: 30 * SECOND, gcTime: 10 * MINUTE },
  list: { staleTime: 30 * SECOND, gcTime: 5 * MINUTE },
  children: { staleTime: 30 * SECOND, gcTime: 10 * MINUTE },
  session: { staleTime: 5 * MINUTE, gcTime: Infinity },
  labels: { staleTime: Infinity, gcTime: Infinity },
  simulation: { staleTime: 0, gcTime: MINUTE },
  drafts: { staleTime: 30 * SECOND, gcTime: 10 * MINUTE },
  stats: { staleTime: 30 * SECOND, gcTime: 10 * MINUTE },
};

/** The past as recorded never changes (ADR 0005): never stale, kept 30 min. */
export const IMMUTABLE = { staleTime: Infinity, gcTime: 30 * MINUTE } as const;

/** The pages of a list kept in memory at most. */
export const MAX_PAGES = 10;

/** The freshness of a family for a key. */
export function freshness(family: Family, key: readonly unknown[]) {
  return isImmutable(key) ? IMMUTABLE : FRESHNESS[family];
}

/** The options of a read, for `fetchQuery`, `prefetchQuery` or an observer. */
export interface ReadOptions<T, K extends QueryKey = QueryKey> {
  queryKey: K;
  queryFn: (context: QueryFunctionContext<K>) => Promise<Entry<T>>;
  staleTime: number;
  gcTime: number;
}

/** The options of a list read page by page (`InfiniteQueryObserver`). */
export interface ListOptions<T, K extends QueryKey = QueryKey> {
  queryKey: K;
  queryFn: (context: QueryFunctionContext<K, string | null>) => Promise<Entry<Page<T>>>;
  initialPageParam: string | null;
  getNextPageParam: (last: Entry<Page<T>>) => string | undefined;
  maxPages: number;
  placeholderData: typeof keepPreviousData;
  staleTime: number;
  gcTime: number;
}

/** The pages of a list, as the cache keeps them. */
export type ListData<T> = InfiniteData<Entry<Page<T>>, string | null>;

type QueryOf<P> = P extends keyof paths
  ? paths[P] extends { get: { parameters: { query?: infer Q } } }
    ? NonNullable<Q>
    : never
  : never;
type Item<T> = T extends { items: (infer I)[] } ? I : unknown;
/** The paths whose `GET` answers a page (`items`, `links`). */
export type ListPath = {
  [P in PathsOf<"get">]: Success<NonNullable<paths[P]["get"]>> extends { items: unknown[] }
    ? P
    : never;
}[PathsOf<"get">];
/**
 * The filters of a list, typed by the contract (021/X41): its query parameters but the
 * dates and the cursor, and a property filter for each declared key.
 */
export type FiltersOf<P> = {
  [
    N in Exclude<
      keyof QueryOf<P>,
      "valid_at" | "as_of" | "cursor" | "page_size" | `prop.{key}${string}`
    >
  ]?: QueryOf<P>[N] | null;
} & { [name: `prop.${string}`]: FilterValue };
/** The item of a list. */
export type ItemOf<P> =
  P extends PathsOf<"get"> ? Item<Success<NonNullable<paths[P]["get"]>>> : never;

/** The path with its parameters, `{id}` → encoded value. */
function expand(template: string, params: PathParams | null): string {
  return template.replace(/\{([^}]+)\}/g, (_, name: string) => {
    const value = params?.[name];
    if (value === undefined) throw new TypeError(`missing path parameter ${name} of ${template}`);
    return encodeURIComponent(value);
  });
}

/** The URL of a read: the filters, then the dates of its key. */
function urlOf(path: string, filters: Record<string, unknown>, time: TimeKey): string {
  return path + search({ ...filters, valid_at: time.valid_at, as_of: time.as_of });
}

/** The first query function of an error: no retry on a problem the client caused. */
function retry(failures: number, error: unknown): boolean {
  if (error instanceof ApiProblem && error.status >= 400 && error.status < 500) return false;
  return failures < 2;
}

/** A new `QueryClient` with the defaults of the SDK. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry,
        staleTime: FRESHNESS.object.staleTime,
        gcTime: FRESHNESS.object.gcTime,
        // Revalidation on focus and reconnection is the fallback without the event stream
        // (design « Flux d'événements », « Repli »): the stream sets `fallback`.
        refetchOnWindowFocus: (query: Query) => fallback.active && !isImmutable(query.queryKey),
        refetchOnReconnect: (query: Query) => fallback.active && !isImmutable(query.queryKey),
      },
    },
  });
}

/** Whether the cache revalidates on its own (no event stream); the stream turns it off. */
export const fallback = { active: true };

export interface CacheDeps {
  client: Client;
  queryClient: QueryClient;
}

/** The query factories of the SDK over a client and a cache. */
export function createQueries({ client, queryClient }: CacheDeps) {
  /** Hashes of the queries to read again without `If-None-Match` (`truncated`, 022). */
  const unconditional = new Set<string>();

  async function conditional<T>(
    key: QueryKey,
    url: string,
    previous: Entry<T> | undefined,
    signal: AbortSignal,
  ): Promise<Entry<T>> {
    const hash = queryClient.getQueryCache().find({ queryKey: key, exact: true })?.queryHash;
    const fresh = hash !== undefined && unconditional.delete(hash);
    const etag = fresh ? null : (previous?.etag ?? null);
    const answer = await client.request("GET", url, { ifNoneMatch: etag, signal });
    if (answer.status === 304 && previous) return previous;
    if (answer.status === 304) {
      // A 304 without an entry (evicted meanwhile): read again, unconditionally.
      const again = await client.request("GET", url, { signal });
      return { data: again.data as T, etag: again.etag };
    }
    return { data: answer.data as T, etag: answer.etag };
  }

  function read<T, K extends QueryKey>(key: K, url: string, family: Family): ReadOptions<T, K> {
    return {
      queryKey: key,
      queryFn: ({ signal }) =>
        conditional<T>(key, url, queryClient.getQueryData<Entry<T>>(key), signal),
      ...freshness(family, key),
    };
  }

  function pages<T, K extends QueryKey>(key: K, first: string, family: Family): ListOptions<T, K> {
    return {
      queryKey: key,
      queryFn: ({ pageParam, signal }) => {
        // The page read before under the same cursor, for its `ETag`.
        const data = queryClient.getQueryData<ListData<T>>(key);
        const index = data?.pageParams.indexOf(pageParam) ?? -1;
        const previous = index >= 0 ? data?.pages[index] : undefined;
        return conditional<Page<T>>(key, pageParam ?? first, previous, signal);
      },
      initialPageParam: null,
      // The cursor carries the filters and the dates: `next` is followed as it is.
      getNextPageParam: (last) => last.data.links?.next ?? undefined,
      maxPages: MAX_PAGES,
      placeholderData: keepPreviousData,
      ...freshness(family, key),
    };
  }

  function collection(type: string): string {
    const path = collectionOf(type);
    if (path === null) throw new TypeError(`unknown type ${type}: registerType() first`);
    return path;
  }

  return {
    /** An object by its type and identifier (`/v1/prefixes/{id}`). */
    object<T = unknown>(type: string, id: string, time?: TimeArg | null) {
      const key = keys.object(type, id, time);
      const url = urlOf(`${collection(type)}/${encodeURIComponent(id)}`, {}, key[4]);
      return read<T, ObjectKey>(key, url, "object");
    },

    /** A list of the contract, page by page, with its typed filters. */
    list<P extends ListPath>(
      path: P,
      filters?: FiltersOf<P> | null,
      time?: TimeArg | null,
      params?: PathParams,
    ): ListOptions<ItemOf<P>, PathKey> {
      const key = keys.list(path, filters as Filters, time, params);
      return pages<ItemOf<P>, PathKey>(key, urlOf(expand(path, key[2]), key[3], key[4]), "list");
    },

    /** Any other read by path: `/v1/me`, `/v1/capabilities`, declarations… */
    read<P extends PathsOf<"get">>(
      path: P,
      options: {
        query?: Filters;
        params?: PathParams;
        time?: TimeArg | null;
        family?: Family;
      } = {},
    ): ReadOptions<Success<NonNullable<paths[P]["get"]>>, PathKey> {
      // A list and a read share the shape of their key: a list is read page by page only.
      if (FILTERS[path]) throw new TypeError(`${path} is a list: reads.list()`);
      const key = keys.list(path, options.query, options.time, options.params);
      return read(key, urlOf(expand(path, key[2]), key[3], key[4]), options.family ?? "object");
    },

    /** A sub-resource of an object read at once (`occupancy`, `properties`…). */
    sub<T = unknown>(
      type: string,
      id: string,
      sub: string,
      params?: Filters | null,
      time?: TimeArg | null,
    ): ReadOptions<T, SubKey> {
      const key = keys.sub(type, id, sub, params, time);
      const url = urlOf(`${collection(type)}/${encodeURIComponent(id)}/${sub}`, key[4], key[5]);
      return read<T, SubKey>(key, url, sub === "children" ? "children" : "object");
    },

    /** A sub-resource read page by page (`children` of a node, `history`). */
    subPages<T = unknown>(
      type: string,
      id: string,
      sub: string,
      params?: Filters | null,
      time?: TimeArg | null,
    ): ListOptions<T, SubKey> {
      const key = keys.sub(type, id, sub, params, time);
      const url = urlOf(`${collection(type)}/${encodeURIComponent(id)}/${sub}`, key[4], key[5]);
      return pages<T, SubKey>(key, url, sub === "children" ? "children" : "list");
    },

    /**
     * A read by URL under a key of the caller, for a route outside the generated contract
     * (`/v1/labels` of the 018 until the contract has it).
     */
    byUrl<T>(
      key: readonly unknown[],
      url: string,
      family: Family,
    ): ReadOptions<T, readonly unknown[]> {
      return read<T, readonly unknown[]>(key, url, family);
    },

    /** The next reads of these queries go without `If-None-Match` (`truncated`, 022). */
    unconditional(predicate: (query: Query) => boolean): void {
      for (const query of queryClient.getQueryCache().findAll({ predicate })) {
        unconditional.add(query.queryHash);
      }
    },
  };
}

export type Queries = ReturnType<typeof createQueries>;
