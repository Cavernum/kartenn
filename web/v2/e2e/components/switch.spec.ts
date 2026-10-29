import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Switch pattern (spec 020, design « Inventaire » line 9,
// « Accessibilité »; REQ-020-16, 18, 28): a switch is named by its label, which does not
// change with its state; Space and Enter toggle `aria-checked`, the setting taking effect
// at once; a disabled switch is out of the tabs. The targets are 24 px at least.

test.describe("Switch (APG Switch)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Switch", "keyboard");
  });

  test("Space and Enter toggle it, its name unchanged, its effect said", async ({ page }) => {
    const reserved = page.getByRole("switch", { name: "Montrer les adresses réservées" });
    await expect(reserved).toHaveAttribute("aria-checked", "false");
    await reserved.focus();
    await page.keyboard.press(" ");
    await expect(reserved).toHaveAttribute("aria-checked", "true");
    await expect(page.getByRole("status")).toHaveText("Adresses réservées montrées");
    await expect(reserved).toHaveAccessibleName("Montrer les adresses réservées");
    await page.keyboard.press("Enter");
    await expect(reserved).toHaveAttribute("aria-checked", "false");
    await expect(page.getByRole("status")).toHaveText("Adresses réservées cachées");
    await expect(reserved).toBeFocused();
  });

  test("a click on its label toggles it once", async ({ page }) => {
    await page.getByText("Montrer les adresses réservées").click();
    await expect(page.getByRole("switch", { name: /réservées/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  test("the disabled one is out of the tabs", async ({ page }) => {
    const reserved = page.getByRole("switch", { name: "Montrer les adresses réservées" });
    await reserved.focus();
    await page.keyboard.press("Tab");
    await expect(reserved).not.toBeFocused();
    await expect(page.getByRole("switch", { name: "Montrer les préfixes vides" })).toBeDisabled();
  });

  test("the snapshot of its tree", async ({ page }) => {
    await expect(page.locator(".k-cat-state__render")).toMatchAriaSnapshot(`
      - switch "Montrer les adresses réservées"
      - switch "Montrer les préfixes vides" [disabled]
      - status: Adresses réservées cachées
    `);
  });
});

test("Switch: the help describes it, outside its name", async ({ page }) => {
  await openState(page, "Switch", "hint");
  const empty = page.getByRole("switch", { name: "Montrer les préfixes vides" });
  await expect(empty).toHaveAccessibleDescription("Un préfixe sans adresse ni plage.");
});

for (const densite of ["compacte", "confortable"] as const) {
  test(`Switch: targets of 24 × 24 px at least, ${densite}`, async ({ page }) => {
    await page.goto(`/_ui/c/Switch?densite=${densite}`);
    await page.locator("[data-state-name]").first().waitFor();
    const small = await page.evaluate(() =>
      [...document.querySelectorAll(".k-cat-state__render .k-switch__hit")]
        .map((el) => ({ el, r: el.getBoundingClientRect() }))
        .filter(({ r }) => r.width < 23.5 || r.height < 23.5)
        .map(
          ({ el, r }) =>
            `${el.textContent?.trim()} ${Math.round(r.width)} × ${Math.round(r.height)}`,
        ),
    );
    expect(small).toEqual([]);
  });
}
