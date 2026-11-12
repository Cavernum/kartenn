import { expect, openState, test } from "../fixtures/index.ts";

// Whose Escape in the bottom panel (task 021-12qa; `ownsEscape`, `usePanelClose`, shared
// with `SidePanel`): the host of the `fields` state says `modified`, so a text field lets
// Escape go. Intact, the panel closes at once; modified, Escape or the cross ask
// « Abandonner les modifications ? » first (Q-021-22): « Continuer » gives the focus back to
// the field, the input kept; « Abandonner » closes the panel, the focus back to the button
// that opened it.

test.describe("BottomPanel, whose Escape", () => {
  test("intact, Escape from the text field closes; modified, it asks first", async ({ page }) => {
    await openState(page, "BottomPanel", "fields");
    const panel = page.getByRole("complementary", { name: "Panneau du bas" });
    const opener = page.getByRole("button", { name: "Détail de 192.0.2.64/26" });
    const label = panel.getByRole("textbox", { name: "Libellé" });
    const ask = page.getByRole("alertdialog", { name: "Abandonner les modifications ?" });
    await panel.getByRole("button", { name: "Fermer le panneau" }).click();
    await expect(panel).toBeHidden();

    // Opened by its button, the focus stays there: the handle, the tab, its panel, the field.
    await opener.focus();
    await page.keyboard.press("Enter");
    await expect(panel).toBeVisible();
    for (let i = 0; i < 4; i++) await page.keyboard.press("Tab");
    await expect(label).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(ask).toHaveCount(0);
    await expect(panel).toBeHidden();
    await expect(opener).toBeFocused();

    await page.keyboard.press("Enter");
    for (let i = 0; i < 4; i++) await page.keyboard.press("Tab");
    await expect(label).toBeFocused();
    await page.keyboard.press("End");
    await page.keyboard.type(" B");
    await page.keyboard.press("Escape");
    await expect(ask.getByRole("button", { name: "Continuer" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(ask).toBeHidden();
    await expect(label).toBeFocused();
    await expect(label).toHaveValue("Supervision B");

    await page.keyboard.press("Escape");
    await page.keyboard.press("Tab");
    await expect(ask.getByRole("button", { name: "Abandonner" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(ask).toBeHidden();
    await expect(panel).toBeHidden();
    await expect(opener).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(label).toHaveValue("Supervision");
  });
});
