import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Locator, Page } from "@playwright/test";
import { apiCall, atRest, DATA, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// Accessibility end to end of the new shell (spec 021, task 21a; REQ-021-29), ported from
// web/e2e/journeys/accessibility.spec.ts and the guards of web/e2e/journeys/smoke.spec.ts:
// every journey runs axe (checked by the coverage test below), every screen of the shell
// passes it, one journey goes through the tree, a table, an action menu and a dialog by the
// keyboard alone; and the guards of the fixtures fail a journey, axe by its violation, the
// CSP by an inline script.

const JOURNEYS = fileURLToPath(new URL(".", import.meta.url));
const SPACES = fileURLToPath(new URL("../../shell/src/frame/spaces.ts", import.meta.url));

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
  expect(Object.keys(files).length).toBeGreaterThan(10);
  expect(uncovered(files)).toEqual([]);
  // The check itself: a journey without axe is named.
  expect(uncovered({ "bare.spec.ts": 'test("x", async ({ page }) => page.goto("/"));' })).toEqual([
    "bare.spec.ts",
  ]);
});

/**
 * The entries of the spaces of the core (shell/src/frame/spaces.ts), by their path: read
 * from the source, so that an entry added is a screen checked. Each entry also has its
 * icon, the only `icon: "…"` of the source: counted apart, an entry whose path is not a
 * literal the expression reads fails the journey instead of escaping it.
 */
