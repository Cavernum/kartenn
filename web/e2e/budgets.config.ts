import { fileURLToPath } from "node:url";
import { defineConfig, devices } from "@playwright/test";
import { BASE_URL, ISSUER } from "./fixtures/env.ts";

// The browser bench (spec 007, REQ-007-32, design « Budgets »), run by the devenv script
// `bench-web`: Chromium alone (CPU slowed down through CDP), against kartenn-server in
// release on the dataset of bench-core (`serve.sh budgets`).
const repository = fileURLToPath(new URL("../..", import.meta.url));

export default defineConfig({
  testDir: "budgets",
  workers: 1,
  timeout: 300_000,
  globalSetup: "./budgets/global-setup.ts",
  use: {
    baseURL: BASE_URL,
    ignoreHTTPSErrors: true,
    locale: "fr-FR",
  },
  projects: [{ name: "budgets", use: { ...devices["Desktop Chrome"] } }],
  webServer: [
    {
      command: "dash web/e2e/serve.sh idp",
      cwd: repository,
      url: `${ISSUER}/.well-known/openid-configuration`,
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      timeout: 900_000,
      stdout: "ignore",
    },
    {
      // A release build and the load of 10⁶ objects.
      command: "dash web/e2e/serve.sh budgets",
      cwd: repository,
      url: `${BASE_URL}/health/ready`,
      ignoreHTTPSErrors: true,
      reuseExistingServer: false,
      timeout: 3_600_000,
      stdout: "ignore",
    },
  ],
});
