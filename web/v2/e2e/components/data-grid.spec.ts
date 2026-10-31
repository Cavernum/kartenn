import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Grid pattern on `DataGrid` (spec 020, design « Inventaire »
// line 20, « Contrat d'édition », « Accessibilité »; REQ-020-18, 19), the keyboard alone
// from the title of the page: one tab stop, arrows, Home, End, Ctrl+Home, Ctrl+End, Page
// Up and Page Down; sort and filter of a header; selection by Space, Shift with an arrow,
// Ctrl+A and the checkboxes; in-place editing (Enter or F2, Escape, Enter or Tab), the
// local check and the host's states after a commit; the width of a column by its handle;
// ten thousand rows virtualised, the focus exact and kept mounted.

/** The row and column keys of the element that has the focus in a grid. */
const focusedCell = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    return `${el.dataset.row ?? "?"}|${el.dataset.column ?? "?"}`;
  });

/** Into the grid from the title of the page, by Tab: the grid is one tab stop. */
async function tabIn(page: Page): Promise<void> {
  await page.locator("#ui-component-title").click();
  await page.keyboard.press("Tab");
}

const first = "192.0.2.0/29";
const row = (i: number) => `192.0.2.${i * 8}/29`;

test.describe("DataGrid (APG Grid)", () => {
  test("one tab stop; arrows, Home, End, Ctrl+Home, Ctrl+End, a page of rows", async ({ page }) => {
    await openState(page, "DataGrid", "keyboard");
    const grid = page.getByRole("grid", { name: "Préfixes du domaine" });
    await tabIn(page);
    // The stop starts on the first header of the data (the checkboxes come before it).
    expect(await focusedCell(page)).toBe("|prefix");
    await expect(grid.locator("[tabindex='0']")).toHaveCount(1);
    await page.keyboard.press("ArrowLeft");
    expect(await focusedCell(page)).toBe("|k-select");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowDown");
    expect(await focusedCell(page)).toBe(`${first}|prefix`);
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe(`${first}|label`);
    await page.keyboard.press("End");
    expect(await focusedCell(page)).toBe(`${first}|owner`);
    await page.keyboard.press("Home");
    expect(await focusedCell(page)).toBe(`${first}|k-select`);
    await page.keyboard.press("Control+End");
    expect(await focusedCell(page)).toBe(`${row(29)}|owner`);
    await page.keyboard.press("PageUp");
    const up = await focusedCell(page);
    expect(up).not.toBe(`${row(29)}|owner`);
    expect(up.endsWith("|owner")).toBe(true);
    await page.keyboard.press("PageDown");
    expect(await focusedCell(page)).toBe(`${row(29)}|owner`);
    await page.keyboard.press("Control+Home");
    expect(await focusedCell(page)).toBe("|k-select");
    // Tab leaves the grid at once; Shift+Tab comes back to the same cell.
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Tab");
    expect(await focusedCell(page)).not.toBe(`${first}|k-select`);
    await page.keyboard.press("Shift+Tab");
    expect(await focusedCell(page)).toBe(`${first}|k-select`);
    await expect(grid.locator("[tabindex='0']")).toHaveCount(1);
  });

  test("a header: Enter sorts (aria-sort), Alt+Down asks for its filter", async ({ page }) => {
    await openState(page, "DataGrid", "keyboard");
    const grid = page.getByRole("grid");
    const prefix = grid.getByRole("columnheader", { name: /^Préfixe/ });
    await tabIn(page);
    await expect(prefix).toBeFocused();
    await expect(prefix).toHaveAttribute("aria-sort", "none");
    await page.keyboard.press("Enter");
    await expect(prefix).toHaveAttribute("aria-sort", "ascending");
    await page.keyboard.press("Space");
    await expect(prefix).toHaveAttribute("aria-sort", "descending");
    await page.keyboard.press("Alt+ArrowDown");
    await expect(prefix).toHaveAccessibleName("Préfixe (filtrée)");
    await page.keyboard.press("Alt+ArrowDown");
    await expect(prefix).toHaveAccessibleName("Préfixe");
    await expect(prefix).toHaveAttribute("aria-keyshortcuts", "Alt+ArrowDown");
    // F2 goes through the controls of the header: its funnel, its handle, back to it.
    await page.keyboard.press("F2");
    await expect(prefix.getByRole("button", { name: "Filtrer : Préfixe" })).toBeFocused();
    await page.keyboard.press("F2");
    await expect(prefix.getByRole("separator")).toBeFocused();
    await page.keyboard.press("F2");
    await expect(prefix).toBeFocused();
    await page.keyboard.press("F2");
    await page.keyboard.press("Escape");
    await expect(prefix).toBeFocused();
  });

  test("selection: Space, Shift+Down from the anchor, Shift+Space, Ctrl+A, the boxes", async ({
    page,
  }) => {
    await openState(page, "DataGrid", "keyboard");
    const grid = page.getByRole("grid");
    const chosen = grid.locator('[role="row"][aria-selected="true"]');
    await expect(grid).toHaveAttribute("aria-multiselectable", "true");
    await tabIn(page);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Space");
    await expect(chosen).toHaveCount(1);
    await page.keyboard.press("Shift+ArrowDown");
    await page.keyboard.press("Shift+ArrowDown");
    await expect(chosen).toHaveCount(3);
    expect(await focusedCell(page)).toBe(`${row(2)}|prefix`);
    // Back up: the range shrinks to the anchor.
    await page.keyboard.press("Shift+ArrowUp");
    await expect(chosen).toHaveCount(2);
    await page.keyboard.press("Shift+Space");
    await expect(chosen).toHaveCount(1);
    await page.keyboard.press("Control+a");
    await expect(chosen).toHaveCount(30);
    const all = grid.getByRole("checkbox", { name: "Sélectionner toutes les lignes" });
    await expect(all).toBeChecked();
    await page.keyboard.press("Control+a");
    await expect(chosen).toHaveCount(0);
    // The box of a row: a click selects it, the focus on its cell (whose keys select).
    await grid.getByRole("checkbox", { name: `Sélectionner ${row(4)}` }).click();
    await expect(chosen).toHaveCount(1);
    expect(await focusedCell(page)).toBe(`${row(4)}|k-select`);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Space");
    await expect(chosen).toHaveCount(2);
    await expect(all).toHaveJSProperty("indeterminate", true);
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("Home");
    await page.keyboard.press("Control+Home");
    await page.keyboard.press("Space");
    await expect(chosen).toHaveCount(30);
  });

  test("editing: Enter or F2 opens, Escape cancels, Enter or Tab commits; the host's states", async ({
    page,
  }) => {
    await openState(page, "DataGrid", "keyboard");
    const grid = page.getByRole("grid");
    await tabIn(page);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowRight");
    const cell = grid.locator(`[data-row="${first}"][data-column="label"]`);
    const before = (await cell.boundingBox())!;
    // F2 opens the editor, its text chosen; the arrows belong to it; Escape cancels.
    await page.keyboard.press("F2");
    const editor = grid.getByRole("textbox", { name: `Libellé — ${first}` });
    await expect(editor).toBeFocused();
    await page.keyboard.press("ArrowLeft");
    await expect(editor).toBeFocused();
    await page.keyboard.type("Nouveau");
    await page.keyboard.press("Escape");
    await expect(cell).toBeFocused();
    await expect(cell).not.toContainText("Nouveau");
    // Enter opens; an empty text is refused by the local check, said, in its bubble.
    await page.keyboard.press("Enter");
    await editor.press("Control+a");
    await page.keyboard.press("Delete");
    await page.keyboard.press("Enter");
    await expect(editor).toBeFocused();
    await expect(editor).toHaveAttribute("aria-invalid", "true");
    await expect(editor).toHaveAccessibleDescription(/Le libellé est obligatoire\./);
    await expect(page.getByRole("alert").filter({ hasText: "obligatoire" })).toContainText(
      `Libellé, ${first} : Le libellé est obligatoire.`,
    );
    // The cell keeps its size, editor and bubble included.
    const editingBox = (await cell.boundingBox())!;
    expect(editingBox.height).toBe(before.height);
    // A text: Enter commits, the focus back on the cell; pending, then in the draft.
    await page.keyboard.type("Serveurs du nord");
    await page.keyboard.press("Enter");
    await expect(cell).toBeFocused();
    await expect(page.locator("[data-demo='commit']")).toHaveText(
      `label de ${first} : Serveurs du nord`,
    );
    await expect(cell).toHaveAttribute("aria-busy", "true");
    await expect(cell).toHaveAttribute("data-state", "draft");
    await expect(cell).toContainText("Serveurs du nord");
    // Tab commits and goes on to the next cell of the row.
    await page.keyboard.press("F2");
    await page.keyboard.type(" 2");
    await page.keyboard.press("Tab");
    expect(await focusedCell(page)).toBe(`${first}|env`);
    // The owner: refused by the host after the commit, its error said and shown, with its code.
    await page.keyboard.press("End");
    const owner = grid.locator(`[data-row="${first}"][data-column="owner"]`);
    await page.keyboard.press("Enter");
    await page.keyboard.type("Personne");
    await page.keyboard.press("Enter");
    await expect(owner).toHaveAttribute("aria-invalid", "true");
    await expect(page.getByRole("alert").filter({ hasText: "inconnu" })).toContainText(
      `Responsable, ${first} : Responsable inconnu. (core.owner.unknown)`,
    );
    await expect(owner).toBeFocused();
    const message = page.locator(`[id="${await owner.getAttribute("aria-describedby")}"]`);
    await expect(message).toBeVisible();
    await expect(message).toContainText("core.owner.unknown");
    expect((await owner.boundingBox())!.height).toBe(before.height);
    // A cell that is not editable: Enter does not open an editor.
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("Enter");
    await expect(grid.getByRole("textbox")).toHaveCount(0);
    await expect(grid.locator(`[data-row="${first}"][data-column="used"]`)).toHaveAttribute(
      "aria-readonly",
      "true",
    );
  });

  test("the width of a column: F2 on its header, arrows, Home, End; Escape back", async ({
    page,
  }) => {
    await openState(page, "DataGrid", "keyboard");
    const grid = page.getByRole("grid");
    const header = grid.getByRole("columnheader", { name: /^Libellé/ });
    await tabIn(page);
    await page.keyboard.press("ArrowRight");
    await expect(header).toBeFocused();
    const handle = grid.getByRole("separator", { name: "Redimensionner la colonne Libellé" });
    await expect(handle).toHaveAttribute("tabindex", "-1");
    await page.keyboard.press("F2");
    await expect(handle).toBeFocused();
    const width = async () => Number(await handle.getAttribute("aria-valuenow"));
    const start = await width();
    await page.keyboard.press("ArrowRight");
    await expect.poll(width).toBe(start + 16);
    await page.keyboard.press("Shift+ArrowRight");
    await expect.poll(width).toBe(start + 80);
    expect(Math.round((await header.boundingBox())!.width)).toBe(start + 80);
    // Home: the narrowest, never under the header's title.
    await page.keyboard.press("Home");
    const min = Number(await handle.getAttribute("aria-valuemin"));
    await expect.poll(width).toBe(min);
    const label = header.locator(".k-data-grid__label");
    expect(await label.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
    // The cells follow the header.
    const cell = grid.locator(`[data-row="${first}"][data-column="label"]`);
    expect(Math.round((await cell.boundingBox())!.width)).toBe(min);
    await page.keyboard.press("Escape");
    await expect(header).toBeFocused();
    // A click on the handle, no drag: the next token width (WCAG 2.5.7).
    await handle.click();
    await expect.poll(width).toBeGreaterThan(min);
  });

  test("10 000 rows: a window in the document, the focus exact and kept mounted", async ({
    page,
  }) => {
    await openState(page, "DataGrid", "virtual");
    const grid = page.getByRole("grid", { name: "Dix mille préfixes" });
    await expect(grid).toHaveAttribute("aria-rowcount", "10001");
    const rows = grid.locator('[role="row"]');
    const count = await rows.count();
    expect(count).toBeGreaterThan(5);
    expect(count).toBeLessThan(300);
    await expect(page.locator("[data-demo='window']")).toContainText("lignes 1 à");
    await tabIn(page);
    await page.keyboard.press("ArrowDown");
    // 500 presses in a row: the focus on the 500th row exactly.
    for (let i = 0; i < 500; i++) await page.keyboard.press("ArrowDown");
    const focused = page.locator(":focus");
    await expect(focused).toHaveAttribute("data-row", "10.1.244.0/24");
    await expect(focused.locator("xpath=..")).toHaveAttribute("aria-rowindex", "502");
    await expect(focused).toBeInViewport();
    // The window follows; the end among the loaded rows.
    await expect(page.locator("[data-demo='window']")).not.toContainText("lignes 1 à");
    expect(await grid.locator('[role="row"]').count()).toBeLessThan(300);
    // Ctrl+End: the last of ten thousand.
    await page.keyboard.press("Control+End");
    await expect(page.locator(":focus")).toHaveAttribute("data-row", "10.39.15.0/24");
    await expect(page.locator(":focus")).toBeInViewport();
    // The view scrolled back to the top (as the wheel does) takes the focused row out of the
    // window: it stays in the document, focused.
    await grid.evaluate((el) => el.scrollTo({ top: 0 }));
    await expect.poll(() => grid.evaluate((el) => el.scrollTop)).toBe(0);
    await expect(grid.locator('[role="row"][aria-rowindex="3"]')).toBeAttached();
    await expect(page.locator(":focus")).toHaveAttribute("data-row", "10.39.15.0/24");
    // Up from there: the row before it, scrolled back into the view.
    await page.keyboard.press("ArrowUp");
    await expect(page.locator(":focus")).toHaveAttribute("data-row", "10.39.14.0/24");
    await expect(page.locator(":focus")).toBeInViewport();
    // Ctrl+A selects all ten thousand (the box of all checked).
    await page.keyboard.press("Control+a");
    await expect(
      grid.getByRole("checkbox", { name: "Sélectionner toutes les lignes" }),
    ).toBeChecked();
  });

  test("an extension column: one custom element per row, given its row", async ({ page }) => {
    await openState(page, "DataGrid", "extension");
    const grid = page.getByRole("grid");
    await expect(grid.getByRole("columnheader", { name: "Supervision (extension)" })).toBeVisible();
    const cells = grid.locator("k-demo-cell");
    await expect(cells).toHaveCount(5);
    await expect(cells.nth(1)).toHaveText("Sondé");
    await expect(cells.nth(0)).toHaveText("Non sondé");
    expect(await cells.nth(1).evaluate((el) => el.hasAttribute("row"))).toBe(false);
  });
});
