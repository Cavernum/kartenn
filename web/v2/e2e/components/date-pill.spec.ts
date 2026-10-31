import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the date pill (APG Button opening a non-modal Dialog; spec 020,
// design « Inventaire » line 27; spec 021, « Cadre », REQ-021-5): named « Date de lecture :
// <context> », it opens the period picker in a layer under it, named by the picker's
// title, the focus on its first field, nothing hidden around it; Escape gives the focus
// back to the pill; Shift+Tab from its start goes back to the pill, Tab past its end to
// what follows the pill, and the layer closes (the focus never left behind a layer that
// covers a narrow window, 2.4.11). Applied, the period goes to the host; the layer closes
// once it is read, or stays open on the server's refusal (a 400 on `as_of`), shown on its
// field, the focus on it. Keys only, from the pill.

const pill = (page: Page) => page.getByRole("button", { name: /^Date de lecture/ });
const after = (page: Page) => page.getByRole("button", { name: "Après la pastille" });
const layer = (page: Page) => page.getByRole("dialog", { name: "Lire à une date" });
/** A field by its label (not the group of its shortcuts, « Raccourcis : <label> »). */
const field = (page: Page, name: string) => page.getByRole("group", { name, exact: true });
const segment = (page: Page, label: string, name: string) =>
  field(page, label).getByRole("spinbutton", { name, exact: true });
const query = (page: Page) => page.locator("[data-query]");

async function openLayer(page: Page): Promise<void> {
  await pill(page).focus();
  await page.keyboard.press("Enter");
  await expect(layer(page)).toBeVisible();
}

test.describe("DatePill (Button + non-modal Dialog)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "DatePill", "keyboard");
  });

  test("named by its context; Enter opens the picker on its first field, nothing hidden", async ({
    page,
  }) => {
    await expect(pill(page)).toHaveAccessibleName("Date de lecture : Présent");
    await expect(pill(page)).toHaveAttribute("aria-haspopup", "dialog");
    await expect(pill(page)).toHaveAttribute("aria-expanded", "false");
    await openLayer(page);
    await expect(pill(page)).toHaveAttribute("aria-expanded", "true");
    await expect(layer(page)).not.toHaveAttribute("aria-modal", "true");
    await expect(segment(page, "Valide au", "Année")).toBeFocused();
    // Non-modal: the page stays in the tree.
    await expect(after(page)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(layer(page)).toHaveCount(0);
    await expect(pill(page)).toBeFocused();
    await expect(pill(page)).toHaveAttribute("aria-expanded", "false");
  });

  test("Shift+Tab from its start goes back to the pill; Tab past its end goes on", async ({
    page,
  }) => {
    await openLayer(page);
    // Before the first field, its close button: the start of the layer.
    await page.keyboard.press("Shift+Tab");
    await expect(layer(page).getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(layer(page)).toHaveCount(0);
    await expect(pill(page)).toBeFocused();
    await page.keyboard.press("Enter");
    const apply = layer(page).getByRole("button", { name: "Appliquer" });
    for (let i = 0; i < 30 && !(await apply.evaluate((el) => el === document.activeElement)); i++)
      await page.keyboard.press("Tab");
    await expect(apply).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(layer(page)).toHaveCount(0);
    await expect(after(page)).toBeFocused();
  });

  test("applied, the period is read: the layer closes, the pill says it", async ({ page }) => {
    await openLayer(page);
    await page.keyboard.type("202511030930");
    await page.keyboard.press("Enter");
    await expect(layer(page)).toHaveCount(0);
    await expect(pill(page)).toBeFocused();
    await expect(pill(page)).toHaveAccessibleName(
      "Date de lecture : 3 nov. 2025, 09:30 Europe/Paris",
    );
    await expect(query(page)).toHaveText("valid_at=2025-11-03T08:30:00Z");
    // Opened again on the period read; back to the present by its button.
    await page.keyboard.press("Enter");
    await expect(segment(page, "Valide au", "Année")).toHaveAttribute("aria-valuenow", "2025");
    const back = layer(page).getByRole("button", { name: "Revenir au présent" });
    for (let i = 0; i < 30 && !(await back.evaluate((el) => el === document.activeElement)); i++)
      await page.keyboard.press("Tab");
    await page.keyboard.press("Enter");
    await expect(layer(page)).toHaveCount(0);
    await expect(query(page)).toHaveText("(présent)");
    await expect(pill(page)).toHaveAccessibleName("Date de lecture : Présent");
  });

  test("refused by the server (a 400 on as_of): on its field, the focus on it, open", async ({
    page,
  }) => {
    await openLayer(page);
    const now = page
      .getByRole("group", { name: /^Raccourcis\s:\sConnu au$/ })
      .getByRole("button", { name: "Maintenant" });
    for (let i = 0; i < 30 && !(await now.evaluate((el) => el === document.activeElement)); i++)
      await page.keyboard.press("Tab");
    await expect(now).toBeFocused();
    await page.keyboard.press("Enter");
    const apply = layer(page).getByRole("button", { name: "Appliquer" });
    for (let i = 0; i < 10 && !(await apply.evaluate((el) => el === document.activeElement)); i++)
      await page.keyboard.press("Tab");
    await page.keyboard.press("Enter");
    // The browser's clock let it through; the demo's server, five minutes behind, refuses.
    await expect(field(page, "Connu au")).toHaveAccessibleDescription(
      /^Refusé par le serveur : cette date dépasse son heure\./,
    );
    await expect(segment(page, "Connu au", "Année")).toBeFocused();
    await expect(layer(page)).toBeVisible();
    await expect(query(page)).toHaveText("(présent)");
    // Closed, the refusal goes with the entry it answered: reopened, neither is there.
    await page.keyboard.press("Escape");
    await expect(pill(page)).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(field(page, "Connu au")).toHaveAccessibleDescription(/^Ce que la base savait/);
    await expect(segment(page, "Valide au", "Année")).toBeFocused();
  });

  test("opened with a refusal of the host, the focus goes to the refused field", async ({
    page,
  }) => {
    await openState(page, "DatePill", "host");
    await expect(layer(page)).toBeVisible();
    await expect(segment(page, "Connu au", "Année")).toBeFocused();
  });
});
