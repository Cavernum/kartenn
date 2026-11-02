import type { Page } from "@playwright/test";
import { ISSUER } from "../fixtures/env.ts";
import { atRest, expect, expectAccessible, test, USERS } from "../fixtures/journeys.ts";

// The session of the new shell (spec 021, task 16; REQ-021-14, REQ-007-28): the login page,
// back to the same URL and dates after a login, a 401 in use that offers to log in again,
// a logout that clears what is sensitive, the end of the session said before it comes.
// Ported from web/e2e/journeys/session.spec.ts; the record of an object comes with task 11,
// a list stands for it here.

const VALID_AT = "2026-01-01T00:00:00Z";
const TARGET = `/prefixes?valid_at=${encodeURIComponent(VALID_AT)}`;

/** Picks `user` on the page of the development issuer. */
async function chooseAtIssuer(page: Page, user: keyof typeof USERS): Promise<void> {
  await page.waitForURL(`${ISSUER}/**`);
  await page.getByRole("button", { name: USERS[user] }).click();
}

const pill = (page: Page) => page.getByRole("button", { name: /^Date de lecture/ });

test("without a session, the login page, then back to the same page and dates", async ({
  page,
  login,
  context,
}) => {
  await login("alice");
  await page.goto(TARGET);
  await expect(page.getByRole("heading", { level: 1, name: "Préfixes" })).toBeVisible();
  // Its plugins loaded before the reload (task 029-4: Firefox reports a load the reload
  // cuts short as an integrity error).
  await atRest(page);

  // The session is gone (expired, or its cookie dropped): the page asks for a login.
  await context.clearCookies();
  await page.reload();
  await expect(page.getByRole("heading", { level: 1, name: "Connexion" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "Compte de secours" })).toBeVisible();
  await expectAccessible(page);
  await page.getByRole("button", { name: "Se connecter par dev" }).click();
  await chooseAtIssuer(page, "alice");

  // The callback lands on `/`, which goes on to the URL kept: same page, same dates.
  await page.waitForURL((url) => url.pathname === "/prefixes");
  expect(new URL(page.url()).searchParams.get("valid_at")).toBe(VALID_AT);
  await expect(page.getByRole("button", { name: "Revenir au présent" })).toBeVisible();
});

// Firefox says in its console that the event stream is refused once the cookie is gone.
test.describe(() => {
  test.use({ toleratedErrors: [/can.t establish a connection to the server at .*\/v1\/events/] });

  test("a 401 in use opens « Session expirée », and logging in again comes back", async ({
    page,
    login,
    context,
  }) => {
    await login("alice");
    // Through the login page this time, so that the issuer is remembered.
    await context.clearCookies();
    await page.goto(TARGET);
    await page.getByRole("button", { name: "Se connecter par dev" }).click();
    await chooseAtIssuer(page, "alice");
    await page.waitForURL((url) => url.pathname === "/prefixes");

    await context.clearCookies();
    // The next read of the API is answered 401: one the shell makes by itself (its data
    // layer), else the check of a period asked here.
    const dialog = page.getByRole("alertdialog", { name: "Session expirée" });
    const opened = await dialog
      .waitFor({ timeout: 3_000 })
      .then(() => true)
      .catch(() => false);
    if (!opened) {
      await pill(page).click();
      await page
        .getByRole("group", { name: /^Raccourcis\s:\sValide au$/ })
        .getByRole("button", { name: "Hier" })
        .click();
      await page
        .getByRole("dialog", { name: "Lire à une date" })
        .getByRole("button", { name: "Appliquer" })
        .click();
    }
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText("Vos brouillons restent au serveur");
    await expectAccessible(page);
    await dialog.getByRole("button", { name: "Se reconnecter" }).click();
    // Straight to the remembered issuer, then back to the same page.
    await chooseAtIssuer(page, "alice");
    await page.waitForURL((url) => url.pathname === "/prefixes");
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
  });
});

test("logout clears what is sensitive and goes on to the issuer", async ({ page, login }) => {
  await login("alice");
  await expect(page.getByRole("heading", { level: 1, name: "Accueil" })).toBeVisible();
  await page.evaluate(() => {
    localStorage.setItem("kartenn.locale", "fr");
    localStorage.setItem("kartenn.issuer", "dev");
    localStorage.setItem("kartenn.density", "comfortable");
    localStorage.setItem(
      "kartenn.recent.objects.0b5e3c1a-0000-4000-8000-000000000009",
      '[{"type":"prefix","id":"p1","value":"192.0.2.0/24","icon":"prefix","to":"/prefixes/p1"}]',
    );
    sessionStorage.setItem("kartenn.return", "/sites");
  });
  await page.getByRole("button", { name: "Compte de Alice Exemple" }).click();
  await page.getByRole("menuitem", { name: "Se déconnecter" }).click();
  await page.waitForURL(`${ISSUER}/end-session?**`);

  await page.goto("/login");
  await expect(page.getByRole("heading", { level: 1, name: "Connexion" })).toBeVisible();
  const kept = await page.evaluate(() => ({
    locale: localStorage.getItem("kartenn.locale"),
    issuer: localStorage.getItem("kartenn.issuer"),
    density: localStorage.getItem("kartenn.density"),
    recent: Object.keys(localStorage).filter((key) => key.startsWith("kartenn.recent.")),
    back: sessionStorage.getItem("kartenn.return"),
  }));
  expect(kept).toEqual({
    locale: "fr",
    issuer: "dev",
    density: "comfortable",
    recent: [],
    back: null,
  });
  expect(await page.evaluate(async () => (await fetch("/v1/me")).status)).toBe(401);
});

