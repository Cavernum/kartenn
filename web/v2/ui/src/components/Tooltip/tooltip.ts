// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { onBeforeUnmount, ref, shallowRef, useId, watch, type Ref, type ShallowRef } from "vue";

/** Delay before a tooltip opens under the pointer; the keyboard focus opens it at once. */
export const TOOLTIP_DELAY = 300;
/** Grace left to the pointer to reach the tooltip from its anchor (WCAG 1.4.13, hoverable). */
const GRACE = 100;

/** Options of the listeners of an anchor. */
export interface TooltipListenerOptions {
  /**
   * Whether a click (a tap, Enter, Space) pins the tooltip open (`pin`), as on a refused
   * control that says why; otherwise the activation closes it. By default, `enabled`.
   */
  pin?: () => boolean;
  /**
   * Whether Escape, under the anchor's focus, also goes on to the focused control and
   * its layers once it has closed the tooltip: for a tooltip that only repeats the
   * anchor's name. By default, no.
   */
  passEscape?: () => boolean;
}

/**
 * The state of one tooltip (APG Tooltip; design « Inventaire », line 15): opened by the
 * pointer after a delay and by the keyboard focus at once (`:focus-visible`: a click
 * focuses without opening it), kept open while the pointer is over the anchor or the
 * tooltip itself, closed by blur, by leaving, or by Escape (WCAG 1.4.13). Opened by its
 * anchor's listeners, Escape closes it wherever the focus is, the layer around it staying
 * (a second Escape closes it; see `onEscape` for a name tooltip). An anchor never shows it while its
 * popup is open (`aria-expanded="true"`). The anchor carries `aria-describedby` to `id`
 * for as long as it has a tooltip: the bubble stays in the document, hidden, when closed;
 * a decorative bubble (one that repeats the anchor's name) may be mounted only when open.
 */
export interface TooltipControl {
  id: string;
  open: Ref<boolean>;
  anchor: ShallowRef<HTMLElement | null>;
  /** Opens the tooltip on `el`, at once or after the pointer delay. */
  show(el: HTMLElement, delayed?: boolean): void;
  /** Closes it, at once or after the grace of the pointer. */
  hide(soon?: boolean): void;
  /** Keeps it open (the pointer entered the bubble). */
  hold(): void;
  /**
   * Opens it on `el` at once, until the next press outside `el` and the bubble: a tap,
   * where no hover comes and not every engine moves the focus (design « Adaptation à la
   * largeur », « Tactile » : nothing reserved to hovering).
   */
  pin(el: HTMLElement): void;
  /** Listeners for the anchor; `enabled` says whether it has a tooltip now. */
  listeners(
    enabled: () => boolean,
    options?: TooltipListenerOptions,
  ): Record<string, (event: Event) => void>;
}

export function useTooltip(): TooltipControl {
  const id = `k-tooltip-${useId()}`;
  const open = ref(false);
  const anchor = shallowRef<HTMLElement | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  const clear = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  let unpin: (() => void) | undefined;
  const release = () => {
    unpin?.();
    unpin = undefined;
  };
  /** A touch pointer leaves its target when the finger lifts: not a reason to close. */
  const isTouch = (event: Event) => (event as PointerEvent).pointerType === "touch";
  /** An anchor whose popup (a menu) is open: its tooltip would cover it. */
  const expanded = (el: HTMLElement) => el.getAttribute("aria-expanded") === "true";

  /**
   * Escape, while a tooltip its anchor's listeners opened is open, wherever the focus is
   * (WCAG 1.4.13, dismissible without moving the pointer or the focus); heard first
   * (window, capture). Under the pointer alone (the focus elsewhere), it closes the
   * tooltip and nothing else: kept from the focused control and the layers around it.
   * Under the anchor's focus, it closes the tooltip and the layers stay (default
   * prevented, Reka's layers honour it), unless `passEscape`: a tooltip that only repeats
   * the anchor's name leaves Escape to the focused control (its dialog, its grid).
   */
  let owned = false;
  /** `passEscape` of the anchor's listeners, once they are made. */
  const escape = { passes: () => false };
  const onEscape = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || !open.value) return;
    const focused = anchor.value !== null && anchor.value === document.activeElement;
    if (!focused) event.stopPropagation();
    if (!focused || !escape.passes()) event.preventDefault();
    control.hide();
  };
  const own = (yes: boolean) => {
    if (yes === owned) return;
    owned = yes;
    if (yes) window.addEventListener("keydown", onEscape, { capture: true });
    else window.removeEventListener("keydown", onEscape, { capture: true });
  };
  watch(open, (now) => {
    if (!now) own(false);
  });

  const control: TooltipControl = {
    id,
    open,
    anchor,
    show(el, delayed = false) {
      clear();
      anchor.value = el;
      if (delayed && !open.value) timer = setTimeout(() => (open.value = true), TOOLTIP_DELAY);
      else open.value = true;
    },
    hide(soon = false) {
      // The pointer leaves (`soon`) an anchor that keeps the keyboard focus: the tooltip
      // stays until the focus leaves or Escape (WCAG 1.4.13, persistent).
      const kept = anchor.value;
      if (soon && kept && kept === document.activeElement && kept.matches(":focus-visible"))
        return clear();
      clear();
      release();
      if (soon) timer = setTimeout(() => (open.value = false), GRACE);
      else open.value = false;
    },
    hold: clear,
    pin(el) {
      control.show(el);
      release();
      const away = (event: PointerEvent) => {
        const target = event.target as Node | null;
        const bubble = document.getElementById(id);
        if (!target || !(el.contains(target) || bubble?.contains(target))) control.hide();
      };
      document.addEventListener("pointerdown", away, { capture: true });
      unpin = () => document.removeEventListener("pointerdown", away, { capture: true });
    },
    listeners: (enabled, options = {}) => {
      const pins = options.pin ?? enabled;
      if (options.passEscape) escape.passes = options.passEscape;
      const opening = (el: HTMLElement, delayed: boolean) => {
        if (!enabled() || expanded(el)) return;
        own(true);
        control.show(el, delayed);
      };
      return {
        pointerenter: (event) => {
          if (!isTouch(event)) opening(event.currentTarget as HTMLElement, true);
        },
        pointerleave: (event) => {
          if (!isTouch(event)) control.hide(true);
        },
        // A tap (or Enter, Space) on a refused control says why, without hover nor focus;
        // any other activation closes the tooltip (a menu opening under it, an action).
        click: (event) => {
          const el = event.currentTarget as HTMLElement;
          if (pins() && !expanded(el)) {
            own(true);
            control.pin(el);
          } else control.hide();
        },
        focus: (event) => {
          const el = event.currentTarget as HTMLElement;
          if (el.matches(":focus-visible")) opening(el, false);
        },
        blur: () => control.hide(),
      };
    },
  };
  onBeforeUnmount(() => {
    clear();
    release();
    own(false);
  });
  return control;
}
