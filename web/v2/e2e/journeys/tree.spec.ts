import type { Locator, Page } from "@playwright/test";
import { DATA, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// The topography of the new shell (spec 021, task 11; REQ-021-9, 22), ported from
// web/e2e/journeys/tree.spec.ts: the first level of the domain, the children of a node read
// on demand page by page, rows virtualised, the keys of the APG treegrid, the content of the
// chosen node beside the tree, its record by Enter.

/** The row of a node, by its value. */
const row = (tree: Locator, value: string) =>
  tree.locator("[role='row'][data-node]").filter({
    has: tree.page().getByText(value, { exact: true }),
  });

/** The rows in the DOM: a few hundreds at most, whatever the size of the tree. */
const MAX_ROWS = 300;

async function topology(page: Page): Promise<Locator> {
  await page.goto("/topology");
  await expect(page.getByRole("heading", { level: 1, name: "Topographie" })).toBeVisible();
  const tree = page.getByRole("treegrid", { name: `Arbre des préfixes du domaine ${DATA.domain}` });
  await expect(row(tree, DATA.prefix)).toBeVisible();
  return tree;
}

test("the tree opens on demand, reads pages as it scrolls, and goes by the keyboard", async ({
  page,
  login,
}) => {
  await login("bob");
  const tree = await topology(page);
  const wide = row(tree, DATA.wide.prefix);
  await expect(wide).toHaveAttribute("aria-expanded", "false");
  await expectAccessible(page);

  // A click chooses the node: its content beside the tree, its id in the URL.
  await wide.click();
  await expect(page).toHaveURL(/[?&]node=[0-9a-f-]{36}/);
  const content = page.getByRole("region", { name: DATA.wide.prefix });
  await expect(content.getByRole("link", { name: "2001:db8:3::/64" })).toBeVisible();

  // Right opens the node: its first page, of unknown size.
  await page.keyboard.press("ArrowRight");
  await expect(wide).toHaveAttribute("aria-expanded", "true");
  const first = row(tree, "2001:db8:3::/64");
  await expect(first).toBeVisible();
  await expect(first).toHaveAttribute("aria-level", "2");
  await expect(first).toHaveAttribute("aria-setsize", "-1");
  await expectAccessible(page);

  // Scrolling reads the pages after it; the DOM keeps a few hundred rows at most.
  await tree.hover();
  await expect(async () => {
    await page.mouse.wheel(0, 3_000);
    const ranks = await tree
      .locator("[role='row'][aria-posinset]")
      .evaluateAll((rows) => rows.map((r) => Number(r.getAttribute("aria-posinset") ?? 0)));
    expect(Math.max(...ranks)).toBeGreaterThan(200);
  }).toPass({ timeout: 30_000 });
  expect(await tree.locator("[role='row']").count()).toBeLessThanOrEqual(MAX_ROWS);

  // The focus stayed on the wide node (kept rendered): Ctrl+Home back to the first row,
  // Enter opens the record of its node.
  await page.keyboard.press("Control+Home");
  await expect(row(tree, DATA.prefix).locator("[role='gridcell']").first()).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/prefixes\/[0-9a-f-]{36}$/);
  await expect(
    // Its kind in the words of the dictionary, then its name.
    page.getByRole("heading", {
      level: 1,
      name: new RegExp(`\\s${DATA.prefix.replaceAll(".", "\\.")}$`),
    }),
  ).toBeVisible();
});

test("the chosen node shows its content, and the URL keeps it", async ({ page, login }) => {
  await login("bob");
  const tree = await topology(page);
  await row(tree, DATA.prefix).click();
  const content = page.getByRole("region", { name: DATA.prefix });
  await expect(content.getByRole("link", { name: DATA.address })).toBeVisible();
  const url = page.url();
  await page.reload();
  await expect(page.getByRole("region", { name: DATA.prefix })).toBeVisible();
  expect(page.url()).toBe(url);
  await expect(row(tree, DATA.prefix)).toHaveAttribute("aria-selected", "true");
});
