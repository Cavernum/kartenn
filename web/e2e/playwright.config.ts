import { defineConfig, devices } from "@playwright/test";

// Browsers come from Nix (PLAYWRIGHT_BROWSERS_PATH, ADR 0024). The real server, the
// development issuer and the data set are wired by task 007-10.
export default defineConfig({
  testDir: "journeys",
  forbidOnly: !!process.env.CI,
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
  ],
});
