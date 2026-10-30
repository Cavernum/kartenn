import type { Entry, Page } from "@kartenn/sdk";
import type { InfiniteData } from "@tanstack/query-core";
import { toValue, watchEffect, type MaybeRefOrGetter, type Ref } from "vue";

// The next page of a list, read before it is needed (REQ-021-22; design « Préchargement »):
// when the rendered window comes within two screens of the end of the loaded rows.

/** How many screens ahead of the end of the loaded rows the next page is read. */
export const SCREENS_AHEAD = 2;

/**
 * Where the view of a list stands. `end` is the index, among the rows **loaded** (the
 * pages the cache keeps, the first row of the first kept page being 0), after the last
 * rendered row; never an index in the whole list, which would read every page once the
 * cache drops the first ones (`MAX_PAGES`). `size` is the rows of one screen.
 */
export interface ListWindow {
  end: number;
  size: number;
}

/** What `useNextPage` reads of a paged query (`useInfiniteQuery` of `reads.list`). */
export interface PagedList {
  data: Readonly<Ref<InfiniteData<Entry<Page<unknown>>, unknown> | undefined>>;
  hasNextPage: Readonly<Ref<boolean>>;
  isFetching: Readonly<Ref<boolean>>;
  /** The last read of the next page failed: no prefetch until a read succeeds again. */
  isFetchNextPageError: Readonly<Ref<boolean>>;
  fetchNextPage: () => Promise<unknown>;
}

/** Whether a window `end`, of `size` rows, is within `screens` screens of `loaded` rows. */
export function nearEnd(loaded: number, { end, size }: ListWindow, screens = SCREENS_AHEAD) {
  return size > 0 && loaded - end < screens * size;
}

const loadedOf = (data: PagedList["data"]["value"]) =>
  data?.pages.reduce((count, page) => count + page.data.items.length, 0) ?? 0;

/**
 * Reads the next page of `list` whenever its window nears the end, one page at a time: not
 * while a read of the list is under way, nor after the last page, nor after a failed next
 * page (a refused prefetch is ignored, design « Erreurs ») until the list is read again —
 * the user's « Réessayer », an invalidation, another query.
 */
export function useNextPage(list: PagedList, view: MaybeRefOrGetter<ListWindow>): void {
  watchEffect(() => {
    const at = toValue(view);
    if (!list.hasNextPage.value || list.isFetching.value) return;
    if (list.isFetchNextPageError.value) return;
    if (nearEnd(loadedOf(list.data.value), at)) void list.fetchNextPage();
  });
}
