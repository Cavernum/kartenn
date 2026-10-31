import { expect, openState, test } from "../fixtures/index.ts";

// The breadcrumb (APG Breadcrumb; spec 020, design « Inventaire » line 17): a named
// `navigation`, the current page `aria-current="page"`; a link cut by the ellipsis gives
// its whole label in a tooltip under the keyboard focus, a whole link none.

test.describe("Breadcrumb (APG Breadcrumb)", () => {
  test("a whole link shows no tooltip; the current page is marked", async ({ page }) => {
    await openState(page, "Breadcrumb", "record");
    const nav = page.getByRole("navigation", { name: "Fil d'Ariane" });
    await expect(nav.locator("[aria-current='page']")).toHaveText("192.0.2.0/26");
    await nav.getByRole("link", { name: "Référentiel" }).focus();
    await page.keyboard.press("Tab");
    await expect(nav.getByRole("link", { name: "Centre de données nord" })).toBeFocused();
    await expect(page.locator(".k-tooltip")).toHaveCount(0);
  });

  test("a cut link gives its whole label under the focus; Escape closes it", async ({ page }) => {
    await openState(page, "Breadcrumb", "cut");
    const nav = page.getByRole("navigation", { name: "Fil d'Ariane" });
    await nav.getByRole("link", { name: "Référentiel" }).focus();
    await page.keyboard.press("Tab");
    await expect(nav.getByRole("link", { name: "Centre de données nord" })).toBeFocused();
    await expect(page.locator(".k-tooltip")).toHaveText("Centre de données nord");
    await page.keyboard.press("Escape");
    await expect(page.locator(".k-tooltip")).toHaveCount(0);
  });
});
