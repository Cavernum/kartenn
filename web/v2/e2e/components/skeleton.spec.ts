import type { Locator, Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// From skeleton to data (spec 020, design « Inventaire » line 25; REQ-020-23): the grid and
// the tree of the catalogue load in skeleton rows of the size of what comes, so that the
// data take their place with no layout shift: the cumulative layout shift is 0 (Layout
// Instability API, shifts without recent input: the data arrive by a script, as from the
// network, not by a press), and neither the view nor what follows it moves.

/** Records the layout shifts from now on, outside of any recent input. */
async function watchShifts(page: Page): Promise<void> {
  await page.evaluate(() => {
    const shifts: number[] = [];
    (window as unknown as { kShifts: number[] }).kShifts = shifts;
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries() as (PerformanceEntry & {
        value: number;
        hadRecentInput: boolean;
      })[])
        if (!entry.hadRecentInput) shifts.push(entry.value);
    }).observe({ type: "layout-shift" });
  });
}

/** The shifts recorded, once two frames have passed (the observer is told after them). */
async function shifts(page: Page): Promise<number[]> {
  return page.evaluate(
    () =>
      new Promise<number[]>((done) =>
        requestAnimationFrame(() =>
          requestAnimationFrame(() =>
            setTimeout(() => done((window as unknown as { kShifts: number[] }).kShifts), 100),
          ),
        ),
      ),
  );
}

const box = async (locator: Locator) => {
  const b = (await locator.boundingBox())!;
  return { y: b.y, height: b.height };
};

for (const [component, role] of [
  ["DataGrid", "grid"],
  ["TreeGrid", "treegrid"],
] as const) {
  test(`${component}: CLS 0 between its skeleton and its data`, async ({ page }) => {
    await openState(page, component, "settle");
    const state = page.locator("[data-state-name='settle']");
    const view = state.getByRole(role);
    const after = state.getByText("Ce qui suit la zone reste à sa place.");
    await expect(view).toHaveAttribute("aria-busy", "true");
    const [viewBefore, afterBefore] = [await box(view), await box(after)];
    await watchShifts(page);
    // The data arrive (a script, not a press of the user).
    await state.evaluate((el) => el.querySelector("button")!.click());
    await expect(view).not.toHaveAttribute("aria-busy");
    await expect(view.locator(".k-skeleton-fill")).toHaveCount(0);
    expect(await shifts(page)).toEqual([]);
    expect(await box(view)).toEqual(viewBefore);
    expect(await box(after)).toEqual(afterBefore);
  });
}

test("TreeGrid: focus on its loading cell goes to the first row when the nodes come", async ({
  page,
}) => {
  await openState(page, "TreeGrid", "settle");
  const state = page.locator("[data-state-name='settle']");
  const tree = state.getByRole("treegrid");
  await state.getByRole("button", { name: "Recevoir les données" }).focus();
  const loading = tree.locator("[tabindex='0']");
  // Past the column chooser of the tree, if it shows one.
  for (let i = 0; i < 3 && !(await loading.evaluate((el) => el === document.activeElement)); i++)
    await page.keyboard.press("Tab");
  await expect(loading).toBeFocused();
  await expect(loading).toContainText("Chargement");
  // The nodes arrive (a script): the focus goes to the stop of the tree, not to the page.
  await state.evaluate((el) => el.querySelector("button")!.click());
  await expect(tree).not.toHaveAttribute("aria-busy");
  const first = tree.getByRole("row").nth(1).getByRole("gridcell").first();
  await expect(first).toBeFocused();
  await expect(first).toHaveAttribute("tabindex", "0");
});

test("a loading grid says it once, in words, its bars hidden", async ({ page }) => {
  await openState(page, "DataGrid", "loading");
  const grid = page.getByRole("grid");
  await expect(grid).toHaveAttribute("aria-busy", "true");
  await expect(grid.getByText("Chargement")).toHaveCount(1);
  await expect(grid.locator(".k-skeleton-fill").first()).toHaveAttribute("aria-hidden", "true");
});
