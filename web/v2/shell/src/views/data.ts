import { ApiProblem, reads, type Entry, type Page } from "@kartenn/sdk";
import type { ProblemFacts } from "@kartenn/ui";
import { computed, watchEffect, type ComputedRef, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery, useLabel, useLocale } from "../data/query.ts";
import type { DeclarationOfModel, FilterWords } from "./filters.ts";

// What the views of the referential share (task 11): the declared properties, the words of
// the filters, the rows of a paged list, and the facts of an error for `ErrorState`.

/** How many pages of declarations are read at most (a page of the contract each). */
const DECLARATION_PAGES = 10;

/** The rows of the pages of a list, in order. */
export function rowsOf<T>(data: { pages: readonly Entry<Page<T>>[] } | undefined): T[] {
  return data?.pages.flatMap((page) => page.data.items) ?? [];
}

/** What an error state shows of an error: the problem of the API as it is, else nothing. */
export function problemOf(error: unknown): ProblemFacts | undefined {
  return error instanceof ApiProblem ? error : undefined;
}

/** The slug of the type of a problem (`urn:kartenn:problem:<slug>`), `null` otherwise. */
export function problemSlug(error: unknown): string | null {
  return error instanceof ApiProblem ? error.code : null;
}

/**
 * The declared properties (003, 016), every page of them, at the present: the model of the
 * filters and the labels of the properties read them. Their pages follow one another.
 */
export function useDeclarations(): {
  declarations: ComputedRef<DeclarationOfModel[]>;
  loading: Readonly<Ref<boolean>>;
  error: Readonly<Ref<unknown>>;
  refetch: () => void;
} {
  const list = useInfiniteQuery(() => reads.list("/v1/property-declarations", null, null));
  watchEffect(() => {
    const pages = list.data.value?.pages.length ?? 0;
    if (list.hasNextPage.value && !list.isFetching.value && pages < DECLARATION_PAGES) {
      void list.fetchNextPage();
    }
  });
  const declarations = computed(() => rowsOf(list.data.value) as unknown as DeclarationOfModel[]);
  return {
    declarations,
    // By the data itself: an observer's result is set field by field, and a list waiting
    // for the declarations must see them as soon as it may read.
    loading: computed(() => list.data.value === undefined && !list.isError.value),
    error: list.error,
    refetch: () => void list.refetch(),
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
