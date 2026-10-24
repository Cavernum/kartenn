// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** A draft of the user, as the menu of the counter lists it (spec 021, REQ-021-11). */
export interface DraftSummary {
  id: string;
  label: string;
  /** Its operations. */
  count: number;
  /** The active draft, the one the counter counts. */
  current?: boolean;
}
