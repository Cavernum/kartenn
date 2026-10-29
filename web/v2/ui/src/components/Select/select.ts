// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";

/**
 * A choice of a `Select` or a suggestion of a `Combobox`: its value (a key, never shown
 * alone when a label exists, REQ-020-21) and its label; `detail` is written after it in
 * fixed pitch (a key, a prefix, a count).
 */
export interface ListOption {
  value: string;
  label: string;
  detail?: string | undefined;
  /** The label in fixed pitch (an address, a prefix, an identifier). */
  mono?: boolean | undefined;
  /** A leading icon, decorative: the label names the choice. */
  icon?: IconName | undefined;
  disabled?: boolean | undefined;
}
