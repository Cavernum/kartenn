import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Menu Button and Menu patterns (spec 020, design « Inventaire »
// line 11, « Accessibilité »; REQ-020-16, 18, 28), on Reka's DropdownMenu: a button
// `aria-haspopup="menu"`; Enter, Space and Down open the menu on its first entry, Up on
// its last; Down and Up go round, Home and End, a letter goes to the next entry it
// starts; Enter chooses and closes, the focus back on the button; Escape closes without
// choosing; Tab and Shift+Tab close and go on from the button. A disabled entry stays
// reachable, says why, and cannot be chosen. Shortcuts are shown at the end: a
// combination in `aria-keyshortcuts`, a sequence in two keycaps and in words.

test.describe("Menu (APG Menu Button, Menu)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Menu", "keyboard");
  });

  test("closed, the button says it opens a menu", async ({ page }) => {
    const button = page.getByRole("button", { name: "Actions" });
    await expect(button).toHaveAttribute("aria-haspopup", "menu");
    await expect(button).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("menu")).toHaveCount(0);
  });

  test("Enter opens on the first entry; the arrows go round; Enter chooses", async ({ page }) => {
    const button = page.getByRole("button", { name: "Actions" });
    await button.focus();
    await page.keyboard.press("Enter");
    const menu = page.getByRole("menu", { name: "Actions" });
    await expect(menu).toBeVisible();
    await expect(button).toHaveAttribute("aria-expanded", "true");
    await expect(menu.getByRole("menuitem", { name: "Modifier" })).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(menu.getByRole("menuitem", { name: "Purger l'historique" })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(menu.getByRole("menuitem", { name: "Modifier" })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(menu.getByRole("menuitem", { name: "Copier l'identifiant" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menu")).toHaveCount(0);
    await expect(button).toBeFocused();
    await expect(page.getByRole("status")).toHaveText("Action choisie : Copier l'identifiant");
  });

  test("Space and Down open on the first entry, Up on the last", async ({ page }) => {
    const button = page.getByRole("button", { name: "Actions" });
    await button.focus();
    await page.keyboard.press(" ");
    await expect(page.getByRole("menuitem", { name: "Modifier" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(button).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("menuitem", { name: "Modifier" })).toBeFocused();
    await page.keyboard.press("Escape");
    await page.keyboard.press("ArrowUp");
    await expect(page.getByRole("menuitem", { name: "Purger l'historique" })).toBeFocused();
  });

  test("Home, End and a letter move; Escape closes without choosing", async ({ page }) => {
    const button = page.getByRole("button", { name: "Actions" });
    await button.focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("End");
    await expect(page.getByRole("menuitem", { name: "Purger l'historique" })).toBeFocused();
    await page.keyboard.press("Home");
    await expect(page.getByRole("menuitem", { name: "Modifier" })).toBeFocused();
    await page.keyboard.press("h");
    await expect(page.getByRole("menuitem", { name: "Historique", exact: true })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("menu")).toHaveCount(0);
    await expect(button).toBeFocused();
    await expect(button).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("status")).toHaveText("Action choisie : aucune");
  });

  test("a disabled entry is reachable, says why, and cannot be chosen", async ({ page }) => {
    await page.getByRole("button", { name: "Actions" }).focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("d");
    const move = page.getByRole("menuitem", { name: "Déplacer vers…" });
    await expect(move).toBeFocused();
    await expect(move).toHaveAttribute("aria-disabled", "true");
    await expect(move).toHaveAccessibleDescription("Parent déduit de l'inclusion");
    await expect(page.locator(".k-tooltip")).toHaveText("Parent déduit de l'inclusion");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menu")).toBeVisible();
    await expect(page.getByRole("status")).toHaveText("Action choisie : aucune");
    // A missing permission says which one.
    await page.keyboard.press("End");
    const purge = page.getByRole("menuitem", { name: "Purger l'historique" });
    await expect(purge).toHaveAttribute("aria-disabled", "true");
    await expect(purge).toHaveAccessibleDescription("Il faut prefix.purge");
  });

  test("shortcuts at the end: a combination in aria-keyshortcuts, a sequence in words", async ({
    page,
  }) => {
    await page.getByRole("button", { name: "Actions" }).focus();
    await page.keyboard.press("Enter");
    const edit = page.getByRole("menuitem", { name: "Modifier" });
    await expect(edit).toHaveAttribute("aria-keyshortcuts", "e");
    const prefixes = page.getByRole("menuitem", { name: "Voir les préfixes" });
    await expect(prefixes).not.toHaveAttribute("aria-keyshortcuts");
    await expect(prefixes).toHaveAccessibleDescription("g puis p");
    // Two keycaps, one per step: never one combination « g p ».
    await expect(prefixes.locator(".k-shortcut-hint > .k-kbd")).toHaveText(["g", "p"]);
  });

  test("Tab and Shift+Tab close the menu and go on from the button", async ({ page }) => {
    const button = page.getByRole("button", { name: "Actions" });
    await button.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menuitem", { name: "Modifier" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("menu")).toHaveCount(0);
    await expect(page.getByRole("textbox", { name: "Description" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(button).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menuitem", { name: "Modifier" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("menu")).toHaveCount(0);
    await expect(page.getByRole("textbox", { name: "Préfixe" })).toBeFocused();
  });
});

test.describe("Menu, entries to tick (menuitemcheckbox)", () => {
  test("each says whether it is ticked; choosing one keeps the menu open", async ({ page }) => {
    await openState(page, "Menu", "checkbox");
    const menu = page.getByRole("menu", { name: "Colonnes" });
    await expect(menu.getByRole("menuitemcheckbox")).toHaveCount(3);
    await expect(menu.getByRole("menuitemcheckbox", { name: "Propriétaire" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    const used = menu.getByRole("menuitemcheckbox", { name: "Occupation" });
    await expect(used).toHaveAttribute("aria-checked", "false");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await expect(used).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(menu).toBeVisible();
    await expect(used).toBeFocused();
  });
});
