// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The children of a tree read page by page (`TreeGrid`, `Tree`; the old shell's
// `tree/model.ts`): a list with pages left ends with a row that asks the host for the next
// page (`load`) as soon as it comes into the view of the tree, or when it is activated; a
// node opened whose children are still to read asks for its first page. One ask per list
// and per number of children read: a page that has not come yet is never asked twice.
// A page that comes is said (its count, and whether the list is whole), WCAG 4.1.3.

import { onBeforeUnmount, onMounted, onUpdated, type Ref } from "vue";

/** The attribute of the row that reads the next page: the parent's id, `""` for the roots. */
export const MORE_ATTRIBUTE = "data-more";

export function useTreePages(
  scroller: Ref<HTMLElement | null>,
  /** The number of children of `parent` read so far (`null`: the first level). */
  sizeOf: (parent: string | null) => number,
  load: (parent: string | null) => void,
  /** A page came: `count` children more; `whole`, the list has no page left. */
  arrived: (count: number, whole: boolean) => void,
  /** Whether pages remain after the children of `parent` read so far. */
  rest: (parent: string | null) => boolean,
) {
  const asked = new Map<string, number>();
  /** The lists whose page is awaited, with their size when asked. */
  const awaited = new Map<string, { parent: string | null; size: number }>();

  /**
   * Asks for the next page of `parent`, unless asked already at this size; `force` (the
   * row activated: a page that failed) asks again.
   */
  function request(parent: string | null, force = false): void {
    const key = parent ?? "";
    const size = sizeOf(parent);
    if (!force && asked.get(key) === size) return;
    asked.set(key, size);
    awaited.set(key, { parent, size });
    load(parent);
  }

  /** The pages come between renders: each one awaited that grew its list is said. */
  function said(): void {
    for (const [key, { parent, size }] of awaited) {
      const now = sizeOf(parent);
      if (now === size) continue;
      awaited.delete(key);
      arrived(now - size, !rest(parent));
    }
  }

  let observer: IntersectionObserver | undefined;
  /** Each render: the rows that read a page, observed again (one that stayed in view asks
   * again once its page came). */
  function observe(): void {
    const root = scroller.value;
    if (!root || typeof IntersectionObserver === "undefined") return;
    observer ??= new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          const parent = (entry.target as HTMLElement).getAttribute(MORE_ATTRIBUTE);
          if (parent !== null) request(parent === "" ? null : parent);
        }
      },
      { root },
    );
    observer.disconnect();
    for (const row of root.querySelectorAll(`[${MORE_ATTRIBUTE}]`)) observer.observe(row);
  }
  onMounted(observe);
  onUpdated(() => {
    said();
    observe();
  });
  onBeforeUnmount(() => observer?.disconnect());

  return { request };
}
