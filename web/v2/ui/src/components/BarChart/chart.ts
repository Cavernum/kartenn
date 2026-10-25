// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** A point of a chart: a stable key, its label (a day, a group), its value. */
export interface ChartPoint {
  key: string;
  label: string;
  value: number;
}

/** The round steps a scale may end on, within one power of ten. */
const STEPS = [1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10];

/**
 * The top of the scale of a chart: the smallest round number not below `peak` (1, 1.2, 1.5,
 * 2, 2.5, 3, 4, 5, 6, 8 times a power of ten), so that its graduation reads well and the
 * highest bar nearly reaches it; 1 for an empty or null series.
 */
export function niceCeil(peak: number): number {
  if (!(peak > 0)) return 1;
  const magnitude = 10 ** Math.floor(Math.log10(peak));
  const step = STEPS.find((s) => s * magnitude >= peak - magnitude * 1e-9) ?? 10;
  return Number((step * magnitude).toPrecision(12));
}

/** One bar laid out in CSS pixels: its slot (hit area) and its drawn rectangle. */
export interface BarLayout {
  slotX: number;
  slotWidth: number;
  x: number;
  width: number;
  y: number;
  height: number;
}

/**
 * Lays `values` out as bars across `width` × `height` CSS pixels against `scale`: equal
 * slots in reading order, a gap of a fifth of a slot (one pixel at least) between bars,
 * edges on whole pixels so that the bars stay crisp.
 */
export function layoutBars(
  values: readonly number[],
  width: number,
  height: number,
  scale: number,
): BarLayout[] {
  if (!values.length || width <= 0 || height <= 0) return [];
  const slot = width / values.length;
  const gap = Math.max(1, Math.round(slot / 5));
  return values.map((value, i) => {
    const slotX = i * slot;
    const x = Math.round(slotX + gap / 2);
    const end = Math.round(slotX + slot - gap / 2);
    const h = value > 0 ? Math.max(1, Math.round((Math.min(value, scale) / scale) * height)) : 0;
    return { slotX, slotWidth: slot, x, width: Math.max(1, end - x), y: height - h, height: h };
  });
}
