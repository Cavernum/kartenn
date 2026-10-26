import { MOCKUPS } from "../../shell/src/_ui/ecrans/registry.ts";
import {
  SCREEN_HEIGHT,
  SCREEN_WIDTHS,
  expect,
  expectAccessible,
  open,
  test,
} from "../fixtures/index.ts";

// Captures of the mock-ups E0 to E11 (spec 020, design « Régression visuelle »; REQ-020-25
// to 28, REQ-024-27): each view at 1440, 1280, 1024 and 768 px wide (900 high), in light
// and dark, compact density; no CSP violation at any of them. At 1280 px, the snapshot of
// the accessibility tree and no axe-core violation, per mode. The mock-ups take the whole
// window (no frame of the catalogue) and their slots scroll inside themselves: the
// capture is the window, floating layers included.

/** As for the open layers of the catalogue (catalogue.spec.ts, `MODAL`). */
const MODAL = ["aria-hidden-focus"];

for (const width of SCREEN_WIDTHS) {
  test.describe(`${width} px`, () => {
    test.use({ viewport: { width, height: SCREEN_HEIGHT } });

    for (const mockup of MOCKUPS) {
      for (const view of mockup.views) {
        for (const mode of ["light", "dark"] as const) {
          test(`${mockup.id} ${view.id}, ${mode}`, async ({ page, violations }) => {
            const name = `${mockup.id}-${view.id}-${width}-${mode}`;
            await open(page, `/_ui/ecrans/${mockup.id}?vue=${view.id}&mode=${mode}`);
            const frame = page.locator(".k-ecran");
            await frame.waitFor();
            if (view.focus) await page.locator(view.focus).first().focus();
            await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
            if (width === 1280) {
              await expect(frame).toMatchAriaSnapshot({
                name: `${mockup.id}-${view.id}-${mode}.aria.yml`,
              });
              await expectAccessible(page, view.modal ? MODAL : []);
            }
            expect(violations).toEqual([]);
          });
        }
      }
    }
  });
}
