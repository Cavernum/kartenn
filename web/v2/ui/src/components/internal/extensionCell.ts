// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type FunctionalComponent } from "vue";

// The cell of a column of an extension (a plugin, point `column`, W5), shared by
// `DataGrid` and `TreeGrid`: the custom element whose tag the column names (`element`),
// created for its row. The grids know nothing of plugins: the host registered the tag.

/**
 * The custom element of an extension column, its properties set as such (not attributes):
 * `row`, then those of `extra` (which may give another `row`); `extra` `null`, no element.
 */
export const ExtensionCell: FunctionalComponent<{
  tag: string;
  row: unknown;
  extra?: Record<string, unknown> | null | undefined;
}> = (p) =>
  p.extra === null
    ? null
    : h(p.tag, {
        ".row": p.row,
        ...Object.fromEntries(Object.entries(p.extra ?? {}).map(([k, v]) => [`.${k}`, v])),
      });
ExtensionCell.props = ["tag", "row", "extra"];

/**
 * The controls an element of an extension may render in a cell (a link, a button, a
 * field): out of the tab order of the grid, reached by Enter or F2 from their cell.
 */
export const CELL_WIDGETS =
  "a[href], button, input, select, textarea, [tabindex], [contenteditable]";
