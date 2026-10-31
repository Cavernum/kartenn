// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { MessageKey } from "../../i18n.ts";
import type { IconName } from "../../icons/index.ts";

/** The four tones of a message, inline or toast (design « Inventaire », lines 14, 26). */
export type AlertTone = "info" | "ok" | "warn" | "ko";

/** The icon of each tone. */
export const TONE_ICONS: Record<AlertTone, IconName> = {
  info: "info",
  ok: "circle-check",
  warn: "triangle-alert",
  ko: "circle-x",
};

/**
 * The tone in words too (WCAG 1.3.1), hidden before the title: the icon is decorative, the
 * colour is not read; a warning and an information would otherwise sound the same.
 */
export const TONE_WORDS: Record<AlertTone, MessageKey> = {
  info: "toneInfo",
  ok: "toneOk",
  warn: "toneWarn",
  ko: "toneKo",
};
