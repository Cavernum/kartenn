import type { Locator, Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Treegrid pattern on `TreeGrid` (spec 020, design
// « Inventaire » line 21, « Contrat d'édition », « Contrat de déplacement »; REQ-020-18,
// 19, 31, 32), the keyboard alone from the title of the page: one tab stop; on the node
// cell, Up, Down, Home, End, Right (opens, then into the row), Left (closes, then up),
// Enter to the « + »; along the cells, the arrows, Home, End, Ctrl+Home, Ctrl+End; in-place
// editing (Enter or F2, Escape, Enter or Tab) with the host's states; moves of declared
// parents (drag checked first, Ctrl+X and Ctrl+V, « Déplacer vers… »); a hundred thousand
// nodes, the focus exact after 500 presses and kept mounted; children page by page.

/** The row and column of the cell that has the focus (`""`: the node cell). */
const focusedCell = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    return `${el.dataset.row ?? "?"}|${el.dataset.column ?? "?"}`;
  });

/** The status region of the tree. */
const said = (page: Page) => page.locator(".k-tree-grid-frame > [role='status']");

/** Tab until `target` has the focus (at most `max` presses), by keys only. */
async function tabTo(page: Page, target: Locator, max = 6): Promise<void> {
  for (let i = 0; i < max; i++) {
    if (await target.evaluate((el) => el === document.activeElement)) return;
    await page.keyboard.press("Tab");
  }
  await expect(target).toBeFocused();
}

/** Into the treegrid from the title of the page, by Tab (past the host's button, if any). */
async function tabIn(page: Page, presses = 1): Promise<void> {
  await page.locator("#ui-component-title").click();
  for (let i = 0; i < presses; i++) await page.keyboard.press("Tab");
}

