// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { currentLocale } from "./i18n.ts";

/**
 * An instant in the language of the user and a named time zone (design « Inventaire »,
 * line 7): day, abbreviated month, year, hours and minutes on 24 hours. The zone is
 * always named by the caller, never the machine's by accident.
 */
export function formatInstant(at: Date | string, timeZone: string): string {
  return new Intl.DateTimeFormat(currentLocale(), {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
    timeZone,
  }).format(typeof at === "string" ? new Date(at) : at);
}
