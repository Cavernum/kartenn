import type { Locator, Page } from "@playwright/test";
import {
  apiCall,
  atRest,
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
// first, then deleted there, said gone in the first by the event `draft` (task 14k); one
// renamed from « Mes brouillons » (task 14f); an operation modified in its edit panel,
// replaced at its place (task 14l). Under
// `valid_at` alone the draft is submitted from that date, its dates of effect in the foot;
// an addition under `valid_at` dates its draft, kept with it back at the present (task
// 14e); an answer lost at the submission, « Réessayer » sends the same key and the server
// replays its answer: one changeset (REQ-006-15), the focus then on what says the tab has
// no draft (task 14h). Under `as_of` the draft is read only (REQ-021-5). Each test writes
// in a domain of its own (RFC 5737, TEST-NET-2 and 3).

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
    // A motive typed (Q-021-22, task 021-12qb): Escape from it asks before the panel drops
    // it; « Continuer la saisie » gives it back, as typed.
    await carol.keyboard.press("Escape");
    const ask = carol.getByRole("alertdialog", { name: "Abandonner les modifications ?" });
    await expect(ask.getByRole("button", { name: "Continuer la saisie" })).toBeFocused();
    await carol.keyboard.press("Enter");
    await expect(summary).toBeFocused();
    await expect(summary).toHaveValue("Bascule des VIP de la salle B");
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

    // Here, where it was active: the event `draft` says it gone (024 § A), without a reload.
    await expect(
      page
        .getByRole("region", { name: "Notifications" })
        .getByText("Le brouillon actif n'existe plus : supprimé, soumis ou repris ailleurs."),
    ).toBeVisible();
    await expect(counter(page)).toHaveAccessibleName(/^Brouillon 0 opération/);
    await expect(row).toHaveCount(0);
    await expectAccessible(page);
  } finally {
    await context.close();
  }
});

/** A draft of the server changing the nature of `prefix`: its identifier. */
async function serverDraft(page: Page, title: string, prefix: string): Promise<string> {
  const created = await apiCall(page, "POST", "/v1/drafts", {
    title,
    operations: [{ op: "update_prefix", id: { id: prefix }, nature: "container" }],
  });
  expect(created.status, JSON.stringify(created.body)).toBe(201);
  return (created.body as { id: string }).id;
}

/** A segment of a date field of the foot, by the label of the field. */
const segment = (scope: Locator, field: string, name: string) =>
  scope.getByRole("group", { name: field }).getByRole("spinbutton", { name, exact: true });

test("under valid_at, submitted from that date: valid_from set, valid_to chosen, both on the changeset", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { prefixes: ids } = await ownedDomain(page, unique(browserName));
  const id = await serverDraft(page, `daté ${browserName} ${Date.now()}`, ids[0]!);
  const at = "2031-03-01T08:00:00.000Z";
  await page.goto(`/drafts/${id}?valid_at=${encodeURIComponent(at)}`);
  const main = page.getByRole("main");
  const result = main.locator(".k-draft-submit__result");
  // Not under `as_of`: the draft is submitted, its period from the date read.
  await expect(result).toContainText("Simulation valide");
  await expect(result).toContainText("effet à partir du");
  await expect(main.getByRole("group", { name: "Effet à partir du" })).toBeVisible();
  await expect(main.locator(".k-split-button__main")).toHaveText(/^Appliquer/);

  // The end of the effect, typed in its segments: in the zone of the browser.
  await segment(main, "Effet jusqu'au", "Année").click();
  await page.keyboard.type("203106302200");
  await expect(result).toContainText("effet du ");
  const end = await page.evaluate(() => new Date(2031, 5, 30, 22, 0).toISOString());
  await expectAccessible(page);

  await main.locator(".k-split-button__main").click();
  await expect(page.getByRole("status").filter({ hasText: "Appliqué :" }).first()).toBeVisible();
  await page.getByRole("button", { name: "Voir le changeset" }).click();
  await expect(page).toHaveURL(/\/changesets\/[0-9a-f-]+/);
  const changeset = new URL(page.url()).pathname.split("/").pop()!;
  const read = (await apiCall(page, "GET", `/v1/changesets/${changeset}`)).body as {
    valid_from: string;
    valid_to: string | null;
  };
  expect(new Date(read.valid_from).toISOString()).toBe(at);
  expect(new Date(read.valid_to ?? "").toISOString()).toBe(end);
});

