// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { onBeforeUnmount, ref, shallowRef, useId, type Ref, type ShallowRef } from "vue";

/** Delay before a tooltip opens under the pointer; the keyboard focus opens it at once. */
export const TOOLTIP_DELAY = 300;
/** Grace left to the pointer to reach the tooltip from its anchor (WCAG 1.4.13, hoverable). */
const GRACE = 100;

/**
 * The state of one tooltip (APG Tooltip; design « Inventaire », line 15): opened by the
 * pointer after a delay and by the focus at once, kept open while the pointer is over the
 * anchor or the tooltip itself, closed by blur, by leaving, or by Escape (WCAG 1.4.13).
 * The anchor carries `aria-describedby` to `id` for as long as it has a tooltip: the
 * bubble stays in the document, hidden, when closed.
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
  listeners(enabled: () => boolean): Record<string, (event: Event) => void>;
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
    listeners: (enabled) => ({
      pointerenter: (event) => {
        if (enabled() && !isTouch(event)) control.show(event.currentTarget as HTMLElement, true);
      },
      pointerleave: (event) => {
        if (!isTouch(event)) control.hide(true);
      },
      // A tap (or Enter, Space) on a refused control says why, without hover nor focus.
      click: (event) => {
        if (enabled()) control.pin(event.currentTarget as HTMLElement);
      },
      focus: (event) => {
        if (enabled()) control.show(event.currentTarget as HTMLElement);
      },
      blur: () => control.hide(),
      keydown: (event) => {
        // Escape closes the tooltip only, not the layer around it (WCAG 1.4.13, dismissible).
        if ((event as KeyboardEvent).key === "Escape" && open.value) {
          event.preventDefault();
          control.hide();
        }
      },
    }),
  };
  onBeforeUnmount(() => {
    clear();
    release();
  });
  return control;
}
