// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { t, type MessageKey } from "../../i18n.ts";

const GLYPHS: Record<string, string> = {
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Mod: "Ctrl",
  Control: "Ctrl",
};

/** The keys printed with a word, which changes with the language (Maj, Origine, Suppr). */
const NAMED: Record<string, MessageKey> = {
  Escape: "keyEscape",
  Enter: "keyEnter",
  " ": "keySpace",
  Space: "keySpace",
  Tab: "keyTab",
  Shift: "keyShift",
  Home: "keyHome",
  End: "keyEnd",
  PageUp: "keyPageUp",
  PageDown: "keyPageDown",
  Delete: "keyDelete",
  Backspace: "keyBackspace",
};

/**
 * The visible name of a key, as `KeyboardEvent.key` or `Mod` (the modifier of the
 * platform's shortcuts) names it: arrows as glyphs, named keys in the language, anything
 * else as is: a letter alone stays lower case (`e`, not `E`, which would read as
 * Shift+E), a letter with a modifier is in capitals as keyboards print it (Ctrl K).
 */
export function keyLabel(key: string, withModifier = false): string {
  if (key in GLYPHS) return GLYPHS[key]!;
  if (key in NAMED) return t(NAMED[key]!);
  return withModifier && key.length === 1 ? key.toUpperCase() : key;
}

const MODIFIERS = new Set(["Mod", "Control", "Alt", "Shift", "Meta"]);

/** Whether a combination holds a modifier (then its letter is shown in capitals: Ctrl K). */
export function hasModifier(keys: readonly string[]): boolean {
  return keys.some((key) => MODIFIERS.has(key));
}
