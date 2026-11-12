import { expect, openState, shiftTabTo, test } from "../fixtures/index.ts";

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

// Whose Escape (tasks 021-12p, 021-12qa; `ownsEscape`, `usePanelClose`): an open popup or a
// composition keeps it; anything else lets it go to the panel, a text field too. In a
// reference field (`Combobox`), Escape closes the list and the second goes to the panel,
// as its header promises. Intact, the panel closes at once; modified (the catalogue's host
// says `modified`), Escape, the cross or « Retour » ask « Abandonner les modifications ? »
// first (Q-021-22): the focus on « Continuer la saisie », which gives it back to the field
// or the button, the input kept; « Abandonner » closes the panel, the focus back to what
// opened it, else to the strip.

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
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
    await expect(panel).toBeHidden();
    await expect(page.getByRole("button", { name: "Ouvrir le panneau" })).toBeFocused();
  });

  test("a closed list in a cell: Escape closes the list, then cancels the edit; the panel stays", async ({
    page,
  }) => {
    await openState(page, "SidePanel", "cell-choice");
    const panel = page.locator("#k-demo-panel");
    const cell = panel.locator('[data-row="192.0.2.10"][data-column="usage"]');
    const list = page.getByRole("listbox", { name: "Usage — 192.0.2.10" });
    // Reopened by its strip, the keyboard alone: the tab, its panel, the grid's header,
    // then the cell of the usage.
    const reopen = page.getByRole("button", { name: "Ouvrir le panneau" });
    await panel.getByRole("button", { name: "Fermer le panneau" }).click();
    await expect(reopen).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(panel.getByRole("tab", { name: "Détail" })).toBeFocused();
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowRight");
    await expect(cell).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(list.getByRole("option", { name: "Statique" })).toBeFocused();
    // The first Escape is the list's; the second, the editor's: the panel stays open.
    await page.keyboard.press("Escape");
    await expect(list).toHaveCount(0);
    await expect(cell.locator(".k-cell-select")).toBeFocused();
    await expect(panel).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(cell).toBeFocused();
    await expect(cell).not.toHaveAttribute("data-editing");
    await expect(panel).toBeVisible();
    // From the cell, Escape is the panel's: it closes, the focus to its strip.
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(reopen).toBeFocused();
  });

  test("intact, Escape from the text field closes at once", async ({ page }) => {
    await openState(page, "SidePanel", "fields");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    const reopen = page.getByRole("button", { name: "Ouvrir le panneau" });
    await panel.getByRole("button", { name: "Fermer le panneau" }).click();
    await expect(reopen).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(panel.getByRole("tab", { name: "Détail" })).toBeFocused();
    // The tab, its panel, then the fields.
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(panel.getByRole("textbox", { name: "Libellé" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
    await expect(panel).toBeHidden();
    await expect(reopen).toBeFocused();
  });

  test("modified: Escape asks, « Continuer la saisie » keeps the input and the focus, « Abandonner » closes", async ({
    page,
  }) => {
    await openState(page, "SidePanel", "fields");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    const reopen = page.getByRole("button", { name: "Ouvrir le panneau" });
    const label = panel.getByRole("textbox", { name: "Libellé" });
    const ask = page.getByRole("alertdialog", { name: "Abandonner les modifications ?" });
    const keep = ask.getByRole("button", { name: "Continuer la saisie" });
    await panel.getByRole("button", { name: "Fermer le panneau" }).click();
    await page.keyboard.press("Enter");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(label).toBeFocused();
    // Tab selects the text: End, then the typing goes after it.
    await page.keyboard.press("End");
    await page.keyboard.type(" B");
    await page.keyboard.press("Escape");
    await expect(ask).toBeVisible();
    await expect(ask).toHaveAccessibleDescription(
      "Les modifications de ce panneau seront perdues.",
    );
    await expect(keep).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(ask).toBeHidden();
    await expect(panel).toBeVisible();
    await expect(label).toBeFocused();
    await expect(label).toHaveValue("Supervision B");
    // Escape in the question is « Continuer la saisie ».
    await page.keyboard.press("Escape");
    await expect(ask).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(ask).toBeHidden();
    await expect(label).toBeFocused();
    await expect(label).toHaveValue("Supervision B");
    // « Abandonner »: closed, the focus to what opened it (the strip), the input gone.
    await page.keyboard.press("Escape");
    await expect(keep).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(ask.getByRole("button", { name: "Abandonner" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(ask).toBeHidden();
    await expect(panel).toBeHidden();
    await expect(reopen).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(label).toHaveValue("Supervision");
  });

  test("modified, a held Escape asks once and does not answer", async ({ page }) => {
    await openState(page, "SidePanel", "fields");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    const label = panel.getByRole("textbox", { name: "Libellé" });
    const ask = page.getByRole("alertdialog", { name: "Abandonner les modifications ?" });
    await label.focus();
    await page.keyboard.press("End");
    await page.keyboard.type(" B");
    // Held: the second keydown repeats.
    await page.keyboard.down("Escape");
    await expect(ask).toBeVisible();
    await page.keyboard.down("Escape");
    await page.keyboard.down("Escape");
    await page.keyboard.up("Escape");
    await expect(ask).toBeVisible();
    await expect(ask.getByRole("button", { name: "Continuer la saisie" })).toBeFocused();
    await expect(panel).toBeVisible();
  });

  test("modified by the reference: its second Escape asks; the cross asks too, and gets the focus back", async ({
    page,
  }) => {
    await openState(page, "SidePanel", "fields");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    const domain = panel.getByRole("combobox", { name: "Domaine" });
    const close = panel.getByRole("button", { name: "Fermer le panneau" });
    const ask = page.getByRole("alertdialog", { name: "Abandonner les modifications ?" });
    const keep = ask.getByRole("button", { name: "Continuer la saisie" });
    await domain.focus();
    await page.keyboard.type("pro");
    await expect(domain).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("Escape");
    await expect(domain).toHaveAttribute("aria-expanded", "false");
    await expect(ask).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(keep).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(domain).toBeFocused();
    await expect(domain).toHaveValue("pro");
    // The cross: asked first as well, « Continuer la saisie » gives the focus back to it.
    await close.focus();
    await page.keyboard.press("Enter");
    await expect(keep).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(ask).toBeHidden();
    await expect(panel).toBeVisible();
    await expect(close).toBeFocused();
  });

  test("a sheet, modified: « Retour » asks, then closes on « Abandonner »", async ({ page }) => {
    await openState(page, "SidePanel", "sheet");
    const panel = page.getByRole("dialog", { name: "Panneau droit" });
    const summary = panel.getByRole("textbox", { name: /Résumé/ });
    const back = panel.getByRole("button", { name: "Retour" });
    const ask = page.getByRole("alertdialog", { name: "Abandonner les modifications ?" });
    await summary.focus();
    await page.keyboard.press("End");
    await page.keyboard.type(" B");
    await shiftTabTo(page, back);
    await page.keyboard.press("Enter");
    await expect(ask.getByRole("button", { name: "Continuer la saisie" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(back).toBeFocused();
    await expect(summary).toHaveValue("Maintenance du référentiel B");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Tab");
    await expect(ask.getByRole("button", { name: "Abandonner" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(ask).toBeHidden();
    await expect(panel).toBeHidden();
  });
});
