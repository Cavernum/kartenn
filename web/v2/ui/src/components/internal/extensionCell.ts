// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import {
  h,
  onBeforeUnmount,
  onMounted,
  watch,
  type FunctionalComponent,
  type Ref,
  type VNode,
} from "vue";
import type { GridColumn } from "../DataGrid/grid.ts";

// The cell of a column of an extension (a plugin, point `column`, W5), shared by
// `DataGrid` and `TreeGrid`: the custom element whose tag the column names (`element`),
// created for its row, or what the host puts in the slot `extension` of the grid (the
// shell: its host of an extension, which confines an error of the plugin to the cell). The
// grids know nothing of plugins: the host registered the tag and fills the slot.

/** What the slot `extension` of a grid receives for a cell of an extension column. */
export interface ExtensionSlotProps {
  /** The row as the element receives it: the grid's, unless `elementProps` gives another. */
  row: unknown;
  column: GridColumn;
  /** The other properties of the element (`elementProps`, its `row` left out). */
  properties: Record<string, unknown>;
}

/**
 * The custom element of an extension column, its properties set as such (not attributes):
 * `row`, then those of `extra` (which may give another `row`); `extra` `null`, no element.
 * Given a default slot (the grid passes its slot `extension` on), that slot stands in its
 * place, called with the same properties.
 */
export const ExtensionCell: FunctionalComponent<
  {
    tag: string;
    row: unknown;
    column: GridColumn;
    extra?: Record<string, unknown> | null | undefined;
  },
  Record<string, never>,
  { default?: (given: ExtensionSlotProps) => VNode[] }
> = (p, { slots }) => {
  if (p.extra === null) return null;
  if (slots.default) {
    const { row, ...properties } = p.extra ?? {};
    return slots.default({
      row: p.extra && "row" in p.extra ? row : p.row,
      column: p.column,
      properties,
    });
  }
  return h(p.tag, {
    ".row": p.row,
    ...Object.fromEntries(Object.entries(p.extra ?? {}).map(([k, v]) => [`.${k}`, v])),
  });
};
ExtensionCell.props = ["tag", "row", "column", "extra"];

/**
 * The controls an element of an extension may render in a cell (a link, a button, a
 * field): out of the tab order of the grid, reached by Enter or F2 from their cell.
 */
export const CELL_WIDGETS =
  "a[href], button, input, select, textarea, [tabindex], [contenteditable]";

/**
 * A control of a cell leaves the tab order (APG Grid: one tab stop): a control under a cell
 * `cell` (never the cell itself, `[data-column]`), outside of `except` (an editor).
 */
export function tameControl(el: Element, cell: string, except?: string): void {
  if (!(el instanceof HTMLElement) || !el.matches(CELL_WIDGETS) || el.matches("[data-column]"))
    return;
  if (!el.parentElement?.closest(cell) || (except && el.closest(except))) return;
  if (el.tabIndex !== -1) el.tabIndex = -1;
}

/**
 * The element of an extension renders when it likes (its plugin, after a read): its controls
 * leave the tab order as they come or turn focusable, in the light DOM (ADR 0038, § 4), each
 * mutation looked at in its own subtree by `tame`. Observed while `extended` says so: the tree
 * grid only with a column that has an element, the data grid always, for the widgets its
 * host's components render on their own.
 */
export function useExtensionControls(
  grid: Readonly<Ref<HTMLElement | null>>,
  extended: () => boolean,
  tame: (el: Element) => void,
): void {
  let mutations: MutationObserver | undefined;
  function onMutations(records: MutationRecord[]): void {
    for (const record of records) {
      if (record.type === "attributes") tame(record.target as Element);
      for (const node of record.addedNodes) {
        if (!(node instanceof Element)) continue;
        tame(node);
        for (const el of node.querySelectorAll(CELL_WIDGETS)) tame(el);
      }
    }
  }
  function observe(): void {
    mutations?.disconnect();
    mutations = undefined;
    if (!extended() || !grid.value || typeof MutationObserver === "undefined") return;
    mutations = new MutationObserver(onMutations);
    mutations.observe(grid.value, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["tabindex", "href", "contenteditable"],
    });
  }
  onMounted(observe);
  watch(extended, observe, { flush: "post" });
  onBeforeUnmount(() => mutations?.disconnect());
}
