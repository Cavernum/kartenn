import type { Entry, Page } from "@kartenn/sdk";
import type { InfiniteData } from "@tanstack/query-core";
import { toValue, watchEffect, type MaybeRefOrGetter, type Ref } from "vue";

// The next page of a list, read before it is needed (REQ-021-22; design « Préchargement »):
// when the rendered window comes within two screens of the end of the loaded rows.

/** How many screens ahead of the end of the loaded rows the next page is read. */
export const SCREENS_AHEAD = 2;

/** Where the view of a list stands: the index after its last rendered row, rows per screen. */
export interface ListWindow {
  end: number;
  size: number;
}

/** What `useNextPage` reads of a paged query (`useInfiniteQuery` of `reads.list`). */
export interface PagedList {
  data: Readonly<Ref<InfiniteData<Entry<Page<unknown>>, unknown> | undefined>>;
  hasNextPage: Readonly<Ref<boolean>>;
  isFetching: Readonly<Ref<boolean>>;
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
 * while a read of the list is under way, nor after the last page.
 */
export function useNextPage(list: PagedList, view: MaybeRefOrGetter<ListWindow>): void {
  watchEffect(() => {
    const at = toValue(view);
    if (!list.hasNextPage.value || list.isFetching.value) return;
    if (nearEnd(loadedOf(list.data.value), at)) void list.fetchNextPage();
  });
}
