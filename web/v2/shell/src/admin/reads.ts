import { reads, type FiltersOf, type ItemOf, type ListPath } from "@kartenn/sdk";
import { computed, toValue, watchEffect, type MaybeRefOrGetter } from "vue";
import { useInfiniteQuery } from "../data/query.ts";
import { rowsOf } from "../views/data.ts";

// The lists of the administration, short (groups, roles, the members of a group, its
// bindings, the identities): read whole, page after page, ten pages at most, at the present
// (the rights of now are administered). Their rows, and the read for the three states.

/** At most so many pages of a list of the administration. */
export const ADMIN_PAGES = 10;

export function useList<P extends ListPath>(
  path: P,
  filters?: MaybeRefOrGetter<FiltersOf<P> | null>,
  params?: MaybeRefOrGetter<Record<string, string> | undefined>,
  enabled: MaybeRefOrGetter<boolean> = true,
) {
  const query = useInfiniteQuery(() => ({
    ...reads.list(path, toValue(filters) ?? null, null, toValue(params)),
    enabled: toValue(enabled),
  }));
  watchEffect(() => {
    if (
      query.hasNextPage.value &&
      !query.isFetching.value &&
      !query.isFetchNextPageError.value &&
      (query.data.value?.pages.length ?? 0) < ADMIN_PAGES
    ) {
      void query.fetchNextPage();
    }
  });
  const rows = computed(() => rowsOf(query.data.value) as ItemOf<P>[]);
  return { query, rows };
}
