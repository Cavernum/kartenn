// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// A grid that scrolls under the side panel (spec 020, « Réalisé (2026-10-26,
// relectures) »; WCAG 2.4.11). Over the end of the surface, `SidePanel` marks its container
// `data-k-side-panel`. When a cell of a grid in that container takes the focus, and the grid
// scrolls along the inline axis and reaches under the panel, the part of its scrollport the
// panel covers becomes its `--k-grid-panel-inset`: its `scroll-padding-inline-end` (the
// focused cell scrolls out from under the panel) and its `padding-inline-end` (the room to
// scroll its last columns out). A grid that ends before the panel, or does not scroll, keeps
// its layout: its tracks already at their least, the padding only lengthens its scroll.
// The panel clears these insets when it closes or changes width; the next focus measures
// again.

const INSET = "--k-grid-panel-inset";

/** An extent along the inline axis of the page, in CSS pixels. */
export interface Span {
  left: number;
  right: number;
}

/**
 * The part of the scrollport `port` the panel `panel` covers: the panel stands at the inline
 * end of their container (the right, or the left when written right to left) and runs to
 * that end, so what of the port it covers lies at the port's inline end.
 */
export function coveredInset(port: Span, panel: Span): number {
  const covered = Math.min(port.right, panel.right) - Math.max(port.left, panel.left);
  return Math.round(Math.max(0, covered));
}

/** The span of the scrollport of `el`: its box without borders nor scrollbar. */
function portOf(el: HTMLElement): Span {
  const box = el.getBoundingClientRect();
  const left = box.left + el.clientLeft;
  return { left, right: left + el.clientWidth };
}

/** The open side panel over the end of what holds `el`, unless `el` is inside it. */
function panelOver(el: HTMLElement): HTMLElement | null {
  const host = el.closest<HTMLElement>("[data-k-side-panel]");
  const panel = host?.querySelector<HTMLElement>(":scope > .k-side-panel");
  return panel && !panel.contains(el) ? panel : null;
}

/** The inset of `scroller` now, in CSS pixels. */
const applied = (scroller: HTMLElement): number =>
  Number.parseFloat(scroller.style.getPropertyValue(INSET)) || 0;

/**
 * On `focusin` in a grid whose scroller is `scroller`: its inset set to what the panel
 * covers of it if it scrolls along the inline axis, else none; when that changed, the
 * focused element scrolled again into the part left visible.
 */
export function fitPanelInset(scroller: HTMLElement, focused: EventTarget | null): void {
  const before = applied(scroller);
  const panel = panelOver(scroller);
  const scrolls = scroller.scrollWidth - before > scroller.clientWidth + 1;
  const inset =
    panel && scrolls ? coveredInset(portOf(scroller), panel.getBoundingClientRect()) : 0;
  if (inset === before) return;
  if (inset) scroller.style.setProperty(INSET, `${inset}px`);
  else scroller.style.removeProperty(INSET);
  if (focused instanceof HTMLElement)
    focused.scrollIntoView({ block: "nearest", inline: "nearest" });
}

/** The insets of the grids in `host` cleared: the panel closed or changed width. */
export function clearPanelInsets(host: HTMLElement): void {
  for (const el of host.querySelectorAll<HTMLElement>('[style*="--k-grid-panel-inset"]'))
    el.style.removeProperty(INSET);
}
