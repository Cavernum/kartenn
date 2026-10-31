import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of PeriodPicker (spec 020, design « Inventaire » line 7; spec 021,
// « Cadre »; REQ-007-22, 23): `valid_at` and `as_of`, each a DateTimeField in the named
// zone, empty for the present; the shortcuts under each, said; Enter in a segment sends
// the form; an impossible date and an `as_of` later than now are refused with their reason
// on their field, the focus on it; away from the present, « Revenir au présent » clears
// both. The period applied is written as the query of the URL. Keys only, from the year
// of « Valide au ».

/** A field by its label (not the group of its shortcuts, « Raccourcis : <label> »). */
const field = (page: Page, name: string) => page.getByRole("group", { name, exact: true });
const segment = (page: Page, label: string, name: string) =>
  field(page, label).getByRole("spinbutton", { name, exact: true });
const query = (page: Page) => page.locator("[data-query]");

/** The wall clock now in Paris, as the shortcuts read it. */
function parisNow(): Record<"year" | "month" | "day" | "hour" | "minute", string> {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: "Europe/Paris",
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).formatToParts(Date.now());
  const get = (type: string) => parts.find((p) => p.type === type)!.value;
  return {
    year: get("year"),
    month: get("month"),
    day: get("day"),
    hour: get("hour"),
    minute: get("minute"),
  };
}

test.describe("PeriodPicker", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "PeriodPicker", "keyboard");
  });

  test("a form named by its title; two fields described; their shortcuts in order", async ({
    page,
  }) => {
    await expect(page.getByRole("form", { name: "Lire à une date" })).toBeVisible();
    await expect(field(page, "Valide au")).toHaveAccessibleDescription(
      "Ce qui est vrai à cette date, passée ou à venir ; vide : maintenant.",
    );
    await expect(field(page, "Connu au")).toContainText("Europe/Paris");
    await expect(page.getByRole("button", { name: "Revenir au présent" })).toHaveCount(0);
    await segment(page, "Valide au", "Année").focus();
    // Four segments more, the button of the calendar, then the shortcuts.
    for (let i = 0; i < 5; i++) await page.keyboard.press("Tab");
    await expect(page.locator(".k-date-field__open").first()).toBeFocused();
    await page.keyboard.press("Tab");
    const shortcuts = page.getByRole("group", { name: /^Raccourcis\s:\sValide au$/ });
    await expect(shortcuts.getByRole("button", { name: "Maintenant" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(shortcuts.getByRole("button", { name: "Hier" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(shortcuts.getByRole("button", { name: "Début du mois" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(segment(page, "Connu au", "Année")).toBeFocused();
  });

  test("typed by keys, Enter in a segment applies; the way back to the present comes", async ({
    page,
  }) => {
    await segment(page, "Valide au", "Année").focus();
    await page.keyboard.type("202511030930");
    await page.keyboard.press("Enter");
    await expect(query(page)).toHaveText("valid_at=2025-11-03T08:30:00Z");
    const back = page.getByRole("button", { name: "Revenir au présent" });
    await expect(back).toBeVisible();
    // Back to the present by keys: from the minutes of « Valide au », past its calendar,
    // its shortcuts and the whole of « Connu au » (five segments, calendar, shortcuts).
    await expect(segment(page, "Valide au", "Minutes")).toBeFocused();
    for (let i = 0; i < 14; i++) await page.keyboard.press("Tab");
    await expect(back).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(query(page)).toHaveText("(présent)");
    await expect(segment(page, "Valide au", "Année")).toHaveAttribute("aria-valuetext", "vide");
  });

  test("« Début du mois » fills the field at midnight, said; then applied", async ({ page }) => {
    await segment(page, "Connu au", "Année").focus();
    for (let i = 0; i < 8; i++) await page.keyboard.press("Tab");
    const start = page
      .getByRole("group", { name: /^Raccourcis\s:\sConnu au$/ })
      .getByRole("button", { name: "Début du mois" });
    await expect(start).toBeFocused();
    await page.keyboard.press("Enter");
    const now = parisNow();
    await expect(segment(page, "Connu au", "Année")).toHaveText(now.year);
    await expect(segment(page, "Connu au", "Mois")).toHaveText(now.month);
    await expect(segment(page, "Connu au", "Jour")).toHaveText("01");
    await expect(segment(page, "Connu au", "Heure")).toHaveText("00");
    await expect(segment(page, "Connu au", "Minutes")).toHaveText("00");
    await expect(page.getByRole("status").filter({ hasText: "Connu au" })).toHaveText(
      /^Connu au\s:\s1 .+ 00:00\.$/,
    );
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Appliquer" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(query(page)).toHaveText(/^as_of=\d{4}-\d{2}-\d{2}T\d{2}:00:00Z$/);
  });

  test("« Maintenant » and « Hier »: now to the minute, the same time a day before", async ({
    page,
  }) => {
    await segment(page, "Valide au", "Année").focus();
    for (let i = 0; i < 7; i++) await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Hier" }).first()).toBeFocused();
    await page.keyboard.press("Enter");
    const now = parisNow();
    await expect(segment(page, "Valide au", "Heure")).toHaveText(now.hour);
    await expect(segment(page, "Valide au", "Jour")).not.toHaveText(now.day);
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Enter");
    await expect(segment(page, "Valide au", "Jour")).toHaveText(parisNow().day);
  });

  test("an impossible date is refused on sending: its reason, the focus on its day", async ({
    page,
  }) => {
    await segment(page, "Valide au", "Année").focus();
    await page.keyboard.type("202502291200");
    await page.keyboard.press("Enter");
    await expect(segment(page, "Valide au", "Jour")).toBeFocused();
    await expect(field(page, "Valide au")).toHaveAccessibleDescription(
      /^Le 29 février 2025 n'existe pas : février 2025 a 28 jours\./,
    );
    await expect(query(page)).toHaveText("(présent)");
  });

  test("an as_of later than now is refused: the base does not know the future", async ({
    page,
  }) => {
    await segment(page, "Connu au", "Année").focus();
    await page.keyboard.type("209901010000");
    await page.keyboard.press("Enter");
    const asOf = field(page, "Connu au");
    await expect(asOf).toHaveAccessibleDescription(/^La base ne connaît pas l'avenir/);
    await expect(segment(page, "Connu au", "Année")).toBeFocused();
    await expect(query(page)).toHaveText("(présent)");
    // Changed, the refusal goes.
    await page.keyboard.type("2025");
    await expect(asOf).toHaveAccessibleDescription(/^Ce que la base savait/);
  });
});
