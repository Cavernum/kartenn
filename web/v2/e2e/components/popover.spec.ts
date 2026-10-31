import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the popover (a non-modal Dialog; spec 020, design « Inventaire »
// line 16, « Accessibilité »; REQ-020-16, 18, 28), on Reka's Popover: the button says
// `aria-haspopup="dialog"` and `aria-expanded`; open, the dialog is named by its title (a
// heading) and has the focus; its controls come by Tab; Escape and the
// close button close it, the focus back on the button; Shift+Tab from its start goes back
// to the button, Tab past its end to what follows the button, and it closes. Nothing
// around it is hidden.

test.describe("Popover (non-modal Dialog)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Popover", "keyboard");
  });

  test("closed, the button says it opens a dialog", async ({ page }) => {
    const button = page.getByRole("button", { name: "Héritée" });
    await expect(button).toHaveAttribute("aria-haspopup", "dialog");
    await expect(button).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("dialog")).toHaveCount(0);
  });

  test("Enter opens it, named by its title, with the focus; Escape gives it back", async ({
    page,
  }) => {
    const button = page.getByRole("button", { name: "Héritée" });
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Propriétaire : valeur héritée" });
    await expect(dialog).toBeFocused();
    // Its title is a heading (browse mode); no description: its body is read from it.
    await expect(
      dialog.getByRole("heading", { name: "Propriétaire : valeur héritée" }),
    ).toBeVisible();
    await expect(dialog).not.toHaveAttribute("aria-describedby");
    await expect(button).toHaveAttribute("aria-expanded", "true");
    // Not modal: the rest of the page stays in the tree.
    await expect(page.getByRole("textbox", { name: "Préfixe" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(button).toBeFocused();
  });

  test("Tab goes through it, then past its end to what follows the button", async ({ page }) => {
    const button = page.getByRole("button", { name: "Héritée" });
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Propriétaire : valeur héritée" });
    await expect(dialog).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: /^Copier 01JD4K8B2C/ })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page.getByRole("textbox", { name: "Description" })).toBeFocused();
  });

  test("Shift+Tab from its start goes back to the button; the close button closes", async ({
    page,
  }) => {
    const button = page.getByRole("button", { name: "Héritée" });
    await button.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog")).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(button).toBeFocused();
    await page.keyboard.press(" ");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(button).toBeFocused();
  });

  test("Shift+Tab from « Fermer » goes back to the button", async ({ page }) => {
    const button = page.getByRole("button", { name: "Héritée" });
    await button.focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("Tab");
    await expect(page.getByRole("dialog").getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(button).toBeFocused();
  });

  test("a press outside closes it and leaves the focus where it went", async ({ page }) => {
    const button = page.getByRole("button", { name: "Héritée" });
    await button.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog")).toBeFocused();
    // The field above the button: the popover opens below, over what follows it.
    const field = page.getByRole("textbox", { name: "Préfixe" });
    await field.click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(field).toBeFocused();
    await expect(button).not.toBeFocused();
  });
});

test.describe("Popover, edges of the tab order", () => {
  test("nothing after the button: Tab past the end comes back to it", async ({ page }) => {
    await openState(page, "Popover", "closed");
    const button = page.getByRole("button", { name: "Héritée" });
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Propriétaire : valeur héritée" });
    await expect(dialog).toBeFocused();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: /^Copier/ })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(button).toBeFocused();
  });

  test("at 400 % zoom (320 × 256), a body that scrolls is a tab stop", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 256 });
    await openState(page, "Popover", "long");
    const name = "Propriétaire : historique de la valeur";
    const dialog = page.getByRole("dialog", { name });
    await expect(dialog).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(dialog.getByRole("button", { name: "Fermer" })).toBeFocused();
    await page.keyboard.press("Tab");
    const body = dialog.getByRole("group", { name });
    await expect(body).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => body.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
  });

  test("in a modal dialog, Tab past its end stays in the dialog", async ({ page }) => {
    await openState(page, "Popover", "in-dialog");
    const modal = page.getByRole("dialog", { name: "Propriétés du préfixe" });
    const button = modal.getByRole("button", { name: "Héritée" });
    await button.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Propriétaire : valeur héritée" });
    await expect(dialog).toBeFocused();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(dialog).toHaveCount(0);
    await expect(modal.getByRole("button", { name: "Actions" })).toBeFocused();
  });
});
