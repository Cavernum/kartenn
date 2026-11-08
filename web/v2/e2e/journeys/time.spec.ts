import type { Page } from "@playwright/test";
import { atRest, DATA, expect, expectAccessible, loginIn, test } from "../fixtures/journeys.ts";

// The dated context in the frame (spec 021, task 9; REQ-021-5, 007/W9): carried by the URL
// and by every internal navigation, shown by the date pill and the band, applied from the
// period picker once the API reads at it, back to the present from the band. Ported from
// web/e2e/journeys/time.spec.ts. The writes (task 12c): under `as_of`, the editors and the
// actions of a record off, each saying why; under `valid_at` alone, its edit panel opens to
// the dated draft, « Appliquer » refused with its reason. The draft's own submission at
// that date is the journey `draft`'s (task 14e).

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

/** The record of the prefix of the data set: its path. */
async function prefixRecord(page: Page): Promise<string> {
  const id = await page.evaluate(
    async (url) => {
      const response = await fetch(url);
      return ((await response.json()) as { items: { id: string }[] }).items[0]!.id;
    },
    `/v1/prefixes?value=${encodeURIComponent(DATA.prefix)}`,
  );
  return `/prefixes/${id}`;
}
const main = (page: Page) => page.getByRole("main");
const modify = (page: Page) => main(page).getByRole("button", { name: "Modifier" });
const editPanel = (page: Page) =>
  page.getByRole("form", { name: `Modifier : Préfixe ${DATA.prefix}` });

test("under as_of, the editors and the actions of the record are off, each saying why", async ({
  page,
  login,
}) => {
  await login("alice");
  const record = await prefixRecord(page);
  await page.goto(record);
  await expect(modify(page)).not.toHaveAttribute("aria-disabled");

  // Known a moment ago: the prefix of the data set already there, nothing written.
  const asOf = new Date(Date.now() - 1_000).toISOString();
  // Each page left at rest, its plugins loaded (`atRest`: Firefox and the loads cut short).
  await atRest(page);
  await page.goto(`${record}?as_of=${encodeURIComponent(asOf)}`);
  await expect(band(page)).toContainText("lecture seule");
  await expect(
    page.getByRole("heading", { level: 1, name: `Préfixe ${DATA.prefix}` }),
  ).toBeAttached();
  await expect(modify(page)).toHaveAttribute("aria-disabled", "true");
  await expect(modify(page)).toHaveAccessibleDescription(/revenez au présent pour modifier/);
  // Its key does nothing either.
  await page.keyboard.press("e");
  await expect(editPanel(page)).toHaveCount(0);
  await main(page).getByRole("button", { name: "Actions" }).click();
  const close = page.getByRole("menuitem", { name: /^Clore…/ });
  await expect(close).toHaveAttribute("aria-disabled", "true");
  await expect(close).toHaveAccessibleDescription(/revenez au présent pour modifier/);
  await expectAccessible(page);
  await page.keyboard.press("Escape");

  // The list at that date: no editor in a cell.
  await atRest(page);
  await page.goto(`/prefixes?domain=${DATA.domain}&as_of=${encodeURIComponent(asOf)}`);
  const cell = page
    .getByRole("grid", { name: "Préfixes" })
    .locator('[role="gridcell"][data-column="value"]')
    .first();
  await expect(cell).toHaveAttribute("aria-readonly", "true");
  await expect(cell).toHaveAccessibleDescription(/hors du présent/);
});

