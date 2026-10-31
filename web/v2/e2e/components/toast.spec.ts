import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the toasts (spec 020, design « Inventaire » line 14; REQ-020-16, 18,
// 28; WCAG 2.2.1, 4.1.3): two live regions, present before any toast, `status` for the
// ordinary ones and `alert` for the errors; no toast takes the focus; Tab reaches them
// after the rest of the page, the region of the toasts is a landmark; an ordinary toast
// goes after its duration (4 s in this state), held while the pointer is over the toasts
// or the focus in them; an error stays until dismissed; Escape or the close button
// dismisses, the focus going to the next toast, else back where it came from.

const region = (page: import("@playwright/test").Page) =>
  page.getByRole("region", { name: "Notifications" });

test.describe("Toast (status, alert)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Toast", "keyboard");
  });

  test("the live regions are there before any toast; a toast never takes the focus", async ({
    page,
  }) => {
    const lanes = page.locator(".k-toasts__lane");
    await expect(lanes.and(page.locator("[role='status']"))).toHaveAttribute(
      "aria-atomic",
      "false",
    );
    await expect(lanes.and(page.locator("[role='alert']"))).toHaveAttribute("aria-atomic", "false");
    await expect(region(page)).toHaveCount(0);
    const add = page.getByRole("button", { name: "Ajouter" });
    await add.focus();
    await page.keyboard.press("Enter");
    const toast = page.locator("[role='status'] .k-toast");
    // The tone in words, hidden, before the title (its own element: no space in the text).
    await expect(toast).toContainText(/Succès\s:\s*Préfixe .* ajouté au brouillon\./);
    await expect(add).toBeFocused();
    await expect(region(page)).toBeVisible();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Enter");
    const error = page.locator("[role='alert'] .k-toast");
    await expect(error).toContainText(/Erreur\s:\s*L'enregistrement a échoué/);
    await expect(page.getByRole("button", { name: "Enregistrer" })).toBeFocused();
  });

  test("an ordinary toast goes after its duration; an error stays", async ({ page }) => {
    await page.getByRole("button", { name: "Ajouter" }).click();
    await page.getByRole("button", { name: "Enregistrer" }).click();
    await page.mouse.move(0, 0);
    await expect(page.locator("[role='status'] .k-toast")).toHaveCount(1);
    await expect(page.locator("[role='status'] .k-toast")).toHaveCount(0, { timeout: 8000 });
    await expect(page.locator("[role='alert'] .k-toast")).toHaveCount(1);
  });

  test("the pointer over the toasts holds them (2.2.1)", async ({ page }) => {
    await page.getByRole("button", { name: "Ajouter" }).click();
    const toast = page.locator("[role='status'] .k-toast");
    await toast.hover();
    await page.waitForTimeout(5000);
    await expect(toast).toHaveCount(1);
    await page.mouse.move(0, 0);
    await expect(toast).toHaveCount(0, { timeout: 8000 });
  });

  test("the focus in a toast holds it; Escape dismisses it, the focus back", async ({ page }) => {
    const add = page.getByRole("button", { name: "Ajouter" });
    await add.focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    const close = region(page).getByRole("button", { name: "Fermer la notification" });
    await expect(close).toBeFocused();
    await page.waitForTimeout(5000);
    await expect(close).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.locator(".k-toast")).toHaveCount(0);
    // Back where the focus came from into the toasts.
    await expect(page.getByRole("button", { name: "Enregistrer" })).toBeFocused();
    // The toasts that come next are not held by the focus that was in the last one.
    await page.mouse.move(0, 0);
    await add.click();
    await page.mouse.move(0, 0);
    await expect(page.locator("[role='status'] .k-toast")).toHaveCount(1);
    await expect(page.locator("[role='status'] .k-toast")).toHaveCount(0, { timeout: 8000 });
  });

  test("dismissed, a toast hands the focus to the next one", async ({ page }) => {
    const save = page.getByRole("button", { name: "Enregistrer" });
    await save.focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await expect(page.locator("[role='alert'] .k-toast")).toHaveCount(2);
    // Its code, its Request-Id, then its close button.
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    const closes = region(page).getByRole("button", { name: "Fermer la notification" });
    await expect(closes.first()).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("[role='alert'] .k-toast")).toHaveCount(1);
    await expect(closes.first()).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator(".k-toast")).toHaveCount(0);
    await expect(save).toBeFocused();
  });
});
