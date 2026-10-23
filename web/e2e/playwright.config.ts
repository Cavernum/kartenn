import { fileURLToPath } from "node:url";
import { defineConfig, devices } from "@playwright/test";
import { BASE_URL, ISSUER } from "./fixtures/env.ts";

// End-to-end journeys against the real kartenn-server and PostgreSQL (spec 007, REQ-007-34):
// the development issuer and the instance are started by serve.sh, in this order, then
// global-setup.ts loads the bindings and the data set through the API. Browsers come from
// Nix (PLAYWRIGHT_BROWSERS_PATH, ADR 0024).
const repository = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: "journeys",
  forbidOnly: !!process.env.CI,
  // CI runners are slower and noisier than a workstation: one retry there, reported as
  // « flaky » in the summary rather than hidden. Locally a failure stays a failure.
  retries: process.env.CI ? 1 : 0,
  // One instance and one data set: journeys of both browsers share them.
  workers: 2,
  globalSetup: "./global-setup.ts",
  use: {
    baseURL: BASE_URL,
    // Certificates of the development authority (web/e2e/.state/ca.pem).
    ignoreHTTPSErrors: true,
    locale: "fr-FR",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
  ],
  webServer: [
    {
      command: "dash web/e2e/serve.sh idp",
      cwd: repository,
      url: `${ISSUER}/.well-known/openid-configuration`,
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      // The first run builds kartenn-server and kartenn-dev-idp.
      timeout: 900_000,
      stdout: "ignore",
    },
    {
      command: "dash web/e2e/serve.sh kartenn",
      cwd: repository,
      url: `${BASE_URL}/health/ready`,
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      timeout: 300_000,
      stdout: "ignore",
    },
  ],
});
