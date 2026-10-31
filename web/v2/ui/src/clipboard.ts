// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** Outcome of a copy, announced by the component that asked for it. */
export type CopyOutcome = "copied" | "failed";

/**
 * Copies `text` to the clipboard (REQ-020-22). The asynchronous Clipboard API needs a
 * secure context and the focus of the document; without it, or refused, the copy command
 * of the document takes over, its `copy` event given the text (no selection made, no focus
 * moved). A refusal of both is an outcome, never a throw: the component then selects the
 * text and says how to copy it by hand (`selectText`). Without the API, the command runs
 * inside the user's activation (before any `await`); after a refusal of the API, it runs
 * a microtask later, which Chromium accepts (proven by the scripts) and other engines may
 * refuse: the refusal path then holds.
 */
export async function copyText(text: string): Promise<CopyOutcome> {
  try {
    if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) return byCommand(text);
    await navigator.clipboard.writeText(text);
    return "copied";
  } catch {
    return byCommand(text);
  }
}

/** The copy command of the document, its `copy` event filled with `text`. */
function byCommand(text: string): CopyOutcome {
  if (typeof document === "undefined") return "failed";
  let filled = false;
  const fill = (event: ClipboardEvent) => {
    if (!event.clipboardData) return;
    event.clipboardData.setData("text/plain", text);
    event.preventDefault();
    filled = true;
  };
  document.addEventListener("copy", fill, { capture: true });
  try {
    // Deprecated, and the only copy left outside a secure context.
    return document.execCommand("copy") && filled ? "copied" : "failed";
  } catch {
    return "failed";
  } finally {
    document.removeEventListener("copy", fill, { capture: true });
  }
}

/**
 * Selects the text of `node`, so that the keys of the system copy it when no copy could
 * (a refused clipboard).
 */
export function selectText(node: Node | null | undefined): void {
  const selection = node ? window.getSelection() : null;
  if (!node || !selection) return;
  const range = document.createRange();
  range.selectNodeContents(node);
  selection.removeAllRanges();
  selection.addRange(range);
}
