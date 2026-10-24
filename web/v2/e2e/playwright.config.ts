import { fileURLToPath } from "node:url";
import { defineConfig, devices } from "@playwright/test";

// Visual regression and transverse checks of the design system (spec 020, design
// « Régression visuelle »; REQ-020-14, 26, 27): Chromium alone (browser of Nix,
// PLAYWRIGHT_BROWSERS_PATH), fixed viewport, scale, motion, clock, locale and time zone,
// against `vite preview` of the test build of the shell (catalogue included). References
// are generated in devenv only (`k-web-shots --update`), without a platform suffix: one
// platform, the one of Nix.
const shell = fileURLToPath(new URL("../shell", import.meta.url));
export const BASE_URL = "http://127.0.0.1:4173";

export default defineConfig({
  testDir: "visual",
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 2,
  snapshotPathTemplate: "{testDir}/__screenshots__/{testFilePath}/{arg}{ext}",
  expect: {
    // threshold: the colour distance (YIQ) under which a pixel counts as equal; the default
    // (0.2) let a token change from #0b5cad to #0b6cad pass unseen.
    toHaveScreenshot: {
      animations: "disabled",
      caret: "hide",
      maxDiffPixelRatio: 0.001,
      threshold: 0.01,
    },
  },
  use: {
    baseURL: BASE_URL,
    locale: "fr-FR",
    timezoneId: "Europe/Paris",
  },
  projects: [
    {
      name: "visual",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1280, height: 800 },
        deviceScaleFactor: 1,
        colorScheme: "light",
        reducedMotion: "reduce",
        // CodeBadge and KeyLabel copy to the clipboard (REQ-020-22).
        permissions: ["clipboard-read", "clipboard-write"],
      },
    },
  ],
  webServer: {
    command: "pnpm run build:catalogue && pnpm exec vite preview --mode catalogue",
    cwd: shell,
    url: `${BASE_URL}/_ui`,
    reuseExistingServer: false,
    timeout: 120_000,
    stdout: "ignore",
  },
});
