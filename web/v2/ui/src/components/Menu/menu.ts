// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";

/** An entry of a menu; a disabled one says why (design « Inventaire », line 11). */
export interface MenuEntry {
  kind?: "item";
  id: string;
  label: string;
  icon?: IconName;
  /** The shortcut shown at the end, as `KeyboardEvent.key` names the keys. */
  shortcut?: string[];
  /** Disabled: it stays in the menu and focusable (`aria-disabled`), with its reason. */
  disabled?: boolean;
  /** Why the entry is disabled: its tooltip and its description. */
  reason?: string;
  /** The permission the user lacks: disabled, « Il faut <permission> » (024/M14). */
  forbidden?: string;
  danger?: boolean;
}

/** A separator, or the title of a group. */
export type MenuDivider = { kind: "separator" } | { kind: "label"; label: string };

export type MenuItem = MenuEntry | MenuDivider;
