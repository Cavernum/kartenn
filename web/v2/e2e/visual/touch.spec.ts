import type { Page } from "@playwright/test";
import { MOCKUPS } from "../../shell/src/_ui/ecrans/registry.ts";
import { expect, open, test } from "../fixtures/index.ts";

// Touch (spec 020, design « Adaptation à la largeur », « Tactile »): under a coarse
// pointer, every target of the mock-ups is at least 24 × 24 px or spaced as WCAG 2.5.8
// asks (a 24 px circle on its centre meets no other target nor the circle of another
// small one), and the main actions (primary, danger buttons) are 44 px high
// (`--k-size-target-coarse`). Touch emulation makes `(any-pointer: coarse)` and
// `(hover: none)` true. Exempt as 2.5.8 says: a link inside a sentence.

/** What a finger can activate: focusable controls, plus the funnels of grid headers. */
const TARGETS = [
  "button",
  "a[href]",
  "input:not([type='hidden'])",
  "textarea",
  "select",
  "summary",
  "[role='tab']",
  "[role='menuitem']",
  "[role='separator'][tabindex]",
  ".k-data-grid__filter",
].join(", ");

interface Finding {
  target: string;
  size: string;
}

/** The small targets that fail the spacing exception, and the main actions under 44 px. */
function audit(page: Page): Promise<{ small: Finding[]; main: Finding[] }> {
  return page.evaluate((selector) => {
    const name = (el: Element) =>
      `${el.tagName.toLowerCase()}.${[...el.classList].join(".")} « ${(
        el.getAttribute("aria-label") ??
        el.textContent ??
        ""
      )
        .trim()
        .slice(0, 40)} »`;
    const inSentence = (el: Element) =>
      el.tagName === "A" && getComputedStyle(el).display === "inline";
    // A target under a layer (the surface under the right panel, which lies over it at
    // every width) is no target a finger reaches: the layer takes the touch.
    const covered = (el: Element) => {
      const r = el.getBoundingClientRect();
      const top = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
      return top !== null && !el.contains(top) && !top.contains(el);
    };
    const els = [...document.querySelectorAll(selector)].filter(
      (el) =>
        !el.closest("[inert], [hidden], .k-visually-hidden") &&
        el.checkVisibility({ visibilityProperty: true, opacityProperty: true }) &&
        !inSentence(el) &&
        !covered(el),
    );
    const boxes = els
      .map((el) => ({ el, r: el.getBoundingClientRect() }))
      .filter(({ r }) => r.width > 0 && r.height > 0);
    const small = boxes.filter(({ r }) => r.width < 23.5 || r.height < 23.5);
    const centre = (r: DOMRect) => [r.x + r.width / 2, r.y + r.height / 2] as const;
    const distance = (r: DOMRect, x: number, y: number) =>
      Math.hypot(Math.max(r.left - x, 0, x - r.right), Math.max(r.top - y, 0, y - r.bottom));
    const failing = small.filter(({ el, r }) => {
      const [x, y] = centre(r);
      return boxes.some((other) => {
        if (other.el === el || other.el.contains(el) || el.contains(other.el)) return false;
        if (distance(other.r, x, y) < 12) return true;
        if (!small.includes(other)) return false;
        const [ox, oy] = centre(other.r);
        return Math.hypot(ox - x, oy - y) < 24;
      });
    });
    const main = boxes.filter(
      ({ el, r }) =>
        el.matches(".k-button[data-variant='primary'], .k-button[data-variant='danger']") &&
        r.height < 43.5,
    );
    const say = ({ el, r }: { el: Element; r: DOMRect }) => ({
      target: name(el),
      size: `${Math.round(r.width)} × ${Math.round(r.height)}`,
    });
    return { small: failing.map(say), main: main.map(say) };
  }, TARGETS);
}

async function expectTouchable(page: Page, where: string): Promise<void> {
  const { small, main } = await audit(page);
  expect(small, `${where}: targets under 24 px, too close to another`).toEqual([]);
  expect(main, `${where}: main actions under 44 px`).toEqual([]);
}

