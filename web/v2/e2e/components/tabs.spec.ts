import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Tabs pattern with manual activation (spec 020, design
// « Inventaire » line 10, « Accessibilité »; REQ-020-16, 18, 28), on Reka's Tabs: a named
// `tablist`, one tab stop (the selected tab); Right and Left move the focus and go round,
// Home and End, without changing the panel; Enter or Space activates the focused tab;
// Tab goes on to the panel, named by its tab. A plugin tab loading or failed says so.

test.describe("Tabs (APG Tabs, manual activation)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Tabs", "keyboard");
  });

  test("one tab stop, the selected tab; its panel named by it", async ({ page }) => {
    const list = page.getByRole("tablist", { name: "Fiche du préfixe" });
    const first = list.getByRole("tab", { name: "Attributs" });
    await expect(first).toHaveAttribute("aria-selected", "true");
    await expect(page.getByRole("tabpanel", { name: "Attributs" })).toHaveText(
      "Panneau « Attributs ».",
    );
    await expect(list.getByRole("tab", { name: "VLAN (en chargement)" })).toBeVisible();
    await expect(list.getByRole("tab", { name: "Flux (en erreur)" })).toBeVisible();
    await page.getByRole("textbox", { name: "Préfixe" }).focus();
    await page.keyboard.press("Tab");
    await expect(first).toBeFocused();
  });

  test("the arrows move the focus only; Enter activates", async ({ page }) => {
    await page.getByRole("textbox", { name: "Préfixe" }).focus();
    await page.keyboard.press("Tab");
    await page.keyboard.press("ArrowRight");
    const addresses = page.getByRole("tab", { name: "Adresses 42" });
    await expect(addresses).toBeFocused();
    // Manual activation: the panel is still the first one.
    await expect(addresses).toHaveAttribute("aria-selected", "false");
    await expect(page.getByRole("tabpanel", { name: "Attributs" })).toBeVisible();
    await page.keyboard.press("Enter");
    await expect(addresses).toHaveAttribute("aria-selected", "true");
    await expect(page.getByRole("tabpanel", { name: "Adresses 42" })).toHaveText(
      "Panneau « Adresses ».",
    );
    await expect(addresses).toBeFocused();
  });

  test("Left goes round, Home and End; Space activates; Tab goes to the panel", async ({
    page,
  }) => {
    await page.getByRole("textbox", { name: "Préfixe" }).focus();
    await page.keyboard.press("Tab");
    await page.keyboard.press("ArrowLeft");
    await expect(page.getByRole("tab", { name: "Flux (en erreur)" })).toBeFocused();
    await page.keyboard.press("Home");
    await expect(page.getByRole("tab", { name: "Attributs" })).toBeFocused();
    await page.keyboard.press("End");
    await expect(page.getByRole("tab", { name: "Flux (en erreur)" })).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    const history = page.getByRole("tab", { name: "Historique" });
    await expect(history).toBeFocused();
    await page.keyboard.press(" ");
    await expect(history).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Tab");
    const panel = page.getByRole("tabpanel", { name: "Historique" });
    await expect(panel).toBeFocused();
    // Back in the list: the selected tab is its only stop.
    await page.keyboard.press("Shift+Tab");
    await expect(history).toBeFocused();
  });
});
