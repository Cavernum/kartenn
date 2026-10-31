import type { Page } from "@playwright/test";
import { DATA, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// The shortcuts and the palette of the new shell (spec 021, task 10; REQ-021-3, 7, 8): `g c`,
// the help `?` and its page, « gc » typed in a field without effect, the single keys turned
// off (Ctrl+K stays), `[` and `]`, the palette at the keyboard alone (a value, a page, the
// recent objects).

const palette = (page: Page) => page.getByRole("dialog", { name: "Palette de commandes" });
const field = (page: Page) => palette(page).getByRole("combobox");

/** The home page shown, the focus nowhere in particular (on the document). */
async function home(
  page: Page,
  login: (user: "alice" | "bob") => Promise<void>,
  user: "alice" | "bob",
) {
  await login(user);
  await expect(page.getByRole("heading", { level: 1, name: "Accueil" })).toBeVisible();
}

test("`g` then `c` goes to the changesets", async ({ page, login }) => {
  await home(page, login, "alice");
  await page.keyboard.press("g");
  await page.keyboard.press("c");
  await expect(page).toHaveURL(/\/changesets$/);
  await expect(page.getByRole("heading", { level: 1, name: "Changesets" })).toBeVisible();
  await page.keyboard.press("g");
  await page.keyboard.press("h");
  await expect(page).toHaveURL(/\/$/);
});

test("`?` shows the help of every shortcut, also at /help/shortcuts", async ({ page, login }) => {
  await home(page, login, "alice");
  await page.keyboard.press("Shift+?");
  const help = page.getByRole("dialog", { name: "Raccourcis clavier" });
  await expect(help).toBeVisible();
  const grid = help.getByRole("grid", { name: "Raccourcis clavier" });
  await expect(grid.getByRole("row", { name: /g puis c Aller aux changesets/ })).toBeVisible();
  await expect(grid.getByRole("row", { name: /fermer la liste, puis le panneau/ })).toBeVisible();
  await expect(grid.getByRole("row", { name: /Palette de commandes/ })).toBeVisible();
  await expect(help.getByText(/Les raccourcis à une touche sont actifs/)).toBeVisible();
  await expectAccessible(page);
  // Ctrl+K over the help opens no second modal layer: one Escape then closes the help (a
  // palette over it would have taken that Escape).
  await page.keyboard.press("Control+k");
  await page.keyboard.press("Escape");
  await expect(help).toBeHidden();
  await expect(palette(page)).toHaveCount(0);

  // The help from the palette gives the focus back to what opened the palette (2.4.3).
  const search = page
    .getByRole("navigation", { name: "Navigation principale" })
    .getByRole("button", { name: "Rechercher" });
  // Past the skip link, the first control of the sidebar.
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await expect(search).toBeFocused();
  await page.keyboard.press("Enter");
  await page.keyboard.type("raccourcis");
  await page.keyboard.press("Enter");
  await expect(help).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(help).toBeHidden();
  await expect(search).toBeFocused();

  await page.goto("/help/shortcuts");
  await expect(page.getByRole("heading", { level: 1, name: "Raccourcis clavier" })).toBeVisible();
  await expect(page).toHaveTitle("Raccourcis clavier — kartenn");
  await expect(page.getByRole("grid", { name: "Raccourcis clavier" })).toBeVisible();
  await expectAccessible(page);
});

test("a reader is not offered the navigation out of their rights", async ({ page, login }) => {
  await home(page, login, "bob");
  await page.keyboard.press("Shift+?");
  const grid = page.getByRole("dialog", { name: "Raccourcis clavier" }).getByRole("grid");
  await expect(grid.getByRole("row", { name: /Aller aux préfixes/ })).toBeVisible();
  await expect(grid.getByRole("row", { name: /Aller à l'administration/ })).toHaveCount(0);
  await page.keyboard.press("Escape");
  // The help gone (its exit over), the keys are the page's again.
  await expect(page.getByRole("dialog", { name: "Raccourcis clavier" })).toHaveCount(0);
  await page.keyboard.press("g");
  await page.keyboard.press("x");
  // Then a shortcut that navigates: it arrives, and the page before it is still the home.
  await page.keyboard.press("g");
  await page.keyboard.press("p");
  await expect(page).toHaveURL(/\/prefixes$/);
  await page.goBack();
  await expect(page).toHaveURL(/\/$/);
});

test("« gc » typed in a field goes nowhere", async ({ page, login }) => {
  await home(page, login, "alice");
  await page.keyboard.press("Control+k");
  await expect(field(page)).toBeFocused();
  await page.keyboard.type("gc");
  await expect(field(page)).toHaveValue("gc");
  await page.keyboard.press("Escape");
  await expect(palette(page)).toBeHidden();
  // Then a shortcut that navigates: it arrives, and the page before it is still the home.
  await page.keyboard.press("g");
  await page.keyboard.press("p");
  await expect(page).toHaveURL(/\/prefixes$/);
  await page.goBack();
  await expect(page).toHaveURL(/\/$/);
});

test("the single keys turned off, `g c` does nothing and Ctrl+K stays", async ({ page, login }) => {
  await home(page, login, "alice");
  await page.getByRole("button", { name: "Compte de Alice Exemple" }).click();
  await page.getByRole("menuitemcheckbox", { name: "Raccourcis à une touche" }).click();
  await page.keyboard.press("Escape");
  await page.reload();
  await expect(page.getByRole("heading", { level: 1, name: "Accueil" })).toBeVisible();
  await page.keyboard.press("g");
  await page.keyboard.press("c");
  await page.keyboard.press("Shift+?");
  // Ctrl+K opens the palette: no help is open (it would refuse it).
  await page.keyboard.press("Control+k");
  await expect(field(page)).toBeFocused();
  await expect(page.getByRole("dialog", { name: "Raccourcis clavier" })).toHaveCount(0);
  // No hint of a single key while they are off.
  await expect(palette(page).getByText("puis")).toHaveCount(0);
  // A page from the palette: it arrives, and the page before it is still the home.
  await page.keyboard.type("préfixes");
  await expect(
    palette(page)
      .getByRole("option", { name: /Préfixes/ })
      .first(),
  ).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/prefixes$/);
  await page.goBack();
  await expect(page).toHaveURL(/\/$/);
  // The help says them off.
  await page.goto("/help/shortcuts");
  await expect(page.getByRole("row", { name: /Aller aux changesets \(désactivé\)/ })).toBeVisible();
});

test("`[` collapses the sidebar, `]` opens and closes the right panel", async ({ page, login }) => {
  await home(page, login, "alice");
  const nav = page.getByRole("navigation", { name: "Navigation principale" });
  await page.keyboard.press("[");
  await expect(nav).toHaveAttribute("data-state", "collapsed");
  await page.keyboard.press("[");
  await expect(nav).toHaveAttribute("data-state", "expanded");
  const panel = page.getByRole("complementary", { name: "Panneau droit" });
  await page.keyboard.press("]");
  await expect(panel).toBeVisible();
  await page.keyboard.press("]");
  await expect(panel).toBeHidden();
});

test("the palette at the keyboard alone: a value, then the recent object, then a page", async ({
  page,
  login,
}) => {
  await home(page, login, "bob");
  await page.keyboard.press("Control+k");
  await expect(field(page)).toBeFocused();
  await page.keyboard.type("192.0.2.");
  const option = palette(page).getByRole("option", { name: new RegExp(DATA.prefix) });
  await expect(option).toBeVisible();
  await expect(option).toHaveAttribute("aria-selected", "true");
  await expectAccessible(page);
  await page.keyboard.press("Enter");
  await expect(palette(page)).toBeHidden();
  await expect(page).toHaveURL(/\/prefixes\/[0-9a-f-]+$/);
  const record = page.url();

  // Back home, the object is among the recent ones.
  await page.keyboard.press("g");
  await page.keyboard.press("h");
  await expect(page).toHaveURL(/\/$/);
  await page.keyboard.press("Control+k");
  await expect(palette(page).getByRole("group", { name: "Récents" })).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(record);

  // A page by its name, the first entry: Enter.
  await page.keyboard.press("Control+k");
  await page.keyboard.type("adresses");
  await expect(
    palette(page)
      .getByRole("option", { name: /Adresses/ })
      .first(),
  ).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses$/);
  await expect(page.getByRole("heading", { level: 1, name: "Adresses" })).toBeVisible();
});
