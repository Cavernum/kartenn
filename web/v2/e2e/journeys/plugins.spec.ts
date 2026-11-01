import type { Page } from "@playwright/test";
import { apiCall, DATA, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// The plugins in the new shell (spec 021, task 16; REQ-021-13, 26; REQ-007-9, 10, 13), with
// the test plugins of web/v2/e2e/plugins: `notes` fills every point (its views under the
// referential, `g n` free and `g p` the shell's, a view that throws, one for administrators);
// `incompatible` (ui ^3), `throws` (exception while loading) and `silent` (no
// definePlugin) are set aside; the digest of `forged` is forged here. Ported from
// web/e2e/journeys/plugins.spec.ts. The tile of the home page, the tab and the panel of a
// prefix and the column of the addresses are placed by the screens of task 11; the actions
// on the type `changeset` by its record (task 14).

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });
const about = (page: Page, name: string) =>
  page
    .getByRole("main")
    .getByRole("listitem")
    .filter({ hasText: `${name} 1.0.0` });

test("plugins that fail are set aside with their reason and code, the shell stays usable", async ({
  page,
  login,
}) => {
  await login("bob");
  await page.goto("/about");
  await expect(page.getByRole("heading", { level: 1, name: "À propos" })).toBeVisible();
  // `silent` is given up after 5 s.
  await expect(about(page, "silent")).toContainText("aucune extension définie dans le délai", {
    timeout: 15_000,
  });
  await expect(about(page, "silent")).toContainText("plugin.timeout");
  await expect(about(page, "notes")).toContainText("Chargé");
  await expect(about(page, "example")).toContainText("Chargé");
  await expect(about(page, "forged")).toContainText("Chargé");
  await expect(about(page, "incompatible")).toContainText(
    "version du SDK, des composants ou de Vue incompatible",
  );
  await expect(about(page, "incompatible")).toContainText("plugin.incompatible");
  await expect(about(page, "throws")).toContainText("erreur pendant le chargement");
  await expect(about(page, "throws")).toContainText("plugin.exception");
  // `g p` is the shell's: refused to `notes`, which stays loaded.
  await expect(about(page, "notes")).toContainText("Raccourci refusé");
  await expect(about(page, "notes")).toContainText("demandée par la vue broken");
  const nav = navigation(page);
  // « À propos » is in no space: the referential opens on demand.
  await nav.getByRole("button", { name: "Référentiel" }).click();
  await expect(nav.getByRole("link", { name: "Notes", exact: true })).toBeVisible();
  await expect(nav.getByRole("link", { name: "Incompatible" })).toHaveCount(0);
  await expectAccessible(page);

  await nav.getByRole("link", { name: "Préfixes" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Préfixes" })).toBeVisible();
});

test.describe("a forged digest", () => {
  test.use({ toleratedErrors: [/digest|integrity|intégrité|SRI/i] });

  test("sets the plugin aside before any of its code runs", async ({ page, login }) => {
    await login("bob");
    await page.route("**/v1/capabilities", async (route) => {
      // Uncompressed: Firefox hands `route.fetch` a zstd body as it came.
      const response = await route.fetch({
        headers: { ...route.request().headers(), "accept-encoding": "identity" },
      });
      const capabilities = (await response.json()) as {
        ui_plugins: { name: string; integrity: Record<string, string> }[];
      };
      for (const plugin of capabilities.ui_plugins) {
        if (plugin.name === "forged") plugin.integrity["index.js"] = `sha384-${"A".repeat(64)}`;
      }
      await route.fulfill({ status: 200, json: capabilities });
    });
    await page.goto("/about");
    await expect(about(page, "forged")).toContainText("fichier introuvable ou empreinte invalide");
    await expect(about(page, "forged")).toContainText("plugin.load-failed");
    await expect(about(page, "notes")).toContainText("Chargé");
  });
});

test.describe("a view of a plugin", () => {
  test.use({
    toleratedCsp: [/^connect-src https:\/\/third-party\.invalid/],
    toleratedErrors: [/third-party\.invalid/],
  });

  test("opens from the sidebar under its space, and may not reach another origin", async ({
    page,
    login,
  }) => {
    await login("bob");
    await navigation(page).getByRole("link", { name: "Notes", exact: true }).click();
    await expect(page).toHaveURL(/\/x\/notes\/overview$/);
    await expect(
      page.getByRole("heading", { level: 1, name: "Vue d'ensemble des notes" }),
    ).toBeVisible();
    await expect(page.getByText("Appel d'un tiers bloqué")).toBeVisible();
    const crumbs = page.getByRole("navigation", { name: "Fil d'Ariane" });
    await expect(crumbs.getByRole("link", { name: "Référentiel" })).toBeVisible();
    await expect(
      navigation(page).getByRole("link", { name: "Notes", exact: true }),
    ).toHaveAttribute("aria-current", "page");
    const violations = await page.evaluate(() => window.__kartennCsp ?? []);
    expect(violations.some((v) => v.startsWith("connect-src"))).toBe(true);
  });

  test("opens by the letter its manifest proposes; the shell keeps its own", async ({
    page,
    login,
  }) => {
    await login("bob");
    await expect(navigation(page).getByRole("link", { name: "Notes", exact: true })).toBeVisible();
    // `n`, free: `m` is the machines plugin's (design « Raccourcis »).
    await page.keyboard.press("g");
    await page.keyboard.press("n");
    await expect(page).toHaveURL(/\/x\/notes\/overview$/);
    await page.keyboard.press("g");
    await page.keyboard.press("p");
    await expect(page).toHaveURL(/\/prefixes$/);
  });
});

test.describe("an element that throws", () => {
  test.use({ toleratedErrors: [/e2e plugin element throws when connected/] });

  test("stays in its place, said with its code; the frame stays usable", async ({
    page,
    login,
  }) => {
    await login("bob");
    await navigation(page).getByRole("link", { name: "Notes en panne" }).click();
    await expect(page).toHaveURL(/\/x\/notes\/broken$/);
    const alert = page.getByRole("main").getByRole("alert");
    await expect(alert).toContainText("« Notes en panne » n'a pas pu s'afficher");
    await expect(page.getByRole("main")).toContainText("plugin.element");
    await expectAccessible(page);
    await navigation(page).getByRole("link", { name: "Préfixes" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Préfixes" })).toBeVisible();
  });
});

test("a view shows by its permission, never by its URL alone", async ({ page, login }) => {
  await login("bob");
  await expect(navigation(page).getByRole("link", { name: "Notes", exact: true })).toBeVisible();
  await expect(navigation(page).getByRole("link", { name: "Notes d'administration" })).toHaveCount(
    0,
  );
  await page.goto("/x/notes/admin");
  await expect(page.getByText("Rien à cette adresse")).toBeVisible();
  await expect(page.locator("notes-admin")).toHaveCount(0);
});

test("the screens of the referential place the tile, the tab, the panel and the column", async ({
  page,
  login,
}) => {
  await login("bob");
  // The tile of the home page, after the core's.
  const tile = page.getByRole("main").locator("section").filter({ hasText: "plugin notes" });
  await expect(tile.locator("notes-home")).toContainText("Trois notes ouvertes");
  await expectAccessible(page);

  // The column of the addresses: an element of the plugin in each row.
  await page.goto("/addresses");
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("columnheader", { name: /Repère/ })).toBeAttached();
  await expect(grid.locator("notes-flag").first()).toHaveText("•");

  // The tab and the panel of a prefix, each given the object.
  await page.goto("/prefixes");
  await page
    .getByRole("grid", { name: "Préfixes" })
    .getByRole("link", { name: DATA.prefix })
    .click();
  await expect(
    page.getByRole("heading", {
      level: 1,
      name: new RegExp(`\\s${DATA.prefix.replaceAll(".", "\\.")}$`),
    }),
  ).toBeVisible();
  const id = page.url().split("/").pop()!;
  await expect(page.locator("notes-summary")).toContainText(id);
  await page
    .getByRole("tablist", { name: "Sections de la fiche" })
    .getByRole("tab", { name: "Notes" })
    .click();
  await expect(page.locator("notes-tab")).toHaveText(`Notes de prefix ${id}`);
  await expectAccessible(page);

  // A type of the core since spec 026: the tab of the record of a VLAN, given its object.
  await page.goto("/vlans");
  await page
    .getByRole("grid", { name: "VLAN" })
    .getByRole("link", { name: String(DATA.vlan.vid), exact: true })
    .click();
  await expect(page).toHaveURL(/\/vlans\/[0-9a-f-]+$/);
  const vlan = page.url().split("/").pop()!;
  await page
    .getByRole("tablist", { name: "Sections de la fiche" })
    .getByRole("tab", { name: "Notes" })
    .click();
  await expect(page.locator("notes-tab")).toHaveText(`Notes de vlan ${vlan}`);
  await expectAccessible(page);
});

test("an action on the type changeset: in « … » of its record, its dialog closed by kartenn-close", async ({
  page,
  login,
}) => {
  await login("alice");
  const listed = (await apiCall(page, "GET", "/v1/changesets?page_size=1")).body as {
    items: { id: string }[];
  };
  const id = listed.items[0]!.id;
  await page.goto(`/changesets/${id}`);
  const more = page
    .getByRole("toolbar", { name: "Actions de revue" })
    .getByRole("button", { name: "Plus d'actions" });
  await more.click();
  await page.getByRole("menuitem", { name: "Relire les notes" }).click();
  const dialog = page.getByRole("dialog", { name: "Relire les notes" });
  await expect(dialog.locator("notes-review")).toContainText(`Notes du changeset ${id}`);
  await expectAccessible(page);
  // The element closes its dialog (`kartenn-close`); the focus goes back to « … ».
  await dialog.locator("notes-review").getByRole("button", { name: "Fermer" }).click();
  await expect(dialog).toBeHidden();
  await expect(more).toBeFocused();
});
