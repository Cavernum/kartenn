import type { Locator, Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Tree View pattern on `Tree` (spec 020, design « Inventaire »
// line 21, « Vue et Reka UI », « Contrat de déplacement »; REQ-020-18, 19, 31, 33), the
// keyboard alone from the title of the page: one tab stop; Up, Down, Home, End, Right,
// Left, `*`; the search by typing (Shift for capitals, no stray character); Enter and F2
// rename; moves of declared parents (drag checked first, Ctrl+X and Ctrl+V,
// « Déplacer vers… » listing accepted targets only); a hundred thousand nodes, the focus
// exact after 500 presses and kept mounted; children read page by page.

/** The node of the row that has the focus. */
const focused = (page: Page) =>
  page.evaluate(() => (document.activeElement as HTMLElement).dataset.node ?? "?");

/** The status region of the tree (a refusal, a cut, a move asked). */
const said = (page: Page) => page.locator(".k-tree-frame > [role='status']");

/** Tab until `target` has the focus (at most `max` presses), by keys only. */
async function tabTo(page: Page, target: Locator, max = 6): Promise<void> {
  for (let i = 0; i < max; i++) {
    if (await target.evaluate((el) => el === document.activeElement)) return;
    await page.keyboard.press("Tab");
  }
  await expect(target).toBeFocused();
}

/** Into the tree from the title of the page, by Tab (past the host's button, if any). */
async function tabIn(page: Page, presses = 1): Promise<void> {
  await page.locator("#ui-component-title").click();
  for (let i = 0; i < presses; i++) await page.keyboard.press("Tab");
}

test.describe("Tree (APG Tree View)", () => {
  test("one tab stop; Up, Down, Home, End; Right opens then goes in, Left closes then up; *", async ({
    page,
  }) => {
    await openState(page, "Tree", "expanded");
    const tree = page.getByRole("tree", { name: "Sites" });
    await tabIn(page);
    // The stop is on the selected row.
    expect(await focused(page)).toBe("s12");
    await expect(tree.locator("[tabindex='0']")).toHaveCount(1);
    await page.keyboard.press("ArrowDown");
    expect(await focused(page)).toBe("s121");
    await page.keyboard.press("ArrowLeft");
    expect(await focused(page)).toBe("s12");
    await page.keyboard.press("ArrowLeft");
    await expect(tree.locator("[data-node='s12']")).toHaveAttribute("aria-expanded", "false");
    await expect(tree.locator("[data-node='s121']")).toHaveCount(0);
    await page.keyboard.press("ArrowRight");
    await expect(tree.locator("[data-node='s12']")).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("ArrowRight");
    expect(await focused(page)).toBe("s121");
    await page.keyboard.press("End");
    expect(await focused(page)).toBe("s3");
    await page.keyboard.press("Home");
    expect(await focused(page)).toBe("s1");
    // `*` opens the closed siblings that have children (Site B), not the leaf (Site C).
    await page.keyboard.press("*");
    await expect(tree.locator("[data-node='s2']")).toHaveAttribute("aria-expanded", "true");
    await expect(tree.locator("[data-node='s3']")).not.toHaveAttribute("aria-expanded");
    // Space selects the row of the focus.
    await page.keyboard.press("Space");
    await expect(tree.locator("[data-node='s1']")).toHaveAttribute("aria-selected", "true");
    // Tab leaves the tree; Shift+Tab comes back to the same row.
    await page.keyboard.press("Tab");
    expect(await focused(page)).not.toBe("s1");
    await page.keyboard.press("Shift+Tab");
    expect(await focused(page)).toBe("s1");
    await expect(tree.locator("[tabindex='0']")).toHaveCount(1);
  });

  test("the search by typing: Shift for capitals, no stray character, a pause starts again", async ({
    page,
  }) => {
    await openState(page, "Tree", "keyboard");
    await tabIn(page, 2);
    expect(await focused(page)).toBe("n1");
    // Shift, Control, an arrow it does not use and a function key never enter the search.
    await page.keyboard.press("Shift");
    await page.keyboard.press("Control");
    await page.keyboard.press("F4");
    await page.keyboard.press("Shift+S");
    expect(await focused(page)).toBe("n12");
    await page.keyboard.press("t");
    expect(await focused(page)).toBe("n13");
    await page.waitForTimeout(600);
    await page.keyboard.press("P");
    expect(await focused(page)).toBe("n2");
    // A letter no label starts with: the focus stays.
    await page.waitForTimeout(600);
    await page.keyboard.press("z");
    expect(await focused(page)).toBe("n2");
  });

  test("Enter or F2 renames: Escape cancels, Enter commits; refused empty, refused by the host", async ({
    page,
  }) => {
    await openState(page, "Tree", "keyboard");
    const tree = page.getByRole("tree", { name: "Entités" });
    await tabIn(page, 2);
    await page.keyboard.press("ArrowDown");
    const row = tree.locator("[data-node='n11']");
    await page.keyboard.press("F2");
    const editor = tree.getByRole("textbox", { name: "Réseaux" });
    await expect(editor).toBeFocused();
    await page.keyboard.type("Nouveau");
    await page.keyboard.press("Escape");
    await expect(row).toBeFocused();
    await expect(row).toHaveAccessibleName("Réseaux");
    // Enter opens; an empty name is refused, said, the editor kept.
    await page.keyboard.press("Enter");
    await editor.press("Control+a");
    await page.keyboard.press("Delete");
    await page.keyboard.press("Enter");
    await expect(editor).toBeFocused();
    await expect(editor).toHaveAttribute("aria-invalid", "true");
    await expect(page.getByRole("alert")).toContainText("Le nom est obligatoire. Échap annule");
    await page.keyboard.type("Réseaux du nord");
    await page.keyboard.press("Enter");
    await expect(row).toBeFocused();
    await expect(row).toHaveAccessibleName("Réseaux du nord");
    await expect(row).toHaveAccessibleDescription(/en brouillon/);
    // The host refuses « Personne »: the error and its code on the row, said.
    await page.keyboard.press("F2");
    await page.keyboard.press("Control+a");
    await page.keyboard.type("Personne");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Nom réservé. (entity.name-reserved)");
    await expect(row).toContainText("entity.name-reserved");
    // A node that is not renamed: Enter activates it.
    await page.keyboard.press("Home");
    await page.keyboard.press("Enter");
    await expect(page.locator("[data-demo='activated']")).toHaveText("Ouvrir n1");
  });

  test("derived: nothing to drag, Ctrl+X does nothing", async ({ page }) => {
    await openState(page, "Tree", "expanded");
    const tree = page.getByRole("tree");
    await expect(tree.locator("[draggable]")).toHaveCount(0);
    await tabIn(page);
    await page.keyboard.press("Control+x");
    await expect(tree.locator("[data-move]")).toHaveCount(0);
    await expect(said(page)).toHaveText("");
  });

  test("declared: a refused target is never marked, its drop does nothing; an accepted one moves", async ({
    page,
  }) => {
    await openState(page, "Tree", "keyboard");
    const tree = page.getByRole("tree");
    const row = (id: string) => tree.locator(`[data-node='${id}']`);
    await expect(row("n11")).toHaveAttribute("draggable", "true");
    await tabIn(page, 2);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowRight");
    // Réseaux over its own child: refused, its reason and code, nothing marked accepted.
    await row("n11").hover();
    await page.mouse.down();
    await row("n111").hover();
    await row("n112").hover();
    await row("n111").hover();
    await expect(row("n111")).toHaveAttribute("data-drop", "refused");
    await expect(row("n111")).toContainText("tree.cycle");
    await expect(tree.locator("[data-drop='ok']")).toHaveCount(0);
    await expect(said(page)).toContainText("Cible refusée");
    await page.mouse.up();
    await expect(row("n111")).not.toHaveAttribute("data-drop");
    await expect(said(page)).not.toContainText("demandé");
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
    await expect(said(page)).toContainText("Réseaux dans Postes de travail");
    // Under its new parent now, closed: the row goes, the focus to that parent.
    await expect(row("n11")).toHaveCount(0);
    expect(await focused(page)).toBe("n2");
    expect(
      await tree.getByRole("treeitem").evaluateAll((items) => items.map((i) => i.dataset.node)),
    ).toEqual(["n1", "n12", "n13", "n2", "n3"]);
    await page.keyboard.press("ArrowRight");
    // After its new siblings, still open: Cœur and Accès under it.
    await page.keyboard.press("End");
    for (let i = 0; i < 3; i++) await page.keyboard.press("ArrowUp");
    expect(await focused(page)).toBe("n11");
  });

  test("Ctrl+X then Ctrl+V; a refused paste says why; the host's refusal with its code", async ({
    page,
  }) => {
    await openState(page, "Tree", "keyboard");
    const tree = page.getByRole("tree");
    const row = (id: string) => tree.locator(`[data-node='${id}']`);
    await tabIn(page, 2);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Control+x");
    await expect(row("n11")).toHaveAttribute("data-move", "cut");
    await expect(said(page)).toContainText("Réseaux coupé");
    // Into itself: refused, still cut.
    await page.keyboard.press("Control+v");
    await expect(said(page)).toContainText("Déplacement refusé");
    await expect(row("n11")).toHaveAttribute("data-move", "cut");
    // Escape cancels the cut; cut again, pasted into « Archives », refused by the host.
    await page.keyboard.press("Escape");
    await expect(row("n11")).not.toHaveAttribute("data-move");
    await page.keyboard.press("Control+x");
    await page.keyboard.press("End");
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
    await openState(page, "Tree", "keyboard");
    const tree = page.getByRole("tree");
    const row = (id: string) => tree.locator(`[data-node='${id}']`);
    await tabIn(page, 2);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Space");
    await expect(row("n11")).toHaveAttribute("aria-selected", "true");
    // The host's action, by the keyboard: Shift+Tab to its button.
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("button", { name: "Déplacer vers…" })).toBeFocused();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Déplacer Réseaux" });
    await expect(dialog).toBeVisible();
    const field = dialog.getByRole("combobox", { name: "Nouveau parent" });
    await expect(field).toBeFocused();
    await page.keyboard.press("ArrowDown");
    const options = dialog.page().getByRole("option");
    await expect(options.first()).toBeVisible();
    const listed = await options.allTextContents();
    // Never itself nor one of its descendants.
    expect(listed.join("|")).not.toMatch(/Réseaux|Cœur|Accès/);
    expect(listed.join("|")).toMatch(/Premier niveau/);
    expect(listed.join("|")).toMatch(/Postes de travail/);
    await page.keyboard.type("Postes");
    await expect(options).toHaveText(["Postes de travail"]);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await expect(field).toHaveValue("Postes de travail");
    await tabTo(page, dialog.getByRole("button", { name: "Déplacer", exact: true }));
    await page.keyboard.press("Enter");
    await expect(dialog).toBeHidden();
    await expect(row("n11")).toHaveAttribute("data-move", "pending");
    // Réseaux moved under the closed « Postes de travail », its siblings stayed (the
    // prototype removed the last child instead).
    await expect(row("n11")).toHaveCount(0);
    expect(
      await tree.getByRole("treeitem").evaluateAll((items) => items.map((i) => i.dataset.node)),
    ).toEqual(["n1", "n12", "n13", "n2", "n3"]);
    await expect(row("n2")).toHaveAttribute("aria-expanded", "false");
    // The focus back on the host's button. Escape closes the list first, the dialog
    // after (as FilterPicker, decision A8).
    await expect(page.getByRole("button", { name: "Déplacer vers…" })).toBeFocused();
    await page.keyboard.press("Enter");
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option").first()).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeVisible();
    // The list gone (its fade over), the next Escape is the dialog's.
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
  });

  test("100 000 nodes: a window in the document, the focus exact and kept mounted", async ({
    page,
  }) => {
    await openState(page, "Tree", "virtual");
    const tree = page.getByRole("tree", { name: "Cent mille nœuds" });
    const rows = tree.getByRole("treeitem");
    expect(await rows.count()).toBeLessThan(300);
    await tabIn(page);
    expect(await focused(page)).toBe("v1");
    // 500 presses in a row in a node of a thousand children: its 500th child exactly.
    for (let i = 0; i < 500; i++) await page.keyboard.press("ArrowDown");
    expect(await focused(page)).toBe("v1-500");
    await expect(page.locator(":focus")).toBeInViewport();
    expect(await tree.getByRole("treeitem").count()).toBeLessThan(300);
    // The wheel takes it out of the window: it stays in the document, focused.
    const scroller = page.locator(".k-tree");
    await scroller.evaluate((el) => el.scrollTo({ top: 0 }));
    await expect.poll(() => scroller.evaluate((el) => el.scrollTop)).toBe(0);
    await expect(tree.locator("[data-node='v1-1']")).toBeAttached();
    expect(await focused(page)).toBe("v1-500");
    await page.keyboard.press("ArrowDown");
    expect(await focused(page)).toBe("v1-501");
    await expect(page.locator(":focus")).toBeInViewport();
    // A move by the keyboard alone, without a drag: into « Domaine 2 ».
    await page.keyboard.press("Control+x");
    await page.keyboard.press("End");
    for (let i = 0; i < 98; i++) await page.keyboard.press("ArrowUp");
    expect(await focused(page)).toBe("v2");
    await page.keyboard.press("Control+v");
    await expect(said(page)).toContainText("Nœud 1.501 dans Domaine 2 demandé");
    // Domaine 2 has it, after its thousand; Domaine 1 a thousand less one.
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(page.locator(":focus")).toHaveAttribute("aria-setsize", "1001");
    await page.keyboard.press("Home");
    await page.keyboard.press("ArrowDown");
    await expect(page.locator(":focus")).toHaveAttribute("aria-setsize", "999");
  });

  test("children page by page: aria-setsize -1, the row of the rest reads the next page", async ({
    page,
  }) => {
    await openState(page, "Tree", "paged");
    const tree = page.getByRole("tree", { name: "Groupes de VLAN" });
    const pages = page.locator("[data-demo='pages']");
    const deliver = page.getByRole("button", { name: "Envoyer les pages demandées" });
    // Five of an unknown number; in view, the row of the rest asked for the next page.
    await expect(tree.locator("[data-node='g1-1']")).toHaveAttribute("aria-setsize", "-1");
    await expect(pages).toHaveText("Pages demandées : 1");
    await expect(tree.locator("[data-node='more:g1']")).toHaveAttribute("aria-busy", "true");
    // Asked once only while it comes; given, the row still in view asks for the next one.
    await tabIn(page);
    await page.keyboard.press("End");
    await page.keyboard.press("ArrowUp");
    expect(await focused(page)).toBe("more:g1");
    await tabTo(page, deliver);
    await page.keyboard.press("Enter");
    await expect(tree.locator("[data-node='g1-10']")).toBeAttached();
    await expect(said(page)).toHaveText("5 éléments de plus chargés.");
    await expect(pages).toHaveText("Pages demandées : 2");
    await page.keyboard.press("Enter");
    await expect(tree.locator("[data-node='g1-12']")).toHaveAttribute("aria-setsize", "12");
    await expect(tree.locator("[data-node='more:g1']")).toHaveCount(0);
    // A node opened with its children to read asks for their first page.
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("End");
    await page.keyboard.press("ArrowRight");
    await expect(pages).toHaveText("Pages demandées : 3");
    await expect(tree.locator("[data-node='more:g2']")).toBeAttached();
    await tabTo(page, deliver);
    await page.keyboard.press("Enter");
    await expect(tree.locator("[data-node='g2-3']")).toHaveAttribute("aria-setsize", "3");
  });
});
