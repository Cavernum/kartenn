// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { ariaKeyshortcuts } from "../Button/shortcut.ts";

/**
 * A shortcut, as `KeyboardEvent.key` names the keys (`Mod`: the modifier of the
 * platform): one combination, keys pressed together (`["Mod", "k"]`), or a sequence of
 * combinations pressed one after the other (`[["g"], ["p"]]`: `g` then `p`, spec 021,
 * « Raccourcis »).
 */
export type Shortcut = string[] | string[][];

/** The steps of a shortcut: one combination per step, one step for a combination. */
export function shortcutSteps(shortcut: Shortcut): string[][] {
  return shortcut.every((step) => Array.isArray(step))
    ? (shortcut as string[][])
    : [shortcut as string[]];
}

/**
 * The value of `aria-keyshortcuts` for a shortcut, or nothing for a sequence: WAI-ARIA 1.2
 * has no syntax for one (a space separates two shortcuts), and a sequence read as one
 * combination would be false. A sequence is said in words (`ShortcutHint`) instead.
 */
export function shortcutAria(shortcut: Shortcut): string | undefined {
  const steps = shortcutSteps(shortcut);
  return steps.length === 1 && steps[0]!.length > 0 ? ariaKeyshortcuts(steps[0]!) : undefined;
}
