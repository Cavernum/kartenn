// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";

/** A tab: its value (the name of its panel slot), label, icon, counter and state. */
export interface TabItem {
  value: string;
  label: string;
  icon?: IconName;
  count?: number;
  /** A plugin tab whose view is loading or failed (design « Inventaire », line 10). */
  state?: "loading" | "error";
  disabled?: boolean;
}
