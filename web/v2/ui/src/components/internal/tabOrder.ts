// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

const TABBABLE =
  "a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), " +
  "textarea:not(:disabled), [tabindex]:not([tabindex='-1'])";

/** The elements of `root` that Tab reaches, in document order (shown, not `tabindex=-1`). */
export function tabbables(root: ParentNode): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(TABBABLE)].filter(
    (el) => el.tabIndex >= 0 && el.getClientRects().length > 0 && !el.closest("[inert]"),
  );
}

/**
 * The element Tab (`step` 1) or Shift+Tab (`step` -1) reaches from `anchor` as if the
 * focus were on it, leaving out what is inside `skip` (a layer in the portal, which the
 * focus is leaving). `anchor` itself may be out of the tab order (a widget of a grid
 * cell): its place in the document is what counts.
 */
export function besideInTabOrder(
  anchor: HTMLElement,
  step: 1 | -1,
  skip?: Element | null,
): HTMLElement | undefined {
  const all = tabbables(document).filter((el) => el !== anchor && !skip?.contains(el));
  const after = all.findIndex(
    (el) => anchor.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING,
  );
  const next = after === -1 ? all.length : after;
  return all[step === 1 ? next : next - 1];
}
