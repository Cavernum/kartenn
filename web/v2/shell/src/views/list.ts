import { queryClient, reads, type ListPath } from "@kartenn/sdk";
import { checkFilter, type Filter, type FilterModel, type GridWindow } from "@kartenn/ui";
import { computed, ref, shallowRef, watch, type ComputedRef, type Ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useInfiniteQuery, type UseInfiniteQueryResult } from "../data/query.ts";
import { useNextPage, type ListWindow } from "../prefetch/nextPage.ts";
import { useTime } from "../time/index.ts";
import { problemSlug, rowsOf, useDeclarations, useFilterWords } from "./data.ts";
import {
  filtersOfQuery,
  listFilterModel,
  queryOfFilters,
  sentFilters,
  type FilterWords,
} from "./filters.ts";

// A list of the API filtered by the URL (REQ-021-18, 22): its model of filters, the
// filters of the route's query (drafts kept beside them until applied), the pages read
// through the cache at the dates of the page, the next one read before the window reaches
// the end, and a refused cursor that starts the list again. The views of the lists, of the
// anomalies and of the properties read it.

export interface FilteredList<T> {
  model: ComputedRef<FilterModel>;
  words: FilterWords;
  /** The filters shown on the bar: applied, invalid (read from the URL) or drafts. */
  filters: Ref<Filter[]>;
  /** A change of the bar: the applied ones go to the URL, which the list follows. */
  onFilters: (next: Filter[]) => void;
  /** The keys of the filters applied (neither drafts nor invalid). */
  applied: ComputedRef<Set<string>>;
  list: UseInfiniteQueryResult<unknown>;
  rows: ComputedRef<T[]>;
  /** The list failed (not its next page, which only stops the prefetch). */
  failed: ComputedRef<boolean>;
  /** Every page is read. */
  complete: ComputedRef<boolean>;
  /** A refused cursor started the list again. */
  restarted: Ref<boolean>;
  onWindow: (window: GridWindow) => void;
}

export function useFilteredList<T>(path: () => ListPath): FilteredList<T> {
  const route = useRoute();
  const router = useRouter();
  const time = useTime();
  const { declarations, loading } = useDeclarations();
  const words = useFilterWords(declarations);
  const model = computed(() => listFilterModel(path(), declarations.value, words));

  const filters = shallowRef<Filter[]>([]);
  watch(
    [() => route.query, model],
    ([query]) => {
      const drafts = filters.value.filter((filter) => filter.draft);
      filters.value = [...filtersOfQuery(model.value, query), ...drafts];
    },
    { immediate: true },
  );
  function onFilters(next: Filter[]): void {
    filters.value = next;
    const query = queryOfFilters(model.value, next, route.query);
    const now = queryOfFilters(model.value, filtersOfQuery(model.value, route.query), route.query);
    if (JSON.stringify(query) !== JSON.stringify(now)) void router.replace({ query });
  }
  /** The filters of the URL: the list reads them, never the bar's copy, a render late. */
  const ofUrl = computed(() => filtersOfQuery(model.value, route.query));
  const applied = computed(
    () =>
      new Set(
        ofUrl.value
          .filter((filter) => !checkFilter(model.value, filter))
          .map((filter) => filter.key),
      ),
  );
  const sent = computed(() => sentFilters(path(), model.value, ofUrl.value, declarations.value));
  /** A property filter of the URL waits for the declarations that type it, never dropped. */
  const waiting = computed(
    () => loading.value && Object.keys(route.query).some((name) => name.startsWith("prop.")),
  );

  const read = () =>
    reads.list(path(), sent.value as never, {
      validAt: time.value.validAt,
      asOf: time.value.asOf,
    });
  const list = useInfiniteQuery(() => ({ ...read(), enabled: !waiting.value }));
  const rows = computed(() => rowsOf(list.data.value) as unknown as T[]);
  const view = ref<ListWindow>({ end: 0, size: 0 });
  useNextPage(list, view);

  // A refused cursor (expired, or of another list): the list starts again from its first
  // page, and says so.
  const restarted = ref(false);
  watch(
    () => list.isFetchNextPageError.value && problemSlug(list.error.value) === "invalid-cursor",
    (refused) => {
      if (!refused) return;
      restarted.value = true;
      void queryClient.resetQueries({ queryKey: read().queryKey, exact: true });
    },
  );

  return {
    model,
    words,
    filters,
    onFilters,
    applied,
    list: list as UseInfiniteQueryResult<unknown>,
    rows,
    failed: computed(() => list.isError.value && !list.isFetchNextPageError.value),
    complete: computed(() => !list.hasNextPage.value && list.isSuccess.value),
    restarted,
    onWindow: (window) => (view.value = { end: window.end, size: window.size }),
  };
}
