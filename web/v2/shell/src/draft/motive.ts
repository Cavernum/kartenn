// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The motive typed for a draft and not sent yet (Q-021-22, task 021-12qb): kept outside the
// form (`DraftSubmit`), so that it survives what unmounts the form — the panel's other tab,
// the page of the draft (`/drafts/:id`), where the same form shows it again — and goes only
// when it is sent (the draft's reason then says it) or abandoned (`dropMotive`, the panel
// closed after « Abandonner »). The frame tells the panel it is modified while one is kept.

import { reactive } from "vue";

export interface Motive {
  summary: string;
  description: string;
}

const typed = reactive(new Map<string, Motive>());

/** The motive typed for `draft`, if one is kept. */
export function typedMotive(draft: string): Motive | undefined {
  return typed.get(draft);
}

/** Keeps the motive typed for `draft` (`null`: nothing of the user's own any more). */
export function keepMotive(draft: string, motive: Motive | null): void {
  if (motive) typed.set(draft, { ...motive });
  else typed.delete(draft);
}

/** Drops the motive typed for `draft` (« Abandonner »). */
export function dropMotive(draft: string): void {
  typed.delete(draft);
}
