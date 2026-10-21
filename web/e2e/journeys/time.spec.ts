import { expect, expectAccessible, loginIn, test } from "../fixtures/index.ts";

// The dated context (spec 007, task 16; REQ-007-22 to 24): carried by the URL, shown by a
// banner, writes off under as_of.

const VALID_AT = "2026-01-01T00:00:00Z";
const AS_OF = "2026-06-30T12:00:00Z";
const DATED = `valid_at=${encodeURIComponent(VALID_AT)}&as_of=${encodeURIComponent(AS_OF)}`;

/** The identifier of the prefix `value`, read by the page itself. */
async function prefixId(page: import("@playwright/test").Page, value: string): Promise<string> {
  return page.evaluate(
    async (url) => {
      const response = await fetch(url);
      return ((await response.json()) as { items: { id: string }[] }).items[0]!.id;
    },
    `/v1/prefixes?value=${encodeURIComponent(value)}`,
  );
}

test("a dated link shows the same dates to whoever opens it", async ({ page, login, browser }) => {
  await login("bob");
  await page.goto(`/about?${DATED}`);
  const banner = page.getByRole("status").filter({ hasText: "Revenir au présent" });
  await expect(banner.locator(`time[datetime="${VALID_AT}"]`)).toBeVisible();
  await expect(banner.locator(`time[datetime="${AS_OF}"]`)).toBeVisible();
  await expectAccessible(page);

  // Internal navigation keeps the dates.
  await page
    .getByRole("navigation", { name: "Navigation principale" })
    .getByRole("link", {
      name: "Arbre",
    })
    .click();
  await expect(page.getByRole("heading", { level: 1, name: "Arbre des préfixes" })).toBeVisible();
  const link = new URL(page.url());
  expect(link.pathname).toBe("/");
  expect(link.searchParams.get("valid_at")).toBe(VALID_AT);
  expect(link.searchParams.get("as_of")).toBe(AS_OF);

  // A second browser context, another session: the same dates from the link alone.
  const other = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  try {
    const second = await other.newPage();
    await loginIn(second, "alice");
    await second.goto(link.href);
    const shown = second.getByRole("status").filter({ hasText: "Revenir au présent" });
    await expect(shown.locator(`time[datetime="${VALID_AT}"]`)).toBeVisible();
    await expect(shown.locator(`time[datetime="${AS_OF}"]`)).toBeVisible();
  } finally {
    await other.close();
  }

  await banner.getByRole("button", { name: "Revenir au présent" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("button", { name: "Revenir au présent" })).toHaveCount(0);
});

test("under as_of, writes are off", async ({ page, login }) => {
  await login("alice");
  const prefix = await prefixId(page, "10.1.0.0/24");
  await page.goto(`/prefixes/${prefix}`);
  await expect(page.getByRole("button", { name: "Actions" })).toBeEnabled();

  await page.goto(`/prefixes/${prefix}?as_of=${encodeURIComponent(AS_OF)}`);
  await expect(page.getByRole("button", { name: "Actions" })).toBeDisabled();
  await expectAccessible(page);

  // A valid_at off the present leaves the draft, hence the actions.
  await page.goto(`/prefixes/${prefix}?valid_at=${encodeURIComponent(VALID_AT)}`);
  await expect(page.getByRole("button", { name: "Actions" })).toBeEnabled();
});
