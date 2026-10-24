import AxeBuilder from "@axe-core/playwright";
import { test as base, expect, type Page } from "@playwright/test";

/** The fixed instant of every capture (design « Régression visuelle »). */
export const FIXED_TIME = new Date("2026-01-15T10:00:00+01:00");

/**
 * The catalogue at `path`, in a fixed state: clock frozen, mode, density and language set
 * by the query, fonts loaded.
 */
export async function open(page: Page, path: string): Promise<void> {
  await page.clock.setFixedTime(FIXED_TIME);
  await page.goto(path);
  await page.locator("main").waitFor();
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
}

/**
 * One state, one capture (REQ-020-26) and one snapshot of the accessibility tree
 * (REQ-020-27), named after the state.
 */
export async function snap(page: Page, state: string): Promise<void> {
  await expect(page).toHaveScreenshot(`${state}.png`, { fullPage: true });
  await expect(page.locator("main")).toMatchAriaSnapshot({ name: `${state}.aria.yml` });
}

/**
 * No axe-core violation for WCAG 2.0 to 2.2, A and AA (REQ-020-28). `preload: false`: axe
 * copies the sheets into `<style>` elements, which the CSP refuses (as in web/e2e).
 * `disabled`: rules left out, each with its reason at the call.
 */
export async function expectAccessible(page: Page, disabled: string[] = []): Promise<void> {
  const results = await new AxeBuilder({ page })
    .options({ preload: false })
    .disableRules(disabled)
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(
    results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`),
    "axe-core violations",
  ).toEqual([]);
}

/** The CSP violations a page raised, recorded from its first script (REQ-020-14). */
export const test = base.extend<{ violations: string[] }>({
  violations: async ({ page }, use) => {
    const violations: string[] = [];
    await page.exposeFunction("__kartennViolation", (line: string) => violations.push(line));
    await page.addInitScript(() => {
      document.addEventListener("securitypolicyviolation", (event) => {
        const report = (window as unknown as { __kartennViolation(line: string): void })
          .__kartennViolation;
        report(
          `${event.violatedDirective} ${event.blockedURI} ${event.sourceFile}:${event.lineNumber}`,
        );
      });
    });
    await use(violations);
  },
});

export { expect };