test("an answer lost at the submission, then « Réessayer »: the same key, one changeset", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  await page.goto(`/prefixes?domain=${slug}`);
  await draftNature(page, "container");
  await counter(page).click();
  await expect(
    panel(page).getByRole("status").filter({ hasText: "Simulation valide" }),
  ).toBeVisible();
  const motive = `réponse perdue ${browserName} ${Date.now()}`;
  await panel(page)
    .getByRole("textbox", { name: /^Résumé/ })
    .fill(motive);

  // The server applies the changeset; its answer never reaches the page.
  const keys: string[] = [];
  await page.route(/\/v1\/drafts\/[^/]+\/submission/, async (route) => {
    if (route.request().method() !== "POST") return route.fallback();
    keys.push(route.request().headers()["idempotency-key"] ?? "");
    if (keys.length > 1) return route.fallback();
    await route.fetch();
    await route.abort("connectionreset");
  });
  await split(page).click();
  // In the foot, or past it when the draft has left the tab (its event come first).
  const retry = panel(page).getByRole("button", { name: "Réessayer" });
  await expect(retry).toBeVisible();
  await retry.click();
  await expect(page.getByRole("status").filter({ hasText: "Appliqué :" }).first()).toBeVisible();
  await expect(retry).toHaveCount(0);
  // « Réessayer » gone with the draft, its focus goes to what says the tab has none.
  await expect(panel(page).getByText(/^Aucun brouillon actif dans cet onglet/)).toBeFocused();
  expect(keys).toHaveLength(2);
  expect(keys[0]).not.toBe("");
  expect(keys[1]).toBe(keys[0]);
  // The replay is the answer of the first: one changeset with this motive.
  const listed = (await apiCall(page, "GET", "/v1/changesets?page_size=200")).body as {
    items: { motive: string }[];
  };
  expect(listed.items.filter((item) => item.motive.startsWith(motive))).toHaveLength(1);
  await expect(cellOf(page, "nature")).toContainText("Conteneur");
});

test("under as_of the draft is read only: said at its head, each write refused", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { prefixes: ids } = await ownedDomain(page, unique(browserName));
  const title = `passé ${browserName} ${Date.now()}`;
  const id = await serverDraft(page, title, ids[0]!);
  const past = new Date(Date.now() - 30_000).toISOString();
  const dated = `as_of=${encodeURIComponent(past)}`;
  // Resumed all the same (a choice of the tab, nothing written), from « Mes brouillons »,
  // where deleting it is refused; then the panel, then its page.
  await page.goto(`/drafts?${dated}`);
  const row = page.getByRole("grid", { name: "Mes brouillons" }).getByRole("row").filter({
    hasText: title,
  });
  await choose(page, row, title, "Reprendre");
  // The list read again by the other browser's drafts may close the menu: opened again then.
  const remove = page.getByRole("menuitem", { name: `Supprimer « ${title} »…` });
  await expect(async () => {
    if (!(await remove.isVisible())) {
      await row.getByRole("button", { name: `Actions sur « ${title} »` }).click();
    }
    await expect(remove).toHaveAttribute("aria-disabled", "true", { timeout: 2_000 });
  }).toPass({ timeout: 15_000 });
  await page.keyboard.press("Escape");
  await counter(page).click();
  await assertReadOnly(panel(page));
  await page.goto(`/drafts/${id}?${dated}`);
  await assertReadOnly(page.getByRole("main"));
  await expectAccessible(page);
  // Nothing was written: the draft keeps its operation.
  const read = (await apiCall(page, "GET", `/v1/drafts/${id}`)).body as { operations: unknown[] };
  expect(read.operations).toHaveLength(1);
});

