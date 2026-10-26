// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** The width of the panel when the host gives none, in CSS pixels (`--k-size-panel-lg`). */
export const PANEL_STANDARD = 420;

/** A width chosen in one click from the menu of the panel (WCAG 2.5.7). */
export interface WidthPreset {
  id: "narrow" | "standard" | "wide";
  width: number;
}

/**
 * The widths of the menu « Largeur du panneau », the single-pointer way to resize the panel
 * besides dragging its handle (WCAG 2.5.7): the lower bound, the standard width, the upper
 * bound, each within the bounds.
 */
export function widthPresets(min: number, max: number, standard = PANEL_STANDARD): WidthPreset[] {
  return [
    { id: "narrow", width: min },
    { id: "standard", width: Math.max(min, Math.min(max, standard)) },
    { id: "wide", width: max },
  ];
}
