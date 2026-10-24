// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The surface of the draft offered to plugins (W10, amended by the 024): operations in the
// format of the contract, added to the active draft of the tab, a draft of the server
// created when there is none (021 design « Écritures »).

import type { components } from "./api/schema";
import type { Draft, DraftOutcome } from "./data/drafts.ts";
import { drafts } from "./instance.ts";

/** An operation of a changeset, as the contract defines it. */
export type Operation = components["schemas"]["Operation"];

let onAdd: ((draft: Draft) => void) | null = null;

export const draft = {
  /** The identifier of the active draft of the tab, `null` if none. */
  active(): string | null {
    return drafts.active();
  },
  /**
   * Adds an operation at the end of the active draft, creating one if there is none; the
   * shell then opens the draft.
   */
  async add(op: Operation): Promise<DraftOutcome<readonly Operation[]>> {
    const outcome = await drafts.addToActive([op]);
    if (outcome.kind === "saved") onAdd?.(outcome.draft);
    return outcome;
  },
};

/** The shell's side: what an addition by a plugin does. */
export const draftHost = {
  onAdd(handler: ((draft: Draft) => void) | null): void {
    onAdd = handler;
  },
};
