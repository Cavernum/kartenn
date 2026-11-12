// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What `SidePanel` and `BottomPanel` share (spec 020, « Adaptation à la largeur »): an empty
// slot told from a filled one, the controls that own Escape before the panel does, how
// they close (asked first when modified), and the scroll cue of their tab panels.

import {
  Comment,
  Fragment,
  Text,
  isVNode,
  nextTick,
  ref,
  watch,
  type Ref,
  type VNodeArrayChildren,
} from "vue";

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
 * (tasks 021-12p, 021-12qa). A control that takes Escape for itself prevents its default (a
 * cell editor that cancels, an error or a tooltip that closes), and the panel leaves a
 * prevented Escape alone; this answers for what does not: a popup its control opened and
 * still shows (a combobox's list, a menu button's menu: Reka closes them from the document,
 * after the panel heard the key) and a composition in progress (an IME, whose Escape
 * cancels it; under WebKit the composition ends before the keydown, which keeps its
 * `keyCode` 229). Anything else lets it go, as in a dialog: a closed combobox (its second
 * Escape, which its header promises to the layer around), a button, and a field typed in,
 * since a modified panel asks before dropping what it holds (Q-021-22, `usePanelClose`).
 */
export function ownsEscape(event: KeyboardEvent): boolean {
  // `keyCode` is deprecated, but the only trace of a composition WebKit has just ended.
  if (event.isComposing || event.keyCode === 229) return true;
  const target = event.target;
  if (!(target instanceof HTMLElement)) return false;
  // A disclosure (`aria-expanded` without a popup) shows its content in place: not a popup.
  const haspopup = target.getAttribute("aria-haspopup");
  const popup =
    target.getAttribute("role") === "combobox" || (haspopup !== null && haspopup !== "false");
  return popup && target.getAttribute("aria-expanded") === "true";
}

/** What a panel gives `usePanelClose`. */
export interface PanelClose {
  /** The rendered panel, while it is open. */
  panel: Ref<HTMLElement | null>;
  /** Its model. */
  open: Ref<boolean>;
  /** Whether a field was modified since it opened, as its host says. */
  modified: () => boolean;
  /** Says it closed (its `close` event). */
  closed: () => void;
  /** Where the focus goes when its opener is gone (the strip of a `SidePanel`). */
  fallback?: () => HTMLElement | null | undefined;
}

/**
 * How a panel closes (`SidePanel`, `BottomPanel`; tasks 021-12p, 021-12qa, Q-021-22).
 * Escape (that the focused control does not own), the cross, the back button and the host
 * through the panel's exposed `close()` `request` it: intact, the panel closes at once;
 * modified, it first asks « Abandonner les modifications ? » (`ask`, `asking`, the
 * panel's `PanelLeave`): « Continuer la saisie » or Escape leave it open, the focus back on
 * the field or the button it came from (the dialog's own return); « Abandonner » closes it.
 * `ask()` alone asks without closing (a host's guard: a navigation that would drop the
 * input), and says whether to drop it. Closed, the focus goes back to what had it when the
 * panel opened (WCAG 2.4.3; APG Alert Dialog), else to `fallback`, if it was in the panel
 * when the close was asked. The host's model closing it asks nothing and withdraws a
 * question in progress, the focus to the opener; `modified` turned false withdraws it too,
 * the focus back where the question came from.
 */
export function usePanelClose(o: PanelClose): {
  asking: Ref<boolean>;
  request: (event?: Event) => void;
  ask: () => Promise<boolean>;
  answer: (drop: boolean) => void;
  onKeydown: (event: KeyboardEvent) => void;
} {
  let opener: HTMLElement | null = null;
  const asking = ref(false);
  /** Whether the focus was in the panel when its close was asked. */
  let inside = false;
  /** The question in progress, and what settles it. */
  let pending: { promise: Promise<boolean>; settle: (drop: boolean) => void } | null = null;

  /** Asks « Abandonner les modifications ? » when modified: true to drop them. */
  function ask(): Promise<boolean> {
    if (!o.modified()) return Promise.resolve(true);
    if (!pending) {
      let settle: (drop: boolean) => void = () => {};
      const promise = new Promise<boolean>((resolve) => (settle = resolve));
      pending = { promise, settle };
      asking.value = true;
    }
    return pending.promise;
  }
  /** The answer (« Abandonner »: true), or the question withdrawn (false). */
  function answer(drop: boolean): void {
    const was = pending;
    pending = null;
    // Settled before the question closes: « Abandonner » makes the panel inert before the
    // dialog gives the focus back.
    was?.settle(drop);
    asking.value = false;
  }
  // `modified` turned false (the host dropped or saved the input): nothing left to ask.
  watch(
    () => o.modified(),
    (now) => {
      if (!now && pending) answer(false);
    },
  );

  /** Once the panel is gone, the focus to its opener, else to `fallback`, if it was in. */
  async function refocus(): Promise<void> {
    await nextTick();
    if (inside && opener && opener !== document.body && opener.isConnected) opener.focus();
    else if (inside) o.fallback?.()?.focus();
    opener = null;
    inside = false;
  }
  // The question gives the focus back to the field it came from once it is gone, which can
  // be after the panel started to leave: the leaving panel takes no focus, the opener
  // keeps it.
  const leave = (): void => o.panel.value?.setAttribute("inert", "");

  watch(
    o.open,
    (now) => {
      // Closed by the host's model while it asks: the question goes, unanswered, and the
      // focus it held goes to the opener as after « Abandonner ».
      if (!now && pending) {
        answer(false);
        leave();
        void refocus();
      }
      if (typeof document === "undefined") return;
      if (now) opener = document.activeElement as HTMLElement | null;
    },
    { immediate: true },
  );

  function close(): void {
    o.open.value = false;
    o.closed();
    void refocus();
  }
  /**
   * Escape, the cross, the back button. A click that left the focus where it was (Safari
   * and Firefox on macOS do not focus a clicked button) puts it on the button, which the
   * question gives it back to.
   */
  function request(event?: Event): void {
    const panel = o.panel.value;
    const target = event?.target instanceof Element ? event.target.closest("button") : null;
    if (target && panel?.contains(target) && !panel.contains(document.activeElement))
      target.focus();
    inside = panel?.contains(document.activeElement) ?? false;
    if (!o.modified()) {
      close();
      return;
    }
    void ask().then((drop) => {
      if (!drop || !o.open.value) return;
      leave();
      close();
    });
  }
  function onKeydown(event: KeyboardEvent): void {
    // A held Escape asks once: its repeats would answer the question as it opens.
    if (event.key !== "Escape" || event.repeat || event.defaultPrevented) return;
    if (ownsEscape(event)) return;
    event.preventDefault();
    request();
  }
  return { asking, request, ask, answer, onKeydown };
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
