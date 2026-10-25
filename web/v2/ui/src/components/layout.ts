// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** A step of the spacing scale (tokens `--k-space-*`), for the gaps of the layouts. */
export type Space = "0" | "1" | "2" | "3" | "4" | "6" | "8";

/** Alignment along the cross axis of a layout. */
export type Align = "start" | "center" | "end" | "stretch" | "baseline";

/** The three sizes of the controls (REQ-020-17). */
export type ControlSize = "sm" | "md" | "lg";

/**
 * The width classes of the frame (spec 020, « Adaptation à la largeur »): the window from
 * `min` CSS pixels (inclusive) up to the next class. Thresholds chosen by kartenn; a media
 * query cannot read a custom property, so the sheets of the frame copy them, equal by test.
 */
export const WIDTH_CLASSES = [
  { id: "compact", min: 0 },
  { id: "narrow", min: 600 },
  { id: "standard", min: 1024 },
  { id: "wide", min: 1280 },
] as const;

/** A width class of the frame (`data-width` on its root). */
export type WidthClass = (typeof WIDTH_CLASSES)[number]["id"];

/** The class of a window `width` CSS pixels wide. */
export function widthClass(width: number): WidthClass {
  let found: WidthClass = WIDTH_CLASSES[0].id;
  for (const c of WIDTH_CLASSES) if (width >= c.min) found = c.id;
  return found;
}

/** The media query of a class, in range syntax (Media Queries 4, § 2.4.3). */
export function widthQuery(id: WidthClass): string {
  const i = WIDTH_CLASSES.findIndex((c) => c.id === id);
  const from = WIDTH_CLASSES[i]!.min;
  const next = WIDTH_CLASSES[i + 1];
  if (!next) return `(width >= ${from}px)`;
  return from === 0 ? `(width < ${next.min}px)` : `(${from}px <= width < ${next.min}px)`;
}

/**
 * The widths of a component's container that its sheet queries (`@container`), in CSS
 * pixels: under `xs` a toolbar keeps its main action alone, under `sm` a simple list
 * stacks, under `md` and `lg` a grid hides its columns of priority 2 and 3. Copied in the
 * sheets, equal by test.
 */
export const CONTAINER_WIDTHS = { xs: 360, sm: 480, md: 640, lg: 960 } as const;