test("the end of the session is said before it comes, and staying pushes it back", async ({
  page,
  login,
}) => {
  await page.clock.install();
  await login("bob");
  await expect(page.getByRole("heading", { level: 1, name: "Accueil" })).toBeVisible();
  const ends = await page.evaluate(async () => {
    const me = (await (await fetch("/v1/me")).json()) as { session: { expires_at: string } };
    return Date.parse(me.session.expires_at) - Date.now();
  });
  // A minute and a half before the end, nothing done in between.
  await page.clock.fastForward(ends - 90_000);
  // The toast's title, after the tone said in hidden words (« Avertissement : »).
  const warning = page
    .getByRole("region", { name: "Notifications" })
    .getByText(/Votre session prend fin à/);
  await expect(warning).toBeVisible();
  await expectAccessible(page);
  await page.getByRole("button", { name: "Rester connecté" }).click();
  await expect(warning).toHaveCount(0);
  await expect(page.getByRole("heading", { level: 1, name: "Accueil" })).toBeVisible();
});

/** A request of the API to the shell, but the event stream and its pages. */
const isRead = (url: string) => {
  const path = new URL(url).pathname;
  return path.startsWith("/v1/") && !path.startsWith("/v1/events");
};

test("the reads of the background say so (Kartenn-Background), the reads asked do not", async ({
  page,
  login,
}) => {
  await page.clock.install();
  await login("bob");
  await expect(page.getByRole("heading", { level: 1, name: "Accueil" })).toBeVisible();
  // A minute without a gesture: the reads of the home page are stale, the user is away.
  await page.clock.fastForward(60_000);
  // A read of the home page still on its way (asked just after the login, its event late
  // under load) may come first: the one awaited is a read of the revalidation, marked.
  const background = page.waitForRequest(
    async (request) =>
      isRead(request.url()) && (await request.headerValue("kartenn-background")) === "?1",
  );
  // The tab hidden then shown again: the data layer revalidates on focus (no gesture).
  await page.evaluate(() => {
    for (const state of ["hidden", "visible"]) {
      Object.defineProperty(document, "visibilityState", { value: state, configurable: true });
      document.dispatchEvent(new Event("visibilitychange", { bubbles: true }));
    }
  });
  await background;

  // A navigation, clicked: the reads of its screen are the user's.
  const nav = page.getByRole("navigation", { name: "Navigation principale" });
  const asked = page.waitForRequest(
    (request) => new URL(request.url()).pathname === "/v1/prefixes",
  );
  await nav.getByRole("link", { name: "Préfixes" }).click();
  expect(await (await asked).headerValue("kartenn-background")).toBeNull();
  await expect(page.getByRole("heading", { level: 1, name: "Préfixes" })).toBeVisible();
});

test("the absolute end of the session is said before it comes, without staying", async ({
  page,
  login,
}) => {
  // A session at the end of its lifetime: `/v1/me` says its absolute end, the nearer one
  // (the server's shortest lifetime is an hour, too long for a journey).
  await page.route("**/v1/me", async (route) => {
    const response = await route.fetch();
    const me = (await response.json()) as {
      session: { expires_at: string; absolute_expires_at: string };
    };
    const end = new Date(Date.parse(me.session.expires_at) - 5 * 60_000).toISOString();
    me.session.expires_at = end;
    me.session.absolute_expires_at = end;
    await route.fulfill({ response, json: me });
  });
  await page.clock.install();
  await login("bob");
  await expect(page.getByRole("heading", { level: 1, name: "Accueil" })).toBeVisible();
  const ends = await page.evaluate(async () => {
    const me = (await (await fetch("/v1/me")).json()) as {
      session: { absolute_expires_at: string };
    };
    return Date.parse(me.session.absolute_expires_at) - Date.now();
  });
  await page.clock.fastForward(ends - 90_000);
  const notifications = page.getByRole("region", { name: "Notifications" });
  await expect(notifications.getByText(/au terme de sa durée maximale/)).toBeVisible();
  await expect(notifications).toContainText("Elle ne peut pas être prolongée");
  await expect(page.getByRole("button", { name: "Rester connecté" })).toHaveCount(0);
  await expectAccessible(page);
});
