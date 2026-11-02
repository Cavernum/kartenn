import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { apiCall, DATA, expect, expectAccessible, loginIn, test } from "../fixtures/journeys.ts";

// The administration of the rights by groups in the new shell (spec 021, task 15a;
// REQ-021-12, REQ-025-12; maquette E7), rewritten from web/e2e/journeys/admin.spec.ts (its
// roles and bindings, now the groups): a group chosen in its list opens in the right panel,
// its members and its roles on scopes; a member removed and a role added go to the draft,
// marked on their rows, the removal cancelled from its row, the draft applied; the
// delivered group `secours` says at its top why it is read only; a role lists its
// permissions under their served name, and the groups bound to it; a reader reaches no
// screen of the administration. Task 15b: a service account created, its secret shown once
// then nowhere (neither in the page, nor stored, nor in the draft), a new secret, one
// revoked in the draft; the break-glass account, member of `secours`, read with its
// rights; an issuer detached in the draft; Dana, administrator of one entity, reads her
// identity (groups, effective rights, groups of her IdP) and may manage no account. Task
// 15c: the audit searched by event from its column, an event read in the panel, the same
// search exported as NDJSON; an import followed, resumed, then cancelled once confirmed;
// an import whose sort fails, its violation said by the rank of its element.

const panel = (page: Page) => page.getByRole("complementary", { name: "Panneau droit" });
const counter = (page: Page) => page.getByRole("button", { name: /^Brouillon \d+ opération/ });
/** The draft holds no operation: its counter gone, or at zero. */
const noOperation = (page: Page) =>
  expect(page.getByRole("button", { name: /^Brouillon [1-9]\d* opération/ })).toHaveCount(0);
const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

/** A group of its own for the journey, with a group of the IdP and the role `reader`. */
async function ownGroup(
  page: Page,
  key: string,
  scope: object = { kind: "global" },
): Promise<string> {
  const created = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end administration",
    operations: [
      { op: "set_group", key, labels: { fr: `Équipe ${key}` } },
      { op: "add_member", group: key, member: { kind: "idp_group", issuer: "dev", group: key } },
      { op: "bind", group: key, role: "reader", scope },
    ],
  });
  expect(created.status, JSON.stringify(created.body)).toBe(201);
  const groups = (await apiCall(page, "GET", "/v1/groups?page_size=500")).body as {
    items: { id: string; key: string }[];
  };
  return groups.items.find((g) => g.key === key)!.id;
}

