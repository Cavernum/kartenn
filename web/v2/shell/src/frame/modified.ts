// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What the right panel holds that is not written yet (Q-021-22, task 021-12qb): each form
// it shows (`EditPanel`, `ClosePanel`, `DraftSubmit`) says, while it is mounted, whether
// the user changed it since it opened; the frame gives `SidePanel` its `modified`, so that
// Escape, the cross, `]` and the counter ask « Abandonner les modifications ? » first, and
// asks the same before a navigation that would drop an edit (`closeEditWithPage`).

import { computed, onBeforeUnmount, shallowReactive, type ComputedRef } from "vue";

interface Source {
  /** Whether the user changed the form since it opened. */
  modified: () => boolean;
  /** Whether the form goes with the page left (an edit, `closeEditWithPage`). */
  withPage: boolean;
}

const sources = shallowReactive(new Set<Source>());

/**
 * In the `setup` of a form of the right panel: says whether it is modified, until it is
 * unmounted. `withPage`: an edit, which another page drops.
 */
export function reportModified(modified: () => boolean, withPage = false): void {
  const source: Source = { modified, withPage };
  sources.add(source);
  onBeforeUnmount(() => sources.delete(source));
}

/** Whether a form of the right panel is modified (its `modified`). */
export function panelModified(): ComputedRef<boolean> {
  return computed(() => [...sources].some((s) => s.modified()));
}

/** Whether an edit another page would drop is modified (the frame's navigation guard). */
export function editModified(): boolean {
  return [...sources].some((s) => s.withPage && s.modified());
}
