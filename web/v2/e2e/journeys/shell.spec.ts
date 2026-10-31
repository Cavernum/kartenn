import type { Page } from "@playwright/test";
import { expect, expectAccessible, test } from "../fixtures/journeys.ts";

// The frame of the new shell (spec 021, task 9; REQ-021-1, 2, 4, 6): the skip link first,
// the spaces by rights, the breadcrumb, the sidebar collapsed and kept, the settings of the
// user menu kept, the draft counter as the toggle of the right panel, no shift from the
// skeleton of index.html (CLS ≤ 0.05), the focus never hidden by the bar or the band
// (2.4.11). Ported from web/e2e/journeys/shell.spec.ts.

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

test("the skip link is the first stop and leads to the content", async ({ page, login }) => {
  await login("bob");
  await expect(page.getByRole("heading", { level: 1, name: "Accueil" })).toBeVisible();
  await page.keyboard.press("Tab");
  const skip = page.getByRole("link", { name: "Aller au contenu" });
  await expect(skip).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("main")).toBeFocused();
});

test("a reader sees the referential and the network, not the administration", async ({
  page,
  login,
}) => {
  await login("bob");
  const nav = navigation(page);
  await expect(nav.getByRole("link", { name: "Préfixes" })).toBeVisible();
  await expect(nav.getByRole("button", { name: "Réseau" })).toBeVisible();
  await expect(nav.getByRole("button", { name: "Administration" })).toHaveCount(0);
  await expect(nav.getByRole("button", { name: "Changesets" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /^Date de lecture/ })).toBeVisible();
  await expect(page.getByRole("button", { name: "Compte de Bob Exemple" })).toBeVisible();
  await expectAccessible(page);
});

test("an administrator reaches the administration; the breadcrumb links its space", async ({
  page,
  login,
}) => {
  await login("alice");
  const nav = navigation(page);
  await nav.getByRole("button", { name: "Administration" }).click();
  await nav.getByRole("link", { name: "Rôles" }).click();
  await expect(page).toHaveURL(/\/admin\/roles$/);
  await expect(page.getByRole("heading", { level: 1, name: "Rôles" })).toBeVisible();
  await expect(nav.getByRole("link", { name: "Rôles" })).toHaveAttribute("aria-current", "page");
  const crumbs = page.getByRole("navigation", { name: "Fil d'Ariane" });
  await expect(page).toHaveTitle("Rôles — kartenn");
  await crumbs.getByRole("link", { name: "Administration" }).click();
  await expect(page).toHaveURL(/\/admin\/groups$/);
  await expect(page.getByRole("heading", { level: 1, name: "Groupes" })).toBeVisible();
  await expectAccessible(page);
});

test("the sidebar collapsed stays collapsed after a reload", async ({ page, login }) => {
  await login("bob");
  const nav = navigation(page);
  await nav.getByRole("button", { name: "Barre latérale" }).click();
  await expect(nav).toHaveAttribute("data-state", "collapsed");
  // Collapsed, each entry keeps its name.
  await expect(nav.getByRole("link", { name: "Préfixes" })).toBeVisible();
  await page.reload();
  await expect(navigation(page)).toHaveAttribute("data-state", "collapsed");
  await navigation(page).getByRole("button", { name: "Barre latérale" }).click();
  await expect(navigation(page)).toHaveAttribute("data-state", "expanded");
});

test("language, theme and density are chosen from the user menu and kept", async ({
  page,
  login,
}) => {
  await login("bob");
  await page.getByRole("button", { name: "Compte de Bob Exemple" }).click();
  await expectAccessible(page);
  await page.getByRole("menuitemradio", { name: "English" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Home" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");

  await page.getByRole("button", { name: "Account of Bob Exemple" }).click();
  await page.getByRole("menuitemradio", { name: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: "Account of Bob Exemple" }).click();
  await page.getByRole("menuitemradio", { name: "Comfortable" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-density", "comfortable");
  await expectAccessible(page);

  await page.reload();
  await expect(page.getByRole("heading", { level: 1, name: "Home" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(page.locator("html")).toHaveAttribute("data-density", "comfortable");
});

test("the draft counter opens the panel on its tab, then closes it", async ({ page, login }) => {
  await login("alice");
  const counter = page.getByRole("button", { name: /^Brouillon\s*\d+\s*opérations?$/ });
  await expect(counter).toHaveAttribute("aria-expanded", "false");
  await counter.click();
  const panel = page.getByRole("complementary", { name: "Panneau droit" });
  await expect(panel.getByRole("tab", { name: /^Brouillon/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(counter).toHaveAttribute("aria-expanded", "true");
  await expect(counter).toHaveAttribute("aria-controls", (await panel.getAttribute("id"))!);
  await expectAccessible(page);
  await counter.click();
  await expect(counter).toHaveAttribute("aria-expanded", "false");
  await expect(panel.getByRole("tab")).toHaveCount(0);
});

test("the frame replaces the skeleton without a shift", async ({ page, login, browserName }) => {
  test.skip(browserName !== "chromium", "Layout Instability API: Chromium only");
  await login("bob");
  await page.addInitScript(() => {
    const shifts: number[] = [];
    (window as unknown as { shifts: number[] }).shifts = shifts;
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries() as unknown as {
        value: number;
        hadRecentInput: boolean;
      }[])
        if (!entry.hadRecentInput) shifts.push(entry.value);
    }).observe({ type: "layout-shift", buffered: true });
  });
  await page.goto("/prefixes");
  await expect(page.getByRole("heading", { level: 1, name: "Préfixes" })).toBeVisible();
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  // A moment for any late shift (the drafts, the fonts).
  await page.waitForTimeout(500);
  const cls = await page.evaluate(() =>
    (window as unknown as { shifts: number[] }).shifts.reduce((a, b) => a + b, 0),
  );
  expect(cls).toBeLessThanOrEqual(0.05);
});

test("neither the bar nor the band hides the focus in the surface", async ({ page, login }) => {
  await login("bob");
  await page.goto("/prefixes?as_of=2026-01-01T00%3A00%3A00Z");
  await expect(page.getByRole("status").filter({ hasText: "lecture seule" })).toBeVisible();
  // A surface longer than the window: its links, reached by Tab one after the other.
  await page.getByRole("main").evaluate((main) => {
    for (let i = 0; i < 40; i++) {
      const link = document.createElement("a");
      link.href = "#k-surface";
      link.textContent = `Lien ${i}`;
      link.className = "k-test-link";
      const line = document.createElement("p");
      line.append(link);
      main.append(line);
    }
  });
  await page.locator(".k-test-link").first().focus();
  for (let i = 0; i < 40; i++) {
    // The point at the centre of the focused element is the element: nothing covers it.
    const covered = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement;
      const box = el.getBoundingClientRect();
      const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
      return hit === el || el.contains(hit) ? null : `${el.textContent} under ${hit?.className}`;
    });
    expect(covered).toBeNull();
    await page.keyboard.press("Tab");
  }
});
