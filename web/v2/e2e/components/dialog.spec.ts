import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the modal dialogs (APG Dialog (Modal), Alert Dialog; spec 020, design
// « Inventaire » lines 12 and 13; REQ-020-16, 18, 28), on Reka's Dialog and AlertDialog:
// open, the dialog is named by its title, described by its description, says it is modal
// (`aria-modal`, which Reka does not write) and hides the rest of the page; the focus goes
// in (the first field; « Annuler » for a confirmation), Tab and Shift+Tab go round inside
// it; Escape closes it and the focus goes back to what opened it: its button, a button of
// the host, or the button of the menu whose entry opened it (gone by then).

test.describe("Dialog (APG Dialog (Modal))", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Dialog", "keyboard");
  });

  test("its button says it opens a dialog; Enter opens it on its first field", async ({ page }) => {
    const button = page.getByRole("button", { name: "Nouveau préfixe" });
    await expect(button).toHaveAttribute("aria-haspopup", "dialog");
    await expect(button).toHaveAttribute("aria-expanded", "false");
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Nouveau préfixe" });
    await expect(dialog).toHaveAttribute("aria-modal", "true");
    await expect(dialog).toHaveAccessibleDescription("Le préfixe rejoint le brouillon.");
    await expect(dialog.getByRole("textbox", { name: "Préfixe" })).toBeFocused();
    // The rest of the page is out of the tree.
    await expect(page.getByRole("textbox", { name: "Note" })).toHaveCount(0);
  });

  test("Tab and Shift+Tab go round inside; Escape gives the focus back", async ({ page }) => {
    const button = page.getByRole("button", { name: "Nouveau préfixe" });
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Nouveau préfixe" });
    const first = dialog.getByRole("textbox", { name: "Préfixe" });
    await expect(first).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("textbox", { name: "Nom" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: "Annuler" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: "Créer" })).toBeFocused();
    // Past the last control, round to the first one: the close button of the header.
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(first).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog.getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog.getByRole("button", { name: "Créer" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(button).toBeFocused();
    await expect(button).toHaveAttribute("aria-expanded", "false");
  });

  test("the close button closes it and gives the focus back", async ({ page }) => {
    const button = page.getByRole("button", { name: "Nouveau préfixe" });
    await button.focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("Shift+Tab");
    const dialog = page.getByRole("dialog", { name: "Nouveau préfixe" });
    await expect(dialog.getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(button).toBeFocused();
  });

  test("opened by an entry of a menu, it gives the focus back to the menu's button", async ({
    page,
  }) => {
    const actions = page.getByRole("button", { name: "Actions" });
    await actions.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menuitem", { name: "Renommer…" })).toBeFocused();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Renommer" });
    await expect(dialog.getByRole("textbox", { name: "Nom" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(actions).toBeFocused();
  });
});

test.describe("AlertDialog (APG Alert Dialog)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "AlertDialog", "keyboard");
  });

  test("opens on « Annuler »; Tab goes round; Escape cancels, the focus back", async ({ page }) => {
    const button = page.getByRole("button", { name: "Abandonner le brouillon" });
    await expect(button).toHaveAttribute("aria-haspopup", "dialog");
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("alertdialog", { name: "Abandonner le brouillon ?" });
    await expect(dialog).toHaveAttribute("aria-modal", "true");
    await expect(dialog).toHaveAccessibleDescription(
      "Les 12 opérations en brouillon seront perdues.",
    );
    const cancel = dialog.getByRole("button", { name: "Annuler" });
    await expect(cancel).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: "Abandonner" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(cancel).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog.getByRole("button", { name: "Abandonner" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
    await expect(button).toBeFocused();
    await expect(page.locator("[data-outcome]")).toHaveText("Abandon annulé.");
  });

  test("a press outside does not close it; the confirming button confirms", async ({ page }) => {
    const button = page.getByRole("button", { name: "Abandonner le brouillon" });
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("alertdialog", { name: "Abandonner le brouillon ?" });
    await page.mouse.click(4, 4);
    await expect(dialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(button).toBeFocused();
    await page.keyboard.press("Enter");
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: "Abandonner" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
    await expect(button).toBeFocused();
    await expect(page.locator("[data-outcome]")).toHaveText("Brouillon abandonné.");
  });
});

test.describe("Drawer (modal side dialog)", () => {
  test("opened by a button of the host: its first field, round, Escape, the focus back", async ({
    page,
  }) => {
    await openState(page, "Drawer", "keyboard");
    const button = page.getByRole("button", { name: "Modifier le site" });
    await button.focus();
    await page.keyboard.press("Enter");
    const drawer = page.getByRole("dialog", { name: "Modifier le site" });
    await expect(drawer).toHaveAttribute("aria-modal", "true");
    const first = drawer.getByRole("textbox", { name: "Nom" });
    await expect(first).toBeFocused();
    await expect(page.getByRole("textbox", { name: "Note" })).toHaveCount(0);
    await page.keyboard.press("Shift+Tab");
    await expect(drawer.getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(drawer.getByRole("button", { name: "Enregistrer" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(drawer.getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(button).toBeFocused();
  });
});
