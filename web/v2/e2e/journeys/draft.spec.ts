import type { Locator, Page } from "@playwright/test";
import {
  apiCall,
  expect,
  expectAccessible,
  loginIn,
  ownedDomain,
  test,
} from "../fixtures/journeys.ts";

// The draft of the new shell (spec 021, task 14; REQ-021-6, 11, 37; spec 024 § A; maquette
// E5), rewritten from web/e2e/journeys/draft.spec.ts: the drafts are the server's now, no
// longer kept in the browser. An edit in place goes to the draft; the counter opens the
// panel on it; the simulation runs; « Appliquer » is the default for who holds
// `approval.self`, Ctrl+Entrée applies with the summary prefilled. Without it, « Appliquer »
// stays in the menu, refused with its permission, and the draft goes to review. A draft
// with a violation says it on its operation, with its code; removed, the draft is valid. A
// draft resumed from another browser of the same identity, with what was written in the
// first, then deleted there. Each test writes in a domain of its own (RFC 5737, TEST-NET-2 and 3).

const unique = (browser: string) => `zz-draft-${browser}-${Date.now().toString(36)}`;

const prefixes = (page: Page) => page.getByRole("grid", { name: "Préfixes" });
const cellOf = (page: Page, column: string) =>
  prefixes(page).locator(`[role="gridcell"][data-column="${column}"]`).first();
const counter = (page: Page) => page.getByRole("button", { name: /^Brouillon \d+ opération/ });
const panel = (page: Page) => page.getByRole("complementary", { name: "Panneau droit" });
const split = (page: Page) => panel(page).locator(".k-split-button__main");

/**
 * An entry of the menu of a row of « Mes brouillons ». The journeys of both browsers write
 * drafts of Alice side by side: the event `draft` of the other one reads the list again,
 * which may close a menu just opened — opened again then.
 */
async function choose(page: Page, row: Locator, title: string, entry: string): Promise<void> {
  await expect(async () => {
    const item = page.getByRole("menuitem", { name: entry });
    if (!(await item.isVisible())) {
      await row.getByRole("button", { name: `Actions sur « ${title} »` }).click();
    }
    await item.click({ timeout: 2_000 });
  }).toPass({ timeout: 15_000 });
}

/** The nature of the only prefix of the list changed in place, Enter: to the draft. */
async function draftNature(page: Page, nature: string): Promise<void> {
  await expect(cellOf(page, "nature")).toContainText("Réseau");
  await prefixes(page)
    .getByRole("columnheader", { name: /^Valeur/ })
    .click({ position: { x: 4, y: 4 } });
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Control+a");
  await page.keyboard.type(nature);
  await page.keyboard.press("Enter");
  await expect(cellOf(page, "nature")).toHaveAttribute("data-state", "draft");
}

test("an edit to the draft, simulated, applied by Ctrl+Entrée with the summary prefilled", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  await page.goto(`/prefixes?domain=${slug}`);
  await draftNature(page, "container");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);

  // The counter opens the panel on the draft, and closes it.
  await counter(page).click();
  await expect(panel(page).getByRole("tab", { name: /Brouillon/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  const operations = panel(page).getByRole("list", { name: /^Opérations du brouillon/ });
  await expect(operations).toContainText("update_prefix");
  await expect(
    panel(page).getByRole("status").filter({ hasText: "Simulation valide" }),
  ).toBeVisible();
  await expect(split(page)).toHaveText(/^Appliquer/);
  const summary = panel(page).getByRole("textbox", { name: /^Résumé/ });
  await expect(summary).toHaveValue("Maintenance du référentiel");
  await expectAccessible(page);

  await summary.focus();
  await page.keyboard.press("Control+Enter");
  await expect(page.getByRole("status").filter({ hasText: "Appliqué :" }).first()).toBeVisible();
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 0 opération/);
  await expect(cellOf(page, "nature")).toContainText("Conteneur");
  // The changeset applied, by its link.
  await page.getByRole("button", { name: "Voir le changeset" }).click();
  await expect(page.getByRole("main").locator(".k-status-badge")).toContainText("Appliqué");
});

