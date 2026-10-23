import { expectAccessible, open, snap, test } from "../fixtures/index.ts";

// Captures of the catalogue (spec 020, task 9; REQ-020-26, 27, 28): each page and each
// declared state of each component, in both modes; the tokens in both densities too.

const MODES = ["light", "dark"] as const;

for (const mode of MODES) {
  test.describe(`mode ${mode}`, () => {
    test("index", async ({ page }) => {
      await open(page, `/_ui?mode=${mode}`);
      await expectAccessible(page);
      await snap(page, `index-${mode}`);
    });

    for (const densite of ["compacte", "confortable"] as const) {
      test(`tokens, ${densite}`, async ({ page }) => {
        await open(page, `/_ui/jetons?mode=${mode}&densite=${densite}`);
        // The values are read from the root once the switches are applied.
        await test.expect(page.getByText("--k-row-height").locator("..")).toContainText("px");
        await expectAccessible(page);
        await snap(page, `jetons-${mode}-${densite}`);
      });
    }

    test("icons", async ({ page }) => {
      await open(page, `/_ui/icones?mode=${mode}`);
      await expectAccessible(page);
      await snap(page, `icones-${mode}`);
    });

    test("components, state by state", async ({ page }) => {
      await open(page, `/_ui?mode=${mode}`);
      const components = await page
        .getByRole("heading", { name: "Composants" })
        .locator("xpath=following-sibling::ul[1]//a")
        .allTextContents();
      test.expect(components.length).toBeGreaterThan(0);
      for (const component of components) {
        await open(page, `/_ui/c/${component}?mode=${mode}`);
        const states = await page
          .locator("[data-state-name]")
          .evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-state-name")!));
        test.expect(states.length, component).toBeGreaterThan(0);
        await expectAccessible(page);
        for (const state of states) {
          await test.step(`${component} ${state}`, async () => {
            await open(page, `/_ui/c/${component}?mode=${mode}&etat=${state}`);
            await snap(page, `${component}-${state}-${mode}`);
          });
        }
      }
    });
  });
}
