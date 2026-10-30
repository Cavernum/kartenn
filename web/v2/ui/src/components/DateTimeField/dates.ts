// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { currentLocale, fill, t, type MessageKey } from "../../i18n.ts";

/**
 * Dates of `DateTimeField` and `PeriodPicker` (design « Inventaire », line 7): an entry
 * by segments in the order of ISO 8601 (`YYYY-MM-DD hh:mm`), its check (impossible dates,
 * hours of a day when the clocks go forward in a named zone), and its value: a calendar
 * date `YYYY-MM-DD` (ISO 8601) or an instant of RFC 3339 in UTC (`…Z`), as the API takes
 * it (006 « Lectures datées »). The wall clock of a zone is read by `Intl` alone: no
 * library of dates.
 */

/** A day only (`YYYY-MM-DD`), or a day and a time to the minute (an instant). */
export type DateGranularity = "day" | "minute";

export type SegmentKind = "year" | "month" | "day" | "hour" | "minute";

/** The digits typed in each segment, `""` where none. */
export type Entry = Record<SegmentKind, string>;

/** A wall clock: the date and time a zone shows (month 1 to 12). */
export interface Wall {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
}

export const SEGMENTS: Record<DateGranularity, SegmentKind[]> = {
  day: ["year", "month", "day"],
  minute: ["year", "month", "day", "hour", "minute"],
};

/** The bounds and width of each segment (the day's bound is that of its month). */
export const LIMITS: Record<SegmentKind, { min: number; max: number; digits: number }> = {
  year: { min: 1, max: 9999, digits: 4 },
  month: { min: 1, max: 12, digits: 2 },
  day: { min: 1, max: 31, digits: 2 },
  hour: { min: 0, max: 23, digits: 2 },
  minute: { min: 0, max: 59, digits: 2 },
};

const MISSING: Record<SegmentKind, MessageKey> = {
  year: "dateMissingYear",
  month: "dateMissingMonth",
  day: "dateMissingDay",
  hour: "dateMissingHour",
  minute: "dateMissingMinute",
};

const MINUTE = 60_000;
const DAY = 86_400_000;

export const emptyEntry = (): Entry => ({ year: "", month: "", day: "", hour: "", minute: "" });

const pad = (value: number, digits: number) => String(value).padStart(digits, "0");

/** The milliseconds of a wall clock read as UTC (any year from 1, unlike `Date.UTC`). */
export function utcOf(wall: Wall): number {
  const date = new Date(0);
  date.setUTCFullYear(wall.year, wall.month - 1, wall.day);
  date.setUTCHours(wall.hour, wall.minute, 0, 0);
  return date.getTime();
}

/** The wall clock of milliseconds read as UTC. */
function wallOfUtc(ms: number): Wall {
  const date = new Date(ms);
  return {
    year: date.getUTCFullYear(),
    month: date.getUTCMonth() + 1,
    day: date.getUTCDate(),
    hour: date.getUTCHours(),
    minute: date.getUTCMinutes(),
  };
}

/** The days of a month (month 1 to 12). */
export function daysIn(year: number, month: number): number {
  return wallOfUtc(utcOf({ year, month: month + 1, day: 0, hour: 0, minute: 0 })).day;
}

/** The day of the week, Monday 0 to Sunday 6 (ISO 8601). */
export function weekday(year: number, month: number, day: number): number {
  return (new Date(utcOf({ year, month, day, hour: 0, minute: 0 })).getUTCDay() + 6) % 7;
}

/** A calendar date moved by days, months or years (the day kept within its month). */
export function shiftDate(
  date: { year: number; month: number; day: number },
  by: { days?: number; months?: number; years?: number },
): { year: number; month: number; day: number } {
  if (by.days) {
    const moved = wallOfUtc(utcOf({ ...date, hour: 0, minute: 0 }) + by.days * DAY);
    return { year: moved.year, month: moved.month, day: moved.day };
  }
  const months = date.year * 12 + date.month - 1 + (by.months ?? 0) + (by.years ?? 0) * 12;
  const year = Math.floor(months / 12);
  const month = (months % 12) + 1;
  return { year, month, day: Math.min(date.day, daysIn(year, month)) };
}

const zoneFormats = new Map<string, Intl.DateTimeFormat>();

