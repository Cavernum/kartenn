// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { onBeforeUnmount, ref, watch, type Ref } from "vue";
import { copyText, selectText, type CopyOutcome } from "../../clipboard.ts";
import { useTooltip, type TooltipControl } from "../Tooltip/tooltip.ts";

/** How long « Copié » stays shown, in milliseconds. */
export const COPIED_MS = 2000;

/** A copy button and its bubble (`CodeBadge`, `KeyLabel`, `EnvChip`). */
export interface CopyControl {
  tip: TooltipControl;
  /** The outcome of the last copy, shown in the bubble and said by a `status` region. */
  outcome: Ref<CopyOutcome | null>;
  /** The listeners of the button: its bubble, and the copy on activation. */
  listeners: Record<string, (event: Event) => void>;
}

/**
 * The copy of a button (REQ-020-21, 22): a click, Enter or Space copy `text`, and the bubble
 * of the button (`Tooltip`, pinned) says the outcome: « Copié » for a while, or, the
 * clipboard refused, the text selected (`selection`) and how to copy it by hand, until
 * the bubble closes (blur, Escape, a press elsewhere). At rest, the bubble shows under the
 * pointer and the keyboard focus only while `atRest` (a cut code, a key behind its label);
 * Escape then passes on to the layer around when it only repeats the button (`passAtRest`).
 */
export function useCopy(options: {
  text: () => string;
  selection: () => Node | null | undefined;
  atRest: () => boolean;
  passAtRest?: () => boolean;
}): CopyControl {
  const tip = useTooltip();
  const outcome = ref<CopyOutcome | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const base = tip.listeners(() => options.atRest() || outcome.value !== null, {
    pin: () => true,
    passEscape: () => outcome.value === null && options.passAtRest?.() === true,
  });

  async function copy(): Promise<void> {
    clearTimeout(timer);
    outcome.value = null;
    const result = await copyText(options.text());
    if (result === "failed") selectText(options.selection());
    outcome.value = result;
    if (result === "copied")
      timer = setTimeout(() => {
        outcome.value = null;
        if (!options.atRest()) tip.hide();
      }, COPIED_MS);
  }

  // A refusal is said until its bubble closes.
  watch(tip.open, (open) => {
    if (!open && outcome.value === "failed") outcome.value = null;
  });
  onBeforeUnmount(() => clearTimeout(timer));

  return {
    tip,
    outcome,
    listeners: {
      ...base,
      click: (event) => {
        base.click!(event);
        void copy();
      },
    },
  };
}
