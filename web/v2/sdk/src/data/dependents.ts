// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The dependents of an object (design « Clés et fraîcheur », REQ-021-23): the queries
// whose body shows it, as a summary of a linked object (`domain` of a prefix), an embedded
// element (interfaces of a machine card) or an item of a list. An event naming an object
// also invalidates them: that is what the composite ETag (006/H19) is for on the client.

import type { Query, QueryCacheNotifyEvent, QueryClient } from "@tanstack/query-core";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * The identifiers of the objects a body shows: every member `id` holding a UUID, at any
 * depth (summaries, references, embedded elements, items). The walk stops at `limit`
 * nodes, a bound on a pathological body.
 */
export function refsOf(body: unknown, limit = 1_000_000): Set<string> {
  const found = new Set<string>();
  const stack: unknown[] = [body];
  let visited = 0;
  while (stack.length > 0 && visited < limit) {
    const node = stack.pop();
    visited += 1;
    if (Array.isArray(node)) {
      for (const item of node) if (typeof item === "object" && item !== null) stack.push(item);
    } else if (typeof node === "object" && node !== null) {
      for (const [name, value] of Object.entries(node)) {
        if (name === "id" && typeof value === "string" && UUID.test(value)) found.add(value);
        else if (typeof value === "object" && value !== null) stack.push(value);
      }
    }
  }
  return found;
}

/**
 * The revision a body states (`version`), compared at equality with the revision of an
 * event, never with the ETag (opaque, 006/H19). `null` when the body has none: the client
 * then reads again with `If-None-Match`.
 */
export function versionOf(body: unknown): string | null {
  const version = (body as { version?: unknown } | null)?.version;
  return typeof version === "string" ? version : null;
}

/** The index `id → queries` of the cache, kept as answers come and queries go. */
export class Dependents {
  private readonly byId = new Map<string, Set<string>>();
  private readonly ofQuery = new Map<string, Set<string>>();
  private readonly stop: () => void;

  constructor(private readonly queryClient: QueryClient) {
    const cache = queryClient.getQueryCache();
    for (const query of cache.getAll()) this.index(query);
    this.stop = cache.subscribe((event: QueryCacheNotifyEvent) => {
      if (event.type === "removed") this.drop(event.query.queryHash);
      else if (event.type === "updated" && event.action.type === "success") this.index(event.query);
    });
  }

  private index(query: Query): void {
    const hash = query.queryHash;
    this.drop(hash);
    if (query.state.data === undefined) return;
    const ids = refsOf(query.state.data);
    this.ofQuery.set(hash, ids);
    for (const id of ids) {
      let queries = this.byId.get(id);
      if (!queries) this.byId.set(id, (queries = new Set()));
      queries.add(hash);
    }
  }

  private drop(hash: string): void {
    const ids = this.ofQuery.get(hash);
    if (!ids) return;
    this.ofQuery.delete(hash);
    for (const id of ids) {
      const queries = this.byId.get(id);
      queries?.delete(hash);
      if (queries?.size === 0) this.byId.delete(id);
    }
  }

  /** The hashes of the queries whose body shows the object `id`. */
  showing(id: string): ReadonlySet<string> {
    return this.byId.get(id) ?? new Set();
  }

  /** Stops following the cache. */
  close(): void {
    this.stop();
    this.byId.clear();
    this.ofQuery.clear();
  }
}
