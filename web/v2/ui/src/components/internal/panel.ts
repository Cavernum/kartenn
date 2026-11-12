// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What `SidePanel` and `BottomPanel` share (spec 020, « Adaptation à la largeur »): an empty
// slot told from a filled one, the controls that own Escape before the panel does, and the
// scroll cue of their tab panels.

import { Comment, Fragment, Text, isVNode, type VNodeArrayChildren } from "vue";

/** Whether rendered nodes hold anything but comments and blank text (an empty slot). */
export function filled(nodes: VNodeArrayChildren): boolean {
  return nodes.some((node) => {
    if (node === null || node === undefined || typeof node === "boolean") return false;
    if (typeof node === "string" || typeof node === "number") return String(node).trim() !== "";
    if (Array.isArray(node)) return filled(node);
    if (!isVNode(node) || node.type === Comment) return false;
    if (node.type === Text) return String(node.children ?? "").trim() !== "";
    if (node.type === Fragment) return Array.isArray(node.children) && filled(node.children);
    return true;
  });
}

/**
 * Whether an Escape the panel hears belongs to the focused control rather than to the panel
 * (task 021-12p). A control that takes Escape for itself prevents its default (a cell
 * editor that cancels, an error or a tooltip that closes), and the panel leaves a prevented
 * Escape alone; this answers for what does not: a popup its control opened and still shows
 * (a combobox's list, a menu button's menu: Reka closes them from the document, after the
 * panel heard the key), a composition in progress (an IME, whose Escape cancels it; under
 * WebKit the composition ends before the keydown, which keeps its `keyCode` 229), and a
 * field typed in (text field, text area, segment of a date): what it holds would go with
 * the panel, until the panel asks first (Q-021-22, task 021-12q). A closed combobox lets
 * it go (its second Escape, which its header promises to the layer around), as does
 * anything else, as in a dialog.
 */
export function ownsEscape(event: KeyboardEvent): boolean {
  // `keyCode` is deprecated, but the only trace of a composition WebKit has just ended.
  if (event.isComposing || event.keyCode === 229) return true;
  const target = event.target;
  if (!(target instanceof HTMLElement)) return false;
  // A disclosure (`aria-expanded` without a popup) shows its content in place: not a popup.
  const haspopup = target.getAttribute("aria-haspopup");
  const combobox = target.getAttribute("role") === "combobox";
  const popup = combobox || (haspopup !== null && haspopup !== "false");
  if (popup && target.getAttribute("aria-expanded") === "true") return true;
  if (combobox) return false;
  if (target.isContentEditable) return true;
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  return (
    target instanceof HTMLInputElement &&
    !["button", "checkbox", "radio", "submit", "reset", "range", "color", "file"].includes(
      target.type,
    )
  );
}

/**
 * The scroll cue of a panel (relecture d'E11 du 2026-10-27): the scroll regions of `root`
 * that `selector` names carry `data-more` while content lies below their bottom edge; their
 * style paints the shade above the content (encarts compris), never under it. Kept up to
 * date on scroll, on a change of size of a region or of its content, on a change of tab.
 * Returns what stops it.
 */
export function scrollCue(root: HTMLElement, selector: string): () => void {
  const regions = () => [...root.querySelectorAll<HTMLElement>(selector)];
  const update = (): void => {
    for (const el of regions()) {
      const more = el.scrollHeight - el.clientHeight - el.scrollTop > 1;
      if (more !== el.hasAttribute("data-more")) el.toggleAttribute("data-more", more);
    }
  };
  const resized = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(update);
  // The region and its children: a change of content changes the size of a child.
  const observe = (): void => {
    if (!resized) return;
    resized.disconnect();
    for (const el of regions()) {
      resized.observe(el);
      for (const child of el.children) resized.observe(child);
    }
  };
  let frame = 0;
  const changed = (): void => {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      observe();
      update();
    });
  };
  const mutated = typeof MutationObserver === "undefined" ? null : new MutationObserver(changed);
  mutated?.observe(root, { childList: true, subtree: true });
  root.addEventListener("scroll", update, { capture: true, passive: true });
  observe();
  update();
  return () => {
    if (frame) cancelAnimationFrame(frame);
    resized?.disconnect();
    mutated?.disconnect();
    root.removeEventListener("scroll", update, { capture: true });
    for (const el of regions()) el.removeAttribute("data-more");
  };
}
