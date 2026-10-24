// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * The value of `aria-keyshortcuts` (WAI-ARIA 1.2) for a combination named as
 * `KeyboardEvent.key` names keys: `Mod` (the platform's modifier) becomes `Control`, a
 * space `Space`, the keys are joined by `+`.
 */
export function ariaKeyshortcuts(keys: readonly string[]): string {
  return keys.map((key) => (key === "Mod" ? "Control" : key === " " ? "Space" : key)).join("+");
}
