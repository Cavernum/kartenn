// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { inject, provide, shallowReactive, type InjectionKey } from "vue";
import type { AlertTone } from "../InlineAlert/alert.ts";

/**
 * How long an ordinary toast stays, in milliseconds, its time held while the pointer is
 * over the toasts or the focus in them (WCAG 2.2.1). An error, a warning and a toast with
 * an action stay until dismissed; a timed toast never holds the only path to a piece of
 * information (the outcome shows where it happened too: a cell, a row).
 */
export const TOAST_DURATION = 8000;

/** A toast to show (design « Inventaire », line 14): a short outcome, never a question. */
export interface ToastInput {
  /** `ko`: an error, said at once (`alert`) and kept until dismissed; else said politely. */
  tone?: AlertTone;
  title: string;
  description?: string | undefined;
  /** The code of the outcome (a problem's `type`, a rule), copyable. */
  code?: string | undefined;
  /** The `Request-Id` of the failed request, copyable. */
  requestId?: string | undefined;
  /** One action (« Réessayer », « Voir le changeset »): run, then the toast is dismissed. */
  action?: { label: string; run: () => void } | undefined;
  /**
   * Milliseconds before an `info` or `ok` toast without action goes: `TOAST_DURATION`;
   * `Infinity` keeps it until dismissed.
   */
  duration?: number | undefined;
}

/** A toast shown: its id, its tone and its duration settled. */
export interface ToastEntry extends ToastInput {
  id: string;
  tone: AlertTone;
  duration: number;
}

/** The toasts of an application, given to its `ToastRegion` (`provideToasts`). */
export interface Toasts {
  /** In the order they came. */
  readonly list: readonly ToastEntry[];
  /** Shows a toast and returns its id. Never moves the focus. */
  show(input: ToastInput): string;
  dismiss(id: string): void;
}

/** Ids unique in the page, whatever the number of stores. */
let next = 0;

/** A store of toasts; the host creates one, provides it and renders one `ToastRegion`. */
export function createToasts(): Toasts {
  const list = shallowReactive<ToastEntry[]>([]);
  return {
    list,
    show(input) {
      const tone = input.tone ?? "info";
      const id = `k-toast-${++next}`;
      // An error, a warning or a toast with an action stays until dismissed: the time to
      // reach it by Tab is not ours to bound (WCAG 2.2.1).
      const kept = tone === "ko" || tone === "warn" || input.action !== undefined;
      const duration = kept ? Infinity : (input.duration ?? TOAST_DURATION);
      list.push({ ...input, id, tone, duration });
      return id;
    },
    dismiss(id) {
      const at = list.findIndex((toast) => toast.id === id);
      if (at !== -1) list.splice(at, 1);
    },
  };
}

const TOASTS: InjectionKey<Toasts> = Symbol("k-toasts");

/** Gives the toasts to the tree below: its `ToastRegion` and whoever shows one. */
export function provideToasts(toasts: Toasts): void {
  provide(TOASTS, toasts);
}

/** The toasts the host provides, or `null` without one. */
export function injectToasts(): Toasts | null {
  return inject(TOASTS, null);
}