// A touch laptop (wide class: the handle of the panel), a tablet (narrow class) and a
// phone (compact class); on the phone the page under the sheet of the panel is audited
// too, once the sheet is closed.
for (const width of [1280, 768, 390] as const) {
  test.describe(`touch, ${width} px`, () => {
    test.use({ viewport: { width, height: 900 }, hasTouch: true });

    test("touch emulation makes the pointer coarse and hover absent", async ({ page }) => {
      await open(page, "/_ui/ecrans/E0");
      const media = await page.evaluate(() => ({
        coarse: matchMedia("(any-pointer: coarse)").matches,
        hover: matchMedia("(hover: none)").matches,
      }));
      expect(media).toEqual({ coarse: true, hover: true });
    });

    for (const mockup of MOCKUPS) {
      for (const view of mockup.views) {
        test(`${mockup.id} ${view.id}: targets of 24 px, main actions of 44 px`, async ({
          page,
        }) => {
          await open(page, `/_ui/ecrans/${mockup.id}?vue=${view.id}`);
          await page.locator(".k-ecran").waitFor();
          await expectTouchable(page, "page");
          const back = page.locator(".k-side-panel__back, .k-bottom-panel__back");
          if (width === 390 && (await back.count()) > 0 && !view.modal) {
            await back.click();
            await expectTouchable(page, "sheet closed");
          }
        });
      }
    }
  });
}

// Nothing reserved to hovering (`(hover: none)`): what a tooltip says comes on a tap too,
// and stays until a tap elsewhere.
test.describe("touch: a tap instead of hovering", () => {
  test.use({ viewport: { width: 390, height: 900 }, hasTouch: true });
  const bubble = (page: Page) => page.locator(".k-tooltip:not([hidden])");

  test("a refused button says why", async ({ page }) => {
    await open(page, "/_ui/c/Tooltip?etat=forbidden");
    // Refused, the control is `aria-disabled`: Playwright would wait for it to be enabled.
    await page.getByRole("button", { name: "Modifier" }).tap({ force: true });
    await expect(bubble(page)).toContainText("prefix.edit");
    await page.getByRole("heading", { level: 1 }).tap();
    await expect(bubble(page)).toHaveCount(0);
  });

  test("the lock of an end not read names the card's permission (E11)", async ({ page }) => {
    await open(page, "/_ui/ecrans/E11?vue=machine");
    await page
      .locator("#k-ecran-surface")
      .getByRole("button", { name: "Fiche non accessible" })
      .first()
      .tap();
    await expect(bubble(page)).toContainText("Fiche non accessible");
    await expect(bubble(page)).toContainText(/machines\.read|core\.prefix\.read/);
    await page.locator("#k-ecran-surface").getByText("203.0.113.128/25").first().tap();
    await expect(bubble(page)).toHaveCount(0);
  });

  test("a refused menu entry says why", async ({ page }) => {
    await open(page, "/_ui/c/Menu?etat=open");
    await page.getByRole("menuitem", { name: "Purger l'historique" }).tap({ force: true });
    await expect(bubble(page)).toContainText("prefix.purge");
    await expect(page.getByRole("menu")).toBeVisible();
  });

  test("the side panel takes a width in two taps, without dragging (2.5.7)", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    // The real clock: Vue drops an event reaching a handler attached at the same instant.
    await page.goto("/_ui/c/SidePanel?etat=draft");
    await page.locator("[data-state-name]").first().waitFor();
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    await panel.getByRole("button", { name: "Largeur du panneau" }).tap();
    await page.getByRole("menuitemradio", { name: "Étroit" }).tap();
    await expect(page.getByRole("menu")).toBeHidden();
    await expect(
      panel.getByRole("separator", { name: "Redimensionner le panneau" }),
    ).toHaveAttribute("aria-valuenow", "320");
  });

  test("a bar of a chart shows its value", async ({ page }) => {
    await open(page, "/_ui/c/BarChart?etat=days");
    await page.locator(".k-bar-chart__hit").nth(2).tap();
    await expect(bubble(page)).toBeVisible();
    await page.getByRole("heading", { level: 1 }).tap();
    await expect(bubble(page)).toHaveCount(0);
  });
});
