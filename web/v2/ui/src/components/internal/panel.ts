// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What `SidePanel` and `BottomPanel` share (spec 020, « Adaptation à la largeur »): an empty
// slot told from a filled one, and the controls that own Escape before the panel does.

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
