// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";

/** An action of a split button (design « Inventaire », line 32; REQ-020-37). */
export interface SplitAction {
  id: string;
  label: string;
  icon?: IconName;
  /** Impossible now: listed in the menu, disabled, with its reason. */
  disabled?: boolean;
  /** Why it is impossible (a policy that holds the changeset). */
  reason?: string;
  /** The permission the user lacks (024/M14). */
  forbidden?: string;
}
