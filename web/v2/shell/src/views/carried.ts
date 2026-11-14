import { reads, type Entry, type ListPath, type Page } from "@kartenn/sdk";
import { COLLECTIONS } from "./collections.ts";

// What the routes of the referential read of a list before its screen (`routes.ts`, their
// prefetch), apart from the composables of the views (`data.ts`, which gives them too): the
// initial shell holds these, never the composables (task 22n, budget « Shell initial »).

/** The rows of the pages of a list, in order. */
export function rowsOf<T>(data: { pages: readonly Entry<Page<T>>[] } | undefined): T[] {
  return data?.pages.flatMap((page) => page.data.items) ?? [];
}

/**
 * The kind whose declarations a list of `path` types its property filters by (`carrier` of
 * `GET /v1/property-declarations`, task 021-11p7): the kind of a collection of the shell,
 * `null` for a list of no kind (the anomalies, the audit), which no declaration types.
 */
export function carrierOf(path: ListPath): string | null {
  return COLLECTIONS.find((collection) => collection.path === path)?.kind ?? null;
}

/**
 * The read of the declarations a kind carries, at the present: one key of the cache for
 * the filters and the columns of its list, the properties of its records, and the prefetch
 * of its routes (`routes.ts`).
 */
export function carriedRead(carrier: string) {
  return reads.list("/v1/property-declarations", { carrier }, null);
}
