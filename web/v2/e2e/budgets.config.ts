import { fileURLToPath } from "node:url";
import { defineConfig, devices } from "@playwright/test";
import { BASE_URL, ISSUER } from "./fixtures/env.ts";

// The browser bench of the new shell (spec 021, REQ-021-30, design « Budgets »), run by the
// devenv script `bench-web`: Chromium alone, the CPU slowed down four times through CDP,
// against kartenn-server in release on the dataset of bench-core (`serve.sh budgets`). One
// worker: a measure never shares the machine with another.
const repository = fileURLToPath(new URL("../../..", import.meta.url));

export default defineConfig({
  testDir: "budgets",
  testMatch: "**/*.spec.ts",
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  fullyParallel: false,
  timeout: 900_000,
  globalSetup: "./budgets/global-setup.ts",
  use: {
    baseURL: BASE_URL,
    ignoreHTTPSErrors: true,
    locale: "fr-FR",
  },
  projects: [
    {
      name: "budgets",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 } },
    },
  ],
  webServer: [
    {
      command: "dash web/v2/e2e/serve.sh idp",
      cwd: repository,
      url: `${ISSUER}/.well-known/openid-configuration`,
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      timeout: 900_000,
      stdout: "ignore",
    },
    {
      // A release build and the load of 10⁶ objects.
      command: "dash web/v2/e2e/serve.sh budgets",
      cwd: repository,
      url: `${BASE_URL}/health/ready`,
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      timeout: 3_600_000,
      stdout: "ignore",
    },
  ],
});
