import { expect, openState, test } from "../fixtures/index.ts";

// Forced colours (spec 020, design « États », « Environnements »; REQ-020-7, 8, 23; WCAG
// 1.4.1, 1.4.11): the system paints backgrounds and colours, so that what a colour said
// must stand without it: a state keeps its border, its icon and its label; an
// environment its dot (a ring, its border) and its label; a skeleton the outline of its
// bars (a transparent border, painted by the system).

test.use({ forcedColors: "active" });

test("the dot of an environment keeps its ring; a state its border, icon and label", async ({
  page,
}) => {
  await openState(page, "EnvChip", "palette");
  const ring = await page
    .locator(".k-env-chip__dot")
    .first()
    .evaluate((el) => {
      const style = getComputedStyle(el);
      return { width: parseFloat(style.borderTopWidth), color: style.borderTopColor };
    });
  expect(ring.width).toBeGreaterThan(0);
  expect(ring.color).not.toBe("rgba(0, 0, 0, 0)");
  await expect(page.locator("[data-state-name='palette']")).toHaveScreenshot("env-chip.png");

  await openState(page, "StatusBadge", "all");
  const badge = page.locator(".k-status-badge").first();
  expect(
    await badge.evaluate((el) => parseFloat(getComputedStyle(el).borderTopWidth)),
  ).toBeGreaterThan(0);
  await expect(badge.locator("svg")).toBeVisible();
  await expect(page.locator("[data-state-name='all']")).toHaveScreenshot("status-badge.png");
});

test("a skeleton keeps the outline of its bars", async ({ page }) => {
  await openState(page, "Skeleton", "text");
  const color = await page
    .locator(".k-skeleton-fill")
    .first()
    .evaluate((el) => getComputedStyle(el).borderTopColor);
  expect(color).not.toBe("rgba(0, 0, 0, 0)");
  await expect(page.locator("[data-state-name='text']")).toHaveScreenshot("skeleton.png");
});
