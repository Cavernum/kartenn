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
    // « Enregistrer », « Propriétés », then the toast.
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    const close = region(page).getByRole("button", { name: "Fermer la notification" });
    await expect(close).toBeFocused();
    await page.waitForTimeout(5000);
    await expect(close).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.locator(".k-toast")).toHaveCount(0);
    // Back where the focus came from into the toasts.
    await expect(page.getByRole("button", { name: "Propriétés" })).toBeFocused();
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
    // « Propriétés », then its code, its Request-Id and its close button.
    for (let i = 0; i < 4; i++) await page.keyboard.press("Tab");
    const closes = region(page).getByRole("button", { name: "Fermer la notification" });
    await expect(closes.first()).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("[role='alert'] .k-toast")).toHaveCount(1);
    await expect(closes.first()).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator(".k-toast")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Propriétés" })).toBeFocused();
  });

  test("dismissed last, a toast hands the focus to the previous one", async ({ page }) => {
    const add = page.getByRole("button", { name: "Ajouter" });
    await add.focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    for (let i = 0; i < 4; i++) await page.keyboard.press("Tab");
    const closes = region(page).getByRole("button", { name: "Fermer la notification" });
    await expect(closes.nth(1)).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.locator(".k-toast")).toHaveCount(1);
    await expect(closes.first()).toBeFocused();
  });

  test("dismissed by a click, a toast hands the focus to no one", async ({ page }) => {
    await page.getByRole("button", { name: "Enregistrer" }).click();
    await page.getByRole("button", { name: "Enregistrer" }).click();
    const closes = region(page).getByRole("button", { name: "Fermer la notification" });
    await closes.first().click();
    await expect(page.locator(".k-toast")).toHaveCount(1);
    await expect(closes.first()).not.toBeFocused();
    await expect(page.locator("body")).toBeFocused();
  });

  test("a hidden page holds the toasts", async ({ page }) => {
    await page.getByRole("button", { name: "Ajouter" }).click();
    await page.mouse.move(0, 0);
    const setHidden = (hidden: boolean) =>
      page.evaluate((value) => {
        Object.defineProperty(document, "hidden", { value, configurable: true });
        document.dispatchEvent(new Event("visibilitychange"));
      }, hidden);
    await setHidden(true);
    await page.waitForTimeout(5000);
    const toast = page.locator("[role='status'] .k-toast");
    await expect(toast).toHaveCount(1);
    await setHidden(false);
    await expect(toast).toHaveCount(0, { timeout: 8000 });
  });

  test("toasts lie under a modal dialog, held while it covers them", async ({ page }) => {
    await page.getByRole("button", { name: "Enregistrer" }).click();
    await page.getByRole("button", { name: "Ajouter" }).click();
    const button = page.getByRole("button", { name: "Propriétés" });
    await button.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog", { name: "Propriétés du préfixe" })).toBeVisible();
    // The middle of each toast is the dialog's scrim, not the toast.
    for (const toast of await page.locator(".k-toast").all())
      expect(await underPointer(toast)).toBe(false);
    await page.waitForTimeout(5000);
    await page.keyboard.press("Escape");
    await expect(button).toBeFocused();
    await page.mouse.move(0, 0);
    const ok = page.locator("[role='status'] .k-toast");
    await expect(ok).toHaveCount(1);
    await expect(ok).toHaveCount(0, { timeout: 8000 });
  });
});

/** Whether the middle of `locator` is itself (or inside it) for the pointer. */
function underPointer(locator: import("@playwright/test").Locator): Promise<boolean> {
  return locator.evaluate((el) => {
    const box = el.getBoundingClientRect();
    const hit = document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2);
    return hit !== null && el.contains(hit);
  });
}

test.describe("Toast in the frame", () => {
  test("under the compact panel's sheet", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 900 });
    await page.goto("/_ui/ecrans/E9?vue=revue");
    const toast = page.locator(".k-toast");
    await expect(toast).toHaveCount(1);
    expect(await underPointer(toast)).toBe(true);
    await page.locator(".k-draft-counter__toggle").click();
    await expect(page.getByRole("dialog", { name: "Panneau droit" })).toBeVisible();
    expect(await underPointer(toast)).toBe(false);
  });

  test("come in from outside the page, dismissed, the focus goes to the main content", async ({
    page,
  }) => {
    await page.goto("/_ui/ecrans/E9?vue=revue");
    await page.locator(".k-toast").waitFor();
    // From the end of the page backwards: the toast's close button is the last stop.
    await page.keyboard.press("Shift+Tab");
    const close = page.getByRole("button", { name: "Fermer la notification" });
    await expect(close).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator(".k-toast")).toHaveCount(0);
    await expect(page.locator("main#k-ecran-surface")).toBeFocused();
  });
});