test("without approval.self, « Appliquer » is refused with its permission; the draft goes to review", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  const context = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  const carol = await context.newPage();
  try {
    await loginIn(carol, "carol");
    await carol.goto(`/prefixes?domain=${slug}`);
    await draftNature(carol, "container");
    await counter(carol).click();
    await expect(
      panel(carol).getByRole("status").filter({ hasText: "Simulation valide" }),
    ).toBeVisible();
    await expect(split(carol)).toHaveText(/^Soumettre en revue/);
    // In the menu of the split button, « Appliquer » stays, refused, its permission said.
    await panel(carol).getByRole("button", { name: "Choisir l'action" }).click();
    const apply = carol.getByRole("menuitem", { name: /^Appliquer/ });
    await expect(apply).toHaveAttribute("aria-disabled", "true");
    await expect(apply).toHaveAccessibleDescription(/Il faut core\.prefix\.approval\.self/);
    await expectAccessible(carol);
    await carol.keyboard.press("Escape");
    // Only « Appliquer » prefills a summary: the review asks for one.
    await split(carol).click();
    const summary = panel(carol).getByRole("textbox", { name: /^Résumé/ });
    await expect(summary).toBeFocused();
    await summary.fill("Bascule des VIP de la salle B");
    await split(carol).click();
    await expect(
      carol.getByRole("status").filter({ hasText: "Proposé, en revue" }).first(),
    ).toBeVisible();
    await expect(counter(carol)).toHaveAccessibleName(/^Brouillon 0 opération/);
  } finally {
    await context.close();
  }
});

test("a violation on its operation; removed, the draft is valid; resumed and deleted elsewhere", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const { prefixes: ids } = await ownedDomain(page, unique(browserName), [
    "198.51.100.0/24",
    "203.0.113.0/24",
  ]);
  const title = `brouillon ${browserName} ${Date.now()}`;
  // A draft of the server: the second prefix renumbered onto the first (a duplicate), and a
  // valid change of nature.
  const created = await apiCall(page, "POST", "/v1/drafts", {
    title,
    operations: [
      { op: "update_prefix", id: { id: ids[1] }, prefix: "198.51.100.0/24" },
      { op: "update_prefix", id: { id: ids[0] }, nature: "container" },
    ],
  });
  expect(created.status, JSON.stringify(created.body)).toBe(201);

  // Resumed from the page of the drafts: the active draft of the tab.
  await page.goto("/drafts");
  const grid = page.getByRole("grid", { name: "Mes brouillons" });
  const row = grid.getByRole("row").filter({ hasText: title });
  await expectAccessible(page);
  await choose(page, row, title, "Reprendre");
  await expect(row).toContainText("(actif)");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 2 opérations/);

  await counter(page).click();
  const refused = panel(page).getByRole("alert").filter({ hasText: "Opération 1 : refusée" });
  await expect(refused).toBeVisible();
  await expect(refused).toContainText("core.prefix.duplicate");
  await expect(split(page)).toHaveText(/^Enregistrer le brouillon/);
  await expectAccessible(page);

  // Removed: the other one stays, the simulation is valid again.
  await panel(page)
    .getByRole("button", { name: /^Retirer l'opération 1 : update_prefix/ })
    .click();
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);
  await expect(
    panel(page).getByRole("status").filter({ hasText: "Simulation valide" }),
  ).toBeVisible();
  await expect(split(page)).toHaveText(/^Appliquer/);

  // The same identity in another browser: the draft is there, kept by the server, resumed
  // there with what was written here (REQ-021-11); then deleted.
  const context = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  const other = await context.newPage();
  try {
    await loginIn(other, "alice");
    await other.goto("/drafts");
    const there = other.getByRole("grid", { name: "Mes brouillons" }).getByRole("row").filter({
      hasText: title,
    });
    await expect(there).toContainText("1");
    await choose(other, there, title, "Reprendre");
    await expect(counter(other)).toHaveAccessibleName(/^Brouillon 1 opération/);
    await choose(other, there, title, `Supprimer « ${title} »…`);
    await other
      .getByRole("alertdialog", { name: `Supprimer « ${title} » ?` })
      .getByRole("button", { name: "Supprimer" })
      .click();
    await expect(there).toHaveCount(0);
    await expect(counter(other)).toHaveAccessibleName(/^Brouillon 0 opération/);
  } finally {
    await context.close();
  }
});