test("under valid_at, an operation added then removed, another submitted at the date of its addition back at the present", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { prefixes: ids } = await ownedDomain(page, unique(browserName));
  // A month ahead, to the second: the prefix exists then, the present is another date.
  const at = new Date(Math.floor(Date.now() / 1000 + 30 * 86_400) * 1000).toISOString();
  const record = `/prefixes/${ids[0]}`;
  const form = page.getByRole("form", { name: "Modifier : Préfixe 198.51.100.0/24" });
  const toDraft = async (value: string) => {
    await page.getByRole("main").getByRole("button", { name: "Modifier" }).click();
    await form.getByRole("textbox", { name: "Valeur" }).fill(value);
    await page.keyboard.press("Control+Enter");
    await expect(form).toHaveCount(0);
  };
  // The draft in front of the right panel: its tab when the panel is open (the edit panel
  // leaves it so), else the counter, which opens it there (and would close an open one).
  const draftTab = panel(page).getByRole("tab", { name: /Brouillon/ });
  const showDraft = async () => {
    if (await draftTab.isVisible()) await draftTab.click();
    else await counter(page).click();
    await expect(draftTab).toHaveAttribute("aria-selected", "true");
  };
  await page.goto(`${record}?valid_at=${encodeURIComponent(at)}`);
  await toDraft("198.51.100.0/25");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);

  // Simulated at the date of its addition; removed there, the draft keeps that date.
  await showDraft();
  const result = panel(page).locator(".k-draft-submit__result");
  await expect(result).toContainText("Simulation valide");
  await expect(result).toContainText("effet à partir du");
  await panel(page)
    .getByRole("button", { name: /^Retirer l'opération 1 : update_prefix/ })
    .click();
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 0 opération/);
  await toDraft("198.51.100.0/26");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);

  // Back at the present, the page loaded again: the draft is simulated and submitted at the
  // date of its addition, not now.
  await atRest(page);
  await page.goto(record);
  await expect(page.getByRole("status").filter({ hasText: /Lecture au/ })).toHaveCount(0);
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);
  await showDraft();
  await expect(result).toContainText("Simulation valide");
  await expect(result).toContainText("effet à partir du");
  await expect(split(page)).toHaveText(/^Appliquer/);
  await split(page).click();
  await expect(page.getByRole("status").filter({ hasText: "Appliqué :" }).first()).toBeVisible();
  await page.getByRole("button", { name: "Voir le changeset" }).click();
  await expect(page).toHaveURL(/\/changesets\/[0-9a-f-]+/);
  const changeset = new URL(page.url()).pathname.split("/").pop()!;
  const read = (await apiCall(page, "GET", `/v1/changesets/${changeset}`)).body as {
    valid_from: string;
  };
  expect(new Date(read.valid_from).toISOString()).toBe(at);
});

test("renamed from « Mes brouillons »: the title required, the new one listed, the focus back on its menu", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { prefixes: ids } = await ownedDomain(page, unique(browserName));
  const stamp = `${browserName} ${Date.now()}`;
  const title = `à renommer ${stamp}`;
  const renamed = `renommé ${stamp}`;
  const id = await serverDraft(page, title, ids[0]!);
  await page.goto("/drafts");
  const row = page.getByRole("grid", { name: "Mes brouillons" }).getByRole("row").filter({
    hasText: title,
  });
  await choose(page, row, title, "Renommer…");
  const dialog = page.getByRole("dialog", { name: `Renommer « ${title} »` });
  const field = dialog.getByRole("textbox", { name: "Titre" });
  // The focus in the field, its title to replace; emptied, Enter says it is required.
  await expect(field).toBeFocused();
  await expect(field).toHaveValue(title);
  await page.keyboard.press("ControlOrMeta+a");
  await page.keyboard.press("Delete");
  await page.keyboard.press("Enter");
  await expect(field).toHaveAccessibleDescription(/Donnez un titre au brouillon\./);
  await expect(field).toBeFocused();
  await expectAccessible(page);
  await page.keyboard.type(renamed);
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: `Brouillon renommé : ${renamed}` })
      .first(),
  ).toBeVisible();
  // Its row under its new title, the focus on its menu again (2.4.3).
  const again = page.getByRole("grid", { name: "Mes brouillons" }).getByRole("row").filter({
    hasText: renamed,
  });
  await expect(again).toHaveCount(1);
  await expect(again.getByRole("button", { name: `Actions sur « ${renamed} »` })).toBeFocused();
  const read = (await apiCall(page, "GET", `/v1/drafts/${id}`)).body as { title: string };
  expect(read.title).toBe(renamed);
});

