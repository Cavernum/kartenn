// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** Outcome of a copy, announced by the component that asked for it. */
export type CopyOutcome = "copied" | "failed";

/**
 * Copies `text` to the clipboard (REQ-020-22). The asynchronous Clipboard API needs a
 * secure context and the focus of the document; a refusal is an outcome, never a throw.
 */
export async function copyText(text: string): Promise<CopyOutcome> {
  try {
    await navigator.clipboard.writeText(text);
    return "copied";
  } catch {
    return "failed";
  }
}
