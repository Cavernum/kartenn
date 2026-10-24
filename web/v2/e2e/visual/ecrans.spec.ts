import { MOCKUPS } from "../../shell/src/_ui/ecrans/registry.ts";
import { expect, expectAccessible, open, test } from "../fixtures/index.ts";

// Captures of the mock-ups E1 to E10 (spec 020, tasks 13 and 14; REQ-020-25 to 28): each
// view in both modes and both densities, with its snapshot of the accessibility tree, no
// axe-core violation and no CSP violation. The mock-ups take the whole page (no frame of
// the catalogue): the snapshot is the one of the frame of the shell, floating layers
// included.

/** As for the open layers of the catalogue (catalogue.spec.ts, `MODAL`). */
const MODAL = ["aria-hidden-focus"];

for (const mockup of MOCKUPS) {
  for (const view of mockup.views) {
    test.describe(`${mockup.id} ${view.id}`, () => {
      for (const mode of ["light", "dark"] as const) {
        for (const densite of ["compacte", "confortable"] as const) {
          test(`${mode}, ${densite}`, async ({ page, violations }) => {
            const name = `${mockup.id}-${view.id}-${mode}-${densite}`;
            await open(
              page,
              `/_ui/ecrans/${mockup.id}?vue=${view.id}&mode=${mode}&densite=${densite}`,
            );
            const frame = page.locator(".k-ecran");
            await frame.waitFor();
            await expectAccessible(page, view.modal ? MODAL : []);
            await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
            await expect(frame).toMatchAriaSnapshot({ name: `${name}.aria.yml` });
            expect(violations).toEqual([]);
          });
        }
      }
    });
  }
}
