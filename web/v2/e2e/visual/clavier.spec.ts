import type { Page } from "@playwright/test";
import { expect, test } from "../fixtures/index.ts";

// Keyboard scripts of the APG patterns (spec 020, design « Accessibilité »; REQ-020-18,
// 35, 36, 37; 024/M8, M9, M14): each component of the 024 driven by the keyboard alone on
// its catalogue page, its ARIA states read back.

/** The draft counter: « Brouillon », then its count in hidden brackets. */
const COUNTER = { name: /^Brouillon\s*\(\s*3\s*\)$/ };

/**
 * A state of the catalogue, with the real clock: Vue drops an event that reaches a
 * handler attached at the same instant (`Date.now()`, its guard against handlers added
 * while an event propagates), and the frozen clock of the captures makes every instant
 * the same one, so that a listener of an ancestor never hears a key a child heard first.
 */
async function state(page: Page, component: string, name: string): Promise<void> {
  await page.goto(`/_ui/c/${component}?etat=${name}`);
  await page.locator("[data-state-name]").first().waitFor();
}

/** The row and column keys of the element that has the focus in a grid. */
const focusedCell = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    return `${el.dataset.row ?? "?"}|${el.dataset.column ?? "?"}`;
  });

test.describe("SidePanel (complementary + Tabs, Window Splitter)", () => {
  test("the counter opens it on the draft, Escape closes it and gives the focus back", async ({
    page,
  }) => {
    await state(page, "SidePanel", "closed");
    const toggle = page.getByRole("button", COUNTER);
    await toggle.focus();
    await page.keyboard.press("Enter");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    await expect(panel).toBeVisible();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(toggle).toHaveAttribute("aria-controls", "k-demo-panel");
    await expect(panel.getByRole("tab", { name: /Brouillon/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    // The handle: arrows by a step, Home and End to the bounds.
    const handle = panel.getByRole("separator", { name: "Redimensionner le panneau" });
    await handle.focus();
    await expect(handle).toHaveAttribute("aria-valuenow", "420");
    await page.keyboard.press("ArrowLeft");
    await expect(handle).toHaveAttribute("aria-valuenow", "436");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(handle).toHaveAttribute("aria-valuenow", "404");
    await page.keyboard.press("Home");
    await expect(handle).toHaveAttribute("aria-valuenow", "320");
    expect(Math.round((await panel.boundingBox())!.width)).toBe(320);
    await page.keyboard.press("End");
    await expect(handle).toHaveAttribute("aria-valuenow", "720");
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(toggle).toBeFocused();
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  test("the close button closes it; the counter closes it from the draft tab", async ({ page }) => {
    await state(page, "SidePanel", "draft");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    await page.getByRole("button", COUNTER).click();
    await expect(panel).toBeHidden();
    await page.getByRole("button", COUNTER).click();
    await panel.getByRole("button", { name: "Fermer le panneau" }).click();
    await expect(panel).toBeHidden();
  });
});

test.describe("CommitForm", () => {
  test("Ctrl+Enter submits, unless the required summary is empty", async ({ page }) => {
    await state(page, "CommitForm", "empty");
    const summary = page.getByRole("textbox", { name: /Résumé/ });
    await summary.focus();
    await page.keyboard.press("Control+Enter");
    await expect(page.getByText("Le résumé est obligatoire.")).toBeVisible();
    await expect(summary).toHaveAttribute("aria-invalid", "true");
    await expect(summary).toBeFocused();
    await page.keyboard.type("Bascule de la salle B");
    await page.keyboard.press("Tab");
    await expect(page.getByRole("textbox", { name: "Description (facultative)" })).toBeFocused();
    await page.keyboard.press("Control+Enter");
    await expect(page.getByRole("status")).toHaveText("Soumis : Bascule de la salle B");
  });
});

test.describe("SplitButton (Button + Menu Button)", () => {
  test("choosing an entry changes the action without running it", async ({ page }) => {
    await state(page, "SplitButton", "apply");
    const chevron = page.getByRole("button", { name: "Choisir l'action" });
    await expect(chevron).toHaveAttribute("aria-haspopup", "menu");
    await chevron.focus();
    await page.keyboard.press("Enter");
    const menu = page.getByRole("menu");
    await expect(menu).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("menuitem", { name: "Soumettre en revue" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(menu).toBeHidden();
    const main = page.getByRole("button", { name: "Soumettre en revue" });
    await expect(main).toBeVisible();
    await expect(page.getByRole("status")).toHaveText("");
    await main.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("status")).toHaveText("Lancée : review");
  });

  test("a disabled entry stays reachable, with its reason, and cannot be chosen", async ({
    page,
  }) => {
    await state(page, "SplitButton", "held");
    await page.getByRole("button", { name: "Choisir l'action" }).focus();
    await page.keyboard.press("Enter");
    const apply = page.getByRole("menuitem", { name: "Appliquer" });
    await expect(apply).toBeFocused();
    await expect(apply).toHaveAttribute("aria-disabled", "true");
    await expect(apply).toHaveAccessibleDescription(
      "Une politique d'approbation retient ce changeset.",
    );
    // The bubble of a menu is decorative: the entry carries the reason as its description.
    await expect(page.locator(".k-tooltip:not([hidden])")).toHaveText(
      "Une politique d'approbation retient ce changeset.",
    );
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menu")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("button", { name: "Soumettre en revue" })).toBeVisible();
  });

  test("a forbidden action is focusable, refused, and says the permission", async ({ page }) => {
    await state(page, "SplitButton", "forbidden");
    const main = page.getByRole("button", { name: "Appliquer" });
    await expect(main).toHaveAttribute("aria-disabled", "true");
    await main.focus();
    await expect(page.getByRole("tooltip")).toHaveText("Il faut changeset.apply_own");
    await expect(main).toHaveAccessibleDescription("Il faut changeset.apply_own");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("status")).toHaveText("");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("tooltip")).toBeHidden();
  });
});

test.describe("Toolbar", () => {
  test("one tab stop, arrows, Home and End between the actions", async ({ page }) => {
    await state(page, "Toolbar", "wide");
    const bar = page.getByRole("toolbar", { name: "Actions sur les préfixes" });
    await expect(bar.locator("[tabindex='0']")).toHaveCount(1);
    await bar.getByRole("button", { name: "Créer" }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(bar.getByRole("button", { name: "Modifier" })).toBeFocused();
    await page.keyboard.press("End");
    const remove = bar.getByRole("button", { name: "Supprimer" });
    await expect(remove).toBeFocused();
    await expect(remove).toHaveAttribute("aria-disabled", "true");
    await expect(page.getByRole("tooltip")).toHaveText("Il faut prefix.delete");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("status")).toHaveText("");
    await page.keyboard.press("Home");
    await expect(bar.getByRole("button", { name: "Créer" })).toBeFocused();
    await expect(bar.locator("[tabindex='0']")).toHaveCount(1);
    await page.keyboard.press("Enter");
    await expect(page.getByRole("status")).toHaveText("Action : create");
  });

  test("narrow, the folded actions are in « … »", async ({ page }) => {
    await state(page, "Toolbar", "narrow");
    const bar = page.getByRole("toolbar");
    await expect(bar.getByRole("button")).toHaveCount(3);
    await bar.getByRole("button", { name: "Créer" }).focus();
    await page.keyboard.press("End");
    const more = bar.getByRole("button", { name: "Plus d'actions" });
    await expect(more).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menuitem")).toHaveText([
      "Importer",
      "Exporter",
      "Historique",
      /Supprimer/,
    ]);
    await page.getByRole("menuitem", { name: "Exporter" }).click();
    await expect(page.getByRole("status")).toHaveText("Action : export");
  });
});

test.describe("Tooltip", () => {
  test("opens on hover after a delay, stays over itself, closes on leaving", async ({ page }) => {
    await state(page, "Tooltip", "forbidden");
    const edit = page.getByRole("button", { name: "Modifier" });
    await edit.hover();
    const tip = page.getByRole("tooltip");
    await expect(tip).toHaveText("Il faut prefix.edit");
    await tip.hover();
    await expect(tip).toBeVisible();
    await page.mouse.move(0, 0);
    await expect(tip).toBeHidden();
  });
});

test.describe("MatrixGrid (Grid)", () => {
  test("arrows, Home, End, Ctrl+Home, Ctrl+End over headers and cells", async ({ page }) => {
    await state(page, "MatrixGrid", "cell-states");
    const grid = page.getByRole("grid", { name: "Matrice de politique" });
    await expect(grid.locator("[tabindex='0']")).toHaveCount(1);
    await grid.locator("[tabindex='0']").focus();
    expect(await focusedCell(page)).toBe("|");
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe("|servers");
    await page.keyboard.press("ArrowDown");
    expect(await focusedCell(page)).toBe("servers|servers");
    await page.keyboard.press("End");
    expect(await focusedCell(page)).toBe("servers|guests");
    await page.keyboard.press("Home");
    expect(await focusedCell(page)).toBe("servers|");
    await page.keyboard.press("Control+End");
    expect(await focusedCell(page)).toBe("guests|guests");
    await page.keyboard.press("Control+Home");
    expect(await focusedCell(page)).toBe("|");
    await expect(grid.locator("[tabindex='0']")).toHaveCount(1);
  });
});

test.describe("DataGrid (Grid)", () => {
  test("Enter sorts a header (aria-sort), arrows move over the cells", async ({ page }) => {
    await state(page, "DataGrid", "cell-states");
    const grid = page.getByRole("grid");
    const prefix = grid.getByRole("columnheader", { name: "Préfixe" });
    await prefix.focus();
    await page.keyboard.press("Enter");
    await expect(prefix).toHaveAttribute("aria-sort", "ascending");
    await page.keyboard.press("Enter");
    await expect(prefix).toHaveAttribute("aria-sort", "descending");
    await page.keyboard.press("ArrowDown");
    expect(await focusedCell(page)).toBe("192.0.2.0/24|prefix");
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe("192.0.2.0/24|label");
    await page.keyboard.press("Control+End");
    expect(await focusedCell(page)).toBe("203.0.113.0/24|owner");
    await page.keyboard.press("Control+Home");
    expect(await focusedCell(page)).toBe("|prefix");
    await expect(grid.locator("[tabindex='0']")).toHaveCount(1);
  });

  test("Alt+Down on a filterable header asks for its filter", async ({ page }) => {
    await state(page, "DataGrid", "filtered");
    const env = page.getByRole("columnheader", { name: /Environnement/ });
    await page.getByRole("columnheader", { name: /Préfixe/ }).focus();
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(env).toBeFocused();
    await page.keyboard.press("Alt+ArrowDown");
    await expect(env).toContainText("(filtrée)");
    await page.keyboard.press("Alt+ArrowDown");
    await expect(env).not.toContainText("(filtrée)");
  });
});

test.describe("DraftCounter (toggle + Menu Button)", () => {
  test("toggles the panel, lists the drafts", async ({ page }) => {
    await state(page, "DraftCounter", "drafts");
    const toggle = page.getByRole("button", COUNTER);
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await toggle.focus();
    await page.keyboard.press("Enter");
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("Space");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await page.keyboard.press("Tab");
    const list = page.getByRole("button", { name: "Liste des brouillons" });
    await expect(list).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menuitem")).toHaveText([
      "Salle B (3)",
      "Reprise des VIP (12)",
      "Brouillon du 14 janvier (1)",
    ]);
  });
});
