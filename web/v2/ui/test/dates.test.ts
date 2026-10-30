// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import { afterEach, describe, expect, it } from "vitest";
import {
  checkEntry,
  daysIn,
  entryOf,
  instantsOf,
  offsetLabel,
  parseEntry,
  shiftDate,
  shortcut,
  weekday,
} from "../src/components/DateTimeField/dates.ts";
import DateTimeField from "../src/components/DateTimeField/DateTimeField.vue";
import PeriodPicker from "../src/components/PeriodPicker/PeriodPicker.vue";
import { setLocale } from "../src/i18n.ts";

// Dates of DateTimeField and PeriodPicker (spec 020, task 20; design « Inventaire »,
// line 7): an entry in the order of ISO 8601, its check (impossible dates, the hour
// skipped by a named zone), its value (a date, an instant of RFC 3339 in UTC), the
// shortcuts, and both components as rendered.

afterEach(() => setLocale("fr"));

const PARIS = "Europe/Paris";
const check = (text: string, granularity: "day" | "minute" = "minute", zone = PARIS) =>
  checkEntry(parseEntry(text), granularity, zone);

describe("the calendar", () => {
  it("knows the days of each month, leap years included", () => {
    expect(daysIn(2026, 2)).toBe(28);
    expect(daysIn(2024, 2)).toBe(29);
    expect(daysIn(1900, 2)).toBe(28);
    expect(daysIn(2000, 2)).toBe(29);
    expect(daysIn(2026, 4)).toBe(30);
    expect(daysIn(2026, 12)).toBe(31);
  });

  it("counts the weeks from Monday (ISO 8601), years before 100 included", () => {
    expect(weekday(2026, 1, 15)).toBe(3); // a Thursday
    expect(weekday(2024, 1, 1)).toBe(0);
    expect(weekday(1, 1, 1)).toBe(0);
  });

  it("moves by days, months and years, the day kept within its month", () => {
    expect(shiftDate({ year: 2026, month: 1, day: 31 }, { days: 1 })).toEqual({
      year: 2026,
      month: 2,
      day: 1,
    });
    expect(shiftDate({ year: 2026, month: 1, day: 31 }, { months: 1 })).toEqual({
      year: 2026,
      month: 2,
      day: 28,
    });
    expect(shiftDate({ year: 2024, month: 2, day: 29 }, { years: 1 })).toEqual({
      year: 2025,
      month: 2,
      day: 28,
    });
    expect(shiftDate({ year: 2026, month: 1, day: 5 }, { months: -1 })).toEqual({
      year: 2025,
      month: 12,
      day: 5,
    });
  });
});

describe("an entry", () => {
  it("reads the order of ISO 8601, digits kept as typed", () => {
    expect(parseEntry("2026-02-31 10:30")).toEqual({
      year: "2026",
      month: "02",
      day: "31",
      hour: "10",
      minute: "30",
    });
    expect(parseEntry("2026-1-5T9:05")).toMatchObject({ month: "1", day: "5", hour: "9" });
    expect(parseEntry("2026-01")).toMatchObject({ year: "2026", month: "01", day: "" });
  });

  it("is empty, a date or an instant of RFC 3339 in UTC", () => {
    expect(check("")).toEqual({ ok: true, value: null });
    expect(check("2026-01-15", "day")).toEqual({ ok: true, value: "2026-01-15" });
    expect(check("2026-1-5", "day")).toEqual({ ok: true, value: "2026-01-05" });
    // Paris is at UTC+01:00 in winter, UTC+02:00 in summer.
    expect(check("2026-01-15 10:00")).toEqual({ ok: true, value: "2026-01-15T09:00:00Z" });
    expect(check("2025-07-14 08:00")).toEqual({ ok: true, value: "2025-07-14T06:00:00Z" });
    expect(check("2026-01-15 10:00", "minute", "UTC")).toEqual({
      ok: true,
      value: "2026-01-15T10:00:00Z",
    });
  });

  it("refuses an impossible day, its reason in words", () => {
    expect(check("2026-02-31 10:30")).toEqual({
      ok: false,
      reason: "Le 31 février 2026 n'existe pas : février 2026 a 28 jours.",
      segment: "day",
    });
    expect(check("2025-02-29", "day")).toMatchObject({ ok: false, segment: "day" });
    expect(check("2024-02-29", "day")).toEqual({ ok: true, value: "2024-02-29" });
    expect(check("2026-04-00", "day")).toMatchObject({
      reason: "Le jour va de 01 à 30.",
      segment: "day",
    });
    setLocale("en");
    expect(check("2026-02-31", "day")).toMatchObject({
      reason: "February 31, 2026 does not exist: February 2026 has 28 days.",
    });
  });

  it("refuses a month, an hour or a minute out of bounds, and a year 0", () => {
    expect(check("2026-13-01", "day")).toMatchObject({
      reason: "Le mois va de 01 à 12.",
      segment: "month",
    });
    expect(check("2026-03-12 25:00")).toMatchObject({
      reason: "L'heure va de 00 à 23.",
      segment: "hour",
    });
    expect(check("2026-03-12 10:60")).toMatchObject({
      reason: "Les minutes vont de 00 à 59.",
      segment: "minute",
    });
    expect(check("0000-01-01", "day")).toMatchObject({ segment: "year" });
  });

  it("says what is missing", () => {
    expect(check("2026-01-15")).toMatchObject({
      reason: "Date incomplète : il manque l'heure.",
      segment: "hour",
    });
    expect(check("2026--15", "day")).toMatchObject({ segment: "month" });
  });

  it("refuses the hour a zone skips, and takes the earlier of an hour repeated", () => {
    expect(check("2026-03-29 02:30")).toEqual({
      ok: false,
      reason: "02:30 n'existe pas le 29 mars 2026 en Europe/Paris : l'heure avance ce jour-là.",
      segment: "hour",
    });
    expect(check("2026-03-29 03:00")).toEqual({ ok: true, value: "2026-03-29T01:00:00Z" });
    // 25 October 2026, 02:30 comes twice in Paris (UTC+02:00, then UTC+01:00).
    expect(
      instantsOf({ year: 2026, month: 10, day: 25, hour: 2, minute: 30 }, PARIS).map((ms) =>
        new Date(ms).toISOString(),
      ),
    ).toEqual(["2026-10-25T00:30:00.000Z", "2026-10-25T01:30:00.000Z"]);
    expect(check("2026-10-25 02:30")).toEqual({ ok: true, value: "2026-10-25T00:30:00Z" });
  });

  it("reads back a value in its zone, any offset of RFC 3339", () => {
    expect(entryOf("2025-11-03T08:30:00Z", "minute", PARIS)).toMatchObject({
      year: "2025",
      month: "11",
      day: "03",
      hour: "09",
      minute: "30",
    });
    expect(entryOf("2025-11-03T09:30:00+01:00", "minute", "UTC")).toMatchObject({ hour: "08" });
    expect(entryOf("2026-01-15", "day", PARIS)).toMatchObject({ day: "15", hour: "" });
    expect(entryOf("hier", "minute", PARIS)).toBeNull();
    expect(entryOf("2026-01-15", "minute", PARIS)).toBeNull();
  });

  it("names the offset of the zone at the date", () => {
    expect(offsetLabel(Date.parse("2026-01-15T09:00:00Z"), PARIS)).toBe("UTC+01:00");
    expect(offsetLabel(Date.parse("2026-07-15T09:00:00Z"), PARIS)).toBe("UTC+02:00");
    expect(offsetLabel(Date.parse("2026-07-15T09:00:00Z"), "UTC")).toBe("UTC");
    expect(offsetLabel(Date.parse("2026-01-15T09:00:00Z"), "America/St_Johns")).toBe("UTC−03:30");
  });
});

