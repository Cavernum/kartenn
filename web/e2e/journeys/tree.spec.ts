import { expect, expectAccessible, test } from "../fixtures/index.ts";

// The tree of the prefixes (spec 007, task 18; REQ-007-17, 31, 33): roots of the domain,
// children read on demand page by page, rows virtualised, the `treeview` keys of the APG.

/** The row of a node, by the start of its label. */
const item = (tree: import("@playwright/test").Locator, value: string) =>
  tree.getByRole("treeitem", { name: new RegExp(`^${value.replace(/[./]/g, "\\$&")}\\b`) });

/** The rows in the DOM: a few hundreds at most, whatever the size of the tree. */
const MAX_ROWS = 300;
/**
 * A page of the 10⁴ children takes about 2 s, the first up to some 20 s on a busy machine:
 * the server computes every child before the page (see the report of task 18).
 */
const LOAD = 60_000;

test("the tree opens on demand, reads pages as it scrolls, and goes by the keyboard", async ({
  page,
  login,
}) => {
  test.setTimeout(4 * LOAD);
  await login("bob");
  await page.goto("/");
  const tree = page.getByRole("tree", { name: "Préfixes du domaine production" });
  const root = item(tree, "10.0.0.0/8");
  await expect(root).toBeVisible();
  await expect(root).toHaveAttribute("aria-expanded", "false");
  await expectAccessible(page);

  // Right opens, right again enters the first child (aria-activedescendant).
  await tree.focus();
  await page.keyboard.press("ArrowRight");
  await expect(root).toHaveAttribute("aria-expanded", "true");
  await expect(item(tree, "10.3.0.0/16")).toBeVisible({ timeout: LOAD });
  await page.keyboard.press("ArrowRight");
  await expect(tree).toHaveAttribute(
    "aria-activedescendant",
    (await item(tree, "10.1.0.0/24").getAttribute("id"))!,
  );

  // Down to the wide container, opened: its first page, of unknown size.
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  const wide = item(tree, "10.3.0.0/16");
  await expect(tree).toHaveAttribute("aria-activedescendant", (await wide.getAttribute("id"))!);
  await page.keyboard.press("ArrowRight");
  await expect(wide).toHaveAttribute("aria-expanded", "true");
  const first = item(tree, "10.3.0.0/30");
  await expect(first).toBeVisible({ timeout: LOAD });
  await expect(first).toHaveAttribute("aria-level", "3");
  await expect(first).toHaveAttribute("aria-setsize", "-1");
  await expectAccessible(page);

  // End goes to the last row read: the next page comes as it nears the window.
  await page.keyboard.press("End");
  await expect(item(tree, "10.3.1.144/30")).toBeAttached({ timeout: LOAD });
  // Scrolling reads the pages after it; the DOM keeps a few hundred rows at most.
  await tree.hover();
  await expect(async () => {
    await page.mouse.wheel(0, 3_000);
    const ranks = await tree
      .getByRole("treeitem")
      .evaluateAll((rows) => rows.map((row) => Number(row.getAttribute("aria-posinset") ?? 0)));
    expect(Math.max(...ranks)).toBeGreaterThan(200);
  }).toPass({ timeout: LOAD });
  expect(await tree.getByRole("treeitem").count()).toBeLessThanOrEqual(MAX_ROWS);

  // Home back to the first row; left closes; Enter opens the record.
  await page.keyboard.press("Home");
  await expect(tree).toHaveAttribute("aria-activedescendant", (await root.getAttribute("id"))!);
  await page.keyboard.press("ArrowLeft");
  await expect(root).toHaveAttribute("aria-expanded", "false");
  await expect(tree.getByRole("treeitem")).toHaveCount(1);
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/prefixes\/[0-9a-f-]{36}$/);
});