/** The wall clock a named zone shows at an instant. */
export function wallAt(ms: number, timeZone: string): Wall {
  let format = zoneFormats.get(timeZone);
  if (!format) {
    format = new Intl.DateTimeFormat("en-US", {
      timeZone,
      hourCycle: "h23",
      year: "numeric",
      month: "numeric",
      day: "numeric",
      hour: "numeric",
      minute: "numeric",
    });
    zoneFormats.set(timeZone, format);
  }
  const parts = Object.fromEntries(format.formatToParts(ms).map((p) => [p.type, p.value]));
  return {
    year: Number(parts.year),
    month: Number(parts.month),
    day: Number(parts.day),
    hour: Number(parts.hour),
    minute: Number(parts.minute),
  };
}

/** The offset of a zone at an instant, in minutes east of UTC. */
export function offsetAt(ms: number, timeZone: string): number {
  const floored = Math.floor(ms / MINUTE) * MINUTE;
  return Math.round((utcOf(wallAt(floored, timeZone)) - floored) / MINUTE);
}

/** `UTC+01:00`, `UTC−03:30`, `UTC`: the offset of a zone at an instant. */
export function offsetLabel(ms: number, timeZone: string): string {
  const offset = offsetAt(ms, timeZone);
  if (offset === 0) return "UTC";
  const sign = offset > 0 ? "+" : "−";
  const abs = Math.abs(offset);
  return `UTC${sign}${pad(Math.floor(abs / 60), 2)}:${pad(abs % 60, 2)}`;
}

const sameWall = (a: Wall, b: Wall) =>
  a.year === b.year &&
  a.month === b.month &&
  a.day === b.day &&
  a.hour === b.hour &&
  a.minute === b.minute;

/**
 * The instants a wall clock names in a zone, earliest first: none in the hour skipped
 * when the clocks go forward, two in the hour repeated when they go back.
 */
export function instantsOf(wall: Wall, timeZone: string): number[] {
  const local = utcOf(wall);
  const candidates = new Set(
    [local - DAY, local, local + DAY].map((at) => local - offsetAt(at, timeZone) * MINUTE),
  );
  return [...candidates].filter((ms) => sameWall(wallAt(ms, timeZone), wall)).sort((a, b) => a - b);
}

/** An instant of RFC 3339 in UTC to the second, without fraction: `2026-01-15T09:00:00Z`. */
export function instantText(ms: number): string {
  return new Date(ms).toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** The wall clock of a zone as an entry. */
export function entryOfWall(wall: Wall): Entry {
  return {
    year: pad(wall.year, 4),
    month: pad(wall.month, 2),
    day: pad(wall.day, 2),
    hour: pad(wall.hour, 2),
    minute: pad(wall.minute, 2),
  };
}

const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
const INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:\d{2})$/i;

/**
 * The entry of a value: a date `YYYY-MM-DD`, or an instant of RFC 3339 (any offset) shown
 * in the zone; nothing for a value that is neither.
 */
export function entryOf(
  value: string | null | undefined,
  granularity: DateGranularity,
  timeZone: string,
): Entry | null {
  if (!value) return null;
  if (granularity === "day") {
    const match = DATE.exec(value);
    return match ? { ...emptyEntry(), year: match[1]!, month: match[2]!, day: match[3]! } : null;
  }
  if (!INSTANT.test(value)) return null;
  const ms = Date.parse(value);
  return Number.isNaN(ms) ? null : entryOfWall(wallAt(ms, timeZone));
}

/**
 * The entry of a text in the order of ISO 8601 (`2026-02-31`, `2026-02-31 10:30`,
 * `2026-02-31T10:30`), as typed: digits kept even when impossible, so that the check
 * can say why; one separator between two segments (a segment left out stays empty:
 * `2026--15` has no month).
 */
export function parseEntry(text: string): Entry {
  const entry = emptyEntry();
  const groups = text
    .trim()
    .replace(/\s+/g, " ")
    .split(/[^0-9]/);
  const kinds: SegmentKind[] = ["year", "month", "day", "hour", "minute"];
  kinds.forEach((kind, i) => {
    const digits = groups[i];
    if (digits) entry[kind] = digits.slice(0, LIMITS[kind].digits);
  });
  return entry;
}

export const isEmpty = (entry: Entry, granularity: DateGranularity) =>
  SEGMENTS[granularity].every((kind) => entry[kind] === "");