test("a group: its members and roles in the panel, written to the draft, then applied", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const key = `zz-admin-${browserName}-${Date.now().toString(36)}`;
  const id = await ownGroup(page, key);

  await navigation(page).getByRole("button", { name: "Administration" }).click();
  await navigation(page).getByRole("link", { name: "Groupes" }).click();
  await expect(page).toHaveURL(/\/admin\/groups$/);
  const grid = page.getByRole("grid", { name: "Groupes" });
  await grid.getByRole("gridcell", { name: new RegExp(key) }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/groups/${id}$`));
  await expect(page.getByRole("navigation", { name: "Fil d'Ariane" })).toContainText(key);

  const detail = panel(page);
  await expect(detail.getByRole("heading", { level: 2 })).toContainText(key);
  const members = detail.getByRole("list", { name: `Membres de ${key}` });
  const roles = detail.getByRole("list", { name: `Rôles sur périmètres de ${key}` });
  await expect(members.getByRole("listitem")).toHaveText([/dev \/ zz-admin.*groupe de l'IdP/]);
  await expect(roles.getByRole("listitem")).toHaveText([/reader.*Toute l'organisation/]);
  await expectAccessible(page);

  // The member removed: still there, struck, its action now cancels the removal.
  const idp = `dev / ${key}`;
  await members.getByRole("button", { name: `Retirer ${idp} du groupe` }).click();
  await expect(members.getByRole("listitem").first()).toHaveAttribute("data-pending", "removed");
  await expect(detail.getByRole("heading", { name: /^Membres · 1/ })).toContainText(
    "(brouillon : −1, +0)",
  );

  // A role added on the whole organisation, at the end of its list.
  await detail.getByRole("button", { name: "Ajouter un rôle" }).click();
  const dialog = page.getByRole("dialog", { name: `Ajouter un rôle à ${key}` });
  await dialog.getByRole("combobox", { name: /^Rôle/ }).click();
  await page.getByRole("option", { name: /redacteur/ }).click();
  await dialog.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(roles.getByRole("listitem")).toHaveCount(2);
  await expect(roles.getByRole("listitem").last()).toHaveAttribute("data-pending", "added");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 2 opération/);
  await expectAccessible(page);

  // The removal cancelled from its row: out of the draft.
  await members.getByRole("button", { name: `Annuler le retrait de ${idp}` }).click();
  await expect(members.getByRole("listitem").first()).not.toHaveAttribute("data-pending");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);

  // Applied from the draft tab (the counter would close the open panel): the role is the
  // group's.
  await detail.getByRole("tab", { name: /^Brouillon/ }).click();
  await expect(detail.getByRole("status").filter({ hasText: "Simulation valide" })).toBeVisible();
  await detail.locator(".k-split-button__main").click();
  await expect(page.getByRole("status").filter({ hasText: "Appliqué :" }).first()).toBeVisible();
  await detail.getByRole("tab", { name: "Détail" }).click();
  await expect(roles.getByRole("listitem")).toHaveCount(2);
  await expect(roles.getByRole("listitem").last()).not.toHaveAttribute("data-pending");
  await expect(detail.getByRole("heading", { name: /^Rôles sur périmètres · 2/ })).toBeVisible();
});

test("the delivered group says at its top why it is read only", async ({ page, login }) => {
  await login("alice");
  await page.goto("/admin/groups");
  await page
    .getByRole("grid", { name: "Groupes" })
    .getByRole("gridcell", { name: /secours/ })
    .click();
  const detail = panel(page);
  const notice = detail.getByRole("status").filter({ hasText: "Groupe livré : seuls ses membres" });
  await expect(notice).toBeVisible();
  await expect(notice.getByRole("button", { name: /core\.group\.builtin/ })).toBeVisible();
  // Its actions stay, refused with their reason; its members are a global administrator's.
  const edit = detail.getByRole("toolbar", { name: "Actions du groupe" }).getByRole("button", {
    name: /^Modifier/,
  });
  await expect(edit).toHaveAttribute("aria-disabled", "true");
  await expect(detail.getByRole("button", { name: "Ajouter un rôle" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
  await expect(detail.getByRole("button", { name: "Ajouter un membre" })).not.toHaveAttribute(
    "aria-disabled",
  );
  await expectAccessible(page);
});

test("a role: its permissions under their served name, its groups", async ({ page, login }) => {
  await login("alice");
  await page.goto("/admin/roles");
  const grid = page.getByRole("grid", { name: "Rôles" });
  await expect(grid).toContainText("administrator");
  await grid.getByRole("gridcell", { name: /redacteur/ }).click();
  await expect(page).toHaveURL(/\/admin\/roles\/redacteur$/);
  const detail = panel(page);
  const permissions = detail.getByRole("list", { name: "Permissions de redacteur" });
  await expect(permissions.getByRole("listitem")).toHaveCount(10);
  // Each by its served name, copiable.
  await expect(permissions.getByRole("button", { name: "Copier core.prefix.edit" })).toBeVisible();
  await expect(
    detail.getByRole("list", { name: "Groupes liés à redacteur" }).getByRole("listitem"),
  ).toContainText(["redacteurs"]);
  await expectAccessible(page);
});

/** The two simulations of the probe of a group (its closure, its bindings bound again). */
const probed = (page: Page) =>
  Promise.all(
    ['"close_group"', '"bind"'].map((op) =>
      page.waitForResponse(
        (r) =>
          r.url().includes("/v1/changesets?simulate=true") &&
          (r.request().postData() ?? "").includes(op),
      ),
    ),
  );

test("an administrator of one entity: a group of that entity written, one wider read only", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const entities = (await apiCall(page, "GET", `/v1/entities?slug=${DATA.entity}`)).body as {
    items: { id: string }[];
  };
  const entity = { kind: "entity", id: { id: entities.items[0]!.id } };
  const suffix = `${browserName}-${Date.now().toString(36)}`;
  const local = await ownGroup(page, `zz-local-${suffix}`, entity);
  const wide = await ownGroup(page, `zz-wide-${suffix}`);

  const context = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  const dana = await context.newPage();
  try {
    await loginIn(dana, "dana");
    const detail = panel(dana);
    const addMember = detail.getByRole("button", { name: "Ajouter un membre" });

    // Bound on the whole organisation: the server refuses its writes to Dana, said at its top.
    let answers = probed(dana);
    await dana.goto(`/admin/groups/${wide}`);
    await answers;
    const notice = detail
      .getByRole("status")
      .filter({ hasText: "Lecture seule : ce groupe est lié sur un périmètre plus large" });
    await expect(notice).toBeVisible();
    await expect(notice.getByRole("button", { name: /core\.group\.escalation/ })).toBeVisible();
    await expect(addMember).toHaveAttribute("aria-disabled", "true");
    await expect(detail.getByRole("button", { name: /^Modifier le rôle reader/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );

    // Bound on her entity only: written as by any administrator.
    answers = probed(dana);
    await dana.goto(`/admin/groups/${local}`);
    await answers;
    await expect(detail.getByRole("heading", { level: 2 })).toContainText(`zz-local-${suffix}`);
    await expect(detail.getByRole("status").filter({ hasText: "Lecture seule" })).toHaveCount(0);
    await expect(addMember).not.toHaveAttribute("aria-disabled");
    await expect(
      detail.getByRole("button", { name: /^Modifier le rôle reader/ }),
    ).not.toHaveAttribute("aria-disabled");
    await expect(
      detail.getByRole("button", { name: `Retirer dev / zz-local-${suffix} du groupe` }),
    ).not.toHaveAttribute("aria-disabled");
  } finally {
    await context.close();
  }
});

/** The secret a write answered (the creation of an account, a new secret). */
const madeSecret = (page: Page, path: RegExp) =>
  page
    .waitForResponse((r) => r.request().method() === "POST" && path.test(new URL(r.url()).pathname))
    .then(async (r) => ((await r.json()) as { secret: { value: string } }).secret.value);

/** Whether `value` is kept by the browser: its storages (the draft is kept server side). */
const stored = (page: Page, value: string) =>
  page.evaluate((v) => JSON.stringify({ ...localStorage, ...sessionStorage }).includes(v), value);

test("a service account: its secret shown once, a new one, one revoked in the draft", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const name = `zz-sync-${browserName}-${Date.now().toString(36)}`;
  await navigation(page).getByRole("button", { name: "Administration" }).click();
  await navigation(page).getByRole("link", { name: "Comptes de service" }).click();
  await expect(page).toHaveURL(/\/admin\/accounts$/);
  await page
    .getByRole("toolbar", { name: "Actions des comptes de service" })
    .getByRole("button", { name: /^Créer/ })
    .click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toHaveAccessibleName("Créer un compte de service");
  await dialog.getByRole("textbox", { name: /^Nom/ }).fill(name);
  await dialog.getByRole("textbox", { name: /^Motif/ }).fill("end-to-end administration");
  const first = madeSecret(page, /^\/v1\/service-accounts$/);
  await dialog.getByRole("button", { name: "Créer" }).click();
  const value = await first;

  // Shown once, the focus on its copy, never on the cross.
  await expect(dialog).toHaveAccessibleName(`Nouveau secret pour ${name}`);
  await expect(dialog).toContainText("Copiez-le maintenant : il ne sera plus jamais affiché");
  await expect(dialog.getByRole("button", { name: new RegExp(value) })).toBeFocused();
  await expectAccessible(page);
  await dialog.getByRole("button", { name: "J'ai copié le secret" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator("body")).not.toContainText(value);
  expect(await stored(page, value)).toBe(false);
  await noOperation(page);

  // The account and its secret, never its value.
  const grid = page.getByRole("grid", { name: "Comptes de service" });
  await grid.getByRole("gridcell", { name, exact: true }).click();
  await expect(page).toHaveURL(/\/admin\/accounts\/[0-9a-f-]+$/);
  const detail = panel(page);
  await expect(detail.getByRole("heading", { level: 2 })).toHaveText(name);
  const secrets = detail.getByRole("list", { name: `Secrets de ${name}` });
  await expect(secrets.getByRole("listitem")).toHaveCount(1);
  await expect(page.locator("body")).not.toContainText(value);

  // A new secret: shown once too, then the account has two.
  await detail.getByRole("button", { name: "Nouveau secret" }).click();
  await expect(dialog).toHaveAccessibleName(`Nouveau secret pour ${name}`);
  await dialog.getByRole("textbox", { name: /^Motif/ }).fill("end-to-end rotation");
  const second = madeSecret(page, /^\/v1\/service-accounts\/[^/]+\/secrets$/);
  await dialog.getByRole("button", { name: "Générer le secret" }).click();
  const renewed = await second;
  expect(renewed).not.toBe(value);
  await expect(dialog.getByRole("button", { name: new RegExp(renewed) })).toBeFocused();
  // Escape asks first: the secret is lost with the dialog.
  await page.keyboard.press("Escape");
  const leave = page.getByRole("alertdialog", { name: "Fermer sans avoir copié le secret ?" });
  await leave.getByRole("button", { name: "Fermer et l'oublier" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.locator("body")).not.toContainText(renewed);
  expect(await stored(page, renewed)).toBe(false);
  await expect(secrets.getByRole("listitem")).toHaveCount(2);
  await expect(detail.getByRole("heading", { name: /^Secrets actifs · 2/ })).toBeVisible();

  // One revoked in the draft, marked in place; its revocation cancelled from its row.
  await secrets
    .getByRole("button", { name: /^Révoquer le secret créé le/ })
    .last()
    .click();
  await expect(secrets.getByRole("listitem").last()).toHaveAttribute("data-pending", "removed");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);
  await secrets.getByRole("button", { name: /^Annuler la révocation/ }).click();
  await expect(secrets.getByRole("listitem").last()).not.toHaveAttribute("data-pending");
  await noOperation(page);
});

test("the break-glass account: member of `secours`, its rights in read", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto("/admin/identities");
  await page
    .getByRole("grid", { name: "Identités" })
    .getByRole("row", { name: /compte de secours/ })
    .first()
    .getByRole("gridcell")
    .first()
    .click();
  await expect(page).toHaveURL(/\/admin\/identities\/[0-9a-f-]+$/);
  const detail = panel(page);
  await expect(detail.getByRole("heading", { level: 2 })).toContainText("compte de secours");
  const name = (await detail.getByRole("heading", { level: 2 }).textContent())!
    .replace("compte de secours", "")
    .trim();
  await expect(
    detail.getByRole("list", { name: `Groupes de ${name}` }).getByRole("listitem"),
  ).toHaveText([/secours.*direct/]);
  await expect(
    detail.getByRole("list", { name: `Droits effectifs de ${name}` }).getByRole("listitem"),
  ).toHaveText([/administrator.*Toute l'organisation.*via le groupe secours/]);
  await expect(detail).toContainText(
    "Aucun : cette identité ne s'est jamais connectée par un IdP.",
  );
  // Its state is an administrator's; never pseudonymised (not a person).
  const actions = detail.getByRole("toolbar", { name: "Actions de l'identité" });
  await expect(actions.getByRole("button", { name: "Désactiver" })).not.toHaveAttribute(
    "aria-disabled",
  );
  await expect(actions.getByRole("button", { name: /^Pseudonymiser/ })).toHaveCount(0);
  await expectAccessible(page);
});

test("an issuer: detached in the draft, marked in place, then kept", async ({ page, login }) => {
  await login("alice");
  await page.goto("/admin/issuers");
  const grid = page.getByRole("grid", { name: "Émetteurs" });
  const actions = page.getByRole("toolbar", { name: "Actions des émetteurs" });
  // Nothing chosen: « Détacher » says why.
  await expect(actions.getByRole("button", { name: "Détacher" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
  const row = grid.getByRole("row").filter({ hasText: "Toute l'organisation" }).first();
  await row.getByRole("gridcell", { name: "Toute l'organisation" }).click();
  await actions.getByRole("button", { name: "Détacher" }).click();
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);
  await expect(row).toHaveAttribute("data-row-state", "removed");
  await actions.getByRole("button", { name: "Annuler le détachement" }).click();
  await noOperation(page);
  await expectAccessible(page);
});

test.describe("an administrator of one entity", () => {
  test("reads her identity, its groups and rights, and may manage no account", async ({
    browser,
  }) => {
    const context = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
    const dana = await context.newPage();
    try {
      await loginIn(dana, "dana");
      await dana.goto("/admin/identities");
      await dana
        .getByRole("grid", { name: "Identités" })
        .getByRole("gridcell", { name: "Dana Exemple" })
        .click();
      const detail = panel(dana);
      await expect(detail.getByRole("heading", { level: 2 })).toContainText("Dana Exemple");
      await expect(
        detail.getByRole("list", { name: "Groupes de Dana Exemple" }).getByRole("listitem"),
      ).toHaveText([/admins-entite.*via .*admins-entite/]);
      const rights = detail.getByRole("list", { name: "Droits effectifs de Dana Exemple" });
      await expect(rights.getByRole("listitem")).toHaveCount(2);
      await expect(rights).toContainText("Toute l'organisation");
      await expect(rights).toContainText(/Entité .*via le groupe admins-entite/);
      await expect(
        detail
          .getByRole("list", { name: /^Groupes de l'IdP de Dana Exemple/ })
          .getByRole("listitem"),
      ).toHaveText([/admins-entite.*admins-entite/]);
      // The state of an identity is a global administrator's (core.account.admin).
      await expect(
        detail
          .getByRole("toolbar", { name: "Actions de l'identité" })
          .getByRole("button", { name: "Désactiver" }),
      ).toHaveAttribute("aria-disabled", "true");
      await expectAccessible(dana);

      await dana.goto("/admin/accounts");
      await expect(
        dana
          .getByRole("toolbar", { name: "Actions des comptes de service" })
          .getByRole("button", { name: /^Créer/ }),
      ).toHaveAttribute("aria-disabled", "true");
    } finally {
      await context.close();
    }
  });
});

test("the audit: searched by event, an event in the panel, the same search exported", async ({
  page,
  login,
}) => {
  await login("alice");
  await navigation(page).getByRole("button", { name: "Administration" }).click();
  await navigation(page).getByRole("link", { name: "Audit" }).click();
  await expect(page).toHaveURL(/\/admin\/audit$/);
  const grid = page.getByRole("grid", { name: "Audit" });

  // The filter of the event, from its column: Alt+Down on its header opens the picker.
  await grid.getByRole("columnheader", { name: /Événement/ }).click();
  await page.keyboard.press("Alt+ArrowDown");
  const picker = page.getByRole("dialog", { name: /filtre/i });
  await picker.getByRole("textbox", { name: /Valeur/ }).fill("authn.login.succeeded");
  await picker.getByRole("button", { name: "Appliquer" }).click();
  await expect(page).toHaveURL(/[?&]name=authn\.login\.succeeded/);
  // The login of the journey, journaled by the relay: read again until it is there.
  const found = grid.getByRole("gridcell", { name: "authn.login.succeeded" }).first();
  await expect(async () => {
    await page.reload();
    await expect(found).toBeVisible({ timeout: 1_000 });
  }).toPass({ timeout: 20_000 });
  // The event chosen, in the panel, the filter kept.
  await found.click();
  await expect(page).toHaveURL(/\/admin\/audit\/[0-9a-f-]+\?name=authn\.login\.succeeded$/);
  const detail = panel(page);
  await expect(detail.getByRole("heading", { level: 2 })).toHaveText("authn.login.succeeded");
  await expect(detail).toContainText("Réussi");
  await expect(detail.getByRole("list", { name: "Acteur" })).toContainText("Personne");
  await expectAccessible(page);

  // The export of the same search, saved as a file.
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("main").getByRole("button", { name: "Exporter (NDJSON)" }).click(),
  ]);
  expect(download.suggestedFilename()).toMatch(/^kartenn-audit-.*\.ndjson$/);
  const lines = readFileSync((await download.path())!, "utf8")
    .trim()
    .split("\n");
  expect(lines.length).toBeGreaterThan(0);
  for (const line of lines) expect(JSON.parse(line).name).toBe("authn.login.succeeded");
  await expect(page.getByRole("status").filter({ hasText: "Export enregistré" })).toBeVisible();
});

/** An element of an import: an address created in the domain of the data set. */
async function opened(page: Page, source: string, elements: object[]): Promise<string> {
  const made = await apiCall(page, "POST", "/v1/imports", { source });
  expect(made.status, JSON.stringify(made.body)).toBe(201);
  const id = (made.body as { id: string }).id;
  const chunk = await apiCall(page, "PUT", `/v1/imports/${id}/chunks/0`, { elements });
  expect(chunk.status, JSON.stringify(chunk.body)).toBe(201);
  const started = await apiCall(page, "POST", `/v1/imports/${id}/start`);
  expect(started.status, JSON.stringify(started.body)).toBe(202);
  return id;
}

/**
 * The wait for the task of an import: the scheduler of the instance takes one due task
 * every 5 s, the imports of both browsers after the tasks due at its start (006/H18: the
 * sort too is the task's).
 */
const JOB_WAIT = 60_000;

test("an import followed, resumed, then cancelled", async ({ page, login, browserName }) => {
  test.slow();
  await login("alice");
  const source = `e2e-${browserName}-${Date.now().toString(36)}`;
  const domains = (await apiCall(page, "GET", `/v1/domains?slug=${DATA.domain}`)).body as {
    items: { id: string }[];
  };
  // An address that already exists: its batch is refused, the import pauses.
  const id = await opened(page, source, [
    {
      external_id: "e2e-duplicate",
      operation: {
        op: "create_address",
        domain: { id: domains.items[0]!.id },
        address: DATA.address,
        usage: "host",
      },
    },
  ]);

  await navigation(page).getByRole("button", { name: "Changesets" }).click();
  await navigation(page).getByRole("link", { name: "Imports" }).click();
  await expect(page).toHaveURL(/\/imports$/);
  await page.getByRole("grid", { name: "Imports" }).getByRole("gridcell", { name: source }).click();
  await expect(page).toHaveURL(new RegExp(`/imports/${id}$`));
  const detail = panel(page);
  await expect(detail.getByRole("heading", { level: 2 })).toHaveText(source);
  const state = detail.getByRole("status").filter({ hasText: "État" });
  await expect(state).toContainText("En pause", { timeout: JOB_WAIT });
  await expect(detail.getByRole("heading", { name: "Rapport" })).toBeVisible();
  await expectAccessible(page);

  const actions = detail.getByRole("toolbar", { name: "Actions de l'import" });
  const [resumed] = await Promise.all([
    page.waitForResponse((r) => r.url().endsWith(`/imports/${id}/resume`)),
    actions.getByRole("button", { name: "Reprendre" }).click(),
  ]);
  expect(resumed.status()).toBe(202);
  // Its batch refused again, it pauses again.
  await expect(state).toContainText("En pause", { timeout: JOB_WAIT });

  // Cancelled, once confirmed: it does not resume.
  await actions.getByRole("button", { name: "Annuler l'import" }).click();
  const confirm = page.getByRole("alertdialog", { name: `Annuler l'import ${source} ?` });
  await confirm.getByRole("button", { name: "Annuler l'import" }).click();
  await expect(state).toContainText("Annulé");
  await expect(detail.getByRole("button", { name: "Reprendre" })).toHaveCount(0);
  await expect(
    page.getByRole("grid", { name: "Imports" }).getByRole("row").filter({ hasText: source }),
  ).toContainText("Annulé");
});

