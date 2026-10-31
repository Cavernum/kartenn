import type { Page } from "@playwright/test";
import { expect, expectAccessible, loginIn, test } from "../fixtures/journeys.ts";

// The dated context in the frame (spec 021, task 9; REQ-021-5, 007/W9): carried by the URL
// and by every internal navigation, shown by the date pill and the band, applied from the
// period picker once the API reads at it, back to the present from the band. Ported from
// web/e2e/journeys/time.spec.ts; the writes switched off by the band come with the screens
// that write (tasks 12, 14).

const VALID_AT = "2026-01-01T00:00:00Z";
const AS_OF = "2026-06-30T12:00:00Z";
const DATED = `valid_at=${encodeURIComponent(VALID_AT)}&as_of=${encodeURIComponent(AS_OF)}`;
/** The `<time>` of an instant, as the band and the pill write it (`toISOString`). */
const at = (instant: string) => `time[datetime="${new Date(instant).toISOString()}"]`;

const pill = (page: Page) => page.getByRole("button", { name: /^Date de lecture/ });
/** The sentence of the band (its `status` region); its button « Revenir au présent » follows. */
const band = (page: Page) =>
  page.getByRole("status").filter({ hasText: /Lecture au|État connu au/ });
const layer = (page: Page) => page.getByRole("dialog", { name: "Lire à une date" });

test("a dated link shows the same dates to whoever opens it", async ({ page, login, browser }) => {
  await login("bob");
  await page.goto(`/prefixes?${DATED}`);
  await expect(band(page).locator(at(VALID_AT))).toBeVisible();
  await expect(band(page).locator(at(AS_OF))).toBeVisible();
  await expect(band(page)).toContainText("lecture seule");
  await expect(pill(page).locator(at(AS_OF))).toBeVisible();
  await expectAccessible(page);

  // Internal navigation keeps the dates.
  await page
    .getByRole("navigation", { name: "Navigation principale" })
    .getByRole("link", { name: "Adresses" })
    .click();
  await expect(page.getByRole("heading", { level: 1, name: "Adresses" })).toBeVisible();
  const link = new URL(page.url());
  expect(link.pathname).toBe("/addresses");
  expect(link.searchParams.get("valid_at")).toBe(VALID_AT);
  expect(link.searchParams.get("as_of")).toBe(AS_OF);

  // A second browser context, another session: the same dates from the link alone.
  const other = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  try {
    const second = await other.newPage();
    await loginIn(second, "alice");
    await second.goto(link.href);
    await expect(band(second).locator(at(VALID_AT))).toBeVisible();
    await expect(band(second).locator(at(AS_OF))).toBeVisible();
  } finally {
    await other.close();
  }

  // Back to the present: the dates leave the URL, the focus goes to the date pill.
  await page.getByRole("button", { name: "Revenir au présent" }).click();
  await expect(page).toHaveURL(/\/addresses$/);
  await expect(page.getByRole("button", { name: "Revenir au présent" })).toHaveCount(0);
  await expect(pill(page)).toBeFocused();
  await expect(pill(page)).toHaveAccessibleName(/^Date de lecture : Présent/);
});

test("a validity date alone leaves the dated draft possible", async ({ page, login }) => {
  await login("bob");
  await page.goto(`/prefixes?valid_at=${encodeURIComponent(VALID_AT)}`);
  await expect(band(page)).toContainText("brouillon daté possible");
  await expect(band(page)).not.toContainText("lecture seule");
  await expect(band(page)).not.toContainText("désactivé");
});

test("the date pill applies a period to the URL; Back returns to the present", async ({
  page,
  login,
}) => {
  await login("bob");
  await page.goto("/prefixes");
  await pill(page).click();
  await expect(layer(page)).toBeVisible();
  await page
    .getByRole("group", { name: /^Raccourcis\s:\sValide au$/ })
    .getByRole("button", { name: "Hier" })
    .click();
  await layer(page).getByRole("button", { name: "Appliquer" }).click();
  await expect(layer(page)).toHaveCount(0);
  await expect(page).toHaveURL(/\/prefixes\?valid_at=/);
  await expect(band(page)).toContainText("brouillon daté possible");
  // A move in the history goes to the URL as it was: the dates are not carried.
  await page.goBack();
  await expect(page).toHaveURL(/\/prefixes$/);
  await expect(pill(page)).toHaveAccessibleName(/^Date de lecture : Présent/);
  await page.goForward();
  await expect(page).toHaveURL(/\/prefixes\?valid_at=/);
});

test("a date the API refuses comes back on its field, the URL unchanged", async ({
  page,
  login,
}) => {
  await login("bob");
  // The browser's clock a day ahead of the server's: « now » for it is in kartenn's future.
  await page.clock.install({ time: new Date(Date.now() + 86_400_000) });
  await page.goto("/prefixes");
  await pill(page).click();
  await page
    .getByRole("group", { name: /^Raccourcis\s:\sConnu au$/ })
    .getByRole("button", { name: "Maintenant" })
    .click();
  await layer(page).getByRole("button", { name: "Appliquer" }).click();
  const field = page.getByRole("group", { name: "Connu au", exact: true });
  await expect(field).toHaveAccessibleDescription(/postérieure à l'heure de kartenn/);
  await expect(layer(page)).toBeVisible();
  await expect(page).toHaveURL(/\/prefixes$/);
});
