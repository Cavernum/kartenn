import type { Page } from "@playwright/test";
import { MOCKUPS } from "../../shell/src/_ui/ecrans/registry.ts";
import { expect, open, test } from "../fixtures/index.ts";

// Structure before style (spec 020, design « Structure avant style »; REQ-020-34, 36):
// the tables of the catalogue align their header on their cells by construction, at
// every width; their headers stick while they scroll; a toolbar never wraps; no page of
// these components scrolls sideways.

const WIDTHS = [1440, 1280, 1024, 768] as const;

/** The left edges of the cells of each row of each grid on the page, rounded to 0.1 px. */
function edges(page: Page): Promise<number[][][]> {
  return page
    .locator("[role='grid']")
    .evaluateAll((grids) =>
      grids.map((grid) =>
        [...grid.querySelectorAll("[role='row']")].map((row) =>
          [...row.children]
            .filter((cell) =>
              /^(columnheader|rowheader|gridcell)$/.test(cell.getAttribute("role")!),
            )
            .map((cell) => Math.round(cell.getBoundingClientRect().left * 10) / 10),
        ),
      ),
    );
}

async function expectNoSideScroll(page: Page): Promise<void> {
  const { scroll, client } = await page.evaluate(() => ({
    scroll: document.documentElement.scrollWidth,
    client: document.documentElement.clientWidth,
  }));
  expect(scroll, "the page scrolls sideways").toBeLessThanOrEqual(client);
}

for (const width of WIDTHS) {
  test.describe(`${width} px`, () => {
    test.use({ viewport: { width, height: 900 } });

    for (const component of ["DataGrid", "MatrixGrid"]) {
      test(`${component}: header and cells share their left edges`, async ({ page }) => {
        await open(page, `/_ui/c/${component}`);
        await page.locator("[role='grid']").first().waitFor();
        const grids = await edges(page);
        expect(grids.length).toBeGreaterThan(0);
        for (const [g, rows] of grids.entries()) {
          const [head, ...body] = rows;
          for (const [r, row] of body.entries())
            expect(row, `grid ${g + 1}, row ${r + 2}`).toEqual(head);
        }
        await expectNoSideScroll(page);
      });
    }

    for (const component of ["SidePanel", "CommitForm", "SplitButton", "Toolbar", "Tooltip"]) {
      test(`${component}: no sideways scroll`, async ({ page }) => {
        await open(page, `/_ui/c/${component}`);
        await page.locator("[data-state-name]").first().waitFor();
        await expectNoSideScroll(page);
      });
    }

    test("Toolbar: one line, the rest folded", async ({ page }) => {
      await open(page, "/_ui/c/Toolbar?etat=wide");
      const bar = page.getByRole("toolbar");
      await bar.waitFor();
      const tops = await bar
        .locator(":scope > .k-button")
        .evaluateAll((buttons) => buttons.map((b) => Math.round(b.getBoundingClientRect().top)));
      expect(new Set(tops).size, "the toolbar wraps").toBe(1);
      const fits = await bar.evaluate((el) => el.scrollWidth <= el.clientWidth);
      expect(fits, "the toolbar overflows").toBe(true);
    });
  });
}

// The mock-ups (REQ-024-26): from 1024 to 1920 px, neither the page nor a slot of the frame
// (status bar, surface) scrolls sideways — long content scrolls inside its own container
// (a grid, a panel) — and every grid aligns its header on its cells.
for (const width of [1024, 1280, 1440, 1920] as const) {
  test.describe(`mock-ups at ${width} px`, () => {
    test.use({ viewport: { width, height: 900 } });

    for (const mockup of MOCKUPS) {
      for (const view of mockup.views) {
        test(`${mockup.id} ${view.id}: no sideways scroll, aligned grids`, async ({ page }) => {
          await open(page, `/_ui/ecrans/${mockup.id}?vue=${view.id}`);
          await page.locator(".k-ecran").waitFor();
          await expectNoSideScroll(page);
          const slots = await page.locator(".k-ecran__bar, .k-ecran__surface").evaluateAll((els) =>
            els.map((el) => ({
              slot: el.className,
              over: el.scrollWidth - el.clientWidth,
            })),
          );
          for (const { slot, over } of slots)
            expect(over, `${slot} scrolls sideways`).toBeLessThanOrEqual(0);
          for (const [g, rows] of (await edges(page)).entries()) {
            const [head, ...body] = rows;
            for (const [r, row] of body.entries())
              expect(row, `grid ${g + 1}, row ${r + 2}`).toEqual(head);
          }
        });
      }
    }
  });
}

test.describe("sticky headers", () => {
  test("DataGrid: the header stays at the top of its scroll container", async ({ page }) => {
    await open(page, "/_ui/c/DataGrid?etat=sticky");
    const grid = page.getByRole("grid");
    await grid.evaluate((el) => el.scrollTo(0, 300));
    const { grid: top, head } = await grid.evaluate((el) => ({
      grid: el.getBoundingClientRect().top + el.clientTop,
      head: el.querySelector("[role='rowgroup']")!.getBoundingClientRect().top,
    }));
    expect(Math.round(head)).toBe(Math.round(top));
  });

  test("MatrixGrid: both headers and the corner stay in view", async ({ page }) => {
    await open(page, "/_ui/c/MatrixGrid?etat=large");
    const grid = page.getByRole("grid");
    await grid.evaluate((el) => el.scrollTo(400, 300));
    const box = await grid.evaluate((el) => {
      const rect = el.getBoundingClientRect();
      const corner = el.querySelector(".k-matrix__corner")!.getBoundingClientRect();
      const header = el
        .querySelector("[role='columnheader']:nth-child(2)")!
        .getBoundingClientRect();
      const rowHeader = el.querySelector("[role='rowheader']")!.getBoundingClientRect();
      return {
        top: Math.round(rect.top + el.clientTop),
        left: Math.round(rect.left + el.clientLeft),
        corner: [Math.round(corner.top), Math.round(corner.left)],
        header: Math.round(header.top),
        rowHeader: Math.round(rowHeader.left),
        scrolled: [el.scrollLeft, el.scrollTop],
      };
    });
    expect(box.scrolled).toEqual([400, 300]);
    expect(box.corner).toEqual([box.top, box.left]);
    expect(box.header).toBe(box.top);
    expect(box.rowHeader).toBe(box.left);
  });
});
