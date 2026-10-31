import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of DateTimeField (spec 020, design « Inventaire » line 7,
// « Accessibilité »; REQ-020-16, 18, 28): segments in the order of ISO 8601, each an APG
// Spinbutton (digits typed fill it and go on, a separator too; Up and Down step and wrap;
// Home and End; Left and Right between segments; Backspace erases), a named zone and its
// offset; an impossible entry kept and refused when the field is left, its reason linked
// and said; the month grid in a modal dialog (APG Date Picker Dialog): arrows, Home, End,
// Page Up and Down (with Shift, a year), Enter chooses, Escape closes, the focus back on
// the button. Keys only from the field before it.

const segment = (page: Page, field: string, name: string) =>
  page.getByRole("group", { name: field }).getByRole("spinbutton", { name, exact: true });
const value = (page: Page, which: "instant" | "day") => page.locator(`[data-value='${which}']`);
/** The button of the calendar of « Valide au » (by its class: open, the page is hidden). */
const button = (page: Page) => page.locator(".k-date-field__open").first();
const dialog = (page: Page) => page.getByRole("dialog");
const focusedDay = (page: Page) => page.locator(".k-date-calendar__day:focus");
/** The month grid, named by the month shown (its heading, said when it changes). */
const grid = (page: Page, name: string) => dialog(page).getByRole("grid", { name });

/** From the field before it, Tab to the year of « Valide au ». */
async function enter(page: Page): Promise<void> {
  await page.getByRole("textbox", { name: "Note" }).focus();
  await page.keyboard.press("Tab");
  await expect(segment(page, "Valide au", "Année")).toBeFocused();
}

test.describe("DateTimeField (Spinbutton segments, Date Picker Dialog)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "DateTimeField", "keyboard");
  });

  test("a named group of five segments, the zone named; Tab from segment to segment", async ({
    page,
  }) => {
    const group = page.getByRole("group", { name: "Valide au" });
    await expect(group.getByRole("spinbutton")).toHaveCount(5);
    await expect(group).toHaveAccessibleDescription("Date et heure de Paris.");
    await expect(group).toContainText("Europe/Paris");
    await expect(segment(page, "Valide au", "Heure")).toHaveAccessibleDescription(
      /^Europe\/Paris UTC\+0[12]:00$/,
    );
    await enter(page);
    for (const name of ["Mois", "Jour", "Heure", "Minutes"]) {
      await page.keyboard.press("Tab");
      await expect(segment(page, "Valide au", name)).toBeFocused();
    }
    await page.keyboard.press("Tab");
    await expect(
      page.getByRole("button", { name: /^Choisir la date\s:\sValide au$/ }),
    ).toBeFocused();
  });

  test("digits fill each segment and go on; the value is an instant in UTC", async ({ page }) => {
    await enter(page);
    await page.keyboard.type("202511030930");
    await expect(segment(page, "Valide au", "Minutes")).toBeFocused();
    await expect(segment(page, "Valide au", "Mois")).toHaveAttribute(
      "aria-valuetext",
      "11, novembre",
    );
    // 09:30 in Paris on 3 November 2025 is 08:30 UTC (UTC+01:00).
    await expect(value(page, "instant")).toHaveText("2025-11-03T08:30:00Z");
    await expect(page.getByRole("group", { name: "Valide au" })).toContainText("UTC+01:00");
  });

  test("separators and single digits: « 2026-1-5 9:07 », the summer offset", async ({ page }) => {
    await enter(page);
    await page.keyboard.type("2026-1-5 9:07");
    await expect(value(page, "instant")).toHaveText("2026-01-05T08:07:00Z");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await expect(segment(page, "Valide au", "Mois")).toBeFocused();
    await page.keyboard.type("7");
    await expect(value(page, "instant")).toHaveText("2026-07-05T07:07:00Z");
    await expect(page.getByRole("group", { name: "Valide au" })).toContainText("UTC+02:00");
  });

  test("Up and Down step and wrap; Home and End go to the bounds; Left and Right move", async ({
    page,
  }) => {
    await enter(page);
    await page.keyboard.type("20260131");
    await page.keyboard.press("ArrowLeft");
    const month = segment(page, "Valide au", "Mois");
    const day = segment(page, "Valide au", "Jour");
    await page.keyboard.press("ArrowLeft");
    await expect(month).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(month).toHaveAttribute("aria-valuenow", "12");
    await page.keyboard.press("ArrowUp");
    await expect(month).toHaveAttribute("aria-valuenow", "1");
    await expect(month).toHaveAttribute("aria-valuetext", "01, janvier");
    await page.keyboard.press("ArrowRight");
    await expect(day).toBeFocused();
    await expect(day).toHaveAttribute("aria-valuemax", "31");
    await page.keyboard.press("Home");
    await expect(day).toHaveAttribute("aria-valuenow", "1");
    await page.keyboard.press("End");
    await expect(day).toHaveAttribute("aria-valuenow", "31");
    await page.keyboard.press("ArrowUp");
    await expect(day).toHaveAttribute("aria-valuenow", "1");
  });

  test("31 février is refused when the field is left: kept, linked, said, the day at fault", async ({
    page,
  }) => {
    await enter(page);
    await page.keyboard.type("202602311030");
    await expect(value(page, "instant")).toHaveText("—");
    // The button of the calendar is still the field; the next field is not.
    await page.keyboard.press("Tab");
    await expect(page.locator(".k-form-field__error")).toHaveCount(0);
    await page.keyboard.press("Tab");
    await expect(segment(page, "Créé le", "Année")).toBeFocused();
    const reason = "Le 31 février 2026 n'existe pas : février 2026 a 28 jours.";
    const group = page.getByRole("group", { name: "Valide au" });
    await expect(group).toHaveAccessibleDescription(`${reason} Date et heure de Paris.`);
    await expect(segment(page, "Valide au", "Jour")).toHaveAttribute("aria-invalid", "true");
    await expect(segment(page, "Valide au", "Jour")).toHaveText("31");
    await expect(
      page.locator(".k-form-field__live[aria-live='polite']").filter({ hasText: reason }),
    ).toHaveCount(1);
    // Mended, the refusal goes at once and the value is written.
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await expect(segment(page, "Valide au", "Jour")).toBeFocused();
    await page.keyboard.type("28");
    await expect(page.locator(".k-form-field__error")).toHaveCount(0);
    await expect(value(page, "instant")).toHaveText("2026-02-28T09:30:00Z");
  });

  test("25 h and the hour skipped in spring are refused with their reason", async ({ page }) => {
    await enter(page);
    await page.keyboard.type("202603122500");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    const group = page.getByRole("group", { name: "Valide au" });
    await expect(group).toHaveAccessibleDescription(/^L'heure va de 00 à 23\./);
    await expect(segment(page, "Valide au", "Heure")).toHaveText("25");
    // Back to the year by Shift+Tab, a whole new entry: 02:30 on 29 March 2026 in Paris.
    for (let i = 0; i < 6; i++) await page.keyboard.press("Shift+Tab");
    await expect(segment(page, "Valide au", "Année")).toBeFocused();
    await page.keyboard.type("202603290230");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(group).toHaveAccessibleDescription(
      /^02:30 n'existe pas le 29 mars 2026 en Europe\/Paris : l'heure avance ce jour-là\./,
    );
    await expect(value(page, "instant")).toHaveText("—");
  });

  test("a pasted instant keeps its offset, shown in the zone; a spaced date reads", async ({
    page,
    context,
  }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    const note = page.getByRole("textbox", { name: "Note" });
    await note.focus();
    // Copied from the field before by keys, pasted into the year by keys.
    await page.keyboard.type("2026-01-15T09:00:00+05:00");
    await page.keyboard.press("ControlOrMeta+A");
    await page.keyboard.press("ControlOrMeta+C");
    await page.keyboard.press("Tab");
    await page.keyboard.press("ControlOrMeta+V");
    // 09:00 at UTC+05:00 is 04:00 UTC, 05:00 in Paris.
    await expect(value(page, "instant")).toHaveText("2026-01-15T04:00:00Z");
    await expect(segment(page, "Valide au", "Heure")).toHaveText("05");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("ControlOrMeta+A");
    await page.keyboard.type("2025 - 11 - 03 09:30");
    await page.keyboard.press("ControlOrMeta+A");
    await page.keyboard.press("ControlOrMeta+C");
    await page.keyboard.press("Tab");
    await page.keyboard.press("ControlOrMeta+V");
    await expect(value(page, "instant")).toHaveText("2025-11-03T08:30:00Z");
  });

  test("an incomplete entry says what is missing; Backspace erases back to empty", async ({
    page,
  }) => {
    await enter(page);
    await page.keyboard.type("20260115");
    await expect(segment(page, "Valide au", "Heure")).toBeFocused();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(page.getByRole("group", { name: "Valide au" })).toHaveAccessibleDescription(
      /^Date incomplète : il manque l'heure\./,
    );
    // Back to the day, erased segment by segment: empty, the refusal goes.
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await expect(segment(page, "Valide au", "Jour")).toBeFocused();
    for (let i = 0; i < 4; i++) await page.keyboard.press("Backspace");
    await expect(segment(page, "Valide au", "Année")).toBeFocused();
    for (let i = 0; i < 4; i++) await page.keyboard.press("Backspace");
    await expect(segment(page, "Valide au", "Année")).toHaveAttribute("aria-valuetext", "vide");
    await page.keyboard.press("Shift+Tab");
    await expect(page.locator(".k-form-field__error")).toHaveCount(0);
    await expect(value(page, "instant")).toHaveText("—");
  });

  test("a day alone: three segments, 29 February of a leap year only", async ({ page }) => {
    const group = page.getByRole("group", { name: "Créé le" });
    await expect(group.getByRole("spinbutton")).toHaveCount(3);
    await enter(page);
    for (let i = 0; i < 6; i++) await page.keyboard.press("Tab");
    await expect(segment(page, "Créé le", "Année")).toBeFocused();
    await page.keyboard.type("20240229");
    await expect(value(page, "day")).toHaveText("2024-02-29");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.type("2025");
    await expect(value(page, "day")).toHaveText("—");
    // Out of the field, back to the button of the one before.
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Shift+Tab");
    await expect(button(page)).toBeFocused();
    await expect(group).toHaveAccessibleDescription(
      "Le 29 février 2025 n'existe pas : février 2025 a 28 jours.",
    );
  });
});

