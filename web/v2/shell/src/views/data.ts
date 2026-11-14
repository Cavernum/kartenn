import { FRESHNESS } from "@kartenn/sdk";
import { computed, watchEffect, type ComputedRef, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { useQueries } from "../data/queries.ts";
import { useInfiniteQuery, useLabel, useLocale } from "../data/query.ts";
import { carriedRead, rowsOf } from "./carried.ts";
import { cacheSuggest, type DeclarationOfModel, type FilterWords } from "./filters.ts";

// What the views of the referential share (task 11): the declared properties, the words of
// the filters, the rows of a paged list (`carried.ts`, which the routes read without the
// composables); the problems are said by `problems/`.

export { carriedRead, carrierOf, rowsOf } from "./carried.ts";

/**
 * The declared properties a kind carries (003, 016; REQ-021-18), every page of them, at
 * the present: the model of the filters of its list, its columns and the editors of its
 * properties read them. Read filtered by the server (`carrier`, task 021-11p7), never the
 * whole catalog; their pages follow one another, all kept (the cache keeps ten of a list
 * else, and would drop the first ones). `carrier` `null`: nothing read, nothing declared.
 */
export function useDeclarations(carrier: () => string | null): {
  declarations: ComputedRef<DeclarationOfModel[]>;
  /** The first page is coming. */
  loading: ComputedRef<boolean>;
  /** No more page will be read: all of them, or a next page refused. */
  settled: ComputedRef<boolean>;
  /** The first page failed: nothing is known of the declarations. */
  failed: ComputedRef<boolean>;
  error: Readonly<Ref<unknown>>;
  refetch: () => void;
} {
  const list = useInfiniteQuery(() => {
    const kind = carrier();
    return {
      ...carriedRead(kind ?? ""),
      maxPages: Infinity,
      // Another kind's declarations never stand for this one's while they come.
      placeholderData: () => undefined,
      enabled: kind !== null,
    };
  });
  const none = computed(() => carrier() === null);
  const declarations = computed(() =>
    none.value ? [] : (rowsOf(list.data.value) as unknown as DeclarationOfModel[]),
  );
  watchEffect(() => {
    if (list.hasNextPage.value && !list.isFetching.value && !list.isFetchNextPageError.value) {
      void list.fetchNextPage();
    }
  });
  return {
    declarations,
    loading: computed(() => !none.value && list.data.value === undefined && !list.isError.value),
    // By the data itself: an observer's result is set field by field, and a list waiting for
    // the declarations must see them as soon as it may read.
    settled: computed(
      () =>
        none.value ||
        (list.data.value !== undefined &&
          (!list.hasNextPage.value || list.isFetchNextPageError.value)),
    ),
    failed: computed(() => !none.value && list.data.value === undefined && list.isError.value),
    error: list.error,
    refetch: () => void list.refetch(),
  };
}

/**
 * The declarations of some keys, whatever kind carries them (an inherited value, a gap, a
 * property an operation names): each read by its key as the prefix `q` (one page of
 * `GET /v1/property-declarations?q=<key>`, the declaration of that very key kept), never
 * the whole catalog. A key not declared, or not yet read, has none.
 */
export function useDeclarationsOf(keys: () => readonly string[]): {
  of: (key: string | null | undefined) => DeclarationOfModel | undefined;
  /** A read failed: the declaration of a key may be missing for that. */
  failed: ComputedRef<boolean>;
  /** Why, the first refusal. */
  error: ComputedRef<unknown>;
  /** Reads again the keys refused. */
  refetch: () => void;
} {
  const wanted = computed(() => [...new Set(keys())].sort());
  const results = useQueries(() => wanted.value.map(declarationRead));
  const byKey = computed(() => {
    const found = new Map<string, DeclarationOfModel>();
    for (const result of results.value) {
      const declaration = result.data as DeclarationOfModel | null | undefined;
      if (declaration) found.set(declaration.key, declaration);
    }
    return found;
  });
  return {
    of: (key) => (key ? byKey.value.get(key) : undefined),
    failed: computed(() => results.value.some((result) => result.isError)),
    error: computed(() => results.value.find((result) => result.isError)?.error),
    refetch: () => {
      for (const result of results.value) if (result.isError) void result.refetch();
    },
  };
}

/** The read of the declaration of one key: the first page of its prefix, that key kept. */
function declarationRead(key: string) {
  return {
    queryKey: ["shell", "declaration", key] as const,
    queryFn: async ({ signal }: { signal: AbortSignal }) =>
      ((await cacheSuggest("/v1/property-declarations", { q: key }, signal)).find(
        (item) => item.key === key,
      ) as DeclarationOfModel | undefined) ?? null,
    ...FRESHNESS.list,
  };
}

/**
 * The label of a key in a space of the dictionary (018), `undefined` when only its key is
 * known: the screen then shows the key in fixed pitch (REQ-020-21).
 */
export function useKeyLabel(): (space: string, key: string) => string | null {
  const label = useLabel();
  return (space, key) => {
    const found = label(space, key);
    return found === key ? null : found;
  };
}

/** The words of the filters of a list, in the language of the moment. */
export function useFilterWords(declarations: Ref<readonly DeclarationOfModel[]>): FilterWords {
  const { t, te } = useI18n();
  const keyLabel = useKeyLabel();
  const locale = useLocale();
  const known = (key: string) => (te(key) ? t(key) : undefined);
  return {
    field: (name) => known(`views.field.${name}`),
    value: (name, value) => known(`views.value.${name}.${value}`),
    property: (key) =>
      keyLabel("property", key) ??
      declarations.value.find((d) => d.key === key)?.labels?.[locale.value],
    propertyValue: (key, value) => keyLabel("value", `${key}/${value}`) ?? undefined,
  };
}
