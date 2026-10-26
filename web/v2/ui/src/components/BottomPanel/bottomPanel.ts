// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** The height of the panel when the host gives none, in CSS pixels (`--k-size-panel-md`). */
export const PANEL_HEIGHT_STANDARD = 320;

/** A height chosen in one click from the menu of the panel (WCAG 2.5.7). */
export interface HeightPreset {
  id: "low" | "standard" | "high";
  height: number;
}

/**
 * The heights of the menu « Hauteur du panneau », the single-pointer way to resize the panel
 * besides dragging its handle (WCAG 2.5.7): the lower bound, the standard height, the upper
 * bound, each within the bounds.
 */
export function heightPresets(
  min: number,
  max: number,
  standard = PANEL_HEIGHT_STANDARD,
): HeightPreset[] {
  return [
    { id: "low", height: min },
    { id: "standard", height: Math.max(min, Math.min(max, standard)) },
    { id: "high", height: max },
  ];
}