describe("the shortcuts", () => {
  const now = Date.parse("2026-01-15T09:00:42Z"); // 10:00 in Paris

  it("now to the minute, yesterday at the same time, the first of the month at midnight", () => {
    expect(shortcut("now", now, PARIS)).toBe("2026-01-15T09:00:00Z");
    expect(shortcut("yesterday", now, PARIS)).toBe("2026-01-14T09:00:00Z");
    expect(shortcut("monthStart", now, PARIS)).toBe("2025-12-31T23:00:00Z");
    expect(shortcut("monthStart", now, "UTC")).toBe("2026-01-01T00:00:00Z");
  });

  it("across a change of the clocks, the wall clock of the zone", () => {
    // 30 March 2026, 10:00 in Paris (summer): yesterday 10:00 was still in summer time,
    // the first of March in winter time.
    const after = Date.parse("2026-03-30T08:00:00Z");
    expect(shortcut("yesterday", after, PARIS)).toBe("2026-03-29T08:00:00Z");
    expect(shortcut("monthStart", after, PARIS)).toBe("2026-02-28T23:00:00Z");
  });
});

describe("as rendered", () => {
  const render = (component: unknown, props: Record<string, unknown>) =>
    renderToString(createSSRApp({ render: () => h(component as never, props) }));

  it("DateTimeField: a group of spinbuttons named by the label, the zone beside", async () => {
    const html = await render(DateTimeField, {
      label: "Valide au",
      timeZone: PARIS,
      modelValue: "2025-11-03T08:30:00Z",
    });
    expect(html).toContain('role="group"');
    expect(html.match(/role="spinbutton"/g)).toHaveLength(5);
    expect(html).toContain('aria-label="Année"');
    expect(html).toContain('aria-valuetext="11, novembre"');
    expect(html).toContain("Europe/Paris");
    expect(html).toContain("UTC+01:00");
    expect(html).toMatch(/aria-label="Choisir la date\s:\sValide au"/);
  });

  it("DateTimeField: a day has three segments and no zone", async () => {
    const html = await render(DateTimeField, {
      label: "Créé le",
      granularity: "day",
      modelValue: "2026-01-15",
    });
    expect(html.match(/role="spinbutton"/g)).toHaveLength(3);
    expect(html).not.toContain("Europe/");
  });

  it("PeriodPicker: two fields, their shortcuts, the way back only away from the present", async () => {
    const present = await render(PeriodPicker, { timeZone: PARIS });
    expect(present).toContain("Valide au");
    expect(present).toContain("Connu au");
    expect(present.match(/aria-label="Raccourcis\s:\s/g)).toHaveLength(2);
    expect(present).not.toContain("Revenir au présent");
    const past = await render(PeriodPicker, {
      timeZone: PARIS,
      validAt: "2025-11-03T08:30:00Z",
    });
    expect(past).toContain("Revenir au présent");
  });
});
