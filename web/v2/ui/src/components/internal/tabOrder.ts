// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

const TABBABLE =
  "a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), " +
  "textarea:not(:disabled), [tabindex]:not([tabindex='-1'])";

/**
 * The elements of `root` that Tab reaches, in document order (shown, not `tabindex=-1`),
 * leaving out the guards Reka puts at the ends of the page while a layer is open.
 */
export function tabbables(root: ParentNode): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(TABBABLE)].filter(
    (el) =>
      el.tabIndex >= 0 &&
      el.getClientRects().length > 0 &&
      !el.closest("[inert]") &&
      !el.hasAttribute("data-reka-focus-guard"),
  );
}

/**
 * Where Tab goes from `anchor`: the dialog it is in (a modal `Dialog`, whose focus trap
 * takes Tab round inside it, the rest of the page hidden; a popover, whose scope loops
 * too), else the whole document.
 */
function tabScope(anchor: HTMLElement): { root: ParentNode; wraps: boolean } {
  const dialog = anchor.closest("[role='dialog'], [role='alertdialog']");
  return dialog ? { root: dialog, wraps: true } : { root: document, wraps: false };
}

/**
 * The element Tab (`step` 1) or Shift+Tab (`step` -1) reaches from `anchor` as if the
 * focus were on it, leaving out what is inside `skip` (a layer in the portal, which the
 * focus is leaving). `anchor` itself may be out of the tab order (a widget of a grid
 * cell): its place in the document is what counts. Inside a dialog, it stays there and
 * goes round, as the dialog's trap would; with nothing beside it in the page, `undefined`.
 */
export function besideInTabOrder(
  anchor: HTMLElement,
  step: 1 | -1,
  skip?: Element | null,
): HTMLElement | undefined {
  const { root, wraps } = tabScope(anchor);
  const all = tabbables(root).filter((el) => el !== anchor && !skip?.contains(el));
  const after = all.findIndex(
    (el) => anchor.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING,
  );
  const next = after === -1 ? all.length : after;
  const at = step === 1 ? next : next - 1;
  if (!wraps) return all[at];
  return all.length > 0 ? all[(at + all.length) % all.length] : undefined;
}

/**
 * Tab past the end, Shift+Tab from the start of a non-modal layer (`layer`, in the portal)
 * opened from `trigger`: where the focus goes once the layer closes, as if it followed its
 * trigger (Shift+Tab: back to the trigger; Tab: what follows it), or `undefined` while the
 * key stays inside. Heard in capture, before the loop of Reka's focus scope.
 */
export function tabOut(
  event: KeyboardEvent,
  layer: HTMLElement,
  trigger: HTMLElement,
): HTMLElement | undefined {
  if (event.key !== "Tab" || event.ctrlKey || event.altKey || event.metaKey) return undefined;
  const inside = tabbables(layer);
  const at = document.activeElement;
  const first = at === layer || at === inside[0];
  const last = inside.length === 0 || at === inside.at(-1);
  if (event.shiftKey ? !first : !last) return undefined;
  return event.shiftKey ? trigger : (besideInTabOrder(trigger, 1, layer) ?? trigger);
}
