import { fileURLToPath } from "node:url";
import { defineConfig, devices } from "@playwright/test";
import { BASE_URL, ISSUER, QUOTA_URL } from "./fixtures/env.ts";

// Journeys of the new shell against the real kartenn-server and PostgreSQL (spec 021,
// REQ-021-31; design « Migration »): the instance of web/v2/e2e/kartenn.toml, `[web] root`
// and `plugins_dir` on the builds of web/v2, started with the issuer by serve.sh. Run by
// `k-web2-e2e`; the captures keep playwright.config.ts (`k-web2-check`, `k-web-shots`).
const repository = fileURLToPath(new URL("../../..", import.meta.url));
/** The journey at the read quota by default, on its instance alone. */
const QUOTA = /quota\.spec\.ts$/;

export default defineConfig({
  testDir: "journeys",
  // The issuer attached and the bindings of the groups of users.toml (journeys/global-setup.ts).
  globalSetup: "./journeys/global-setup.ts",
  forbidOnly: !!process.env.CI,
  // As web/e2e: one retry on the CI runners, reported as « flaky ».
  retries: process.env.CI ? 1 : 0,
  workers: 2,
  use: {
    baseURL: BASE_URL,
    // Certificates of the development authority (web/e2e/.state/ca.pem).
    ignoreHTTPSErrors: true,
    locale: "fr-FR",
  },
  projects: [
    { name: "chromium", testIgnore: QUOTA, use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", testIgnore: QUOTA, use: { ...devices["Desktop Firefox"] } },
    // The journey at the read quota by default (task 021-11p10-b2) on an instance of its own
    // (web/v2/e2e/quota.toml), started by the project `quota-instance` only when a run
    // selects it, and stopped after it: no `webServer`, which every run would wait for.
    {
      name: "quota-instance",
      testMatch: /quota\.setup\.ts$/,
      teardown: "quota-instance-stop",
    },
    { name: "quota-instance-stop", testMatch: /quota\.teardown\.ts$/ },
    {
      name: "quota-chromium",
      testMatch: QUOTA,
      dependencies: ["quota-instance"],
      use: { ...devices["Desktop Chrome"], baseURL: QUOTA_URL },
    },
    {
      name: "quota-firefox",
      testMatch: QUOTA,
      dependencies: ["quota-instance"],
      use: { ...devices["Desktop Firefox"], baseURL: QUOTA_URL },
    },
  ],
  webServer: [
    {
      command: "dash web/v2/e2e/serve.sh idp",
      cwd: repository,
      url: `${ISSUER}/.well-known/openid-configuration`,
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      // The first run builds kartenn-server and kartenn-dev-idp.
      timeout: 900_000,
      stdout: "ignore",
    },
    {
      command: "dash web/v2/e2e/serve.sh kartenn",
      cwd: repository,
      url: `${BASE_URL}/health/ready`,
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      timeout: 300_000,
      stdout: "ignore",
    },
  ],
});