/** The name of a month in the language (« février »). */
export function monthName(month: number): string {
  return new Intl.DateTimeFormat(currentLocale(), { month: "long", timeZone: "UTC" }).format(
    utcOf({ year: 2000, month, day: 1, hour: 0, minute: 0 }),
  );
}

/** A day in words in the language (« 29 mars 2026 »). */
export function dayText(year: number, month: number, day: number): string {
  return new Intl.DateTimeFormat(currentLocale(), {
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(utcOf({ year, month, day, hour: 0, minute: 0 }));
}

export type DateCheck =
  { ok: true; value: string | null } | { ok: false; reason: string; segment: SegmentKind };

/**
 * The check of an entry: empty (no value), incomplete, a segment out of its bounds, a day
 * its month does not have (31 février), a time the zone skips that day; otherwise its
 * value — the date, or the instant in UTC (the earliest of two when the clocks go back).
 */
export function checkEntry(
  entry: Entry,
  granularity: DateGranularity,
  timeZone: string,
): DateCheck {
  const kinds = SEGMENTS[granularity];
  if (isEmpty(entry, granularity)) return { ok: true, value: null };
  const missing = kinds.find((kind) => entry[kind] === "");
  if (missing) {
    return {
      ok: false,
      reason: fill("dateIncomplete", { part: t(MISSING[missing]) }),
      segment: missing,
    };
  }
  const n = Object.fromEntries(kinds.map((kind) => [kind, Number(entry[kind])])) as Record<
    SegmentKind,
    number
  >;
  if (n.year < LIMITS.year.min) return { ok: false, reason: t("dateYearRange"), segment: "year" };
  if (n.month < 1 || n.month > 12) {
    return { ok: false, reason: t("dateMonthRange"), segment: "month" };
  }
  const days = daysIn(n.year, n.month);
  if (n.day < 1) {
    return { ok: false, reason: fill("dateDayRange", { days }), segment: "day" };
  }
  if (n.day > days) {
    const month = monthName(n.month);
    return {
      ok: false,
      reason: fill("dateDayMissing", { day: n.day, month, year: n.year, days }),
      segment: "day",
    };
  }
  if (granularity === "day") {
    return { ok: true, value: `${pad(n.year, 4)}-${pad(n.month, 2)}-${pad(n.day, 2)}` };
  }
  if (n.hour > 23) return { ok: false, reason: t("dateHourRange"), segment: "hour" };
  if (n.minute > 59) return { ok: false, reason: t("dateMinuteRange"), segment: "minute" };
  const wall: Wall = { year: n.year, month: n.month, day: n.day, hour: n.hour, minute: n.minute };
  const [first] = instantsOf(wall, timeZone);
  if (first === undefined) {
    return {
      ok: false,
      reason: fill("dateGap", {
        time: `${pad(n.hour, 2)}:${pad(n.minute, 2)}`,
        date: dayText(n.year, n.month, n.day),
        zone: timeZone,
      }),
      segment: "hour",
    };
  }
  return { ok: true, value: instantText(first) };
}

/**
 * The instant of a wall clock in a zone; in the hour skipped, the clock read with the
 * offset before the change (02:30 that does not exist is 03:30).
 */
function instantOrAfter(wall: Wall, timeZone: string): number {
  const local = utcOf(wall);
  return instantsOf(wall, timeZone)[0] ?? local - offsetAt(local - DAY, timeZone) * MINUTE;
}

/** The dated context of `PeriodPicker`: an instant of RFC 3339 per axis, `null` for the present. */
export interface Period {
  validAt: string | null;
  asOf: string | null;
}

/** The shortcuts of `PeriodPicker`, as instants in UTC, from `now` in a zone. */
export function shortcut(
  kind: "now" | "yesterday" | "monthStart",
  now: number,
  timeZone: string,
): string {
  const floored = Math.floor(now / MINUTE) * MINUTE;
  if (kind === "now") return instantText(floored);
  const wall = wallAt(floored, timeZone);
  if (kind === "monthStart") {
    return instantText(instantOrAfter({ ...wall, day: 1, hour: 0, minute: 0 }, timeZone));
  }
  const day = shiftDate(wall, { days: -1 });
  return instantText(instantOrAfter({ ...wall, ...day }, timeZone));
}
