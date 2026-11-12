import { expect, openState, test } from "../fixtures/index.ts";

// The host's shortcut of the right panel (`]` in the shell, spec 021 « Raccourcis »; spec
// 020, design « Inventaire » line 30): said on the cross and on the strip's button, in their
// `aria-keyshortcuts` and their tooltip; none without it.

test.describe("SidePanel, the host's shortcut", () => {
  test("the strip's button and the cross declare `]`", async ({ page }) => {
    await openState(page, "SidePanel", "rail-shortcut");
    const reopen = page.getByRole("button", { name: "Ouvrir le panneau" });
    await expect(reopen).toHaveAttribute("aria-keyshortcuts", "]");
    await reopen.click();
    const close = page.getByRole("button", { name: "Fermer le panneau" });
    await expect(close).toHaveAttribute("aria-keyshortcuts", "]");
  });

  test("without a shortcut, neither declares one", async ({ page }) => {
    await openState(page, "SidePanel", "closed");
    const reopen = page.getByRole("button", { name: "Ouvrir le panneau" });
    await expect(reopen).not.toHaveAttribute("aria-keyshortcuts", /./);
  });
});

// Whose Escape (task 021-12p; `ownsEscape`): an open popup keeps it, and a text field,
// whose input would go with the panel (until the panel asks first, Q-021-22, task 021-12q).
// In a reference field (`Combobox`), Escape closes the list and the second closes the
// panel, as its header promises. The focus goes back to what opened the panel, else to
// the strip.

test.describe("SidePanel, whose Escape", () => {
  test("a reference field: the list first, then the panel, the focus to the strip", async ({
    page,
  }) => {
    await openState(page, "SidePanel", "fields");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    const field = panel.getByRole("combobox", { name: "Domaine" });
    await field.focus();
    await page.keyboard.press("Alt+ArrowDown");
    await expect(page.getByRole("listbox", { name: "Domaine" }).getByRole("option")).toHaveCount(4);
    await expect(field).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("Escape");
    await expect(field).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(panel).toBeVisible();
    await expect(field).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(page.getByRole("button", { name: "Ouvrir le panneau" })).toBeFocused();
  });

  test("reopened by the strip: the text field keeps Escape, the typed reference does not", async ({
    page,
  }) => {
    await openState(page, "SidePanel", "fields");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    const reopen = page.getByRole("button", { name: "Ouvrir le panneau" });
    const domain = panel.getByRole("combobox", { name: "Domaine" });
    const label = panel.getByRole("textbox", { name: "Libellé" });
    await panel.getByRole("button", { name: "Fermer le panneau" }).click();
    await expect(reopen).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(panel.getByRole("tab", { name: "Détail" })).toBeFocused();
    // The tab, its panel, then the fields.
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(domain).toBeFocused();
    await page.keyboard.type("pro");
    await expect(domain).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("Escape");
    await expect(domain).toHaveAttribute("aria-expanded", "false");
    await page.keyboard.press("Tab");
    await expect(label).toBeFocused();
    // Tab selects the text: End, then the typing goes after it.
    await page.keyboard.press("End");
    await page.keyboard.type(" B");
    await page.keyboard.press("Escape");
    await expect(panel).toBeVisible();
    await expect(label).toBeFocused();
    await expect(label).toHaveValue("Supervision B");
    await page.keyboard.press("Shift+Tab");
    await expect(domain).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(reopen).toBeFocused();
  });
});