test("a validity date alone leaves the dated draft possible", async ({ page, login }) => {
  await login("alice");
  await page.goto(`/prefixes?valid_at=${encodeURIComponent(VALID_AT)}`);
  await expect(band(page)).toContainText("brouillon daté possible");
  await expect(band(page)).not.toContainText("lecture seule");
  await expect(band(page)).not.toContainText("désactivé");

  // A month ahead: the record opens its panel, to the draft only; nothing applied at once.
  const record = await prefixRecord(page);
  const ahead = new Date(Date.now() + 30 * 86_400_000).toISOString();
  await atRest(page);
  await page.goto(`${record}?valid_at=${encodeURIComponent(ahead)}`);
  await expect(band(page)).toContainText("brouillon daté possible");
  await expect(
    page.getByRole("heading", { level: 1, name: `Préfixe ${DATA.prefix}` }),
  ).toBeAttached();
  await expect(modify(page)).not.toHaveAttribute("aria-disabled");
  await main(page).getByRole("button", { name: "Actions" }).click();
  await expect(page.getByRole("menuitem", { name: /^Clore…/ })).not.toHaveAttribute(
    "aria-disabled",
  );
  await page.keyboard.press("Escape");
  await modify(page).click();
  const form = editPanel(page);
  await expect(form).toBeVisible();
  // Its key too, once the panel is closed (the focus back on « Modifier »).
  await form.getByRole("button", { name: "Annuler" }).click();
  await expect(form).toHaveCount(0);
  await expect(modify(page)).toBeFocused();
  await page.keyboard.press("e");
  await expect(form).toBeVisible();
  await form.getByRole("button", { name: "Choisir l'action" }).click();
  const apply = page.getByRole("menuitem", { name: /^Appliquer/ });
  await expect(apply).toHaveAttribute("aria-disabled", "true");
  await expect(apply).toHaveAccessibleDescription(/va au brouillon daté/);
  await page.keyboard.press("Escape");
  const value = form.getByRole("textbox", { name: "Valeur" });
  await value.fill("192.0.2.0/25");
  await page.keyboard.press("Control+Enter");
  await expect(form).toHaveCount(0);
  await expect(page.getByRole("button", { name: /^Brouillon 1 opération/ })).toBeVisible();
});

test("under a validity date alone, « Créer » of a list and the « + » of the topography go to the dated draft", async ({
  page,
  login,
}) => {
  await login("alice");
  const ahead = encodeURIComponent(new Date(Date.now() + 30 * 86_400_000).toISOString());
  await page.goto(`/addresses?domain=${DATA.domain}&valid_at=${ahead}`);
  await expect(band(page)).toContainText("brouillon daté possible");
  // A cell has no editor there, and says where the change goes: not back to the present.
  const cell = page
    .getByRole("grid", { name: "Adresses" })
    .locator('[role="gridcell"][data-column="usage"]')
    .first();
  await expect(cell).toHaveAttribute("aria-readonly", "true");
  await expect(cell).toHaveAccessibleDescription(/rien ne se modifie en place/);

  // `c` opens the edit panel, to the draft only: « Appliquer » refused, its reason said.
  await page.keyboard.press("c");
  const created = page.getByRole("form", { name: "Créer : Adresse" });
  await expect(created).toBeVisible();
  await created.getByRole("button", { name: "Choisir l'action" }).click();
  const apply = page.getByRole("menuitem", { name: /^Appliquer/ });
  await expect(apply).toHaveAttribute("aria-disabled", "true");
  await expect(apply).toHaveAccessibleDescription(/va au brouillon daté/);
  await page.keyboard.press("Escape");
  await created.getByRole("button", { name: "Annuler" }).click();
  await expect(created).toHaveCount(0);

  // The « + » of a network of the tree: an address added to the dated draft.
  await atRest(page);
  await page.goto(`/topology?valid_at=${ahead}`);
  const tree = page.getByRole("treegrid", { name: `Arbre des préfixes du domaine ${DATA.domain}` });
  const plus = tree.getByRole("button", { name: `Ajouter dans ${DATA.prefix}` });
  await expect(plus).toBeAttached();
  await expect(plus).not.toHaveAttribute("aria-disabled");
  await tree.getByText(DATA.prefix, { exact: true }).click();
  await page.keyboard.press("Tab");
  await expect(plus).toBeFocused();
  await page.keyboard.press("Enter");
  const form = page.getByRole("form", { name: `Ajouter dans ${DATA.prefix}` });
  await expect(form.getByRole("button", { name: "Adresse", pressed: true })).toBeVisible();
  await form.getByRole("textbox", { name: "Valeur" }).fill("192.0.2.77");
  await form.getByRole("combobox", { name: /^Usage/ }).click();
  await page.getByRole("option", { name: /^Hôte/ }).click();
  await form.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(form).toHaveCount(0);
  await expect(page.getByRole("button", { name: /^Brouillon 1 opération/ })).toBeVisible();

  // Under `as_of`, the « + » stays visible, refused, its reason said (`c` refused there:
  // `dated-create.test.ts`).
  await atRest(page);
  await page.goto(
    `/topology?as_of=${encodeURIComponent(new Date(Date.now() - 1_000).toISOString())}`,
  );
  await expect(plus).toHaveAttribute("aria-disabled", "true");
  await expect(plus).toHaveAccessibleDescription(/revenez au présent/);
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
