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
