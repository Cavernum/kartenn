import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { defineConfig, devices } from "@playwright/test";

// Visual regression and transverse checks of the design system (spec 020, design
// « Régression visuelle »; REQ-020-14, 26, 27): Chromium alone (browser of Nix,
// PLAYWRIGHT_BROWSERS_PATH), fixed viewport, scale, motion, clock, locale and time zone,
// against `vite preview` of the test build of the shell (catalogue included). References
// are generated in devenv only (`k-web-shots --update`), without a platform suffix: one
// platform, the one of Nix.
const shell = fileURLToPath(new URL("../shell", import.meta.url));

/**
 * The port of the preview: `KARTENN_PREVIEW_PORT`, else a port free when the run starts,
 * so that worktrees run their captures side by side. The runner picks it once and leaves
 * it in its environment, which its workers inherit and read back when they load this file.
 */
function previewPort(): number {
  const given = Number(process.env.KARTENN_PREVIEW_PORT);
  if (Number.isInteger(given) && given > 0 && given < 65536) return given;
  const free = execFileSync(
    process.execPath,
    [
      "-e",
      "const s = require('node:net').createServer();" +
        "s.listen(0, '127.0.0.1', () => { process.stdout.write(String(s.address().port)); s.close(); });",
    ],
    { encoding: "utf8" },
  );
  process.env.KARTENN_PREVIEW_PORT = free;
  return Number(free);
}

export const PORT = previewPort();
export const BASE_URL = `http://127.0.0.1:${PORT}`;

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
    command: `pnpm run build:catalogue && pnpm exec vite preview --mode catalogue --port ${PORT}`,
    cwd: shell,
    url: `${BASE_URL}/_ui`,
    reuseExistingServer: false,
    timeout: 120_000,
    stdout: "ignore",
  },
});
