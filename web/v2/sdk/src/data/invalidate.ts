// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What a change invalidates (ADR 0037, decision 5; 022 « Contrat consommé par les
// clients », amended by the 021): the object itself unless the `version` its body states is
// already the revision, the lists of its type, the queries that show it. The past as
// recorded is never invalidated (ADR 0005).

import type { Query, QueryClient } from "@tanstack/query-core";
import type { Entry, Queries } from "./cache.ts";
import { Dependents, versionOf } from "./dependents.ts";
import { isImmutable, isObjectKey } from "./keys.ts";
import { collectionOf, qualify } from "./types.ts";

/** An object a change names. */
export interface Changed {
  readonly type: string;
  readonly id: string;
}

export function createInvalidation(queryClient: QueryClient, reads: Queries) {
  const dependents = new Dependents(queryClient);

  function invalidate(predicate: (query: Query) => boolean): Promise<void> {
    return queryClient.invalidateQueries({
      predicate: (query) => !isImmutable(query.queryKey) && predicate(query),
    });
  }

  /** Whether a query is the object itself, already at `revision`. */
  function upToDate(query: Query, revision: string | null): boolean {
    if (revision === null || !isObjectKey(query.queryKey)) return false;
    const entry = query.state.data as Entry | undefined;
    return entry !== undefined && versionOf(entry.data) === revision;
  }

  return {
    dependents,

    /**
     * Objects changed by a changeset of revision `revision` (`null`: a background task,
     * read again without condition): themselves, their sub-resources, the lists of their
     * types, the queries that show them.
     */
    objects(changed: readonly Changed[], revision: string | null): Promise<void> {
      const ids = new Map<string, string>();
      const lists = new Set<string>();
      const showing = new Set<string>();
      for (const { type, id } of changed) {
        const qualified = qualify(type);
        ids.set(id, qualified);
        const collection = collectionOf(qualified);
        if (collection) lists.add(collection);
        for (const hash of dependents.showing(id)) showing.add(hash);
      }
      return invalidate((query) => {
        const [, resource, id] = query.queryKey as readonly unknown[];
        if (typeof id === "string" && ids.get(id) === resource) return !upToDate(query, revision);
        if (typeof resource === "string" && lists.has(resource)) return true;
        return showing.has(query.queryHash);
      });
    },

    /**
     * A broad invalidation (`truncated`, 022): every query of these types and their lists,
     * read again without `If-None-Match` (inherited values, Q-022-12).
     */
    types(types: readonly string[]): Promise<void> {
      const names = new Set<string>();
      for (const type of types) {
        const qualified = qualify(type);
        names.add(qualified);
        const collection = collectionOf(qualified);
        if (collection) names.add(collection);
      }
      const matches = (query: Query) => names.has(query.queryKey[1] as string);
      reads.unconditional((query) => !isImmutable(query.queryKey) && matches(query));
      return invalidate(matches);
    },

    /** Everything but the past as recorded (`ready`, `reset`, return from the bfcache). */
    all(): Promise<void> {
      return invalidate(() => true);
    },

    /** The queries of a path (`/v1/proposals`), at every filter. */
    path(path: string): Promise<void> {
      return invalidate((query) => query.queryKey[1] === path);
    },
  };
}

export type Invalidation = ReturnType<typeof createInvalidation>;
