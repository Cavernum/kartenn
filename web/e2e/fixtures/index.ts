import AxeBuilder from "@axe-core/playwright";
import { test as base, expect, type Page } from "@playwright/test";
import { BASE_URL } from "./env.ts";

export { expect };

/** Users of users.toml, by their subject. */
export const USERS = {
  alice: "Alice Exemple",
  bob: "Bob Exemple",
  refuse: "Refus d'accès",
} as const;

type Guarded = {
  /** Logs `user` in through the page of the development issuer, back on the shell. */
  login: (user: keyof typeof USERS) => Promise<void>;
};

declare global {
  interface Window {
    __kartennCsp?: string[];
  }
}

/**
 * Every journey fails on a CSP violation reported by a page (`securitypolicyviolation`), on
 * an uncaught exception and on an unexpected console error (design « Bout en bout »). A
 * failed resource load is the subject of the journey itself, not of this guard.
 */
export const test = base.extend<Guarded>({
  page: async ({ page }, use) => {
    await page.addInitScript(() => {
      window.__kartennCsp = [];
      document.addEventListener("securitypolicyviolation", (event) => {
        window.__kartennCsp?.push(`${event.effectiveDirective} ${event.blockedURI}`);
      });
    });
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(`exception: ${error.message}`));
    page.on("console", (message) => {
      // Pages of kartenn only: the issuer's page is not under test (its favicon, its CSP).
      if (
        message.type() === "error" &&
        page.url().startsWith(BASE_URL) &&
        !/Failed to load resource/.test(message.text())
      ) {
        errors.push(`console: ${message.text()}`);
      }
    });
    await use(page);
    await expectNoCspViolation(page);
    expect(errors, "uncaught exceptions and console errors").toEqual([]);
  },
  login: async ({ page }, use) => {
    await use(async (user) => {
      await page.goto("/v1/auth/login?issuer=dev");
      await page.getByRole("button", { name: USERS[user] }).click();
      // The callback sends the browser back to the shell (`/`).
      await page.waitForURL(`${BASE_URL}/`);
    });
  },
});

/** No CSP violation reported by the current document. */
export async function expectNoCspViolation(page: Page): Promise<void> {
  if (page.isClosed()) return;
  const violations = await page.evaluate(() => window.__kartennCsp ?? []).catch(() => []);
  expect(violations, "CSP violations").toEqual([]);
}

/** axe-core without violation of WCAG 2.2 A and AA (REQ-007-33, W14), blocking. */
export async function expectAccessible(page: Page): Promise<void> {
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  expect(
    results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`),
    "axe-core violations",
  ).toEqual([]);
}
