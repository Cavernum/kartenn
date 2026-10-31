// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";
import type { Shortcut } from "../ShortcutHint/shortcutHint.ts";

/** An entry of a menu; a disabled one says why (design « Inventaire », line 11). */
export interface MenuEntry {
  kind?: "item";
  id: string;
  label: string;
  icon?: IconName;
  /**
   * The shortcut shown at the end, as `KeyboardEvent.key` names the keys: a combination
   * (`["Mod", "k"]`, also in `aria-keyshortcuts`) or a sequence (`[["g"], ["p"]]`, « g
   * puis p », in the entry's description: ARIA has no syntax for one).
   */
  shortcut?: Shortcut;
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

/**
 * An entry that is on or off (APG Menu, `menuitemcheckbox`): choosing it emits `select`
 * with its id and keeps the menu open; the host flips `checked`.
 */
export interface MenuCheckbox {
  kind: "checkbox";
  id: string;
  label: string;
  checked: boolean;
}

/**
 * One of a set where one only is on (APG Menu, `menuitemradio` in a `group`): consecutive
 * radio entries form one group; choosing one emits `select` with its id and closes the menu.
 */
export interface MenuRadio {
  kind: "radio";
  id: string;
  label: string;
  checked: boolean;
}

export type MenuItem = MenuEntry | MenuDivider | MenuCheckbox | MenuRadio;

/** The items of a menu with each run of consecutive radio entries gathered in a group. */
export type MenuBlock =
  | { kind: "single"; item: Exclude<MenuItem, MenuRadio>; key: string }
  | { kind: "radios"; radios: MenuRadio[]; value: string; key: string };

export function menuBlocks(items: MenuItem[]): MenuBlock[] {
  const blocks: MenuBlock[] = [];
  items.forEach((item, i) => {
    const last = blocks.at(-1);
    if (item.kind === "radio") {
      if (last?.kind === "radios") {
        last.radios.push(item);
        if (item.checked) last.value = item.id;
      } else
        blocks.push({
          kind: "radios",
          radios: [item],
          value: item.checked ? item.id : "",
          key: `radios-${i}`,
        });
    } else blocks.push({ kind: "single", item, key: "id" in item ? item.id : `divider-${i}` });
  });
  return blocks;
}
