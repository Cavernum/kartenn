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
 * Whether Escape belongs to the focused control: a field being typed in (it may cancel its
 * own input), or a popup it opened (a combobox, a menu button).
 */
export function ownsEscape(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const popup = target.getAttribute("role") === "combobox" || target.hasAttribute("aria-haspopup");
  if (target.isContentEditable || (popup && target.getAttribute("aria-expanded") === "true"))
    return true;
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
