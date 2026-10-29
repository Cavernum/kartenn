import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Button pattern (spec 020, design « Accessibilité »;
// REQ-020-18, 28): Button and IconButton on their catalogue page, driven by the keyboard
// alone, their ARIA states read back; and their targets of 24 px at least (WCAG 2.5.8).

const status = (page: Page) => page.getByRole("status");

test.describe("Button (APG Button)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Button", "keyboard");
  });

  test("Tab reaches each button, the disabled one left out of the order", async ({ page }) => {
    const order: string[] = [];
    await page.getByRole("button", { name: "Enregistrer" }).focus();
    for (let i = 0; i < 5; i++) {
      order.push(
        await page.evaluate(
          () =>
            document.activeElement?.getAttribute("aria-label") ??
            document.activeElement?.textContent?.trim() ??
            "",
        ),
      );
      await page.keyboard.press("Tab");
    }
    expect(order).toEqual([
      "Enregistrer",
      "Filtres",
      "Recharger (Chargement)",
      expect.stringMatching(/^Modifier/),
      "Copier",
    ]);
    await expect(page.getByRole("button", { name: "Supprimer" })).toBeDisabled();
  });

  test("Enter and Space activate it", async ({ page }) => {
    const save = page.getByRole("button", { name: "Enregistrer" });
    await save.focus();
    await page.keyboard.press("Enter");
    await expect(status(page)).toHaveText("Activé : Enregistrer (1)");
    await page.keyboard.press(" ");
    await expect(status(page)).toHaveText("Activé : Enregistrer (2)");
    await expect(save).toBeFocused();
  });

  test("a toggle flips aria-pressed, by Space and by Enter", async ({ page }) => {
    const filters = page.getByRole("button", { name: "Filtres" });
    await expect(filters).toHaveAttribute("aria-pressed", "false");
    await filters.focus();
    await page.keyboard.press(" ");
    await expect(filters).toHaveAttribute("aria-pressed", "true");
    await expect(status(page)).toHaveText("Activé : Filtres montrés");
    await page.keyboard.press("Enter");
    await expect(filters).toHaveAttribute("aria-pressed", "false");
  });

  test("busy, it keeps the focus, says so, and runs nothing", async ({ page }) => {
    const reload = page.getByRole("button", { name: "Recharger (Chargement)" });
    await expect(reload).toHaveAttribute("aria-busy", "true");
    await expect(reload).toHaveAttribute("aria-disabled", "true");
    await reload.focus();
    await expect(reload).toBeFocused();
    await page.keyboard.press("Enter");
    await page.keyboard.press(" ");
    await expect(status(page)).toHaveText("");
  });

  test("refused by the rights, it is focusable, says the permission and runs nothing", async ({
    page,
  }) => {
    const edit = page.getByRole("button", { name: "Modifier" });
    await expect(edit).toHaveAttribute("aria-disabled", "true");
    await expect(edit).toHaveAccessibleDescription("Il faut prefix.edit");
    await edit.focus();
    await expect(page.getByRole("tooltip")).toHaveText("Il faut prefix.edit");
    await page.keyboard.press("Enter");
    await page.keyboard.press(" ");
    await expect(status(page)).toHaveText("");
    await expect(page.getByRole("tooltip")).toHaveText("Il faut prefix.edit");
  });

  test("an icon button is named by its label and activates as a button", async ({ page }) => {
    const copy = page.getByRole("button", { name: "Copier", exact: true });
    await copy.focus();
    await page.keyboard.press("Enter");
    await expect(status(page)).toHaveText("Activé : Copier");
  });

  test("the snapshot of its tree", async ({ page }) => {
    await expect(page.locator(".k-cat-state__render")).toMatchAriaSnapshot(`
      - button "Enregistrer"
      - button "Filtres"
      - button "Recharger (Chargement)"
      - button "Supprimer"
      - button "Modifier"
      - button "Copier"
      - status
    `);
  });
});

test.describe("Button: shortcut", () => {
  test("its shortcut is shown, hidden from its name, and given by aria-keyshortcuts", async ({
    page,
  }) => {
    await openState(page, "Button", "shortcut");
    const apply = page.getByRole("button", { name: "Appliquer", exact: true });
    await expect(apply).toHaveAttribute("aria-keyshortcuts", "Control+Enter");
    await expect(apply.locator("kbd").first()).toHaveText("CtrlEntrée");
  });
});

/** Every control of the page under 24 × 24 px, by name and size. */
function smallTargets(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    [...document.querySelectorAll(".k-cat-state__render .k-button")]
      .map((el) => ({ el, r: el.getBoundingClientRect() }))
      .filter(({ r }) => r.width < 23.5 || r.height < 23.5)
      .map(
        ({ el, r }) =>
          `${el.getAttribute("aria-label") ?? el.textContent?.trim()} ${Math.round(r.width)} × ${Math.round(r.height)}`,
      ),
  );
}

// WCAG 2.5.8: the smallest size (`sm`, 24 px in the compact density) is the floor.
for (const component of ["Button", "IconButton"] as const) {
  for (const densite of ["compacte", "confortable"] as const) {
    test(`${component}: targets of 24 × 24 px at least, ${densite}`, async ({ page }) => {
      await page.goto(`/_ui/c/${component}?densite=${densite}`);
      await page.locator("[data-state-name]").first().waitFor();
      expect(await page.locator(".k-cat-state__render .k-button").count()).toBeGreaterThan(10);
      expect(await smallTargets(page)).toEqual([]);
    });
  }
}
