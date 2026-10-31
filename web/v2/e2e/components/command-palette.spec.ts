import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the command palette (APG Combobox, list autocomplete with automatic
// selection, in a modal Dialog; spec 020, design « Inventaire » line 19; spec 021,
// REQ-021-3, « Raccourcis »): opened by a control of the host (the shell opens it by
// Ctrl+K), modal (`aria-modal`, the page hidden), the focus in its field; typing filters
// (the host), the first entry highlighted and selected; Down and Up move the highlight
// (`aria-activedescendant`); Enter runs the entry and closes, Escape closes; the focus goes
// back to what opened it; Tab stays in the field. The count is said by a polite region;
// a shortcut is said as in `Menu` (a combination in `aria-keyshortcuts`, a sequence in the
// entry's description).

const opener = (page: Page) => page.getByRole("button", { name: "Ouvrir la palette" });
const field = (page: Page) => page.getByRole("combobox", { name: /Rechercher une commande/ });
const ran = (page: Page) => page.locator("[data-ran]");

async function openPalette(page: Page): Promise<void> {
  await opener(page).focus();
  await page.keyboard.press("Enter");
  await expect(field(page)).toBeFocused();
}

/** The option the field points to (`aria-activedescendant`). */
async function active(page: Page): Promise<string> {
  const id = await field(page).getAttribute("aria-activedescendant");
  expect(id).toBeTruthy();
  return (await page.locator(`[id="${id}"] .k-palette__text`).textContent()) ?? "";
}

test.describe("CommandPalette (Combobox in a Dialog (Modal))", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "CommandPalette", "keyboard");
  });

  test("opens modal, on its field, the first entry highlighted and selected", async ({ page }) => {
    await expect(opener(page)).toHaveAttribute("aria-haspopup", "dialog");
    await openPalette(page);
    const dialog = page.getByRole("dialog", { name: "Palette de commandes" });
    await expect(dialog).toHaveAttribute("aria-modal", "true");
    await expect(opener(page)).toHaveCount(0);
    await expect(field(page)).toHaveAttribute("aria-expanded", "true");
    expect(await active(page)).toBe("Créer un préfixe");
    const selected = dialog.getByRole("option", { selected: true });
    await expect(selected).toHaveCount(1);
    await expect(selected).toContainText("Créer un préfixe");
    await expect(dialog.getByRole("status")).toHaveText("6 résultats");
  });

  test("typing filters and says the count; the arrows move; Enter runs and closes", async ({
    page,
  }) => {
    await openPalette(page);
    await page.keyboard.type("serv");
    await expect(page.getByRole("status")).toHaveText("1 résultat");
    expect(await active(page)).toBe("Serveurs de bases");
    await page.keyboard.press("Control+a");
    await page.keyboard.type("pré");
    await expect(page.getByRole("status")).toHaveText("2 résultats");
    expect(await active(page)).toBe("Créer un préfixe");
    await page.keyboard.press("ArrowDown");
    expect(await active(page)).toBe("Préfixes");
    await expect(page.getByRole("option", { selected: true })).toContainText("Préfixes");
    await page.keyboard.press("ArrowUp");
    expect(await active(page)).toBe("Créer un préfixe");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(ran(page)).toHaveText("Commande lancée : Préfixes");
    await expect(opener(page)).toBeFocused();
  });

  test("nothing found: said, no option pointed to; Escape closes, the focus back", async ({
    page,
  }) => {
    await openPalette(page);
    await page.keyboard.type("zzz");
    await expect(page.getByRole("status")).toHaveText("Aucun résultat");
    await expect(field(page)).toHaveAttribute("aria-expanded", "false");
    await expect(field(page)).not.toHaveAttribute("aria-activedescendant", /.+/);
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(opener(page)).toBeFocused();
    await expect(ran(page)).toHaveText("Commande lancée : aucune");
  });

  test("Tab and Shift+Tab stay in the field", async ({ page }) => {
    await openPalette(page);
    await page.keyboard.type("pré");
    await page.keyboard.press("Tab");
    await expect(field(page)).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(field(page)).toBeFocused();
    await expect(field(page)).toHaveValue("pré");
  });

  test("shortcuts: a combination in aria-keyshortcuts, a sequence in the description", async ({
    page,
  }) => {
    await openPalette(page);
    const create = page.getByRole("option", { name: "Créer un préfixe" });
    await expect(create).toHaveAttribute("aria-keyshortcuts", "c");
    await expect(create).not.toHaveAttribute("aria-describedby", /.+/);
    const prefixes = page.getByRole("option", { name: "Préfixes" });
    await expect(prefixes).not.toHaveAttribute("aria-keyshortcuts", /.+/);
    await expect(prefixes).toHaveAccessibleDescription("g puis p");
    // The shortcut is out of the name: the name is the entry.
    await expect(page.getByRole("option", { name: "Préfixes", exact: true })).toHaveCount(1);
  });
});
