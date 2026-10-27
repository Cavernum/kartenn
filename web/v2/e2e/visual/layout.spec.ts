import type { Page } from "@playwright/test";
import { MOCKUPS } from "../../shell/src/_ui/ecrans/registry.ts";
import { expect, open, test } from "../fixtures/index.ts";

// Structure before style (spec 020, design « Structure avant style »; REQ-020-34, 36):
// the tables of the catalogue align their header on their cells by construction, at
// every width; their headers stick while they scroll; a toolbar never wraps; no page of
// these components scrolls sideways.

const WIDTHS = [1440, 1280, 1024, 768] as const;

/**
 * The left edges of the cells of each row of each grid on the page, rounded to 0.1 px. A
 * simple list whose rows stack (`stack`, a container under `sm`) has no columns to align:
 * left out.
 */
function edges(page: Page): Promise<number[][][]> {
  return page.locator("[role='grid']").evaluateAll((grids) =>
    grids
      .filter((grid) => {
        // Its container is its content box.
        const frame = grid.closest<HTMLElement>("[data-stack]");
        if (!frame) return true;
        const style = getComputedStyle(frame);
        const inner =
          frame.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
        return inner >= 480;
      })
      .map((grid) =>
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

// Reflow (WCAG 1.4.10; spec 020, « Adaptation à la largeur »): at 320 px (1280 px at
// 400 %), no mock-up scrolls sideways, neither the page nor a slot of the frame; only a
// grid scrolls in its own region.
test.describe("mock-ups at 320 px", () => {
  test.use({ viewport: { width: 320, height: 640 } });

  for (const mockup of MOCKUPS) {
    for (const view of mockup.views) {
      test(`${mockup.id} ${view.id}: no sideways scroll outside the grids`, async ({ page }) => {
        await open(page, `/_ui/ecrans/${mockup.id}?vue=${view.id}`);
        await page.locator(".k-ecran").waitFor();
        await expectNoSideScroll(page);
        const slots = await page
          .locator(
            ".k-ecran__bar, .k-ecran__surface, .k-side-panel .k-tabs__panel, .k-bottom-panel .k-tabs__panel",
          )
          .evaluateAll((els) =>
            els.map((el) => ({ slot: el.className, over: el.scrollWidth - el.clientWidth })),
          );
        for (const { slot, over } of slots)
          expect(over, `${slot} scrolls sideways`).toBeLessThanOrEqual(0);
      });
    }
  }
});

// « Tout s'adapte » (020, « Adaptation à la largeur », 2026-10-26): at the wide and standard
// classes, every view of every mock-up with the right panel open (the draft counter opens
// it where the view leaves it closed) has nothing of its surface under the panel — no
// element, header, filter, grid or footer, painted past its edge — and every toolbar of a
// grid stays within the grid, every toolbar of a panel within the panel.
for (const width of [1280, 1440] as const) {
  test.describe(`panel open at ${width} px`, () => {
    test.use({ viewport: { width, height: 900 } });

    for (const mockup of MOCKUPS) {
      for (const view of mockup.views.filter((v) => !v.modal)) {
        test(`${mockup.id} ${view.id}: nothing under the panel, toolbars inside`, async ({
          page,
        }) => {
          await open(page, `/_ui/ecrans/${mockup.id}?vue=${view.id}`);
          await page.locator(".k-ecran").waitFor();
          const panel = page.locator(".k-side-panel");
          if (!(await panel.isVisible())) {
            await page.getByRole("button", { name: /^Brouillon\s*\d+\s*opération/ }).click();
          }
          await expect(panel).toBeVisible();
          await expect(panel).not.toHaveAttribute("data-cover");
          // Past the exit of a menu the view opened, the layout is settled.
          await page.waitForTimeout(50);
          const report = await page.evaluate(() => {
            const side = document.querySelector<HTMLElement>(".k-side-panel")!;
            const edge = side.getBoundingClientRect().left;
            const surface = document.querySelector<HTMLElement>(".k-ecran__surface")!;
            /** The part of `el` left visible by the ancestors that clip it, up to the surface. */
            const visible = (el: Element) => {
              const r = el.getBoundingClientRect();
              let left = r.left;
              let right = r.right;
              for (
                let a = el.parentElement;
                a && a !== surface.parentElement;
                a = a.parentElement
              ) {
                if (getComputedStyle(a).overflowX === "visible") continue;
                const c = a.getBoundingClientRect();
                left = Math.max(left, c.left);
                right = Math.min(right, c.right);
              }
              return { left, right, width: r.width };
            };
            const under = [...surface.querySelectorAll("*")]
              .filter((el) => {
                const v = visible(el);
                return v.width > 0 && v.right > v.left && v.right > edge + 0.5;
              })
              .map((el) => `${el.tagName.toLowerCase()}.${el.className}`.slice(0, 80));
            // A bar not rendered (a column chooser with nothing to hide at this width,
            // `display: none`) has no box: nothing of it can stand out.
            const inside = (inner: Element, outer: Element) => {
              const i = inner.getBoundingClientRect();
              if (i.width === 0 && i.height === 0) return true;
              const o = outer.getBoundingClientRect();
              return i.left >= o.left - 0.5 && i.right <= o.right + 0.5;
            };
            const outside: string[] = [];
            for (const frame of surface.querySelectorAll(".k-data-grid-frame, .k-tree-grid-frame"))
              for (const bar of frame.querySelectorAll(":scope > .k-column-chooser"))
                if (!inside(bar, frame)) outside.push(`chooser of ${frame.className}`);
            for (const box of surface.querySelectorAll(".k-ecran-panel"))
              for (const bar of box.querySelectorAll("[role='toolbar'], .k-column-chooser"))
                if (!inside(bar, box)) outside.push(`${bar.className} of a panel`);
            return {
              surfaceEnd: surface.getBoundingClientRect().right,
              edge,
              under,
              outside,
            };
          });
          expect(report.surfaceEnd, "the surface ends before the panel").toBeLessThanOrEqual(
            report.edge + 0.5,
          );
          expect(report.under, "painted under the panel").toEqual([]);
          expect(report.outside, "toolbars out of their grid or panel").toEqual([]);
        });
      }
    }
  });
}

// The frame of each width class (WIDTH_CLASSES): its attribute, the sidebar, the panel.
const FRAMES = [
  { width: 1440, name: "wide", sidebar: "expanded", panel: "beside" },
  { width: 1100, name: "standard", sidebar: "expanded", panel: "beside" },
  { width: 768, name: "narrow", sidebar: "collapsed", panel: "cover" },
  { width: 390, name: "compact", sidebar: "drawer", panel: "sheet" },
] as const;

for (const frame of FRAMES) {
  test.describe(`frame at ${frame.width} px`, () => {
    test.use({ viewport: { width: frame.width, height: 900 } });

    test(`${frame.name}: sidebar ${frame.sidebar}, panel ${frame.panel}`, async ({ page }) => {
      // The view whose right panel opens (the add form); the node view uses the bottom one.
      await open(page, "/_ui/ecrans/E1?vue=ajout");
      const root = page.locator(".k-ecran");
      await expect(root).toHaveAttribute("data-width", frame.name);

      const panel = page.locator(".k-side-panel");
      const box = await panel.boundingBox();
      const surface = await page.locator(".k-ecran__surface").boundingBox();
      expect(box && surface).toBeTruthy();
      if (frame.panel === "beside") {
        // Beside the surface, which shrinks to what it leaves (« tout s'adapte »): the panel
        // ends with the body, the surface before it; no strip while it is open.
        const body = await page.locator(".k-ecran__body").boundingBox();
        expect(Math.round(box!.x + box!.width)).toBe(Math.round(body!.x + body!.width));
        expect(Math.round(surface!.x + surface!.width)).toBeLessThanOrEqual(Math.round(box!.x));
        await expect(page.locator(".k-side-panel-rail")).toHaveCount(0);
        for (const grid of await page.locator(".k-ecran__surface [role='grid']").all()) {
          const g = (await grid.boundingBox())!;
          expect(Math.round(g.x + g.width)).toBeLessThanOrEqual(Math.round(box!.x));
        }
      } else if (frame.panel === "cover") {
        expect(Math.round(box!.x)).toBe(Math.round(surface!.x));
      } else {
        expect([box!.x, box!.y, box!.width, box!.height].map(Math.round)).toEqual([
          0,
          0,
          frame.width,
          900,
        ]);
        await page.getByRole("button", { name: "Retour" }).click();
        await expect(panel).toBeHidden();
      }

      // The sheet closed (compact), the bar is reachable again.
      const nav = page.getByRole("navigation", { name: "Navigation principale" });
      const menu = page.getByRole("button", { name: "Navigation principale" });
      if (frame.sidebar === "drawer") {
        await expect(nav).toBeHidden();
        await menu.click();
        await expect(page.getByRole("dialog", { name: "Navigation principale" })).toBeVisible();
        await expect(nav).toBeVisible();
        await page.keyboard.press("Escape");
        await expect(menu).toBeFocused();
      } else {
        await expect(menu).toBeHidden();
        await expect(nav).toHaveAttribute("data-state", frame.sidebar);
      }
    });

    // The bottom panel (E1, the node view): it pushes — under the surface, as wide as it,
    // the surface losing height only; in the compact class, a sheet over the window.
    test(`${frame.name}: bottom panel ${frame.panel === "sheet" ? "sheet" : "under the surface"}`, async ({
      page,
    }) => {
      await open(page, "/_ui/ecrans/E1?vue=noeud");
      const bottom = page.locator(".k-bottom-panel");
      await expect(bottom).toBeVisible();
      const box = (await bottom.boundingBox())!;
      if (frame.panel === "sheet") {
        expect([box.x, box.y, box.width, box.height].map(Math.round)).toEqual([
          0,
          0,
          frame.width,
          900,
        ]);
        return;
      }
      const surface = (await page.locator(".k-ecran__surface").boundingBox())!;
      const work = (await page.locator(".k-ecran__work").boundingBox())!;
      expect(Math.round(box.x)).toBe(Math.round(surface.x));
      expect(Math.round(box.width)).toBe(Math.round(surface.width));
      expect(Math.round(box.y + box.height)).toBe(Math.round(work.y + work.height));
      expect(surface.y + surface.height).toBeLessThanOrEqual(box.y);
    });
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
