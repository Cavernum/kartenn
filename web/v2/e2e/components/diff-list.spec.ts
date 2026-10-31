import { expect, openState, test } from "../fixtures/index.ts";

// DiffList beyond 200 lines (spec 020, design « Inventaire » line 29): virtualised, it
// scrolls in its own place and holds only the changes near the view, each in its place in
// its object (`aria-setsize`, `aria-posinset`); every object heading stays in the document,
// so that Tab and the navigation by headings reach the last object whatever the scroll.

test("420 lines: a few changes in the document, all the headings, Tab to the last", async ({
  page,
}) => {
  await openState(page, "DiffList", "long");
  const list = page.locator("[data-state-name='long'] .k-diff-list");
  await expect(list).toHaveAttribute("data-virtual", "true");
  await expect(list.getByRole("heading", { level: 3 })).toHaveCount(60);
  // It scrolls in its place: nothing of it (hidden words included) stretches the page.
  const height = await page.evaluate(() => document.documentElement.scrollHeight);
  expect(height).toBeLessThan(1200);
  const rendered = await list.locator(".k-diff-list__change").count();
  expect(rendered).toBeGreaterThan(0);
  expect(rendered).toBeLessThan(60);
  const first = list.locator(".k-diff-list__change").first();
  await expect(first).toHaveAttribute("aria-setsize", "6");
  await expect(first).toHaveAttribute("aria-posinset", "1");
  // The scroll of the list brings the last changes into the document.
  await list.evaluate((el) => (el.scrollTop = el.scrollHeight));
  await expect(list.getByText("Sous-réseau 60")).toBeInViewport();
  const last = list
    .locator("section")
    .last()
    .locator(".k-diff-list__change")
    .filter({ hasText: "Attribut 6" });
  await expect(last).toBeVisible();
  await expect(last).toHaveAttribute("aria-posinset", "6");
  // Back to the top; the keyboard goes from heading to heading down to the last object.
  await list.evaluate((el) => (el.scrollTop = 0));
  const toggles = list.getByRole("button", { expanded: true });
  await toggles.first().focus();
  for (let i = 1; i < 60; i++) await page.keyboard.press("Tab");
  await expect(list.getByRole("button", { name: /Sous-réseau 60/ })).toBeFocused();
  await expect(list.getByRole("button", { name: /Sous-réseau 60/ })).toBeInViewport();
});

test("closing an object takes its lines out; under 200, all of them are rendered", async ({
  page,
}) => {
  await openState(page, "DiffList", "open");
  const list = page.locator("[data-state-name='open'] .k-diff-list");
  await expect(list).not.toHaveAttribute("data-virtual");
  await expect(list.locator(".k-diff-list__change")).toHaveCount(6);
  await list.getByRole("button", { name: /Serveurs de bases/ }).focus();
  await page.keyboard.press("Enter");
  await expect(list.getByRole("button", { name: /Serveurs de bases/ })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  await expect(list.locator(".k-diff-list__change:visible")).toHaveCount(3);
});
