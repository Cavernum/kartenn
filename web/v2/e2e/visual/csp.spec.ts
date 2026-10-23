import { expect, open, test } from "../fixtures/index.ts";

// No CSP violation over the whole catalogue (spec 020, task 9; REQ-020-14, 33): the
// preview sends the policy of kartenn-server; a component that injects a <style> or sets a
// style attribute raises `securitypolicyviolation`. Requests stay on the origin (fonts
// included, REQ-020-10).

test("the catalogue raises no CSP violation and loads nothing from elsewhere", async ({
  page,
  violations,
}) => {
  const foreign: string[] = [];
  page.on("request", (request) => {
    if (!request.url().startsWith("http://127.0.0.1:4173/")) foreign.push(request.url());
  });
  const fonts: string[] = [];
  page.on("response", (response) => {
    if (response.url().endsWith(".woff2")) fonts.push(new URL(response.url()).pathname);
  });

  await open(page, "/_ui");
  const pages = await page
    .locator("main a")
    .evaluateAll((links) => links.map((link) => (link as HTMLAnchorElement).pathname));
  for (const path of ["/_ui", ...pages]) {
    for (const query of ["?mode=light", "?mode=dark&densite=confortable"]) {
      await open(page, `${path}${query}`);
    }
  }

  expect(violations).toEqual([]);
  expect(foreign).toEqual([]);
  expect(fonts.length, "self-hosted fonts loaded").toBeGreaterThan(0);
  for (const font of fonts)
    expect(font).toMatch(/^\/assets\/geist(-mono)?-latin(-ext)?-wght-normal-/);
});
