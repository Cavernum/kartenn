import { DATA } from "../fixtures/data.ts";
import { ISSUER } from "../fixtures/env.ts";
import { expect, expectAccessible, expectNoCspViolation, test } from "../fixtures/index.ts";

// Smoke journey of the harness (spec 007, task 10, REQ-007-34): login through the
// development issuer, a read of the data set by the page, logout.
test("login, read and logout through the development issuer", async ({ page, login }) => {
  await login("alice");
  await expect(page.getByRole("heading", { level: 1, name: "Arbre des préfixes" })).toBeVisible();
  await expectAccessible(page);

  // Reads from the page itself: same origin, CSP `connect-src 'self'`, session cookie.
  const read = await page.evaluate(async () => {
    const me = await fetch("/v1/me");
    const domains = await fetch("/v1/domains");
    return {
      groups: ((await me.json()) as { idp_groups: string[] }).idp_groups,
      domains: ((await domains.json()) as { items: { slug: string }[] }).items.map((d) => d.slug),
    };
  });
  expect(read.groups).toContain("kartenn-admins");
  expect(read.domains).toContain(DATA.domain);

  // Logout is a form POST (design « Service du shell », no `form-action`).
  const logout = page.waitForResponse((r) => r.url().endsWith("/v1/auth/logout"));
  await page.evaluate(() => {
    const form = document.createElement("form");
    form.method = "post";
    form.action = "/v1/auth/logout";
    document.body.append(form);
    form.submit();
  });
  expect((await logout).status()).toBe(303);
  // REQ-005-8: on to the issuer, which closes its own session.
  await page.waitForURL(`${ISSUER}/end-session?**`);
  await page.goto("/");
  const after = await page.evaluate(async () => (await fetch("/v1/me")).status);
  expect(after).toBe(401);
});

// The guards themselves: each of these must make a journey fail.
test.describe("guards of every journey", () => {
  test.fail("an axe-core violation fails the journey", async ({ page, login }) => {
    await login("bob");
    await page.evaluate(() => {
      const image = document.createElement("img");
      image.src = "data:image/gif;base64,R0lGODlhAQABAAAAACw=";
      document.querySelector("main")?.append(image);
    });
    await expectAccessible(page);
  });

  test.fail("an inline script fails the journey", async ({ page, login }) => {
    await login("bob");
    await page.evaluate(() => {
      const script = document.createElement("script");
      script.textContent = "document.title = 'injected';";
      document.body.append(script);
    });
    await expect.poll(() => page.evaluate(() => window.__kartennCsp?.length ?? 0)).toBe(1);
    await expectNoCspViolation(page);
  });
});
