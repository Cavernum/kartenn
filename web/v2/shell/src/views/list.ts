import { filterModel, queryClient, reads, type ListPath } from "@kartenn/sdk";
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
  paramsOf,
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
  /** Property filters of the URL left untyped: the list is not read (an error state). */
  blocked: ComputedRef<Blocked | null>;
  /** Why the declarations could not be read, when they could not. */
  declarationsError: Readonly<Ref<unknown>>;
  /** A refused cursor started the list again. */
  restarted: Ref<boolean>;
  onWindow: (window: GridWindow) => void;
}

/** Why a list is not read: the declarations unread, or keys no declaration types. */
export interface Blocked {
  reason: "failed" | "unknown";
  /** The parameters of the URL concerned (`prop.dns.ttl:between`). */
  names: string[];
}

/** Whether a list takes property filters (`prop.<key>` in its contract). */
function takesProperties(path: ListPath): boolean {
  const probe = { key: "probe.key", type: "text", carriers: [] as string[] };
  return filterModel(path, [probe]).some((field) => field.property !== undefined);
}

/** The parameters of the URL the property filters of a model read. */
function typedNames(model: FilterModel): Set<string> {
  const names = new Set<string>();
  for (const property of model) {
    if (!property.key.startsWith("prop.")) continue;
    for (const operator of property.operators) {
      names.add(
        operator === "eq"
          ? property.key
          : operator === "exists" || operator === "absent"
            ? `${property.key}:exists`
            : `${property.key}:${operator}`,
      );
    }
  }
  return names;
}

export function useFilteredList<T>(path: () => ListPath): FilteredList<T> {
  const route = useRoute();
  const router = useRouter();
  const time = useTime();
  const declared = useDeclarations();
  const { declarations } = declared;
  const words = useFilterWords(declarations);
  const model = computed(() => listFilterModel(path(), declarations.value, words));

  // The property filters of the URL that no declaration read types (REQ-021-18): the list
  // waits for every page of the declarations; a key still unknown then (or the
  // declarations unread) is a chip « non reconnu » and an error state — never a list read
  // without the filter the URL holds.
  const untyped = computed(() =>
    takesProperties(path())
      ? [...new Set(paramsOf(route.query).keys())].filter(
          (name) => name.startsWith("prop.") && !typedNames(model.value).has(name),
        )
      : [],
  );
  const blocked = computed<Blocked | null>(() => {
    if (untyped.value.length === 0) return null;
    if (declared.failed.value) return { reason: "failed", names: untyped.value };
    if (declared.settled.value) return { reason: "unknown", names: untyped.value };
    return null;
  });
  /** The chips of the parameters not recognised, their values as the URL holds them. */
  const unrecognised = (query: typeof route.query): Filter[] =>
    blocked.value === null
      ? []
      : untyped.value.flatMap((name) =>
          paramsOf(query)
            .getAll(name)
            .map((value): Filter => ({ key: name, operator: "eq", values: [value] })),
        );

  const filters = shallowRef<Filter[]>([]);
  watch(
    [() => route.query, model, blocked],
    ([query]) => {
      const drafts = filters.value.filter((filter) => filter.draft);
      filters.value = [...filtersOfQuery(model.value, query), ...unrecognised(query), ...drafts];
    },
    { immediate: true },
  );
  function onFilters(next: Filter[]): void {
    filters.value = next;
    // The chips not recognised write their parameter back as it was, or drop it.
    const replaced = new Set(blocked.value?.names ?? []);
    const query = queryOfFilters(model.value, next, route.query, replaced);
    const now = queryOfFilters(
      model.value,
      [...filtersOfQuery(model.value, route.query), ...unrecognised(route.query)],
      route.query,
      replaced,
    );
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

  const read = () =>
    reads.list(path(), sent.value as never, {
      validAt: time.value.validAt,
      asOf: time.value.asOf,
    });
  // Read only once every property filter of the URL is typed.
  const list = useInfiniteQuery(() => ({ ...read(), enabled: untyped.value.length === 0 }));
  const rows = computed(() => rowsOf(list.data.value) as unknown as T[]);
  const view = ref<ListWindow>({ end: 0, size: 0 });
  useNextPage(list, view);

  // A refused cursor (expired, or of another list): the list starts again from its first
  // page, and says so.
  const restarted = ref(false);
  // Said until the next change of the filters, the list read anew.
  watch(sent, () => (restarted.value = false));
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
    blocked,
    declarationsError: declared.error,
    restarted,
    onWindow: (window) => (view.value = { end: window.end, size: window.size }),
  };
}
