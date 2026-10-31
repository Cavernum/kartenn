// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** Keeps a layer off the viewport's edges, as its `collision-padding` does. */
const EDGE = 8;
/** Room left between the layer and the viewport's edge, beyond its offset from its anchor. */
const MARGIN = 12;
/** The least height a layer keeps, its content scrolling inside. */
const LEAST = 120;

/**
 * Bounds a layer anchored to `anchor` by the viewport at any zoom (WCAG 1.4.10), through
 * CSSOM (REQ-020-14): `--k-<name>-room-inline` (the viewport's width but its edges) and
 * `--k-<name>-room-block` (the taller side of the anchor), read by its `max-*-size`.
 */
export function setRoom(layer: HTMLElement, anchor: DOMRect, name: string): void {
  const block = Math.max(window.innerHeight - anchor.bottom, anchor.top) - MARGIN;
  layer.style.setProperty(`--k-${name}-room-inline`, `${window.innerWidth - 2 * EDGE}px`);
  layer.style.setProperty(`--k-${name}-room-block`, `${Math.max(block, LEAST)}px`);
}
