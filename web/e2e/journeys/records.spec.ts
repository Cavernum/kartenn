import { DATA } from "../fixtures/data.ts";
import { expect, expectAccessible, test } from "../fixtures/index.ts";

// Lists and records of the core (spec 007, task 17; REQ-007-17, 31): from a domain to a
// prefix to an address, its inherited property explained, its history; long lists
// virtualised; a refused cursor starts the list again.

test("from a domain to a prefix to an address, its property explained, its history", async ({
  page,
  login,
}) => {
  await login("alice");
  const navigation = page.getByRole("navigation", { name: "Navigation principale" });
  await navigation.getByRole("link", { name: "Domaines" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Domaines" })).toBeVisible();
  await expectAccessible(page);
  await page.getByRole("link", { name: DATA.domain, exact: true }).click();

  await expect(
    page.getByRole("heading", { level: 1, name: `Domaine ${DATA.domain}` }),
  ).toBeVisible();
  const tabs = page.getByRole("tablist", { name: "Sections de la fiche" });
  await tabs.getByRole("tab", { name: "Contenu" }).click();
  await page.getByRole("link", { name: "10.0.0.0/8" }).click();

  await expect(page.getByRole("heading", { level: 1, name: "Préfixe 10.0.0.0/8" })).toBeVisible();
  await tabs.getByRole("tab", { name: "Contenu" }).click();
  await page.getByRole("link", { name: "10.1.0.0/24" }).click();

  await expect(page.getByRole("heading", { level: 1, name: "Préfixe 10.1.0.0/24" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Occupation" })).toContainText("sur 254");
  await expectAccessible(page);
  await tabs.getByRole("tab", { name: "Contenu" }).click();
  await page.getByRole("link", { name: "10.1.0.10" }).click();

  await expect(page.getByRole("heading", { level: 1, name: "Adresse 10.1.0.10" })).toBeVisible();
  await tabs.getByRole("tab", { name: "Propriétés" }).click();
  await page.getByRole("button", { name: "Expliquer core.owner" }).click();
  // Inherited: the value is carried by the domain, the chain walked up to it.
  const explanation = page.locator(".k-explanation");
  const origin = `Domaine ${DATA.domain}`;
  await expect(
    explanation.getByRole("paragraph").getByRole("link", { name: origin }),
  ).toBeVisible();
  // The chain goes up from the address, through its prefixes, to the domain.
  const chain = explanation.getByRole("listitem");
  await expect(chain.first()).toHaveText("Adresse 10.1.0.10");
  await expect(chain.filter({ hasText: "Préfixe 10.1.0.0/24" })).toHaveCount(1);
  await expect(chain.last()).toHaveText(origin);
  await expectAccessible(page);

  await tabs.getByRole("tab", { name: "Historique" }).click();
  const history = page.getByRole("table", { name: "Historique" });
  await expect(history.getByRole("row")).toHaveCount(2);
  await expect(history.getByRole("cell", { name: "10.1.0.10" })).toBeVisible();
  await expectAccessible(page);
});

test("a list of 1 000 rows keeps a few dozen in the page, and opens a row by keyboard", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto("/addresses");
  const table = page.getByRole("table", { name: "Adresses" });
  await expect(table.getByRole("row").nth(1)).toBeVisible();
  await expectAccessible(page);

  // Scroll to the end, page after page, until the whole list is read.
  const scroller = page.locator(".k-table-scroll");
  await expect(async () => {
    await scroller.evaluate((element) => element.scrollTo(0, element.scrollHeight));
    await expect(page.getByText(/^\d+ éléments$/)).toBeVisible({ timeout: 500 });
  }).toPass({ timeout: 20_000 });
  const total = Number((await page.getByText(/^\d+ éléments$/).textContent())!.split(" ")[0]);
  expect(total).toBeGreaterThanOrEqual(DATA.bulk.count);
  expect(await table.getAttribute("aria-rowcount")).toBe(String(total + 1));
  const rendered = await table.locator("tbody tr.k-table-row").count();
  expect(rendered).toBeGreaterThan(5);
  expect(rendered).toBeLessThan(100);

  // Keyboard: Home, then down, then Enter opens the record of the second row.
  const last = table.locator("tbody tr.k-table-row").last();
  await last.focus();
  await page.keyboard.press("Home");
  await expect(table.locator('tbody tr[data-index="0"]')).toBeFocused();
  await page.keyboard.press("ArrowDown");
  const second = await page.evaluate(() => document.activeElement?.getAttribute("aria-rowindex"));
  expect(second).toBe("3");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses\/[0-9a-f-]+$/);
  await expect(page.getByRole("heading", { level: 1, name: /^Adresse / })).toBeVisible();
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
  await page.goto("/addresses");
  const scroller = page.locator(".k-table-scroll");
  await expect(page.getByRole("table", { name: "Adresses" }).getByRole("row").nth(1)).toBeVisible();
  await scroller.evaluate((element) => element.scrollTo(0, element.scrollHeight));
  await expect(
    page.getByRole("status").filter({ hasText: "la liste repart du début" }),
  ).toBeVisible();
  expect(refused).toBe(1);
  // Read again from the start: its first row is there.
  await scroller.evaluate((element) => element.scrollTo(0, 0));
  await expect(page.getByRole("table", { name: "Adresses" }).getByRole("row").nth(1)).toBeVisible();
  await expectAccessible(page);
});
