import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard and pointer script of the APG Tooltip pattern (spec 020, design
// « Accessibilité »; REQ-020-18, 28): the tooltip opens on keyboard focus at once and on
// hover after a delay, and WCAG 1.4.13 holds: dismissible (Escape, without moving the
// pointer nor the focus, and nothing else closes), hoverable (the pointer may go over the
// bubble), persistent (open until the focus or the pointer leaves, or Escape).

/** Any open bubble, decorative ones included (they are out of the accessibility tree). */
const bubble = (page: Page) => page.locator(".k-tooltip:not([hidden])");

test.describe("Tooltip of a refused action (described by it)", () => {
  test("keyboard focus opens it at once; Escape closes it, the focus stays; blur closes it", async ({
    page,
  }) => {
    await openState(page, "Tooltip", "forbidden");
    const edit = page.getByRole("button", { name: "Modifier" });
    await edit.focus();
    const tip = page.getByRole("tooltip");
    await expect(tip).toHaveText("Il faut prefix.edit");
    await expect(edit).toHaveAccessibleDescription("Il faut prefix.edit");
    await page.keyboard.press("Escape");
    await expect(tip).toBeHidden();
    await expect(edit).toBeFocused();
    // Still described, closed: the bubble stays in the document, hidden.
    await expect(edit).toHaveAccessibleDescription("Il faut prefix.edit");
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Tab");
    await expect(tip).toHaveText("Il faut prefix.edit");
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Copier" })).toBeFocused();
    await expect(tip).toBeHidden();
  });

  test("hover opens it after a delay; it stays over itself, closes on leaving", async ({
    page,
  }) => {
    await openState(page, "Tooltip", "forbidden");
    await page.getByRole("button", { name: "Modifier" }).hover();
    const tip = page.getByRole("tooltip");
    await expect(tip).toHaveText("Il faut prefix.edit");
    await tip.hover();
    await page.waitForTimeout(300);
    await expect(tip).toBeVisible();
    await page.mouse.move(0, 0);
    await expect(tip).toBeHidden();
  });

  test("opened by hover, Escape closes it without moving the pointer nor the focus", async ({
    page,
  }) => {
    await openState(page, "Tooltip", "forbidden");
    const copy = page.getByRole("button", { name: "Copier" });
    await copy.focus();
    await page.getByRole("button", { name: "Modifier" }).hover();
    const tip = page.getByRole("tooltip");
    await expect(tip).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(tip).toBeHidden();
    await expect(copy).toBeFocused();
    // Persistent until dismissed: the pointer still on the anchor does not reopen it.
    await page.waitForTimeout(500);
    await expect(tip).toBeHidden();
  });

  test("opened by the focus, the pointer leaving the anchor does not close it", async ({
    page,
  }) => {
    await openState(page, "Tooltip", "forbidden");
    const edit = page.getByRole("button", { name: "Modifier" });
    await edit.focus();
    const tip = page.getByRole("tooltip");
    await expect(tip).toBeVisible();
    await edit.hover();
    await page.mouse.move(0, 0);
    await page.waitForTimeout(300);
    await expect(tip).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(tip).toBeHidden();
  });

  test("an icon button refused: its name, then the reason, which alone describes it", async ({
    page,
  }) => {
    await openState(page, "Tooltip", "icon");
    const trash = page.getByRole("button", { name: "Supprimer" });
    await trash.focus();
    await expect(bubble(page)).toHaveText(/^Supprimer\s*Il faut prefix\.delete$/);
    await expect(trash).toHaveAccessibleDescription("Il faut prefix.delete");
    await page.keyboard.press("Enter");
    await expect(bubble(page)).toBeVisible();
  });
});

test.describe("Tooltip of an icon button (its name, decorative)", () => {
  test("its name on keyboard focus, out of the tree and of the description", async ({ page }) => {
    await openState(page, "IconButton", "focus");
    const edit = page.getByRole("button", { name: "Modifier" });
    await edit.focus();
    await expect(bubble(page)).toHaveText("Modifier");
    await expect(page.getByRole("tooltip")).toHaveCount(0);
    await expect(edit).toHaveAccessibleDescription("");
    await page.keyboard.press("Escape");
    await expect(bubble(page)).toHaveCount(0);
    await expect(edit).toBeFocused();
  });

  test("its name on hover; a click closes it, and the focus of a click opens none", async ({
    page,
  }) => {
    await openState(page, "IconButton", "hover");
    const edit = page.getByRole("button", { name: "Modifier" });
    await edit.hover();
    await expect(bubble(page)).toHaveText("Modifier");
    await edit.click();
    await expect(bubble(page)).toHaveCount(0);
    await expect(edit).toBeFocused();
    await page.waitForTimeout(500);
    await expect(bubble(page)).toHaveCount(0);
  });

  test("its shortcut beside its name, and in aria-keyshortcuts", async ({ page }) => {
    await openState(page, "IconButton", "shortcut");
    const fold = page.getByRole("button", { name: "Replier la barre latérale" });
    await expect(fold).toHaveAttribute("aria-keyshortcuts", "[");
    await fold.focus();
    await expect(bubble(page)).toHaveText(/^Replier la barre latérale\s*\[$/);
    await expect(bubble(page).locator("kbd").first()).toBeVisible();
  });

  test("none over the open menu of its button", async ({ page }) => {
    await openState(page, "Menu", "icon-only");
    const trigger = page.getByRole("button", { name: "Actions sur le préfixe" });
    await trigger.hover();
    await expect(bubble(page)).toHaveText("Actions sur le préfixe");
    await trigger.click();
    await expect(page.getByRole("menu")).toBeVisible();
    await expect(bubble(page)).toHaveCount(0);
  });

  test("none over the menu opened by the keyboard", async ({ page }) => {
    await openState(page, "Menu", "icon-only");
    const trigger = page.getByRole("button", { name: "Actions sur le préfixe" });
    await trigger.focus();
    await expect(bubble(page)).toHaveText("Actions sur le préfixe");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menu")).toBeVisible();
    await expect(bubble(page)).toHaveCount(0);
  });

  test("in a dialog: under the pointer, Escape closes the tooltip alone", async ({ page }) => {
    await openState(page, "ExpandableGrid", "compact");
    const enlarge = page.locator('button[aria-label="Agrandir : Flux non conformes"]');
    await enlarge.focus();
    await expect(bubble(page)).toHaveText("Agrandir : Flux non conformes");
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Flux non conformes" });
    await expect(dialog).toBeVisible();
    const close = dialog.getByRole("button", { name: "Fermer" });
    await dialog.getByRole("columnheader", { name: "Source" }).focus();
    await close.hover();
    await expect(bubble(page)).toHaveText("Fermer");
    await page.keyboard.press("Escape");
    await expect(bubble(page)).toHaveCount(0);
    await expect(dialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(enlarge).toBeFocused();
  });

  test("in a dialog: under the focus, a name tooltip leaves Escape to the dialog", async ({
    page,
  }) => {
    await openState(page, "ExpandableGrid", "compact");
    const enlarge = page.locator('button[aria-label="Agrandir : Flux non conformes"]');
    await enlarge.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Flux non conformes" });
    const close = dialog.getByRole("button", { name: "Fermer" });
    await close.focus();
    await expect(bubble(page)).toHaveText("Fermer");
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(enlarge).toBeFocused();
  });
});
