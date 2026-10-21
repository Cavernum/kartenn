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
  /** Errors a journey provokes on purpose: exceptions and console errors that match. */
  toleratedErrors: RegExp[];
  /** CSP violations a journey provokes on purpose (`<directive> <blocked URI>`). */
  toleratedCsp: RegExp[];
};

/**
 * The test plugin `throws` (web/e2e/plugins) fails while loading in every journey: the
 * shell sets it aside (REQ-007-10), and its exception is no failure of the journey.
 */
const PLUGIN_THROWS = /e2e plugin throws while loading/;

/**
 * A module of a plugin whose download the browser cut because the page was leaving:
 * Firefox then reports its digest as that of no content at all (the SHA-384 of the empty
 * string). Any other mismatch is a real one.
 */
const CUT_SHORT =
  /computed hash is “OLBgp1GsljhM2TJ\+sbHjaiH9txEUvgdDTAzHv2P24donTt6\/529l\+9Ua0vFImLlb”/;

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
  toleratedErrors: [[], { option: true }],
  toleratedCsp: [[], { option: true }],
  page: async ({ page, toleratedErrors, toleratedCsp }, use) => {
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
        // Firefox names an object argument `JSHandle@object`: say where it came from.
        const { url, lineNumber } = message.location();
        errors.push(`console: ${message.text()} (${url}:${lineNumber})`);
      }
    });
    await use(page);
    await expectNoCspViolation(page, toleratedCsp);
    const tolerated = [PLUGIN_THROWS, CUT_SHORT, ...toleratedErrors];
    expect(
      errors.filter((error) => !tolerated.some((pattern) => pattern.test(error))),
      "uncaught exceptions and console errors",
    ).toEqual([]);
  },
  login: async ({ page }, use) => {
    await use((user) => loginIn(page, user));
  },
});

/** Logs `user` in on `page`, of any browser context (a second one, to share a link). */
export async function loginIn(page: Page, user: keyof typeof USERS): Promise<void> {
  await page.goto(`${BASE_URL}/v1/auth/login?issuer=dev`);
  await page.getByRole("button", { name: USERS[user] }).click();
  // The callback sends the browser back to the shell (`/`).
  await page.waitForURL(`${BASE_URL}/`);
}

/** No CSP violation reported by the current document, but those `tolerated`. */
export async function expectNoCspViolation(page: Page, tolerated: RegExp[] = []): Promise<void> {
  if (page.isClosed()) return;
  const violations = await page.evaluate(() => window.__kartennCsp ?? []).catch(() => []);
  expect(
    violations.filter((v) => !tolerated.some((pattern) => pattern.test(v))),
    "CSP violations",
  ).toEqual([]);
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
