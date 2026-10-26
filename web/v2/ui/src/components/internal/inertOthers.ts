// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** The attribute of the element a sheet stops at (the frame; a demo frame of the catalogue). */
export const MODAL_SCOPE = "data-k-modal-scope";

/**
 * Makes `inert` what a layer covers entirely (WCAG 2.4.3, 2.4.11: no focus in hidden
 * content). `"siblings"`: the other children of its parent (a panel over the whole surface);
 * `"scope"`: every element beside the path from the layer up to the nearest `[MODAL_SCOPE]`
 * ancestor, or `body` (a sheet over the window). An element that holds one of `keep` (the
 * target of floating layers, whose menus and tooltips the layer opens) stays live, and so
 * does one already inert, which the returned function leaves as it was when it restores the
 * others.
 */
export function inertOthers(
  layer: HTMLElement,
  reach: "siblings" | "scope",
  keep: (Element | null)[] = [],
): () => void {
  const changed: HTMLElement[] = [];
  const kept = keep.filter((el): el is Element => el !== null);
  let node: HTMLElement = layer;
  while (node.parentElement) {
    const parent = node.parentElement;
    for (const child of Array.from(parent.children)) {
      if (child === node || !(child instanceof HTMLElement) || child.inert) continue;
      if (kept.some((el) => child.contains(el))) continue;
      child.inert = true;
      changed.push(child);
    }
    if (reach === "siblings" || parent === document.body || parent.hasAttribute(MODAL_SCOPE)) break;
    node = parent;
  }
  return () => {
    for (const el of changed) el.inert = false;
  };
}
