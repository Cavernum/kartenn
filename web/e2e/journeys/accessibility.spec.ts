import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Page } from "@playwright/test";
import { expect, expectAccessible, test } from "../fixtures/index.ts";
import { DATA } from "../fixtures/data.ts";

// Accessibility end to end (spec 007, task 25; REQ-007-33, W14): every journey runs axe
// (checked by the coverage test below), every screen of the shell passes it, and one
// journey goes through the tree, a table, an action menu and a dialog by the keyboard
// alone.

const JOURNEYS = fileURLToPath(new URL(".", import.meta.url));

/** The journeys among `files` (name → source) that never call axe. */
export function uncovered(files: Record<string, string>): string[] {
  return Object.entries(files)
    .filter(([, source]) => !/await expectAccessible\(/.test(source))
    .map(([name]) => name)
    .sort();
}

test("every journey runs axe", () => {
  const files = Object.fromEntries(
    readdirSync(JOURNEYS)
      .filter((name) => name.endsWith(".spec.ts"))
      .map((name) => [name, readFileSync(`${JOURNEYS}/${name}`, "utf8")]),
  );
  expect(Object.keys(files).length).toBeGreaterThan(5);
  expect(uncovered(files)).toEqual([]);
  // The check itself: a journey without axe is named.
  expect(uncovered({ "bare.spec.ts": 'test("x", async ({ page }) => page.goto("/"));' })).toEqual([
    "bare.spec.ts",
  ]);
});

/** A call of the API by the page itself, under its session. */
async function read(page: Page, path: string): Promise<unknown> {
  return page.evaluate(async (path) => (await fetch(path)).json(), path);
}

async function idOf(page: Page, path: string): Promise<string> {
  return ((await read(page, path)) as { items: { id: string }[] }).items[0]!.id;
}

test("every screen of the shell passes axe", async ({ page, login, context }) => {
  test.setTimeout(240_000);
  await login("alice");
  const domain = await idOf(page, `/v1/domains?slug=${DATA.domain}`);
  const prefix = await idOf(page, `/v1/prefixes?value=${encodeURIComponent("10.1.0.0/24")}`);
  const address = await idOf(page, `/v1/addresses?value=${DATA.addresses[0]}`);
  const changeset = await idOf(page, "/v1/changesets?page_size=1");
  const screens = [
    `/?domain=${domain}`,
    "/domains",
    `/domains/${domain}`,
    "/prefixes",
    `/prefixes/${prefix}`,
    "/prefixes/new",
    "/ranges",
    "/addresses",
    `/addresses/${address}`,
    "/entities",
    "/sites",
    "/anomalies",
    "/properties",
    `/search?q=${DATA.addresses[0]}`,
    "/changesets",
    `/changesets/${changeset}`,
    "/proposals",
    "/imports",
    "/admin",
    "/admin/roles",
    "/admin/bindings",
    "/admin/issuers",
    "/admin/service-accounts",
    "/admin/identities",
    "/admin/audit",
    "/about",
  ];
  for (const screen of screens) {
    await test.step(screen, async () => {
      await page.goto(screen);
      await expect(page.locator("main h1").first()).toBeVisible();
      await expect(page.getByText(/Chargement/)).toHaveCount(0);
      await expectAccessible(page);
    });
  }
  await context.clearCookies();
  await page.goto("/login");
  await expect(page.getByRole("heading", { level: 1, name: "Connexion" })).toBeVisible();
  await expectAccessible(page);
});

/**
 * Presses Tab until the focused element satisfies `wanted` (described by `what`), at most
 * `max` times.
 */
async function tabTo(
  page: Page,
  what: string,
  wanted: (element: {
    tag: string;
    role: string | null;
    text: string;
    classes: string[];
  }) => boolean,
  max = 80,
): Promise<void> {
  for (let i = 0; i < max; i += 1) {
    await page.keyboard.press("Tab");
    const focused = await page.evaluate(() => {
      const element = document.activeElement as HTMLElement | null;
      return {
        tag: element?.tagName ?? "",
        role: element?.getAttribute("role") ?? null,
        text: element?.textContent?.trim() ?? "",
        classes: [...(element?.classList ?? [])],
      };
    });
    if (wanted(focused)) return;
  }
  throw new Error(`Tab never reached ${what}`);
}

test("the tree, a table, an action menu and a dialog by the keyboard alone", async ({
  page,
  login,
}) => {
  await login("alice");
  const domain = await idOf(page, `/v1/domains?slug=${DATA.domain}`);
  await page.goto(`/?domain=${domain}`);
  const tree = page.getByRole("tree", { name: `Préfixes du domaine ${DATA.domain}` });
  await expect(tree).toBeVisible();

  // The skip link first, then the main content.
  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: "Aller au contenu" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("#k-main")).toBeFocused();

  // The tree: right opens the root, right again enters its first child, Enter opens it.
  await tabTo(page, "the tree", (e) => e.role === "tree");
  await page.keyboard.press("ArrowRight");
  const child = tree.getByRole("treeitem", { name: /^10\.1\.0\.0\/24\b/ });
  await expect(child).toBeVisible();
  await page.keyboard.press("ArrowRight");
  await expect(tree).toHaveAttribute("aria-activedescendant", (await child.getAttribute("id"))!);
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/prefixes\/[0-9a-f-]{36}/);
  await expect(page.getByRole("heading", { level: 1, name: /10\.1\.0\.0\/24/ })).toBeVisible();

  // The action menu: Enter opens it, the arrows move, Enter opens the plugin's dialog.
  await tabTo(page, "the menu « Actions »", (e) => e.tag === "BUTTON" && e.text === "Actions");
  await page.keyboard.press("Enter");
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  const item = menu.getByRole("menuitem", { name: "Ajouter une adresse au brouillon" });
  for (let i = 0; i < 10 && !(await item.evaluate((e) => e === document.activeElement)); i += 1) {
    await page.keyboard.press("ArrowDown");
  }
  await expect(item).toBeFocused();
  await expectAccessible(page);
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Ajouter une adresse au brouillon" });
  await expect(dialog).toBeVisible();
  // Focus is inside the dialog; Escape closes it.
  await expect.poll(() => dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
  await expectAccessible(page);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);

  // A table: from the navigation to the addresses, a row by the arrows, Enter opens it.
  await tabTo(page, "the link « Adresses »", (e) => e.tag === "A" && e.text === "Adresses");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses$/);
  await tabTo(page, "a row of the table", (e) => e.classes.includes("k-table-row"));
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses\/[0-9a-f-]+$/);
  await expect(page.getByRole("heading", { level: 1, name: /^Adresse / })).toBeVisible();
  await expectAccessible(page);
});
