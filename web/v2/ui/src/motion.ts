// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * The motion setting of the user (spec 020, « Mouvement »; WCAG technique SCR40): the
 * system's preference (`prefers-reduced-motion`), or reduced whatever the system says.
 * Reduced, the root carries `data-motion="reduce"`, under which the tokens are those of
 * the system's reduced motion: no movement, colour and opacity fades of 80 ms.
 */
export type MotionSetting = "system" | "reduce";

/** Applies the setting to the root of the document (or to `root`). */
export function applyMotion(setting: MotionSetting, root: HTMLElement = document.documentElement) {
  if (setting === "reduce") root.dataset.motion = "reduce";
  else delete root.dataset.motion;
}

/** The setting the root carries. */
export function currentMotion(root: HTMLElement = document.documentElement): MotionSetting {
  return root.dataset.motion === "reduce" ? "reduce" : "system";
}
