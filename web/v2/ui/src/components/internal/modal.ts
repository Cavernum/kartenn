// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { watch, type Ref } from "vue";

/**
 * Where the focus goes back when a modal layer closes (APG Dialog (Modal); WCAG 2.4.3): what
 * had it when the layer opened, also when the host opens it by its model, without a Reka
 * trigger. An entry of a menu (« Renommer… ») is gone by then: its menu's button instead
 * (Reka names the menu after it). Returns the handler of the layer's `close-auto-focus`.
 */
export function useOpener(open: Ref<boolean>): (event: Event) => void {
  let opener: HTMLElement | null = null;
  let fallback: HTMLElement | null = null;
  watch(open, (now) => {
    if (!now || typeof document === "undefined") return;
    const at = document.activeElement;
    opener = at instanceof HTMLElement && at !== document.body ? at : null;
    const menu = opener?.closest("[role='menu']");
    const button = menu?.getAttribute("aria-labelledby");
    fallback = button ? document.getElementById(button) : null;
  });
  return (event: Event) => {
    const target = opener?.isConnected ? opener : fallback?.isConnected ? fallback : null;
    opener = fallback = null;
    if (!target) return;
    event.preventDefault();
    target.focus();
  };
}

const FOCUSABLE = "button:not([disabled]), a[href], input:not([disabled]), select, textarea";
const FIELD = "input:not([disabled]), select:not([disabled]), textarea:not([disabled])";

/**
 * The `open-auto-focus` of a modal layer whose body holds a form: the focus goes to the
 * element the host marks `data-autofocus` (the safe action of a dialog whose close would
 * lose something: a secret shown once), else to the first field of `body` (a selector in
 * the layer), else where Reka puts it (its first control, the close button).
 */
export function focusFirstField(event: Event, body: string): void {
  const content = event.target as HTMLElement | null;
  const marked = content?.querySelector<HTMLElement>("[data-autofocus]");
  const field =
    (marked?.matches(FOCUSABLE) ? marked : marked?.querySelector<HTMLElement>(FOCUSABLE)) ??
    content?.querySelector<HTMLElement>(`${body} :is(${FIELD})`);
  if (!field) return;
  event.preventDefault();
  field.focus();
}
