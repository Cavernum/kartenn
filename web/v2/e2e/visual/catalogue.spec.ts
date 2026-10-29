import { readdirSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expectAccessible, open, snap, test } from "../fixtures/index.ts";

/** The delay of a tooltip under the pointer (ui/src/components/Tooltip/tooltip.ts). */
const TOOLTIP_DELAY = 300;

// Captures of the catalogue (spec 020, task 9; REQ-020-26, 27, 28): each page and each
// declared state of each component, in both modes; each component page and the tokens in
// the comfortable density too.

const MODES = ["light", "dark"] as const;

/**
 * An open modal layer of Reka hides the rest of the page with `aria-hidden` and traps the
 * focus inside it: the controls left behind are unreachable, which `aria-hidden-focus`
 * cannot see (it does not know of the trap). The rule is left out on those states only.
 */
const MODAL = ["aria-hidden-focus"];

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
  });
}

/** The components that declare their states (`<Nom>.catalogue.ts`), as the index lists them. */
const components = readdirSync(new URL("../../ui/src/components", import.meta.url), {
  withFileTypes: true,
})
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .filter((name) =>
    readdirSync(new URL(`../../ui/src/components/${name}`, import.meta.url)).includes(
      `${name}.catalogue.ts`,
    ),
  )
  .sort();

/** Performs the interaction a state declares (hover, keyboard focus, press) before its capture. */
async function interact(page: Page): Promise<void> {
  const figure = page.locator("[data-state-name]").first();
  const interaction = await figure.getAttribute("data-interaction");
  if (!interaction) return;
  const selector =
    (await figure.getAttribute("data-target")) ??
    "button, a[href], input, select, textarea, [tabindex]:not([tabindex='-1'])";
  const target = figure.locator(".k-cat-state__render").locator(selector).first();
  if (interaction === "hover") {
    await target.hover();
    // A tooltip opens under the pointer after its delay (`TOOLTIP_DELAY`, 300 ms): the
    // capture shows it, not the instant before.
    await page.waitForTimeout(TOOLTIP_DELAY + 100);
  }
  if (interaction === "focus") await target.focus();
  if (interaction === "press") {
    await target.click();
    await page.mouse.move(0, 0);
  }
}

for (const component of components) {
  test.describe(component, () => {
    // One capture per state and mode (design « Régression visuelle »); the page of all states
    // once more in the comfortable density.
    test.describe.configure({ timeout: 180_000 });
    for (const mode of MODES) {
      test(`${mode}, state by state`, async ({ page }) => {
        await open(page, `/_ui/c/${component}?mode=${mode}`);
        await page.locator("[data-state-name]").first().waitFor();
        const states = await page
          .locator("[data-state-name]")
          .evaluateAll((nodes) => nodes.map((node) => node.getAttribute("data-state-name")!));
        test.expect(states.length, component).toBeGreaterThan(0);
        await expectAccessible(page);
        for (const state of states) {
          await test.step(state, async () => {
            await open(page, `/_ui/c/${component}?mode=${mode}&etat=${state}`);
            await interact(page);
            // A state shown alone (an open layer) is checked by axe on its own page.
            if (await page.locator("[data-isolated]").count()) await expectAccessible(page, MODAL);
            await snap(page, `${component}-${state}-${mode}`);
          });
        }
      });

      test(`${mode}, comfortable density`, async ({ page }) => {
        await open(page, `/_ui/c/${component}?mode=${mode}&densite=confortable`);
        await expectAccessible(page);
        await snap(page, `${component}-${mode}-confortable`);
      });
    }
  });
}
