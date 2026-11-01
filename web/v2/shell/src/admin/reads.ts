import { reads, type FiltersOf, type ItemOf, type ListPath } from "@kartenn/sdk";
import { computed, shallowReactive, toValue, watch, watchEffect, type MaybeRefOrGetter } from "vue";
import { useInfiniteQuery, useQueryClient } from "../data/query.ts";
import { collectionOfKind, nameOf, type Item } from "../views/collections.ts";
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

/**
 * The names of the objects of scopes (an entity, a site, a domain, a prefix), read once
 * each when `scopes` names them: `nameOf(id)` while unread is undefined.
 */
export function useScopeNames(scopes: () => readonly { kind: string; id?: string | null }[]) {
  const queryClient = useQueryClient();
  const names = shallowReactive(new Map<string, string>());
  watch(
    scopes,
    (now) => {
      for (const scope of now) {
        const collection = collectionOfKind(scope.kind);
        if (!scope.id || !collection || names.has(scope.id)) continue;
        const id = scope.id;
        void queryClient
          .fetchQuery(reads.object<Item>(collection.kind, id))
          .then((entry) => names.set(id, nameOf(entry.data)))
          .catch(() => undefined);
      }
    },
    { immediate: true },
  );
  return (id: string): string | undefined => names.get(id);
}
