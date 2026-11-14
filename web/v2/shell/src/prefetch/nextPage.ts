import type { Entry, Page } from "@kartenn/sdk";
import type { InfiniteData } from "@tanstack/query-core";
import { toValue, watch, watchEffect, type MaybeRefOrGetter, type Ref } from "vue";

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

type Pages = PagedList["data"]["value"];
const loadedOf = (data: Pages) =>
  data?.pages.reduce((count, page) => count + page.data.items.length, 0) ?? 0;

/**
 * The rows dropped at the head of `before` in `now`: the pages kept before the first page
 * of `now` (its cursor among those of `before`), the cache past `MAX_PAGES` (task 021-22f).
 */
export function droppedRows(before: Pages, now: Pages): number {
  if (!before || !now || now.pageParams.length === 0) return 0;
  const at = before.pageParams.indexOf(now.pageParams[0]);
  return at <= 0 ? 0 : loadedOf({ pages: before.pages.slice(0, at), pageParams: [] });
}

/**
 * Reads the next page of `list` whenever its window nears the end, one page at a time: not
 * while a read of the list is under way, nor after the last page, nor after a failed next
 * page (a refused prefetch is ignored, design « Erreurs ») until the list is read again —
 * the user's « Réessayer », an invalidation, another query.
 *
 * The window counts among the rows the list held when it was given: a page read past
 * `MAX_PAGES` lands with the first one dropped, before the grid gives its window among the
 * rows kept, and that window is moved up by the rows dropped meanwhile. Read as it stood,
 * at the end of the rows before, it would read a page more at once (task 021-22f), which
 * lands under the rows in view, the grid keeping them in their place.
 */
export function useNextPage(list: PagedList, view: MaybeRefOrGetter<ListWindow>): void {
  /** The pages of the list when the window was last given. */
  let given: Pages;
  watch(
    () => toValue(view),
    () => (given = list.data.value),
    { immediate: true, flush: "sync" },
  );
  watchEffect(() => {
    const at = toValue(view);
    if (!list.hasNextPage.value || list.isFetching.value) return;
    if (list.isFetchNextPageError.value) return;
    const data = list.data.value;
    const end = at.end - droppedRows(given, data);
    if (nearEnd(loadedOf(data), { end, size: at.size })) void list.fetchNextPage();
  });
}
