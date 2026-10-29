// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { ListOption } from "../Select/select.ts";

/**
 * Where a `Combobox` takes its suggestions: the host's function (a read of the SDK, from
 * its cache), called with the text of the field once the typing pauses; `signal` aborts
 * when a newer text replaces it, so that a late answer is never shown.
 */
export type Suggest = (query: string, signal: AbortSignal) => Promise<ListOption[]>;

/** Folds case and accents, so that « prefixe » finds « Préfixe ». */
const fold = (text: string) => text.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();

/**
 * The options of a local list that match a text (in their label or their detail),
 * whatever the case and the accents; the empty text matches them all. For a host whose
 * list is already loaded: `suggest = async (q) => filterOptions(list, q)`.
 */
export function filterOptions(options: readonly ListOption[], query: string): ListOption[] {
  const wanted = fold(query.trim());
  if (!wanted) return [...options];
  return options.filter(
    (option) => fold(option.label).includes(wanted) || fold(option.detail ?? "").includes(wanted),
  );
}
