import { expect, test } from "@playwright/test";

// The test instance serves the builds of web/v2 (spec 021, REQ-021-31; task 25):
// `[web] root` on web/v2/dist/shell, under the CSP of kartenn-server with the digest of
// its import map, and `[web] plugins_dir` on web/v2/dist/plugins.

test("kartenn-server serves the new shell under its CSP", async ({ page }) => {
  await page.addInitScript(() => {
    const seen: string[] = [];
    (window as unknown as { violations: string[] }).violations = seen;
    document.addEventListener("securitypolicyviolation", (event) =>
      seen.push(`${event.violatedDirective} ${event.blockedURI}`),
    );
  });

  const response = await page.goto("/");
  expect(response?.headers()["content-security-policy"]).toContain("script-src 'self' 'sha256-");
  await expect(page.getByRole("heading", { level: 1, name: "kartenn" })).toBeVisible();
  // The import map of the new shell shares @kartenn/ui with the plugins (ADR 0038).
  const map = JSON.parse(
    (await page.locator('script[type="importmap"]').textContent()) ?? "{}",
  ) as { imports?: Record<string, string> };
  expect(Object.keys(map.imports ?? {}).sort()).toEqual(["@kartenn/sdk", "@kartenn/ui", "vue"]);
  expect(
    await page.evaluate(() => (window as unknown as { violations: string[] }).violations),
  ).toEqual([]);
});

test("kartenn-server publishes the plugins of web/v2", async ({ request }) => {
  const response = await request.get("/plugins/example/1.0.0/manifest.json");
  expect(response.status()).toBe(200);
  expect(await response.json()).toMatchObject({ name: "example", sdk: "^1.0" });
});