test.describe("DateTimeField, its month grid (APG Date Picker Dialog)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "DateTimeField", "keyboard");
    await enter(page);
    await page.keyboard.type("202601150930");
    await page.keyboard.press("Tab");
  });

  test("Enter opens a modal dialog named by its button, its grid by the month", async ({
    page,
  }) => {
    await expect(button(page)).toBeFocused();
    await expect(button(page)).toHaveAttribute("aria-haspopup", "dialog");
    await page.keyboard.press("Enter");
    await expect(
      page.getByRole("dialog", { name: /^Choisir la date\s:\sValide au$/ }),
    ).toBeVisible();
    await expect(dialog(page)).toHaveAttribute("aria-modal", "true");
    await expect(grid(page, "janvier 2026")).toBeVisible();
    await expect(focusedDay(page)).toHaveText("15");
    await expect(focusedDay(page)).toHaveAttribute("aria-selected", "true");
    await expect(button(page)).toHaveAttribute("aria-expanded", "true");
  });

  test("arrows by day and week, Home and End of the week, Enter chooses", async ({ page }) => {
    await page.keyboard.press("Enter");
    await expect(focusedDay(page)).toHaveText("15");
    await page.keyboard.press("ArrowRight");
    await expect(focusedDay(page)).toHaveText("16");
    await page.keyboard.press("ArrowDown");
    await expect(focusedDay(page)).toHaveText("23");
    await page.keyboard.press("ArrowLeft");
    await expect(focusedDay(page)).toHaveText("22");
    await page.keyboard.press("ArrowUp");
    await expect(focusedDay(page)).toHaveText("15");
    // 15 January 2026 is a Thursday: the week goes from Monday 12 to Sunday 18.
    await page.keyboard.press("Home");
    await expect(focusedDay(page)).toHaveText("12");
    await page.keyboard.press("End");
    await expect(focusedDay(page)).toHaveText("18");
    await page.keyboard.press("Enter");
    await expect(dialog(page)).toHaveCount(0);
    await expect(button(page)).toBeFocused();
    // The day chosen, the time kept.
    await expect(value(page, "instant")).toHaveText("2026-01-18T08:30:00Z");
  });

  test("Page Up and Down by month, with Shift by year; the heading follows", async ({ page }) => {
    await page.keyboard.press("Enter");
    await page.keyboard.press("PageDown");
    await expect(grid(page, "février 2026")).toBeVisible();
    await expect(focusedDay(page)).toHaveText("15");
    await page.keyboard.press("Shift+PageDown");
    await expect(grid(page, "février 2027")).toBeVisible();
    await page.keyboard.press("Shift+PageUp");
    await page.keyboard.press("PageUp");
    await page.keyboard.press("PageUp");
    await expect(grid(page, "décembre 2025")).toBeVisible();
    // Crossing the month by the arrows moves the grid too: Monday 15 December, Sunday 21,
    // the 28th, then 4 January.
    await page.keyboard.press("End");
    await expect(focusedDay(page)).toHaveText("21");
    await page.keyboard.press("ArrowDown");
    await expect(grid(page, "décembre 2025")).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await expect(grid(page, "janvier 2026")).toBeVisible();
    await expect(focusedDay(page)).toHaveText("4");
    await page.keyboard.press(" ");
    await expect(dialog(page)).toHaveCount(0);
    await expect(value(page, "instant")).toHaveText("2026-01-04T08:30:00Z");
  });

  test("Escape closes without choosing; the focus back on the button", async ({ page }) => {
    await page.keyboard.press("Enter");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Escape");
    await expect(dialog(page)).toHaveCount(0);
    await expect(button(page)).toBeFocused();
    await expect(value(page, "instant")).toHaveText("2026-01-15T08:30:00Z");
  });

  test("Tab stays in the dialog: the month buttons, the grid, « Annuler »", async ({ page }) => {
    await page.keyboard.press("Enter");
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Annuler" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Année précédente" })).toBeFocused();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Mois suivant" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(grid(page, "février 2026")).toBeVisible();
    await expect(page.getByRole("button", { name: "Mois suivant" })).toBeFocused();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(focusedDay(page)).toHaveText("15");
    await page.keyboard.press("Enter");
    await expect(value(page, "instant")).toHaveText("2026-02-15T08:30:00Z");
  });

  test("Alt+Down from a segment opens the grid too", async ({ page }) => {
    await page.keyboard.press("Shift+Tab");
    await expect(segment(page, "Valide au", "Minutes")).toBeFocused();
    await page.keyboard.press("Alt+ArrowDown");
    await expect(grid(page, "janvier 2026")).toBeVisible();
    await expect(focusedDay(page)).toHaveText("15");
    await page.keyboard.press("Escape");
    await expect(button(page)).toBeFocused();
  });
});