test.describe("TreeGrid (APG Treegrid)", () => {
  test("one tab stop; Up, Down; Right opens then goes into the row, Left closes or goes up; Enter to the « + »", async ({
    page,
  }) => {
    await openState(page, "TreeGrid", "add");
    const tree = page.getByRole("treegrid");
    const row = (name: string) => tree.getByRole("row", { name: new RegExp(name) });
    await tabIn(page);
    // The stop starts on the selected row.
    expect(await focusedCell(page)).toBe("p3|");
    await expect(tree.locator("[tabindex='0']")).toHaveCount(1);
    await page.keyboard.press("ArrowUp");
    expect(await focusedCell(page)).toBe("p2|");
    await page.keyboard.press("ArrowLeft");
    expect(await focusedCell(page)).toBe("p1|");
    await page.keyboard.press("ArrowLeft");
    await expect(row("Serveurs")).toHaveAttribute("aria-expanded", "false");
    await expect(row("Bases")).toHaveCount(0);
    await page.keyboard.press("ArrowRight");
    await expect(row("Serveurs")).toHaveAttribute("aria-expanded", "true");
    // Open: Right goes into the row, its next cell; Left back to the node cell.
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe("p1|env");
    await page.keyboard.press("ArrowLeft");
    expect(await focusedCell(page)).toBe("p1|");
    await page.keyboard.press("ArrowDown");
    expect(await focusedCell(page)).toBe("p2|");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("button", { name: "Ajouter dans 192.0.2.0/26" })).toBeFocused();
    await page.keyboard.press("Escape");
    expect(await focusedCell(page)).toBe("p2|");
    await page.keyboard.press("End");
    expect(await focusedCell(page)).toBe("p5|");
    await page.keyboard.press("Home");
    expect(await focusedCell(page)).toBe("d1|");
    // Space selects the row.
    await page.keyboard.press("Space");
    await expect(row("Centre de données nord")).toHaveAttribute("aria-selected", "true");
    await expect(tree.locator("[tabindex='0']")).toHaveCount(1);
  });

  test("the cells: arrows along the row and the column, Home, End, Ctrl+Home, Ctrl+End", async ({
    page,
  }) => {
    await openState(page, "TreeGrid", "add");
    await tabIn(page);
    // Supervision is a leaf: Right goes into its row.
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe("p3|env");
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe("p3|used");
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe("p3|used");
    await page.keyboard.press("ArrowDown");
    expect(await focusedCell(page)).toBe("p4|used");
    await page.keyboard.press("Home");
    expect(await focusedCell(page)).toBe("p4|");
    // Postes is closed: Right opens it (APG Treegrid); a leaf goes into its row.
    await page.keyboard.press("ArrowRight");
    await expect(page.getByRole("row", { name: /Postes/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(await focusedCell(page)).toBe("p4|");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("End");
    expect(await focusedCell(page)).toBe("p5|used");
    await page.keyboard.press("Home");
    expect(await focusedCell(page)).toBe("p5|");
    await page.keyboard.press("Control+Home");
    expect(await focusedCell(page)).toBe("d1|");
    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toBe("d1|env");
    await page.keyboard.press("Control+End");
    expect(await focusedCell(page)).toBe("p5|used");
  });

  test("editing: Enter or F2 opens, Escape cancels, Enter or Tab commits; the host's states", async ({
    page,
  }) => {
    await openState(page, "TreeGrid", "keyboard");
    const tree = page.getByRole("treegrid", { name: "Entités" });
    await tabIn(page, 2);
    expect(await focusedCell(page)).toBe("n12|");
    const label = tree.locator('[data-row="n12"][data-column=""]');
    await page.keyboard.press("F2");
    const editor = tree.getByRole("textbox", { name: "Entité — Serveurs" });
    await expect(editor).toBeFocused();
    await page.keyboard.type("Nouveau");
    await page.keyboard.press("Escape");
    await expect(label).toBeFocused();
    await expect(label).not.toContainText("Nouveau");
    // An empty label: refused by the local check, said with the way out.
    await page.keyboard.press("Enter");
    await editor.press("Control+a");
    await page.keyboard.press("Delete");
    await page.keyboard.press("Enter");
    await expect(editor).toBeFocused();
    await expect(editor).toHaveAttribute("aria-invalid", "true");
    await expect(page.getByRole("alert").filter({ hasText: "attendue" })).toContainText(
      "Entité : une valeur est attendue. Échap annule la saisie.",
    );
    await page.keyboard.type("Serveurs du nord");
    // Tab commits and goes on to the next cell of the row.
    await page.keyboard.press("Tab");
    expect(await focusedCell(page)).toBe("n12|owner");
    await expect(label).toContainText("Serveurs du nord");
    // The owner: refused by the host after the commit, its error and code on the cell.
    const owner = tree.locator('[data-row="n12"][data-column="owner"]');
    await page.keyboard.press("Enter");
    await page.keyboard.press("Control+a");
    await page.keyboard.type("Personne");
    await page.keyboard.press("Enter");
    await expect(owner).toBeFocused();
    await expect(owner).toHaveAttribute("aria-busy", "true");
    await expect(owner).toHaveAttribute("aria-invalid", "true");
    await expect(page.getByRole("alert").filter({ hasText: "inconnu" })).toContainText(
      "Responsable, Serveurs du nord : Responsable inconnu. (entity.owner-unknown)",
    );
    const message = page.locator(`[id="${await owner.getAttribute("aria-describedby")}"]`);
    await expect(message).toBeVisible();
    await expect(message).toContainText("entity.owner-unknown");
    // A node cell that is not editable: Enter activates the row.
    await page.keyboard.press("Control+Home");
    await page.keyboard.press("Enter");
    await expect(page.locator("[data-demo='activated']")).toHaveText("Ouvrir n1");
    await expect(tree.getByRole("textbox")).toHaveCount(0);
  });

  test("derived: nothing to drag, Ctrl+X does nothing, no « Déplacer vers… »", async ({ page }) => {
    await openState(page, "TreeGrid", "expanded");
    const tree = page.getByRole("treegrid");
    await expect(tree.locator("[draggable]")).toHaveCount(0);
    await tabIn(page);
    await page.keyboard.press("Control+x");
    await expect(tree.locator("[data-move]")).toHaveCount(0);
    await expect(said(page)).toHaveText("");
    await expect(page.getByRole("dialog")).toHaveCount(0);
  });

  test("declared: a refused target is never marked, its drop does nothing; an accepted one moves", async ({
    page,
  }) => {
    await openState(page, "TreeGrid", "keyboard");
    const tree = page.getByRole("treegrid");
    const row = (id: string) => tree.locator(`[role='row'][data-node='${id}']`);
    await expect(row("n11")).toHaveAttribute("draggable", "true");
    // Réseaux over its own child: refused, its reason and code, nothing marked accepted.
    await row("n11").hover();
    await page.mouse.down();
    await row("n112").hover();
    await row("n111").hover();
    await expect(row("n111")).toHaveAttribute("data-drop", "refused");
    await expect(row("n111")).toContainText("tree.cycle");
    await expect(tree.locator("[data-drop='ok']")).toHaveCount(0);
    await expect(said(page)).toContainText("Cible refusée");
    await page.mouse.up();
    await expect(row("n111")).not.toHaveAttribute("data-drop");
    await expect(said(page)).not.toContainText("demandé");
    await expect(row("n11")).not.toHaveAttribute("data-move");
    // Over « Postes de travail »: accepted, marked; the drop is an intent, the node pending
    // at its place, then under its new parent.
    await row("n11").hover();
    await page.mouse.down();
    await row("n12").hover();
    await row("n2").hover();
    await expect(row("n2")).toHaveAttribute("data-drop", "ok");
    await page.mouse.up();
    await expect(row("n11")).toHaveAttribute("data-move", "pending");
    await expect(row("n11")).toHaveAttribute("aria-level", "2");
    await expect(row("n11")).not.toHaveAttribute("data-move");
    await expect(said(page)).toContainText("Réseaux dans Postes de travail");
    expect(
      await tree
        .locator("[role='row'][data-node]")
        .evaluateAll((rows) => rows.map((r) => (r as HTMLElement).dataset.node)),
    ).toEqual(["n1", "n12", "n13", "n2", "n21", "n22", "n11", "n111", "n112", "n3"]);
  });

  test("Ctrl+X then Ctrl+V; a refused paste says why; the host's refusal with its code", async ({
    page,
  }) => {
    await openState(page, "TreeGrid", "keyboard");
    const tree = page.getByRole("treegrid");
    const row = (id: string) => tree.locator(`[role='row'][data-node='${id}']`);
    await tabIn(page, 2);
    await page.keyboard.press("ArrowUp");
    expect(await focusedCell(page)).toBe("n112|");
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("ArrowUp");
    expect(await focusedCell(page)).toBe("n11|");
    await page.keyboard.press("Control+x");
    await expect(row("n11")).toHaveAttribute("data-move", "cut");
    await expect(said(page)).toContainText("Réseaux coupé");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Control+v");
    await expect(said(page)).toContainText("Déplacement refusé");
    await expect(row("n11")).toHaveAttribute("data-move", "cut");
    await page.keyboard.press("Escape");
    await expect(row("n11")).not.toHaveAttribute("data-move");
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("Control+x");
    await page.keyboard.press("End");
    expect(await focusedCell(page)).toBe("n3|");
    await page.keyboard.press("Control+v");
    await expect(row("n11")).toHaveAttribute("data-move", "pending");
    await expect(row("n11")).toHaveAttribute("data-move", "refused");
    await expect(row("n11")).toContainText("entity.locked");
    await expect(row("n11")).toHaveAttribute("aria-level", "2");
    await expect(said(page)).toContainText("Archives est verrouillé.");
  });

  test("« Déplacer vers… » lists the accepted targets only; the node chosen moves", async ({
    page,
  }) => {
    await openState(page, "TreeGrid", "keyboard");
    const tree = page.getByRole("treegrid");
    const row = (id: string) => tree.locator(`[role='row'][data-node='${id}']`);
    await tabIn(page, 2);
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("Space");
    await expect(row("n11")).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("button", { name: "Déplacer vers…" })).toBeFocused();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Déplacer Réseaux" });
    await expect(dialog).toBeVisible();
    const field = dialog.getByRole("combobox", { name: "Nouveau parent" });
    await expect(field).toBeFocused();
    await page.keyboard.press("ArrowDown");
    const options = page.getByRole("option");
    await expect(options.first()).toBeVisible();
    const listed = (await options.allTextContents()).join("|");
    expect(listed).not.toMatch(/Réseaux|Cœur|Accès/);
    expect(listed).toMatch(/Premier niveau/);
    await page.keyboard.type("Postes");
    await expect(options).toHaveText(["Postes de travail"]);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await expect(field).toHaveValue("Postes de travail");
    await tabTo(page, dialog.getByRole("button", { name: "Déplacer", exact: true }));
    await page.keyboard.press("Enter");
    await expect(dialog).toBeHidden();
    await expect(page.getByRole("button", { name: "Déplacer vers…" })).toBeFocused();
    await expect(row("n11")).toHaveAttribute("data-move", "pending");
    await expect(row("n11")).not.toHaveAttribute("data-move");
    // Réseaux moved, its siblings stayed (the prototype removed the last child instead).
    await expect(row("n12")).toHaveAttribute("aria-level", "2");
    await expect(row("n13")).toBeAttached();
    expect(
      await tree
        .locator("[role='row'][data-node]")
        .evaluateAll((rows) => rows.map((r) => (r as HTMLElement).dataset.node)),
    ).toEqual(["n1", "n12", "n13", "n2", "n21", "n22", "n11", "n111", "n112", "n3"]);
  });

  test("100 000 nodes: a window in the document, the focus exact and kept mounted", async ({
    page,
  }) => {
    await openState(page, "TreeGrid", "virtual");
    const tree = page.getByRole("treegrid", { name: "Cent mille nœuds" });
    await expect(tree).toHaveAttribute("aria-rowcount", "1101");
    expect(await tree.getByRole("row").count()).toBeLessThan(300);
    await tabIn(page);
    expect(await focusedCell(page)).toBe("v1|");
    // 500 presses in a row in a node of a thousand children: its 500th child exactly.
    for (let i = 0; i < 500; i++) await page.keyboard.press("ArrowDown");
    expect(await focusedCell(page)).toBe("v1-500|");
    await expect(page.locator(":focus").locator("xpath=..")).toHaveAttribute(
      "aria-rowindex",
      "502",
    );
    await expect(page.locator(":focus")).toBeInViewport();
    expect(await tree.getByRole("row").count()).toBeLessThan(300);
    // The wheel takes it out of the window: it stays in the document, focused.
    await tree.evaluate((el) => el.scrollTo({ top: 0 }));
    await expect.poll(() => tree.evaluate((el) => el.scrollTop)).toBe(0);
    await expect(tree.locator("[data-node='v1-1']")).toBeAttached();
    expect(await focusedCell(page)).toBe("v1-500|");
    await page.keyboard.press("ArrowDown");
    expect(await focusedCell(page)).toBe("v1-501|");
    await expect(page.locator(":focus")).toBeInViewport();
    // A move by the keyboard alone: into « Domaine 2 ».
    await page.keyboard.press("Control+x");
    await page.keyboard.press("End");
    for (let i = 0; i < 98; i++) await page.keyboard.press("ArrowUp");
    expect(await focusedCell(page)).toBe("v2|");
    await page.keyboard.press("Control+v");
    await expect(said(page)).toContainText("Nœud 1.501 dans Domaine 2 demandé");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowDown");
    await expect(page.locator(":focus").locator("xpath=..")).toHaveAttribute(
      "aria-setsize",
      "1001",
    );
  });

  test("children page by page: aria-setsize -1, the row of the rest reads the next page", async ({
    page,
  }) => {
    await openState(page, "TreeGrid", "paged");
    const tree = page.getByRole("treegrid", { name: "Groupes de VLAN" });
    const pages = page.locator("[data-demo='pages']");
    const deliver = page.getByRole("button", { name: "Envoyer les pages demandées" });
    // Five of an unknown number: the rows counted -1; the row of the rest asked, in view.
    await expect(tree).toHaveAttribute("aria-rowcount", "-1");
    await expect(tree.locator("[data-node='g1-1']")).toHaveAttribute("aria-setsize", "-1");
    await expect(pages).toHaveText("Pages demandées : 1");
    await expect(tree.locator("[data-node='more:g1']")).toHaveAttribute("aria-busy", "true");
    await tabIn(page);
    await tabTo(page, deliver);
    await page.keyboard.press("Enter");
    await expect(tree.locator("[data-node='g1-10']")).toBeAttached();
    await expect(pages).toHaveText("Pages demandées : 2");
    await page.keyboard.press("Enter");
    await expect(tree.locator("[data-node='g1-12']")).toHaveAttribute("aria-setsize", "12");
    await expect(said(page)).toHaveText("2 éléments de plus chargés, fin de la liste.");
    await expect(tree.locator("[data-node='more:g1']")).toHaveCount(0);
    await expect(tree).toHaveAttribute("aria-rowcount", "15");
    // A node opened with its children to read asks for their first page.
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("End");
    expect(await focusedCell(page)).toBe("g2|");
    await page.keyboard.press("ArrowRight");
    await expect(pages).toHaveText("Pages demandées : 3");
    await tabTo(page, deliver);
    await page.keyboard.press("Enter");
    await expect(tree.locator("[data-node='g2-3']")).toHaveAttribute("aria-setsize", "3");
  });
});
