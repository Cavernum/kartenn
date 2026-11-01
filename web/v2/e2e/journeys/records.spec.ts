import type { Locator, Page } from "@playwright/test";
import { DATA, expect, expectAccessible, loginIn, test } from "../fixtures/journeys.ts";

// Lists and records of the new shell (spec 021, task 11; REQ-021-9, 18, 22), ported from
// web/e2e/journeys/records.spec.ts: from a domain to a prefix to an address, its inherited
// property explained, its history; a long list virtualised, its next page read ahead and a
// row opened at the keyboard; a refused cursor starts the list again; a filtered link
// copied opens the same list; the filter of a column is the bar's.

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });
const tabs = (page: Page) => page.getByRole("tablist", { name: "Sections de la fiche" });
/** The title of a record: its kind in words (the dictionary's), then the object's name. */
const titled = (name: string) => new RegExp(`\\s${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);
/** To the end of a list: the grid scrolls in itself once virtualised, the surface before. */
const toEnd = (grid: Locator) =>
  grid.evaluate((element) => {
    element.scrollTo(0, element.scrollHeight);
    document.getElementById("k-surface")?.scrollTo(0, 1e9);
  });
const toStart = (grid: Locator) =>
  grid.evaluate((element) => {
    element.scrollTo(0, 0);
    document.getElementById("k-surface")?.scrollTo(0, 0);
  });

test("from a domain to a prefix to an address, its property explained, its history", async ({
  page,
  login,
}) => {
  await login("alice");
  await navigation(page).getByRole("link", { name: "Domaines" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Domaines" })).toBeVisible();
  const domains = page.getByRole("grid", { name: "Domaines" });
  await expect(domains.getByRole("link", { name: DATA.domain, exact: true })).toBeVisible();
  await expectAccessible(page);
  await domains.getByRole("link", { name: DATA.domain, exact: true }).click();

  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.domain) })).toBeVisible();
  // The breadcrumb names the object once read, not its identifier.
  await expect(page.getByRole("navigation", { name: /fil d'Ariane/i })).toContainText(DATA.domain);
  await tabs(page).getByRole("tab", { name: "Contenu" }).click();
  await page.getByRole("link", { name: DATA.prefix }).click();

  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.prefix) })).toBeVisible();
  await expect(page.getByRole("region", { name: DATA.prefix })).toContainText("sur 254");
  await expectAccessible(page);
  await tabs(page).getByRole("tab", { name: "Contenu" }).click();
  await page.getByRole("link", { name: DATA.address }).click();

  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.address) })).toBeVisible();
  // The properties first: core.owner, inherited from the domain through the prefix.
  await page
    .getByRole("row", { name: /core\.owner/ })
    .getByRole("button", { name: /^Expliquer la valeur/ })
    .click();
  const explanation = page.getByRole("dialog", { name: /d'où vient la valeur/ });
  await expect(explanation).toContainText("Héritée de");
  // The origin, named in the sentence, then again at the end of the chain.
  await expect(explanation.getByRole("link", { name: DATA.domain }).first()).toBeVisible();
  const chain = explanation.getByRole("listitem");
  await expect(chain.first()).toContainText(DATA.address);
  await expect(chain.filter({ hasText: DATA.prefix })).toHaveCount(1);
  await expect(chain.last()).toContainText(DATA.domain);
  await expectAccessible(page);
  await page.keyboard.press("Escape");
  await expect(explanation).toBeHidden();

  await tabs(page).getByRole("tab", { name: "Historique" }).click();
  await expect(page).toHaveURL(/[?&]tab=history/);
  const history = page.getByRole("grid", { name: `Versions de ${DATA.address}` });
  await expect(history.getByRole("row")).toHaveCount(2);
  await expect(history.getByRole("gridcell", { name: DATA.address })).toBeVisible();
  await expectAccessible(page);
});

test("a long list keeps a few dozen rows, reads ahead, and opens a row by keyboard", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto(`/addresses?within=${encodeURIComponent(DATA.bulk.prefix)}`);
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("link", { name: "2001:db8:2::1", exact: true })).toBeVisible();
  await expectAccessible(page);

  // Scroll to the end, page after page, until the whole list is read.
  await expect(async () => {
    await toEnd(grid);
    // The count in the words of the language (« 1 000 », a narrow space in French).
    await expect(page.getByText(/^1\s000 éléments$/)).toBeVisible({
      timeout: 500,
    });
  }).toPass({ timeout: 30_000 });
  expect(await grid.getAttribute("aria-rowcount")).toBe(String(DATA.bulk.count + 1));
  const rendered = await grid.locator("[role='row'][aria-rowindex]").count();
  expect(rendered).toBeGreaterThan(5);
  expect(rendered).toBeLessThan(150);

  // Keyboard: back to the first row, down, Enter into its link, Enter follows it.
  await toStart(grid);
  // The usage of the first row (a plain cell), down, then Home to the key of the second.
  await grid.getByRole("row").nth(1).getByRole("gridcell").nth(1).click();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Home");
  await page.keyboard.press("Enter");
  await expect(grid.getByRole("link", { name: "2001:db8:2::2", exact: true })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses\/[0-9a-f-]+$/);
  await expect(
    page.getByRole("heading", { level: 1, name: titled("2001:db8:2::2") }),
  ).toBeVisible();
});

test("a refused cursor starts the list again from its first page", async ({ page, login }) => {
  await login("alice");
  let refused = 0;
  await page.route(/\/v1\/addresses\?.*cursor=/, async (route) => {
    if (refused > 0) return route.continue();
    refused += 1;
    await route.fulfill({
      status: 400,
      contentType: "application/problem+json",
      body: JSON.stringify({
        type: "urn:kartenn:problem:invalid-cursor",
        title: "Bad Request",
        status: 400,
        detail: "the cursor has expired",
      }),
    });
  });
  await page.goto(`/addresses?within=${encodeURIComponent(DATA.bulk.prefix)}`);
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("link", { name: "2001:db8:2::1", exact: true })).toBeVisible();
  await expect(async () => {
    await toEnd(grid);
    await expect(
      page.getByRole("status").filter({ hasText: "la liste repart du début" }),
    ).toBeVisible({ timeout: 500 });
  }).toPass({ timeout: 20_000 });
  expect(refused).toBe(1);
  await toStart(grid);
  await expect(grid.getByRole("link", { name: "2001:db8:2::1", exact: true })).toBeVisible();
  await expectAccessible(page);
});

test("a filtered link copied opens the same list, its chips and its rows", async ({
  page,
  login,
  browser,
}) => {
  await login("alice");
  await page.goto("/addresses");
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("link", { name: DATA.address, exact: true })).toBeVisible();

  // The filter of the « Valeur » column is the bar's (020 decision A7): Alt+Down on its
  // header opens the picker on that property.
  await grid.getByRole("columnheader", { name: /Valeur/ }).click();
  await page.keyboard.press("Alt+ArrowDown");
  const picker = page.getByRole("dialog", { name: /filtre/i });
  await expect(picker).toBeVisible();
  await picker.getByRole("combobox", { name: /Opérateur/ }).click();
  await page.getByRole("option", { name: "dans" }).click();
  await picker.getByRole("textbox", { name: /Valeur/ }).fill(DATA.prefix);
  await picker.getByRole("button", { name: "Appliquer" }).click();
  await expect(page).toHaveURL(/[?&]within=192\.0\.2\.0(%2F|\/)24/);
  await expect(grid.getByRole("link", { name: DATA.address, exact: true })).toBeVisible();
  await expect(grid.getByRole("link", { name: "2001:db8:2::1", exact: true })).toHaveCount(0);
  await expectAccessible(page);

  // The link, opened by another session: the same chips, the same rows.
  const link = page.url();
  const bar = page.getByRole("group", { name: "Filtres : Adresses" });
  const chips = await bar.getByRole("button").allTextContents();
  const other = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  try {
    const second = await other.newPage();
    await loginIn(second, "alice");
    await second.goto(link);
    const again = second.getByRole("grid", { name: "Adresses" });
    await expect(again.getByRole("link", { name: DATA.address, exact: true })).toBeVisible();
    await expect(again.getByRole("link", { name: "2001:db8:2::1", exact: true })).toHaveCount(0);
    expect(
      await second
        .getByRole("group", { name: "Filtres : Adresses" })
        .getByRole("button")
        .allTextContents(),
    ).toEqual(chips);
  } finally {
    await other.close();
  }
});

test("a record hovered then opened shows without a skeleton (its reads prefetched)", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto("/addresses");
  const link = page.getByRole("grid", { name: "Adresses" }).getByRole("link", {
    name: DATA.address,
    exact: true,
  });
  // The pointer held on the link past its delay (60 ms): the record and its properties read.
  const read = page.waitForResponse(/\/v1\/addresses\/[0-9a-f-]+\/properties/);
  await link.hover();
  await read;
  await link.click();
  await expect(page).toHaveURL(/\/addresses\/[0-9a-f-]+$/);
  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.address) })).toBeVisible();
  // Neither the record nor its properties wait: no skeleton in the main panel at once.
  const main = page.getByRole("region", { name: DATA.address });
  expect(await main.locator(".k-skeleton-fill, [aria-busy='true']").count()).toBe(0);
});
