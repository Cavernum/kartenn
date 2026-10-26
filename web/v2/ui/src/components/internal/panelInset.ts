// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// A grid under the side panel (spec 020, « Adaptation à la largeur », « Réalisé
// (2026-10-26, relectures) »; WCAG 2.4.11). Over the end of the surface, `SidePanel` marks
// its container `data-k-side-panel` and attaches it here while it is open. Every grid in
// that container (its root marked `data-k-panel-fit`) then fits within the part of it the
// panel leaves visible: the part the panel covers, plus a gap, becomes its
// `--k-grid-panel-inset`, the inline-end padding of the frame of a `DataGrid` or a
// `TreeGrid` (their container: the columns of lower priority fold, the rest scrolls in
// line) or the inline-end margin of a `MatrixGrid` (it scrolls in line). Only the grid
// gives up room; nothing else on the page moves. Measured when the panel opens or changes
// width, when the container or a grid changes size, and when a grid mounts under an open
// panel; nothing is animated (design « Mouvement »: no layout animation). A grid that ends
// before the panel keeps its layout, and so does one the panel covers entirely (there is no
// visible part to fit in).

const INSET = "--k-grid-panel-inset";
const FIT = "[data-k-panel-fit]";
/** The covered part, in CSS pixels, a grid gives up now (none when unset). */
const COVERED = "kPanelCovered";

/** An extent along the inline axis of the page, in CSS pixels. */
export interface Span {
  left: number;
  right: number;
}

/**
 * The part of the box `box` the panel `panel` covers: the panel stands at the inline end of
 * their container (the right, or the left when written right to left) and runs to that end,
 * so what of the box it covers lies at the box's inline end. None when the panel covers the
 * whole box: it has no visible part to fit in.
 */
export function coveredInset(box: Span, panel: Span): number {
  const covered = Math.round(Math.min(box.right, panel.right) - Math.max(box.left, panel.left));
  return covered > 0 && covered < Math.round(box.right - box.left) ? covered : 0;
}

/**
 * The span of the open panel in `host`, from the layout (`offsetLeft`), not from its
 * rendered box: while it slides in, its translation would shift the measure.
 */
function panelSpan(host: HTMLElement, panel: HTMLElement): Span {
  const base = panel.offsetParent instanceof HTMLElement ? panel.offsetParent : host;
  const left = base.getBoundingClientRect().left + base.clientLeft + panel.offsetLeft;
  return { left, right: left + panel.offsetWidth };
}

/** The span of `el` as laid out without the room it gave up (a margin shrinks its box). */
function boxOf(el: HTMLElement): Span {
  const box = el.getBoundingClientRect();
  const margin =
    el.dataset.kPanelFit === "margin" && el.dataset[COVERED]
      ? Number.parseFloat(getComputedStyle(el).marginInlineEnd) || 0
      : 0;
  return getComputedStyle(el).direction === "rtl"
    ? { left: box.left - margin, right: box.right }
    : { left: box.left, right: box.right + margin };
}

/** The open panel over the end of `host`. */
const panelOf = (host: HTMLElement) => host.querySelector<HTMLElement>(":scope > .k-side-panel");

/** `el` fitted within what the panel of `host` leaves visible of it. */
function fit(el: HTMLElement, host: HTMLElement, panel: HTMLElement): void {
  if (panel.contains(el)) return;
  const covered = coveredInset(boxOf(el), panelSpan(host, panel));
  if (covered === Number(el.dataset[COVERED] ?? 0)) return;
  if (covered) {
    el.dataset[COVERED] = String(covered);
    // The gap keeps its border clear of the edge and the shadow of the panel.
    el.style.setProperty(INSET, `calc(${covered}px + var(--k-space-3))`);
  } else unfit(el);
}

function unfit(el: HTMLElement): void {
  delete el.dataset[COVERED];
  el.style.removeProperty(INSET);
}

function fitAll(host: HTMLElement): void {
  const panel = panelOf(host);
  if (panel) for (const el of host.querySelectorAll<HTMLElement>(FIT)) fit(el, host, panel);
}

const observers = new WeakMap<HTMLElement, ResizeObserver>();

/**
 * The grids in `host` fitted beside its open panel, and again as long as it stays open
 * when the container or a grid changes size; the returned function undoes it all (the
 * panel closed, went over the whole surface, or changes width).
 */
export function attachPanel(host: HTMLElement): () => void {
  fitAll(host);
  let frame = 0;
  // The next frame, not inside the callback: a margin given up changes the size of an
  // observed grid, and a change inside the callback would go undelivered.
  const observer = new ResizeObserver(() => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => fitAll(host));
  });
  observer.observe(host);
  for (const el of host.querySelectorAll<HTMLElement>(FIT)) observer.observe(el);
  observers.set(host, observer);
  return () => {
    cancelAnimationFrame(frame);
    observer.disconnect();
    observers.delete(host);
    for (const el of host.querySelectorAll<HTMLElement>(FIT)) unfit(el);
  };
}

/** A grid just mounted: fitted at once if a panel is open over its container, then followed. */
export function joinPanel(el: HTMLElement): void {
  const host = el.closest<HTMLElement>("[data-k-side-panel]");
  const observer = host && observers.get(host);
  if (!host || !observer) return;
  const panel = panelOf(host);
  if (panel) fit(el, host, panel);
  observer.observe(el);
}

/** A grid about to unmount: no longer followed. */
export function leavePanel(el: HTMLElement): void {
  const host = el.closest<HTMLElement>("[data-k-side-panel]");
  if (host) observers.get(host)?.unobserve(el);
}
