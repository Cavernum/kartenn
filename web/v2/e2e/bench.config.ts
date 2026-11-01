import { defineConfig, devices } from "@playwright/test";
import base, { BASE_URL } from "./playwright.config.ts";

// The component benches of @kartenn/ui (spec 020, REQ-020-29, design « Budgets »), run by
// the devenv script `bench-ui`: the protocol of the prototype D-1 — the catalogue build
// (production, its pages /_ui/bancs/ included) under `vite preview`, Chromium without
// slowdown, 1440 × 900, five passes and their median. One worker: a bench never shares the
// browser with another.
export default defineConfig({
  testDir: "bench",
  testMatch: "**/*.spec.ts",
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  fullyParallel: false,
  timeout: 900_000,
  use: {
    baseURL: BASE_URL,
    locale: "fr-FR",
    timezoneId: "Europe/Paris",
  },
  projects: [
    {
      name: "components-bench",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1440, height: 900 },
        deviceScaleFactor: 1,
        colorScheme: "light",
        // The fonts of Nix, as the captures (playwright.config.ts).
        launchOptions: base.projects?.[0]?.use?.launchOptions ?? {},
      },
    },
  ],
  webServer: base.webServer!,
});