test("an import whose sort fails is open again with its error", async ({ page, login }) => {
  test.slow();
  await login("alice");
  // An address in a domain that no element defines: the sort of its task fails.
  const id = await opened(page, `e2e-unsorted-${Date.now().toString(36)}`, [
    {
      external_id: "e2e-stray",
      operation: {
        op: "create_address",
        domain: { local: 7 },
        address: "192.0.2.1",
        usage: "host",
      },
    },
  ]);

  await page.goto(`/imports/${id}`);
  const detail = panel(page);
  const state = detail.getByRole("status").filter({ hasText: "État" });
  await expect(state).toContainText("En téléversement", { timeout: JOB_WAIT });
  // A warning (the import waits for more), said politely.
  const alert = detail.getByRole("status").filter({ hasText: "Tri impossible" });
  await expect(alert).toContainText(
    "L'élément de rang 0 utilise une référence locale qu'aucun élément ne définit.",
  );
  await expect(alert).toContainText("import.unknown-reference");
  await expectAccessible(page);
});

test.describe("a reader", () => {
  test.use({ toleratedErrors: [/status of 403/] });

  test("reaches no screen of the administration", async ({ page, login }) => {
    await login("bob");
    await expect(navigation(page).getByRole("button", { name: "Administration" })).toHaveCount(0);
    await page.goto("/admin/groups");
    await expect(page.getByRole("main")).toContainText("Les groupes n'ont pas pu être lus");
  });
});
