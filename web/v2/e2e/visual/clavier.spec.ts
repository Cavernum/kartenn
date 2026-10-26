import type { Page } from "@playwright/test";
import { expect, test } from "../fixtures/index.ts";

// Keyboard scripts of the APG patterns (spec 020, design « Accessibilité »; REQ-020-18,
// 35, 36, 37; 024/M8, M9, M14): each component of the 024 driven by the keyboard alone on
// its catalogue page, its ARIA states read back.

/** The draft counter: « Brouillon », then what its figure counts. */
const COUNTER = { name: /^Brouillon\s*3\s*opérations$/ };

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

  test("the strip reopens it on its tab; closing gives the focus back to the strip", async ({
    page,
  }) => {
    await state(page, "SidePanel", "closed");
    const reopen = page.getByRole("button", { name: "Ouvrir le panneau" });
    await reopen.focus();
    await page.keyboard.press("Enter");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    await expect(panel.getByRole("tab", { name: "Détail" })).toBeFocused();
    await expect(panel.getByRole("tablist", { name: "Onglets du panneau" })).toBeVisible();
    // Escape in a field belongs to the field: the panel stays.
    await panel.getByRole("tab", { name: /Brouillon/ }).click();
    await panel.getByRole("textbox", { name: /Résumé/ }).focus();
    await page.keyboard.press("Escape");
    await expect(panel).toBeVisible();
    await panel.getByRole("tab", { name: /Brouillon/ }).focus();
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(page.getByRole("button", { name: "Ouvrir le panneau" })).toBeFocused();
  });

  test("covering the surface, makes it inert", async ({ page }) => {
    await state(page, "SidePanel", "cover");
    const surface = page.locator(".k-demo-frame__surface");
    await expect(surface).toHaveJSProperty("inert", true);
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    await panel.getByRole("tab", { name: /Brouillon/ }).focus();
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(surface).toHaveJSProperty("inert", false);
  });

  test("as a sheet: a modal dialog, « Retour » first, the frame inert under it", async ({
    page,
  }) => {
    await state(page, "SidePanel", "sheet");
    const sheet = page.getByRole("dialog", { name: "Panneau droit" });
    const surface = page.locator(".k-demo-frame__surface");
    await expect(sheet).toHaveAttribute("aria-modal", "true");
    await expect(surface).toHaveJSProperty("inert", true);
    // « Retour » is the first stop of the sheet, before its tabs.
    await sheet.getByRole("tab", { name: /Brouillon/ }).focus();
    await page.keyboard.press("Shift+Tab");
    const back = sheet.getByRole("button", { name: "Retour" });
    await expect(back).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(sheet).toBeHidden();
    await expect(surface).toHaveJSProperty("inert", false);
    // Reopened, it takes the focus on its tab.
    await expect(page.getByRole("button", { name: "Ouvrir le panneau" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(sheet.getByRole("tab", { name: /Brouillon/ })).toBeFocused();
  });
});

test.describe("Compact frame (390 px)", () => {
  test.use({ viewport: { width: 390, height: 800 } });

  test("no strip of the panel; the counter opens the sheet, modal over the frame", async ({
    page,
  }) => {
    await page.goto("/_ui/ecrans/E3?vue=edition");
    await expect(page.getByRole("button", { name: "Ouvrir le panneau" })).toHaveCount(0);
    const counter = page.getByRole("button", { name: /^Brouillon\s*\d+\s*opération/ });
    await counter.focus();
    await page.keyboard.press("Enter");
    const sheet = page.getByRole("dialog", { name: "Panneau droit" });
    await expect(sheet.getByRole("tab", { name: /Brouillon/ })).toBeFocused();
    await expect(page.locator(".k-ecran__surface")).toHaveJSProperty("inert", true);
    await expect(page.locator(".k-ecran__bar")).toHaveJSProperty("inert", true);
    await sheet.getByRole("button", { name: "Retour" }).click();
    await expect(sheet).toBeHidden();
    await expect(counter).toBeFocused();
    await expect(page.locator(".k-ecran__surface")).toHaveJSProperty("inert", false);
  });
});

test.describe("Narrow frame (768 px)", () => {
  test.use({ viewport: { width: 768, height: 800 } });

  test("the panel over the whole surface makes it inert, not the bar", async ({ page }) => {
    await page.goto("/_ui/ecrans/E3?vue=edition");
    const counter = page.getByRole("button", { name: /^Brouillon\s*\d+\s*opération/ });
    await counter.focus();
    await page.keyboard.press("Enter");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    await expect(panel).toBeVisible();
    await expect(page.locator(".k-ecran__surface")).toHaveJSProperty("inert", true);
    await expect(page.locator(".k-ecran__bar")).toHaveJSProperty("inert", false);
    await page.keyboard.press("Enter");
    await expect(panel).toBeHidden();
    await expect(page.locator(".k-ecran__surface")).toHaveJSProperty("inert", false);
  });
});

test.describe("Wide frame (1280 px)", () => {
  test("the panel over the end of the surface sets its scroll padding", async ({ page }) => {
    await page.goto("/_ui/ecrans/E3?vue=edition");
    const surface = page.locator(".k-ecran__surface");
    await expect(surface).toHaveCSS("scroll-padding-inline-end", "0px");
    await page.getByRole("button", { name: /^Brouillon\s*\d+\s*opération/ }).click();
    await expect(surface).toHaveCSS("scroll-padding-inline-end", "420px");
    await expect(surface).toHaveJSProperty("inert", false);
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
    // Under a container xs (360 px), the main action alone stays beside « … ».
    await expect(bar.getByRole("button")).toHaveCount(2);
    await bar.getByRole("button", { name: "Créer" }).focus();
    await page.keyboard.press("End");
    const more = bar.getByRole("button", { name: "Plus d'actions" });
    await expect(more).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menuitem")).toHaveText([
      "Modifier",
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

test.describe("BarChart (img + table)", () => {
  test("the image takes the focus; the arrows show each bar's value", async ({ page }) => {
    await state(page, "BarChart", "week");
    const chart = page.getByRole("img", { name: /^Changesets appliqués par jour/ });
    const bubble = page.locator(".k-tooltip:not([hidden])");
    await chart.focus();
    await expect(bubble).toBeVisible();
    const last = await bubble.textContent();
    await page.keyboard.press("Home");
    await expect(bubble).not.toHaveText(last!);
    const first = await bubble.textContent();
    await page.keyboard.press("ArrowRight");
    await expect(bubble).not.toHaveText(first!);
    await page.keyboard.press("End");
    await expect(bubble).toHaveText(last!);
    await page.keyboard.press("Escape");
    await expect(bubble).toBeHidden();
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

  test("the crosshair follows the focus, its headers marked beyond colour", async ({ page }) => {
    await state(page, "MatrixGrid", "rest");
    const grid = page.getByRole("grid");
    await grid.locator("[tabindex='0']").focus();
    expect(await focusedCell(page)).toBe("desks|servers");
    await page.keyboard.press("ArrowRight");
    const source = grid.getByRole("rowheader", { name: "Postes du siège" });
    const destination = grid.getByRole("columnheader", { name: "Postes du siège" });
    await expect(source).toHaveAttribute("data-cross", "true");
    await expect(source).toHaveAttribute("data-flow", "source");
    await expect(destination).toHaveAttribute("data-flow", "destination");
    await expect(destination).toHaveCSS("text-decoration-line", "underline");
    await expect(grid.getByRole("columnheader", { name: "Serveurs" })).not.toHaveAttribute(
      "data-cross",
    );
  });

  test("a focused cell scrolls out from under the sticky headers", async ({ page }) => {
    await state(page, "MatrixGrid", "large");
    const grid = page.getByRole("grid");
    await grid.locator("[tabindex='0']").focus();
    await page.keyboard.press("Control+End");
    await page.keyboard.press("Home");
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe("zone-24|zone-1");
    const cell = (await page.locator(":focus").boundingBox())!;
    const header = (await grid.getByRole("rowheader", { name: "zone-24" }).boundingBox())!;
    expect(cell.x).toBeGreaterThanOrEqual(header.x + header.width - 1);
    await page.keyboard.press("Control+Home");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("PageDown");
    await page.keyboard.press("PageUp");
    const top = (await page.locator(":focus").boundingBox())!;
    const head = (await grid.getByRole("columnheader", { name: "zone-01" }).boundingBox())!;
    expect(top.y).toBeGreaterThanOrEqual(head.y + head.height - 1);
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
    // The funnel is a named button, out of the tab order.
    const funnel = env.getByRole("button", { name: "Filtrer : Environnement" });
    await expect(funnel).toHaveAttribute("tabindex", "-1");
    await funnel.click();
    await expect(env).toHaveAccessibleName("Environnement (filtrée)");
  });

  test("Space and Ctrl+A select rows (aria-multiselectable)", async ({ page }) => {
    await state(page, "DataGrid", "selected");
    const grid = page.getByRole("grid");
    await expect(grid).toHaveAttribute("aria-multiselectable", "true");
    const rows = grid.getByRole("row");
    await expect(rows.nth(1)).toHaveAttribute("aria-selected", "false");
    await grid.getByRole("columnheader", { name: /Préfixe/ }).focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Space");
    await expect(rows.nth(1)).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Shift+Space");
    await expect(rows.nth(1)).toHaveAttribute("aria-selected", "false");
    await page.keyboard.press("Control+a");
    await expect(grid.locator('[role="row"][aria-selected="true"]')).toHaveCount(5);
    await page.keyboard.press("Control+a");
    await expect(grid.locator('[role="row"][aria-selected="true"]')).toHaveCount(0);
  });

  test("a widget in a cell: Enter goes in, Escape comes back; one tab stop", async ({ page }) => {
    await state(page, "DataGrid", "cell-states");
    const grid = page.getByRole("grid");
    const error = grid.locator('[role="gridcell"][data-state="error"]');
    const copy = error.getByRole("button");
    await expect(copy).toHaveAttribute("tabindex", "-1");
    await grid.getByRole("columnheader", { name: "Préfixe" }).focus();
    await error.focus();
    await page.keyboard.press("Enter");
    await expect(copy).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(error).toBeFocused();
    await page.keyboard.press("F2");
    await expect(copy).toBeFocused();
    await page.keyboard.press("F2");
    await expect(error).toBeFocused();
  });

  test("the column chooser: checkbox entries; hiding the stop's column keeps a stop", async ({
    page,
  }) => {
    await state(page, "DataGrid", "priorities");
    const grid = page.getByRole("grid");
    const chooser = page.getByRole("button", { name: "Colonnes" });
    await chooser.focus();
    await page.keyboard.press("Enter");
    const used = page.getByRole("menuitemcheckbox", { name: "Occupation" });
    await expect(used).toHaveAttribute("aria-checked", "false");
    await used.focus();
    await page.keyboard.press("Enter");
    // Checked, and the menu stays open.
    await expect(used).toHaveAttribute("aria-checked", "true");
    await page.keyboard.press("Escape");
    await expect(chooser).toBeFocused();
    const header = grid.getByRole("columnheader", { name: "Occupation" });
    await expect(header).toBeVisible();
    // The roving stop on that column, then the column hidden again.
    await grid.getByRole("columnheader", { name: /Préfixe/ }).focus();
    await page.keyboard.press("End");
    await expect(header).toBeFocused();
    await chooser.click();
    await page.getByRole("menuitemcheckbox", { name: "Occupation" }).click();
    await page.keyboard.press("Escape");
    await expect(header).toBeHidden();
    await expect(grid.locator("[tabindex='0']")).toHaveCount(1);
    expect(
      await grid.locator("[tabindex='0']").evaluate((el) => (el as HTMLElement).dataset.column),
    ).toBe("prefix");
  });
});

test.describe("ExpandableGrid (Grid + Dialog (Modal))", () => {
  test("the button opens the whole table in a dialog; Escape gives the focus back", async ({
    page,
  }) => {
    await state(page, "ExpandableGrid", "compact");
    // By its attribute: the open dialog hides the rest of the page from the tree.
    const enlarge = page.locator('button[aria-label="Agrandir : Flux non conformes"]');
    await expect(enlarge).toHaveAttribute("aria-haspopup", "dialog");
    await expect(page.getByRole("grid")).toHaveAttribute("aria-colcount", "3");
    await enlarge.focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Flux non conformes" });
    await expect(dialog).toBeVisible();
    await expect(enlarge).toHaveAttribute("aria-expanded", "true");
    const grid = dialog.getByRole("grid", { name: "Flux non conformes" });
    await expect(grid).toHaveAttribute("aria-colcount", "5");
    await expect(grid).toHaveAttribute("aria-rowcount", "26");
    // The sort is shared: sorting in the dialog sorts the compact grid too.
    const source = grid.getByRole("columnheader", { name: "Source" });
    await source.focus();
    await page.keyboard.press("Enter");
    await expect(source).toHaveAttribute("aria-sort", "descending");
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(enlarge).toBeFocused();
    await expect(page.getByRole("columnheader", { name: "Source" })).toHaveAttribute(
      "aria-sort",
      "descending",
    );
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
    // The active draft is the checked one of a set, not a check icon alone.
    const radios = page.getByRole("menuitemradio");
    await expect(radios).toHaveText([
      "Salle B (3)",
      "Reprise des VIP (12)",
      "Brouillon du 14 janvier (1)",
    ]);
    await expect(radios.nth(0)).toHaveAttribute("aria-checked", "true");
    await expect(radios.nth(1)).toHaveAttribute("aria-checked", "false");
    await expect(page.getByRole("group").getByRole("menuitemradio")).toHaveCount(3);
  });
});

test.describe("Refusals of a read-only object (E7, 024/M14)", () => {
  for (const [view, title, code, member] of [
    [
      "groupe-refus",
      "Lecture seule : ce groupe est lié sur un périmètre plus large que le vôtre.",
      "core.group.escalation",
      "Périmètre plus large que le vôtre (core.group.escalation).",
    ],
    [
      "secours",
      "Groupe livré : seuls ses membres se gèrent, par un administrateur global.",
      "core.group.builtin",
      "Groupe livré : ses membres se gèrent par un administrateur global (core.group.builtin).",
    ],
  ] as const) {
    test(`${view}: said at the top, each action refused with its reason`, async ({ page }) => {
      await page.goto(`/_ui/ecrans/E7?vue=${view}`);
      const panel = page.getByRole("complementary", { name: "Panneau droit" });
      const notice = panel.getByRole("status").filter({ hasText: title });
      await expect(notice).toBeVisible();
      await expect(notice).toContainText(code);
      // Every write of the panel is refused, focusable, its reason on focus and on hover.
      const add = panel.getByRole("button", { name: "Ajouter un membre" });
      await expect(add).toHaveAttribute("aria-disabled", "true");
      await expect(add).toHaveAttribute("data-blocked", "true");
      await add.focus();
      await expect(page.getByRole("tooltip")).toHaveText(member);
      await expect(add).toHaveAccessibleDescription(member);
      await add.blur();
      await expect(page.getByRole("tooltip")).toBeHidden();
      const remove = panel.getByRole("button", { name: /^Retirer .* du groupe$/ }).first();
      await expect(remove).toHaveAttribute("aria-disabled", "true");
      await remove.hover();
      await expect(page.getByRole("tooltip")).toHaveText(member);
      await page.mouse.move(0, 0);
      for (const name of ["Ajouter un rôle", /^Modifier le rôle/, /^Retirer le rôle/]) {
        await expect(panel.getByRole("button", { name }).first()).toHaveAttribute(
          "aria-disabled",
          "true",
        );
      }
      const bar = panel.getByRole("toolbar", { name: "Actions du groupe" });
      for (const name of ["Modifier", "Clore…"]) {
        await expect(bar.getByRole("button", { name })).toHaveAttribute("aria-disabled", "true");
      }
    });
  }
});
