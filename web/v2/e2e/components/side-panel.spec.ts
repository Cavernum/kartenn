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

// Whose Escape (task 021-12p; `ownsEscape`): an open popup keeps it, nothing else does. In
// a reference field (`Combobox`), Escape closes the list and the second closes the panel,
// as its header promises; in a text field, the first closes the panel, as in a dialog. The
// focus goes back to what opened the panel, else to the strip.

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

  test("reopened by the strip: from either field, Escape closes it, the focus to the strip", async ({
    page,
  }) => {
    await openState(page, "SidePanel", "fields");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    const reopen = page.getByRole("button", { name: "Ouvrir le panneau" });
    await panel.getByRole("button", { name: "Fermer le panneau" }).click();
    await expect(reopen).toBeFocused();
    for (const name of ["Domaine", "Libellé"]) {
      await page.keyboard.press("Enter");
      await expect(panel.getByRole("tab", { name: "Détail" })).toBeFocused();
      const field = panel.getByRole(name === "Domaine" ? "combobox" : "textbox", { name });
      for (let i = 0; i < 4 && !(await field.evaluate((el) => el === document.activeElement)); i++)
        await page.keyboard.press("Tab");
      await expect(field).toBeFocused();
      // Typed in, the text stays its own: Escape is still the panel's.
      await page.keyboard.type("pro");
      if (name === "Domaine") {
        await expect(field).toHaveAttribute("aria-expanded", "true");
        await page.keyboard.press("Escape");
        await expect(field).toHaveAttribute("aria-expanded", "false");
      }
      await page.keyboard.press("Escape");
      await expect(panel).toBeHidden();
      await expect(reopen).toBeFocused();
    }
  });
});
