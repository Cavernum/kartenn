import AxeBuilder from "@axe-core/playwright";
import { test as base, expect, type Page } from "@playwright/test";

/** The fixed instant of every capture (design « Régression visuelle »). */
export const FIXED_TIME = new Date("2026-01-15T10:00:00+01:00");

/**
 * The widths of the captures of the mock-ups and screens, at a height of 900 px
 * (REQ-020-26 revised by the 024, REQ-024-27); the states of the components stay at the
 * 1280 × 800 of the `visual` project. 390 px shows the compact class of the frame (spec 020,
 * « Adaptation à la largeur »).
 */
export const SCREEN_WIDTHS = [1440, 1280, 1024, 768, 390] as const;
export const SCREEN_HEIGHT = 900;

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
 * A state of the catalogue for a keyboard script, with the real clock: Vue drops an event
 * that reaches a handler attached at the same instant (`Date.now()`, its guard against
 * handlers added while an event propagates), and the frozen clock of the captures makes
 * every instant the same one (kartenn-ui, « Pièges de Reka UI »).
 */
export async function openState(page: Page, component: string, name: string): Promise<void> {
  await page.goto(`/_ui/c/${component}?etat=${name}`);
  await page.locator("[data-state-name]").first().waitFor();
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
  // A layer fades in, even under reduced motion (spec 020, « Mouvement »): the finite
  // animations and transitions end first, so that axe reads the colours of an opaque layer.
  await page.evaluate(() =>
    Promise.all(
      document
        .getAnimations()
        .filter((a) => Number.isFinite(Number(a.effect?.getComputedTiming().endTime)))
        .map((a) => a.finished.catch(() => undefined)),
    ),
  );
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