function entries(): string[] {
  const source = readFileSync(SPACES, "utf8");
  const paths = [...source.matchAll(/\bto: "(\/[^"]*)"/g)].map((m) => m[1]!);
  const icons = [...source.matchAll(/\bicon: "[^"]+"/g)].length;
  expect(paths.length, "the entries of spaces.ts, by their paths and by their icons").toBe(icons);
  expect(new Set(paths).size, "an entry of spaces.ts per path").toBe(paths.length);
  return paths;
}

/**
 * The records whose first object is checked: the collection of the API, the route of its
 * record in the shell.
 */
const RECORDS = [
  ["domains", "/domains"],
  ["prefixes", "/prefixes"],
  ["ranges", "/ranges"],
  ["addresses", "/addresses"],
  ["entities", "/entities"],
  ["sites", "/sites"],
  ["vlan-groups", "/vlan-groups"],
  ["vlans", "/vlans"],
  ["changesets", "/changesets"],
  ["groups", "/admin/groups"],
] as const;

/** The record of the first object of `collection`, `null` if it holds none. */
async function firstRecord(page: Page, collection: string, route: string): Promise<string | null> {
  const listed = await apiCall(page, "GET", `/v1/${collection}?page_size=1`);
  expect(listed.status, `/v1/${collection}`).toBe(200);
  const id = (listed.body as { items: { id: string }[] }).items[0]?.id;
  return id === undefined ? null : `${route}/${id}`;
}

/** A draft of the page's user, empty, for its record; dropped by `dropDraft`. */
async function newDraft(page: Page, title: string): Promise<string> {
  const created = await apiCall(page, "POST", "/v1/drafts", { title, operations: [] });
  expect(created.status, JSON.stringify(created.body)).toBe(201);
  return (created.body as { id: string }).id;
}

/** Deletes the draft `id` (under `If-Match`: its ETag read first). */
async function dropDraft(page: Page, id: string): Promise<void> {
  const status = await page.evaluate(async (id) => {
    const read = await fetch(`/v1/drafts/${id}`);
    const response = await fetch(`/v1/drafts/${id}`, {
      method: "DELETE",
      headers: {
        "If-Match": read.headers.get("ETag") ?? "",
        "Idempotency-Key": crypto.randomUUID(),
      },
    });
    return response.status;
  }, id);
  expect(status).toBe(204);
}

/** The screen shown drawn: its title, nothing left loading in the content. */
async function drawn(page: Page): Promise<void> {
  const main = page.getByRole("main");
  await expect(main.locator("h1").first()).toBeVisible();
  await expect(main.locator(".k-skeleton-fill, [aria-busy='true']")).toHaveCount(0);
}

test("every screen of the shell passes axe", async ({ page, login, context, browserName }) => {
  test.setTimeout(300_000);
  await login("alice");
  const records = (
    await Promise.all(RECORDS.map(([collection, route]) => firstRecord(page, collection, route)))
  ).filter((record) => record !== null);
  // The lists of the referential (the old journey `accessibility` was their only proof).
  expect(entries()).toEqual(expect.arrayContaining(["/ranges", "/entities", "/sites"]));
  expect(records.length).toBeGreaterThanOrEqual(5);
  const draft = await newDraft(page, `axe ${browserName} ${Date.now()}`);
  const screens = [
    ...entries(),
    ...records,
    `/drafts/${draft}`,
    // The view of a plugin, in the frame.
    "/x/example/overview",
    "/help/shortcuts",
    "/about",
  ];
  for (const screen of screens) {
    await test.step(screen, async () => {
      await page.goto(screen);
      // Its plugins loaded before the next one (`atRest`: Firefox and the loads cut short).
      await atRest(page);
      await drawn(page);
      await expectAccessible(page);
    });
  }
  await dropDraft(page, draft);
  await context.clearCookies();
  await page.goto("/login");
  await expect(page.getByRole("heading", { level: 1, name: "Connexion" })).toBeVisible();
  await expectAccessible(page);
});

/** Whether the focus is on `target` or inside it. */
const holdsFocus = (target: Locator) =>
  target.evaluate((element) => element.contains(document.activeElement));

/** Presses `key` (Tab by default) until the focus is on or inside `target`, `max` at most. */
async function tabTo(page: Page, target: Locator, key = "Tab", max = 80): Promise<void> {
  for (let i = 0; i < max; i += 1) {
    if (await holdsFocus(target)) return;
    await page.keyboard.press(key);
  }
  await expect(target).toBeFocused();
}

/** The row of a node of the tree, by its value. */
const node = (tree: Locator, value: string) =>
  tree.locator("[role='row'][data-node]").filter({
    has: tree.page().getByText(value, { exact: true }),
  });

/** A title that ends with the name of the object (its kind in words comes first). */
const titled = (name: string) => new RegExp(`\\s${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);

test("the tree, a table, an action menu and a dialog by the keyboard alone", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto("/topology");
  await atRest(page);
  const tree = page.getByRole("treegrid", { name: `Arbre des préfixes du domaine ${DATA.domain}` });
  await expect(node(tree, DATA.prefix)).toBeVisible();

  // The skip link first, then the main content.
  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: "Aller au contenu" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("main")).toBeFocused();

  // The tree, one tab stop: the first node, down to the wide container, Right opens it,
  // Down enters its first child, Enter opens its record.
  await tabTo(page, tree);
  await page.keyboard.press("Control+Home");
  await expect(node(tree, DATA.prefix).locator("[role='gridcell']").first()).toBeFocused();
  const wide = node(tree, DATA.wide.prefix);
  for (let i = 0; i < 20 && !(await holdsFocus(wide)); i += 1) {
    await page.keyboard.press("ArrowDown");
  }
  expect(await holdsFocus(wide)).toBe(true);
  await page.keyboard.press("ArrowRight");
  await expect(wide).toHaveAttribute("aria-expanded", "true");
  const child = node(tree, "2001:db8:3::/64");
  await expect(child).toBeVisible();
  await page.keyboard.press("ArrowDown");
  expect(await holdsFocus(child)).toBe(true);
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/prefixes\/[0-9a-f-]{36}$/);
  await expect(
    page.getByRole("heading", { level: 1, name: titled("2001:db8:3::/64") }),
  ).toBeVisible();

  // The action menu: Enter opens it, the arrows move, Enter opens the plugin's dialog.
  const actions = page.getByRole("main").getByRole("button", { name: "Actions" });
  await tabTo(page, actions);
  await page.keyboard.press("Enter");
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  const item = menu.getByRole("menuitem", { name: "Ajouter une adresse au brouillon" });
  for (let i = 0; i < 10 && !(await holdsFocus(item)); i += 1) {
    await page.keyboard.press("ArrowDown");
  }
  await expect(item).toBeFocused();
  await expectAccessible(page);
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Ajouter une adresse au brouillon" });
  await expect(dialog).toBeVisible();
  // The focus is inside the dialog; Escape closes it, back on the menu's button.
  await expect.poll(() => holdsFocus(dialog)).toBe(true);
  await expectAccessible(page);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(actions).toBeFocused();

  // A table: back to the navigation, its link to the addresses; the grid is one tab stop,
  // on its first header; down to the first row, Enter into its link, Enter follows it.
  const navigation = page.getByRole("navigation", { name: "Navigation principale" });
  await tabTo(page, navigation.getByRole("link", { name: "Adresses" }), "Shift+Tab");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses$/);
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("row").nth(1)).toBeVisible();
  await tabTo(page, grid);
  await expect(page.locator(":focus")).toHaveAttribute("role", "columnheader");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  const link = grid.getByRole("row").nth(1).getByRole("link").first();
  await expect(link).toBeFocused();
  const address = (await link.textContent())!.trim();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses\/[0-9a-f-]{36}$/);
  await expect(page.getByRole("heading", { level: 1, name: titled(address) })).toBeVisible();
  await expectAccessible(page);
});

// The guards of every journey (fixtures/journeys.ts and fixtures/index.ts): each must make
// a journey fail.
test.describe("guards of every journey", () => {
  test("an axe-core violation fails the journey", async ({ page, login }) => {
    await login("bob");
    await expectAccessible(page);
    await page.evaluate(() => {
      const image = document.createElement("img");
      image.src = "data:image/gif;base64,R0lGODlhAQABAAAAACw=";
      document.querySelector("main")?.append(image);
    });
    // The call every journey makes throws, on the rule and the element.
    await expect(expectAccessible(page)).rejects.toThrow(/image-alt: [^\n]*img/);
  });

  test.describe("the CSP", () => {
    // The browsers also say the refusal on their console, which the guard of the console
    // errors would catch: tolerated here, so that the guard of the CSP alone fails.
    test.use({ toleratedErrors: [/^console: .*Content.Security.Policy/] });

    // The guard of the CSP runs after the journey, as the fixture `page` ends: only an
    // expected failure can show it. Declared once the script is known refused and its
    // violation recorded, so that any other failure (a login broken, a CSP that lets the
    // script run) fails the test for real.
    test("an inline script fails the journey", async ({ page, login }) => {
      await login("bob");
      await page.evaluate(() => {
        const script = document.createElement("script");
        script.textContent = "window.__kartennInjected = true;";
        document.body.append(script);
      });
      await expect
        .poll(() => page.evaluate(() => window.__kartennCsp ?? []))
        .toEqual([expect.stringMatching(/^script-src(-elem)? inline$/)]);
      expect(
        await page.evaluate(() => "__kartennInjected" in window),
        "the inline script ran",
      ).toBe(false);
      test.fail();
    });
  });
});
