import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Checkbox pattern, two and three states (spec 020, design
// « Inventaire » line 8, « Accessibilité »; REQ-020-16, 18, 28): Space toggles a box; the
// box of all the rows is mixed while some only are chosen, checks them all when toggled,
// then none; a disabled box is out of the tabs. The targets are 24 px at least.

const all = (page: Page) => page.getByRole("checkbox", { name: "Tout sélectionner (3)" });
const row = (page: Page, address: string) =>
  page.getByRole("checkbox", { name: `Sélectionner ${address}` });

test.describe("Checkbox (APG Checkbox)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Checkbox", "keyboard");
  });

  test("Space toggles a box, the focus stays; Enter does nothing", async ({ page }) => {
    const second = row(page, "192.0.2.12");
    await second.focus();
    await page.keyboard.press(" ");
    await expect(second).toBeChecked();
    await expect(second).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(second).toBeChecked();
    await page.keyboard.press(" ");
    await expect(second).not.toBeChecked();
  });

  test("three states: mixed while some rows are chosen, all, then none", async ({ page }) => {
    await expect(all(page)).toBeChecked({ indeterminate: true });
    await all(page).focus();
    await page.keyboard.press(" ");
    await expect(all(page)).toBeChecked();
    for (const address of ["192.0.2.10", "192.0.2.12", "192.0.2.72"])
      await expect(row(page, address)).toBeChecked();
    await page.keyboard.press(" ");
    await expect(all(page)).not.toBeChecked();
    await expect(page.getByRole("checkbox", { checked: true })).toHaveCount(0);
    // One row chosen again by the keyboard: the box of all goes back to mixed.
    await page.keyboard.press("Tab");
    await expect(row(page, "192.0.2.10")).toBeFocused();
    await page.keyboard.press(" ");
    await expect(all(page)).toBeChecked({ indeterminate: true });
  });

  test("a click on the label toggles its box", async ({ page }) => {
    await page.getByText("Sélectionner 192.0.2.72").click();
    await expect(row(page, "192.0.2.72")).toBeChecked();
    await expect(all(page)).toBeChecked({ indeterminate: true });
  });

  test("Tab goes from box to box, the disabled one left out", async ({ page }) => {
    await all(page).focus();
    for (const address of ["192.0.2.10", "192.0.2.12", "192.0.2.72"]) {
      await page.keyboard.press("Tab");
      await expect(row(page, address)).toBeFocused();
    }
    await expect(page.getByRole("checkbox", { name: "Anycast" })).toBeDisabled();
  });

  test("the snapshot of its tree", async ({ page }) => {
    await expect(page.locator(".k-cat-state__render")).toMatchAriaSnapshot(`
      - checkbox "Tout sélectionner (3)" [checked=mixed]
      - checkbox "Sélectionner 192.0.2.10" [checked]
      - checkbox "Sélectionner 192.0.2.12"
      - checkbox "Sélectionner 192.0.2.72"
      - checkbox "Anycast" [disabled]
    `);
  });
});

test("Checkbox: the help describes the box, outside its name", async ({ page }) => {
  await openState(page, "Checkbox", "hint");
  const box = page.getByRole("checkbox", { name: "Anycast", exact: true });
  await expect(box).toHaveAccessibleDescription("La même adresse annoncée en plusieurs lieux.");
});

// WCAG 2.5.8: the target of a box is its label row, 24 × 24 px at least (a hidden label too).
for (const densite of ["compacte", "confortable"] as const) {
  test(`Checkbox: targets of 24 × 24 px at least, ${densite}`, async ({ page }) => {
    await page.goto(`/_ui/c/Checkbox?densite=${densite}`);
    await page.locator("[data-state-name]").first().waitFor();
    const small = await page.evaluate(() =>
      [...document.querySelectorAll(".k-cat-state__render .k-checkbox__hit")]
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
