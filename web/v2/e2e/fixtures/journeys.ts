import { test as base, expect, type Page } from "@playwright/test";
import { BASE_URL } from "./env.ts";

export { expect };
export { expectAccessible } from "./index.ts";

// The fixtures of the journeys against kartenn-server (spec 021, REQ-021-31), taken from
// web/e2e/fixtures: users of the development issuer, login through its page, and the guard
// of every journey (CSP violation, uncaught exception, unexpected console error).

/** Users of web/e2e/users.toml, by their subject. */
export const USERS = {
  alice: "Alice Exemple",
  bob: "Bob Exemple",
} as const;

/** Roles bound to the groups of users.toml, on the whole organisation (global-setup.ts). */
export const GROUPS: Record<string, readonly string[]> = {
  "kartenn-admins": ["administrator"],
  lecteurs: ["reader"],
};

type Guarded = {
  /** Logs `user` in through the page of the development issuer, back on the shell. */
  login: (user: keyof typeof USERS) => Promise<void>;
  /** Errors a journey provokes on purpose: exceptions and console errors that match. */
  toleratedErrors: RegExp[];
};

declare global {
  interface Window {
    __kartennCsp?: string[];
  }
}

/**
 * Every journey fails on a CSP violation reported by a page, on an uncaught exception and
 * on an unexpected console error. A failed resource load (a 401 of `/v1/me` signed out) is
 * the subject of a journey, not of this guard.
 */
export const test = base.extend<Guarded>({
  toleratedErrors: [[], { option: true }],
  page: async ({ page, toleratedErrors }, use) => {
    await page.addInitScript(() => {
      window.__kartennCsp = [];
      document.addEventListener("securitypolicyviolation", (event) => {
        window.__kartennCsp?.push(`${event.effectiveDirective} ${event.blockedURI}`);
      });
    });
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(`exception: ${error.message}`));
    page.on("console", (message) => {
      // Pages of kartenn only: the issuer's page is not under test.
      if (
        message.type() === "error" &&
        page.url().startsWith(BASE_URL) &&
        !/Failed to load resource/.test(message.text())
      ) {
        const { url, lineNumber } = message.location();
        errors.push(`console: ${message.text()} (${url}:${lineNumber})`);
      }
    });
    await use(page);
    if (!page.isClosed()) {
      const violations = await page.evaluate(() => window.__kartennCsp ?? []).catch(() => []);
      expect(violations, "CSP violations").toEqual([]);
    }
    expect(
      errors.filter((error) => !toleratedErrors.some((pattern) => pattern.test(error))),
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
