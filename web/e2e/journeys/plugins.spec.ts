import type { Page } from "@playwright/test";
import { expect, expectAccessible, test } from "../fixtures/index.ts";

// Plugins loaded by the shell (spec 007, task 15; REQ-007-9, 10, 13), with the test plugins
// of web/e2e/plugins: `notes` fills the five points; `incompatible` (sdk ^9), `throws`
// (exception while loading) and `silent` (no definePlugin) are set aside; the digest of
// `forged` is forged here.

/**
 * The identifier of the object of `collection` whose value is `value`, read by the page
 * itself (`page.request` of Firefox runs beside the page's own requests and was seen to
 * spoil them).
 */
async function idOf(page: Page, collection: string, value: string): Promise<string> {
  const items = await page.evaluate(
    async (url) => {
      const response = await fetch(url);
      return ((await response.json()) as { items: { id: string }[] }).items;
    },
    `/v1/${collection}?value=${encodeURIComponent(value)}`,
  );
  expect(items).toHaveLength(1);
  return items[0]!.id;
}

const about = (page: Page) => page.getByRole("main").getByRole("listitem");

test("plugins that fail are set aside with their reason, the shell stays usable", async ({
  page,
  login,
}) => {
  await login("bob");
  await page.goto("/about");
  // `silent` is given up after 5 s.
  await expect(about(page).filter({ hasText: "silent 1.0.0" })).toContainText(
    "aucune extension définie dans le délai",
    { timeout: 15_000 },
  );
  await expect(about(page).filter({ hasText: "notes 1.0.0" })).toContainText("chargé");
  await expect(about(page).filter({ hasText: "incompatible 1.0.0" })).toContainText(
    "version du SDK incompatible",
  );
  await expect(about(page).filter({ hasText: "throws 1.0.0" })).toContainText(
    "erreur pendant le chargement",
  );
  await expect(about(page).filter({ hasText: "forged 1.0.0" })).toContainText("chargé");
  const extensions = page.getByRole("navigation", { name: "Navigation principale" });
  await expect(extensions.getByRole("link", { name: "Notes" })).toBeVisible();
  await expect(extensions.getByRole("link", { name: "Incompatible" })).toHaveCount(0);
  await expectAccessible(page);

  await extensions.getByRole("link", { name: "Arbre" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Arbre des préfixes" })).toBeVisible();
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
    await expect(about(page).filter({ hasText: "forged 1.0.0" })).toContainText(
      "fichier introuvable ou empreinte invalide",
    );
    await expect(about(page).filter({ hasText: "notes 1.0.0" })).toContainText("chargé");
  });
});

test.describe("a view of a plugin", () => {
  test.use({
    toleratedCsp: [/^connect-src https:\/\/third-party\.invalid/],
    toleratedErrors: [/third-party\.invalid/],
  });

  test("opens from the navigation, and may not reach another origin", async ({ page, login }) => {
    await login("bob");
    await page
      .getByRole("navigation", { name: "Navigation principale" })
      .getByRole("link", { name: "Notes" })
      .click();
    await expect(page).toHaveURL(/\/x\/notes\/overview$/);
    await expect(
      page.getByRole("heading", { level: 1, name: "Vue d'ensemble des notes" }),
    ).toBeVisible();
    await expect(page.getByText("Appel d'un tiers bloqué")).toBeVisible();
    const violations = await page.evaluate(() => window.__kartennCsp ?? []);
    expect(violations.some((v) => v.startsWith("connect-src"))).toBe(true);
  });
});

test("tabs and panels follow the type of the object", async ({ page, login }) => {
  await login("bob");
  const prefix = await idOf(page, "prefixes", "10.1.0.0/24");
  const address = await idOf(page, "addresses", "10.1.0.10");

  await page.goto(`/prefixes/${prefix}`);
  const tabs = page.getByRole("tablist", { name: "Sections de la fiche" });
  await tabs.getByRole("tab", { name: "Notes" }).click();
  await expect(page.getByText(`Notes de prefix ${prefix}`)).toBeVisible();
  await expect(page.getByRole("heading", { name: "Résumé des notes" })).toBeVisible();
  await expectAccessible(page);

  await page.goto(`/addresses/${address}`);
  await expect(tabs.getByRole("tab", { name: "Attributs" })).toBeVisible();
  await expect(tabs.getByRole("tab", { name: "Notes" })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Résumé des notes" })).toHaveCount(0);
});

test("an action is absent without its permission", async ({ page, login }) => {
  await login("bob");
  const prefix = await idOf(page, "prefixes", "10.1.0.0/24");
  await page.goto(`/prefixes/${prefix}`);
  await expect(page.getByRole("tab", { name: "Notes" })).toBeVisible();
  // Bob edits addresses (spec 024): the example's draft action is his, not « Réserver »,
  // which asks apply_own.
  await page.getByRole("button", { name: "Actions" }).click();
  await expect(page.getByRole("menuitem").first()).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Réserver" })).toHaveCount(0);
});

test("an action opens in a dialog that its element closes", async ({ page, login }) => {
  await login("alice");
  const prefix = await idOf(page, "prefixes", "10.1.0.0/24");
  await page.goto(`/prefixes/${prefix}`);
  await page.getByRole("button", { name: "Actions" }).click();
  await page.getByRole("menuitem", { name: "Réserver" }).click();
  const dialog = page.getByRole("dialog", { name: "Réserver" });
  await expect(dialog.getByText(`Réserver dans ${prefix}`)).toBeVisible();
  await expectAccessible(page);
  await dialog.getByRole("button", { name: "Fermer" }).click();
  await expect(dialog).toHaveCount(0);
});
