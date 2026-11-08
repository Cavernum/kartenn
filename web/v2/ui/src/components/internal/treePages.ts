// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The children of a tree read page by page (`TreeGrid`, `Tree`; the old shell's
// `tree/model.ts`): a list with pages left ends with a row that asks the host for the next
// page (`load`) as soon as it comes into the view of the tree, or when it is activated; a
// node opened whose children are still to read asks for its first page. One ask per list
// and per number of children read: a page that has not come yet is never asked twice.
// The page that comes is said (its count, and whether the list is whole), WCAG 4.1.3:
// only the children new to the list since the ask, the nodes the tree asked to move into
// it left out; a list that shrinks (a node moved away) says nothing and keeps waiting. A
// notice (`TreeNode.notice`, a level capped) is no child: out of the count, it is said
// after it, and the list is not said whole.

import { onBeforeUnmount, onMounted, onUpdated, type Ref } from "vue";

/** The attribute of the row that reads the next page: the parent's id, `""` for the roots. */
export const MORE_ATTRIBUTE = "data-more";

/** What a tree gives its pages. */
export interface TreePagesHost {
  /** The ids of the children of `parent` read so far (`null`: the first level), notices out. */
  children(parent: string | null): readonly string[];
  /** The label of the notice under the children of `parent`, if any. */
  notice(parent: string | null): string | undefined;
  /** The nodes the tree asked to move into `parent` (they are no page). */
  movedIn(parent: string | null): ReadonlySet<string>;
  load(parent: string | null): void;
  /**
   * A page came: `count` children more; `whole`, the list has no page left and no notice;
   * `notice`, the label of the notice that ends it.
   */
  arrived(count: number, whole: boolean, notice?: string): void;
  /** Whether pages remain after the children of `parent` read so far. */
  rest(parent: string | null): boolean;
}

/** The new children of a list that came by a page: not known when asked, not moved in. */
export function pageGrowth(
  before: ReadonlySet<string>,
  now: readonly string[],
  movedIn: ReadonlySet<string>,
): number {
  return now.filter((id) => !before.has(id) && !movedIn.has(id)).length;
}

export function useTreePages(scroller: Ref<HTMLElement | null>, host: TreePagesHost) {
  const asked = new Map<string, number>();
  /** The lists whose page is awaited, with their children when asked. */
  const awaited = new Map<string, { parent: string | null; known: Set<string> }>();

  /**
   * Asks for the next page of `parent`, unless asked already at this size; `force` (the
   * row activated: a page that failed) asks again.
   */
  function request(parent: string | null, force = false): void {
    const key = parent ?? "";
    const children = host.children(parent);
    if (!force && asked.get(key) === children.length) return;
    asked.set(key, children.length);
    awaited.set(key, { parent, known: new Set(children) });
    host.load(parent);
  }

  /** The pages come between renders: each awaited one that brought new children is said. */
  function said(): void {
    for (const [key, { parent, known }] of awaited) {
      const count = pageGrowth(known, host.children(parent), host.movedIn(parent));
      if (count === 0) continue;
      awaited.delete(key);
      const notice = host.notice(parent);
      host.arrived(count, !host.rest(parent) && notice === undefined, notice);
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
