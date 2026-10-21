import type { Page } from "@playwright/test";
import { ISSUER } from "../fixtures/env.ts";
import { expect, expectAccessible, test, USERS } from "../fixtures/index.ts";

// The session (spec 007, task 14; REQ-007-28): login page, back to the same URL after a
// login, a 401 that offers to log in again, a logout that clears what is sensitive.

const VALID_AT = "2026-01-01T00:00:00Z";

async function prefixId(page: Page, value: string): Promise<string> {
  return page.evaluate(
    async (url) => {
      const response = await fetch(url);
      return ((await response.json()) as { items: { id: string }[] }).items[0]!.id;
    },
    `/v1/prefixes?value=${encodeURIComponent(value)}`,
  );
}

/** Picks `user` on the page of the development issuer. */
async function chooseAtIssuer(page: Page, user: keyof typeof USERS): Promise<void> {
  await page.waitForURL(`${ISSUER}/**`);
  await page.getByRole("button", { name: USERS[user] }).click();
}

test("without a session, the login page, then back to the same record and dates", async ({
  page,
  login,
  context,
}) => {
  await login("alice");
  const record = `/prefixes/${await prefixId(page, "10.1.0.0/24")}?valid_at=${encodeURIComponent(VALID_AT)}`;
  await page.goto(record);
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();

  // The session is gone (expired, or its cookie dropped): the page asks for a login.
  await context.clearCookies();
  await page.reload();
  await expect(page.getByRole("heading", { level: 1, name: "Connexion" })).toBeVisible();
  await expectAccessible(page);
  await page.getByRole("button", { name: "Se connecter par dev" }).click();
  await chooseAtIssuer(page, "alice");

  // The callback lands on `/`, which goes on to the URL kept: same record, same dates.
  await page.waitForURL((url) => url.pathname.startsWith("/prefixes/"));
  const back = new URL(page.url());
  expect(back.pathname).toBe(new URL(record, page.url()).pathname);
  expect(back.searchParams.get("valid_at")).toBe(VALID_AT);
  await expect(page.getByRole("button", { name: "Revenir au présent" })).toBeVisible();
});

test("a 401 in use opens « Session expirée », and logging in again comes back", async ({
  page,
  login,
  context,
}) => {
  await login("alice");
  const record = `/prefixes/${await prefixId(page, "10.1.0.0/24")}?valid_at=${encodeURIComponent(VALID_AT)}`;
  // Through the login page this time, so that the issuer is remembered.
  await context.clearCookies();
  await page.goto(record);
  await page.getByRole("button", { name: "Se connecter par dev" }).click();
  const read = page.waitForResponse((r) => new URL(r.url()).pathname.startsWith("/v1/prefixes/"));
  await chooseAtIssuer(page, "alice");
  // Read with the new session (404: the prefix did not exist yet at that date).
  expect((await read).status()).not.toBe(401);

  await context.clearCookies();
  // A read of the record in another context: the API answers 401.
  await page.getByRole("button", { name: "Revenir au présent" }).click();
  const dialog = page.getByRole("alertdialog", { name: "Session expirée" });
  await expect(dialog).toBeVisible();
  await expectAccessible(page);
  await dialog.getByRole("button", { name: "Se reconnecter" }).click();
  // Straight to the remembered issuer.
  await chooseAtIssuer(page, "alice");
  await page.waitForURL((url) => url.pathname.startsWith("/prefixes/"));
  expect(new URL(page.url()).search).toBe("");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
});

test("logout clears what is sensitive and goes on to the issuer", async ({ page, login }) => {
  await login("alice");
  await page.evaluate(() => {
    localStorage.setItem("kartenn.locale", "fr");
    localStorage.setItem("kartenn.issuer", "dev");
    localStorage.setItem("kartenn.recent.searches", '["10.1.0.10"]');
    sessionStorage.setItem("kartenn.return", "/sites");
  });
  await page.getByRole("button", { name: /menu de l'utilisateur/ }).click();
  await page.getByRole("menuitem", { name: "Se déconnecter" }).click();
  await page.waitForURL(`${ISSUER}/end-session?**`);

  await page.goto("/login");
  await expect(page.getByRole("heading", { level: 1, name: "Connexion" })).toBeVisible();
  const kept = await page.evaluate(() => ({
    locale: localStorage.getItem("kartenn.locale"),
    issuer: localStorage.getItem("kartenn.issuer"),
    recent: localStorage.getItem("kartenn.recent.searches"),
    back: sessionStorage.getItem("kartenn.return"),
  }));
  expect(kept).toEqual({ locale: "fr", issuer: "dev", recent: null, back: null });
  expect(await page.evaluate(async () => (await fetch("/v1/me")).status)).toBe(401);
});
