import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the sidebar (Disclosure in a `navigation` landmark; spec 020, design
// « Inventaire » line 18; spec 021, « Cadre », REQ-021-2, 29): collapsed in icons, every
// control keeps its name (the space names its list, a counter is in its entry's name),
// and a tooltip after it on keyboard focus and on hover repeats the name, the counter and
// the shortcut (decorative; Escape closes it, the focus stays); the collapse toggle says
// its state (`aria-expanded`) and its shortcut `[`; expanded, the spaces are disclosures.

const nav = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });
/** The open bubble, decorative (out of the accessibility tree). */
const bubble = (page: Page) => page.locator(".k-tooltip:not([hidden])");
const toggle = (page: Page) => nav(page).getByRole("button", { name: "Barre latérale" });

test.describe("SidebarNav (Disclosure in navigation)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "SidebarNav", "keyboard");
  });

  test("collapsed: names kept, lists named by their space, shortcuts said", async ({ page }) => {
    await expect(toggle(page)).toHaveAttribute("aria-expanded", "false");
    await expect(toggle(page)).toHaveAttribute("aria-keyshortcuts", "[");
    const search = nav(page).getByRole("button", { name: "Rechercher" });
    await expect(search).toHaveAttribute("aria-keyshortcuts", "Control+K");
    await expect(search).toHaveAttribute("aria-haspopup", "dialog");
    const referential = nav(page).getByRole("list", { name: "Référentiel" });
    await expect(referential.getByRole("link", { name: "Préfixes" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expect(referential.getByRole("link", { name: "Anomalies 4" })).toBeVisible();
    await expect(
      nav(page)
        .getByRole("list", { name: "Changesets" })
        .getByRole("link", { name: "À instruire 2" }),
    ).toBeVisible();
    // No disclosure while collapsed: every space is open.
    await expect(nav(page).getByRole("button", { name: "Référentiel" })).toHaveCount(0);
  });

  test("collapsed: the keyboard focus shows the tooltip at once; Escape closes it", async ({
    page,
  }) => {
    await nav(page).getByRole("button", { name: "Rechercher" }).focus();
    await expect(bubble(page)).toHaveText(/^Rechercher\s*Ctrl\s*K$/);
    await page.keyboard.press("Tab");
    await expect(nav(page).getByRole("link", { name: "Accueil" })).toBeFocused();
    await expect(bubble(page)).toHaveText("Accueil");
    for (let i = 0; i < 6; i++) await page.keyboard.press("Tab");
    await expect(nav(page).getByRole("link", { name: "Anomalies 4" })).toBeFocused();
    await expect(bubble(page)).toHaveText("Anomalies4");
    await page.keyboard.press("Escape");
    await expect(bubble(page)).toHaveCount(0);
    await expect(nav(page).getByRole("link", { name: "Anomalies 4" })).toBeFocused();
    // Changesets, À instruire, Brouillon, Administration, then the toggle.
    for (let i = 0; i < 5; i++) await page.keyboard.press("Tab");
    await expect(toggle(page)).toBeFocused();
    await expect(bubble(page)).toHaveText(/^Barre latérale\s*\[$/);
  });

  test("collapsed: hovering an entry shows its tooltip after a delay", async ({ page }) => {
    await nav(page).getByRole("link", { name: "Domaines" }).hover();
    await expect(bubble(page)).toHaveText("Domaines");
    await page.mouse.move(600, 600);
    await expect(bubble(page)).toHaveCount(0);
  });

  test("the toggle expands it: labels shown, no tooltip, spaces as disclosures", async ({
    page,
  }) => {
    await toggle(page).focus();
    await page.keyboard.press("Enter");
    await expect(toggle(page)).toHaveAttribute("aria-expanded", "true");
    await expect(bubble(page)).toHaveCount(0);
    const space = nav(page).getByRole("button", { name: "Changesets" });
    await expect(space).toHaveAttribute("aria-expanded", "true");
    await space.focus();
    await expect(bubble(page)).toHaveCount(0);
    await page.keyboard.press("Enter");
    await expect(space).toHaveAttribute("aria-expanded", "false");
    await expect(nav(page).getByRole("link", { name: "À instruire 2" })).toBeHidden();
    await page.keyboard.press("Space");
    await expect(nav(page).getByRole("link", { name: "À instruire 2" })).toBeVisible();
    // « Rechercher » asks the host for the palette.
    const search = nav(page).getByRole("button", { name: /^Rechercher/ });
    await search.focus();
    await page.keyboard.press("Enter");
    await expect(page.locator("[data-palette]")).toHaveText("Palette demandée : 1");
  });
});
