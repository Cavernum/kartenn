import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the draft counter (APG Button as a toggle, and Menu Button; spec 020,
// design « Inventaire » line 27; spec 021, « Cadre »; 024/M8, which delivered it): its name
// says what its figure counts; Enter and Space toggle the panel (`aria-expanded`); its
// chevron opens the drafts of the user, the active one the
// checked entry of a set (`menuitemradio`), the arrows go round, Escape and a choice give
// the focus back to the chevron. Keys only.

const toggle = (page: Page) =>
  page.getByRole("button", { name: /^Brouillon\s*\d+\s*opérations?$/ });
const chevron = (page: Page) => page.getByRole("button", { name: "Liste des brouillons" });

test.describe("DraftCounter (toggle + Menu Button)", () => {
  test("named by what it counts; Enter and Space toggle the panel", async ({ page }) => {
    await openState(page, "DraftCounter", "drafts");
    await expect(toggle(page)).toHaveAccessibleName(/^Brouillon\s*3\s*opérations$/);
    await expect(toggle(page)).toHaveAttribute("aria-expanded", "false");
    await toggle(page).focus();
    await page.keyboard.press("Enter");
    await expect(toggle(page)).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("Space");
    await expect(toggle(page)).toHaveAttribute("aria-expanded", "false");
    await expect(toggle(page)).toBeFocused();
  });

  test("at zero, still named and reachable", async ({ page }) => {
    await openState(page, "DraftCounter", "empty");
    await expect(toggle(page)).toHaveAccessibleName(/^Brouillon\s*0\s*opération$/);
    await toggle(page).focus();
    await page.keyboard.press("Enter");
    await expect(toggle(page)).toHaveAttribute("aria-expanded", "true");
  });

  test("the chevron lists the drafts, the active one checked; Escape and a choice", async ({
    page,
  }) => {
    await openState(page, "DraftCounter", "drafts");
    await toggle(page).focus();
    await page.keyboard.press("Tab");
    await expect(chevron(page)).toBeFocused();
    await expect(chevron(page)).toHaveAttribute("aria-haspopup", "menu");
    await page.keyboard.press("Enter");
    const radios = page.getByRole("menuitemradio");
    await expect(radios).toHaveText([
      "Salle B (3)",
      "Reprise des VIP (12)",
      "Brouillon du 14 janvier (1)",
    ]);
    await expect(radios.nth(0)).toHaveAttribute("aria-checked", "true");
    await expect(radios.nth(1)).toHaveAttribute("aria-checked", "false");
    await expect(page.getByRole("group").getByRole("menuitemradio")).toHaveCount(3);
    await expect(radios.nth(0)).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(radios.nth(2)).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(radios.nth(0)).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("menu")).toHaveCount(0);
    await expect(chevron(page)).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await expect(radios.nth(1)).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menu")).toHaveCount(0);
    await expect(chevron(page)).toBeFocused();
  });
});
