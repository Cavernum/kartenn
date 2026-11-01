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
  carol: "Carol Exemple",
} as const;

/** Roles bound to the groups of users.toml, on the whole organisation (global-setup.ts). */
export const GROUPS: Record<string, readonly string[]> = {
  "kartenn-admins": ["administrator"],
  lecteurs: ["reader"],
  redacteurs: ["redacteur"],
};

/**
 * A role of the journeys (global-setup.ts): the reads and the edition of the referential,
 * without `approval.self` — what Carol may propose, never apply herself (REQ-021-37).
 */
export const EDITOR_WITHOUT_APPLY = {
  key: "redacteur",
  labels: { fr: "Rédacteur sans application" },
  entries: [
    "core.entity.read",
    "core.site.read",
    "core.domain.read",
    "core.prefix.read",
    "core.range.read",
    "core.address.read",
    "core.prefix.edit",
    "core.range.edit",
    "core.address.edit",
  ],
} as const;

/**
 * The objects of the referential the journeys read (global-setup.ts), in documentation
 * addresses (RFC 5737): what the palette finds by value and by name (REQ-021-3).
 */
export const DATA = {
  entity: "exemple",
  domain: "production",
  prefix: "192.0.2.0/24",
  address: "192.0.2.10",
  /** A network with 1 000 addresses: a long list (REQ-021-22), in RFC 3849 space. */
  bulk: { prefix: "2001:db8:2::/64", count: 1000 },
  /** A container with 1 000 direct children: the virtualised tree, its pages. */
  wide: { prefix: "2001:db8:3::/48", count: 1000 },
  /** A group of VLAN anchored on the domain, and one VLAN in it (026). */
  vlan: { group: "campus", vid: 10, name: "serveurs" },
} as const;

/** The addresses of the long list: 2001:db8:2::1 onwards. */
export function bulkAddresses(): string[] {
  return Array.from({ length: DATA.bulk.count }, (_, i) => `2001:db8:2::${(i + 1).toString(16)}`);
}

/** The children of the wide container: /64 networks, 2001:db8:3::/64 onwards. */
export function widePrefixes(): string[] {
  return Array.from({ length: DATA.wide.count }, (_, i) => `2001:db8:3:${i.toString(16)}::/64`);
}

type Guarded = {
  /** Logs `user` in through the page of the development issuer, back on the shell. */
  login: (user: keyof typeof USERS) => Promise<void>;
  /** Errors a journey provokes on purpose: exceptions and console errors that match. */
  toleratedErrors: RegExp[];
  /** CSP violations a journey provokes on purpose (`<directive> <blocked URI>`). */
  toleratedCsp: RegExp[];
};

/**
 * The test plugin `throws` (web/v2/e2e/plugins) throws while its module loads, in every
 * session: set aside by the shell, its exception is the subject of the journey `plugins`.
 */
const PLUGIN_THROWS = /e2e plugin throws while loading/;

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
      expect(
        violations.filter((v) => !toleratedCsp.some((pattern) => pattern.test(v))),
        "CSP violations",
      ).toEqual([]);
    }
    const tolerated = [PLUGIN_THROWS, ...toleratedErrors];
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