test("an operation modified: its edit panel prefilled, replaced at its place, simulated again", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { prefixes: ids } = await ownedDomain(page, unique(browserName), [
    "198.51.100.0/24",
    "203.0.113.0/24",
  ]);
  const title = `à modifier ${browserName} ${Date.now()}`;
  // The second prefix renumbered onto the first (a duplicate), then a change of nature.
  const created = await apiCall(page, "POST", "/v1/drafts", {
    title,
    operations: [
      { op: "update_prefix", id: { id: ids[1] }, prefix: "198.51.100.0/24" },
      { op: "update_prefix", id: { id: ids[0] }, nature: "container" },
    ],
  });
  expect(created.status, JSON.stringify(created.body)).toBe(201);
  const id = (created.body as { id: string }).id;
  await page.goto("/drafts");
  const row = page.getByRole("grid", { name: "Mes brouillons" }).getByRole("row").filter({
    hasText: title,
  });
  await choose(page, row, title, "Reprendre");
  await counter(page).click();
  await expect(
    panel(page).getByRole("alert").filter({ hasText: "Opération 1 : refusée" }),
  ).toBeVisible();

  // « Modifier »: the edit panel of the prefix, the value of the operation in its field. The
  // line is named by what the operation writes, which changes with it.
  const modify = panel(page).getByRole("button", {
    name: /^Modifier l'opération 1 : update_prefix/,
  });
  await expect(modify).toHaveAccessibleName(
    "Modifier l'opération 1 : update_prefix 198.51.100.0/24",
  );
  await modify.click();
  const form = panel(page).getByRole("form", {
    name: "Modifier l'opération 1 : update_prefix 198.51.100.0/24",
  });
  const value = form.getByRole("textbox", { name: /^Valeur/ });
  await expect(value).toBeFocused();
  await expect(value).toHaveValue("198.51.100.0/24");
  await expectAccessible(page);
  await page.keyboard.press("ControlOrMeta+a");
  await page.keyboard.type("203.0.113.0/25");
  await page.keyboard.press("Control+Enter");

  // Replaced at its place: the draft tab again, the focus on that line, valid now.
  await expect(form).toBeHidden();
  await expect(
    page.getByRole("status").filter({ hasText: "Opération 1 remplacée" }).first(),
  ).toBeVisible();
  await expect(modify).toBeFocused();
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 2 opérations/);
  await expect(
    panel(page).getByRole("status").filter({ hasText: "Simulation valide" }),
  ).toBeVisible();
  await expect(split(page)).toHaveText(/^Appliquer/);
  await expectAccessible(page);
  const read = (await apiCall(page, "GET", `/v1/drafts/${id}`)).body as {
    operations: Record<string, unknown>[];
  };
  expect(read.operations).toHaveLength(2);
  expect(read.operations[0]).toMatchObject({
    op: "update_prefix",
    id: { id: ids[1] },
    prefix: "203.0.113.0/25",
  });
  expect(read.operations[1]).toMatchObject({ id: { id: ids[0] }, nature: "container" });
});

/** A draft read only: said at its head, its removal, motive, dates and actions refused. */
async function assertReadOnly(scope: Locator): Promise<void> {
  await expect(
    scope.getByRole("status").filter({ hasText: "Lecture seule : sous un état passé" }),
  ).toBeVisible();
  await expect(
    scope.getByRole("button", { name: /^Retirer l'opération 1 : update_prefix/ }),
  ).toHaveAttribute("aria-disabled", "true");
  await expect(
    scope.getByRole("button", { name: /^Modifier l'opération 1 : update_prefix/ }),
  ).toHaveAttribute("aria-disabled", "true");
  await expect(scope.locator(".k-split-button__main")).toHaveText(/^Enregistrer le brouillon/);
  await expect(scope.locator(".k-split-button__main")).toHaveAttribute("aria-disabled", "true");
  // The motive read only, in the tab order, its reason said (024/M14).
  const summary = scope.getByRole("textbox", { name: /^Résumé/ });
  await expect(summary).toBeEnabled();
  await expect(summary).not.toBeEditable();
  await expect(summary).toHaveAccessibleDescription(/date passée/);
  await expect(scope.getByRole("button", { name: "Choisir les dates d'effet" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
}
