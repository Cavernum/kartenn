import type { Browser, Page } from "@playwright/test";
import {
  apiCall,
  expect,
  expectAccessible,
  loginIn,
  ownedDomain,
  test,
} from "../fixtures/journeys.ts";

// The changesets and their review in the new shell (spec 021, task 14; REQ-021-10, 37;
// maquette E4), rewritten from web/e2e/journeys/changesets.spec.ts: a proposal of Carol
// (an editor without `approval.self`) is pending; she reads it among hers, « Valider »
// refused with the permission she lacks (REQ-024-11), « Retirer » offered; Alice finds it in
// « À instruire », counted in the sidebar, validates it, then undoes it — a new changeset —
// and Carol finds hers applied in « Mes changesets »; another proposal rejected with its
// motive, asked in a dialog; on a narrow window, « Retirer » folded in « … », refused to
// Alice with its reason (task 020-31); one withdrawn by Carol, then found by Alice by its author, its
// state and its period (task 14c); a sensitive value masked in the differential to Carol,
// who may not read it (REQ-005-21); the violations of a refused changeset. Each test writes
// in a domain of its own (RFC 5737, TEST-NET-2).

const unique = (browser: string) => `zz-review-${browser}-${Date.now().toString(36)}`;

/** The identity of the person logged in on `page`. */
async function identityOf(page: Page): Promise<string> {
  return ((await apiCall(page, "GET", "/v1/me")).body as { identity: string }).identity;
}

/** A day of the browser's calendar, `days` from today, as the filters type it (AAAA-MM-JJ). */
const day = (page: Page, days = 0) =>
  page.evaluate((days) => {
    const at = new Date();
    at.setDate(at.getDate() + days);
    const two = (n: number) => String(n).padStart(2, "0");
    return `${at.getFullYear()}-${two(at.getMonth() + 1)}-${two(at.getDate())}`;
  }, days);

/**
 * Adds a filter by « Ajouter un filtre » of the bar of the changesets: its property, then
 * its value typed — or, for a closed list (`option`), typed then taken among the suggestions.
 */
async function addFilter(
  page: Page,
  property: RegExp,
  value: string,
  option?: RegExp,
): Promise<void> {
  await page
    .getByRole("group", { name: "Filtres : Changesets" })
    .getByRole("button", { name: "Ajouter un filtre" })
    .click();
  const picker = page.getByRole("dialog", { name: /filtre/i });
  await page.getByRole("option", { name: property }).click();
  if (option) {
    await picker.getByRole("combobox", { name: /Valeur/ }).fill(value);
    await page.getByRole("option", { name: option }).click();
  } else {
    await picker.getByRole("textbox", { name: /Valeur/ }).fill(value);
  }
  await picker.getByRole("button", { name: "Appliquer" }).click();
  await expect(picker).toHaveCount(0);
}

/** Carol, in a context of her own, proposes the creation of an address in `domain`. */
async function propose(browser: Browser, domain: string, address: string, motive: string) {
  const context = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  const carol = await context.newPage();
  await loginIn(carol, "carol");
  const domains = (await apiCall(carol, "GET", `/v1/domains?slug=${domain}`)).body as {
    items: { id: string }[];
  };
  const proposed = await apiCall(carol, "POST", "/v1/changesets?approval=self", {
    motive,
    operations: [
      { op: "create_address", domain: { id: domains.items[0]!.id }, address, usage: "host" },
    ],
  });
  expect(proposed.status, JSON.stringify(proposed.body)).toBe(202);
  return {
    carol,
    context,
    id: (proposed.body as { changeset: { id: string } }).changeset.id,
  };
}

const actions = (page: Page) => page.getByRole("toolbar", { name: "Actions de revue" });
const state = (page: Page, label: string) =>
  page.getByRole("main").locator(".k-status-badge").filter({ hasText: label });

test("a proposal of Carol, validated by Alice from « À instruire », then undone, applied in her list", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  const motive = `proposition ${browserName} ${Date.now()}`;
  const { carol, context, id } = await propose(browser, slug, "198.51.100.20", motive);
  try {
    // Carol reads hers: pending, the permission she lacks to apply it, « Retirer » offered.
    await carol.goto("/changesets");
    await carol.getByRole("button", { name: "Mes changesets" }).click();
    await expect(carol).toHaveURL(/author=/);
    const row = carol.getByRole("grid", { name: "Changesets" }).getByRole("row").filter({
      hasText: motive,
    });
    await expect(row).toContainText("En attente");
    await expect(row).toContainText("vous");
    await row.getByRole("link", { name: motive }).click();
    await expect(carol).toHaveURL(new RegExp(`/changesets/${id}$`));
    await expect(state(carol, "En attente")).toBeVisible();
    const validate = actions(carol).getByRole("button", { name: /^Valider/ });
    await expect(validate).toHaveAttribute("aria-disabled", "true");
    await expect(validate).toHaveAccessibleDescription(/Il faut core\.address\.approval\.self/);
    await expect(actions(carol).getByRole("button", { name: "Retirer" })).not.toHaveAttribute(
      "aria-disabled",
    );
    await expectAccessible(carol);

    await validatedThenUndone(page, id, motive);

    // Back in Carol's « Mes changesets »: her proposal applied, undone by a changeset apart.
    await carol.goto("/changesets");
    await carol.getByRole("button", { name: "Mes changesets" }).click();
    await expect(carol).toHaveURL(/author=/);
    await expect(row).toContainText("Appliqué");
    await expect(row).toContainText("vous");
  } finally {
    await context.close();
  }
});

/**
 * Alice, on `page`, finds the proposal `id` in « À instruire », counted in the sidebar,
 * validates it, then undoes it, confirmed: a new changeset, which its link opens.
 */
async function validatedThenUndone(page: Page, id: string, motive: string): Promise<void> {
  await page.goto("/proposals");
  await expect(
    page.getByRole("navigation", { name: "Navigation principale" }).getByRole("link", {
      name: /À instruire/,
    }),
  ).toContainText(/\d/);
  const proposals = page.getByRole("grid", { name: "À instruire" });
  const proposal = proposals.getByRole("row").filter({ hasText: motive });
  // The permission its author lacks, by its served name.
  await expect(proposal).toContainText(/core\.address\./);
  await expectAccessible(page);
  await proposal.getByRole("link", { name: motive }).click();
  await expect(page).toHaveURL(new RegExp(`/changesets/${id}$`));
  await expect(page.getByRole("tab", { name: /Différentiel/ })).toBeVisible();
  await actions(page)
    .getByRole("button", { name: /^Valider/ })
    .click();
  await expect(state(page, "Appliqué")).toBeVisible();
  await expect(page.getByRole("status").filter({ hasText: "Validé et appliqué" })).toBeVisible();

  // Then undoes it, confirmed: a new changeset, which its link opens.
  await actions(page).getByRole("button", { name: "Annuler ce changeset" }).click();
  const confirm = page.getByRole("alertdialog", { name: /^Annuler « / });
  await expect(confirm).toBeVisible();
  await confirm.getByRole("button", { name: "Annuler ce changeset" }).click();
  const undone = page.getByRole("status").filter({ hasText: /Annulation de « / });
  await expect(undone).toBeVisible();
  await page.getByRole("button", { name: "Voir le changeset" }).click();
  await expect(page).not.toHaveURL(new RegExp(`/changesets/${id}$`));
  await page.getByRole("tab", { name: /Opérations/ }).click();
  await expect(page.getByRole("list", { name: "Opérations" })).toContainText("close");
}

test("a proposal rejected with its motive, asked in a dialog", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  const motive = `rejet ${browserName} ${Date.now()}`;
  const { context, id } = await propose(browser, slug, "198.51.100.30", motive);
  await context.close();

  await page.goto(`/changesets/${id}`);
  await actions(page).getByRole("button", { name: "Rejeter…" }).click();
  const dialog = page.getByRole("dialog", { name: `Rejeter « ${motive} »` });
  const reason = dialog.getByRole("textbox", { name: /Motif du rejet/ });
  await expect(reason).toBeFocused();
  // Without a motive, nothing is sent: the field says why.
  await dialog.getByRole("button", { name: /^Rejeter/ }).click();
  await expect(dialog.getByText("Le motif du rejet est obligatoire.")).toBeVisible();
  await expectAccessible(page);
  await reason.fill("Le réseau change de site d'abord.");
  await dialog.getByRole("button", { name: /^Rejeter/ }).click();
  await expect(dialog).toBeHidden();
  await expect(state(page, "Rejeté")).toBeVisible();
  await expect(page.getByRole("main")).toContainText("Le réseau change de site d'abord.");
  // Nothing more to decide.
  await expect(actions(page).getByRole("button", { name: /^Valider/ })).toHaveCount(0);
});

test("on a narrow window, an action folded in « … » keeps its refusal and its reason", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  // Task 020-31 (REQ-021-37, 024/M14): « Retirer », refused to Alice by a reason alone
  // (only its author withdraws it), folds into « … » and stays refused there, said.
  await page.setViewportSize({ width: 420, height: 800 });
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  const motive = `étroit ${browserName} ${Date.now()}`;
  const { context, id } = await propose(browser, slug, "198.51.100.40", motive);
  await context.close();

  await page.goto(`/changesets/${id}`);
  await expect(state(page, "En attente")).toBeVisible();
  await expect(actions(page).getByRole("button", { name: "Retirer" })).toHaveCount(0);
  const more = actions(page).getByRole("button", { name: "Plus d'actions" });
  await more.press("Enter");
  const menu = page.getByRole("menu", { name: "Plus d'actions" });
  await expect(menu).toBeVisible();
  const withdraw = menu.getByRole("menuitem", { name: "Retirer" });
  await expect(withdraw).toHaveAttribute("aria-disabled", "true");
  await expect(withdraw).toHaveAccessibleDescription("Seul l'auteur retire son changeset.");
  // Reached by the arrows, chosen at the keyboard: the menu stays, nothing is withdrawn.
  const entries = await menu.getByRole("menuitem").count();
  for (let i = 0; i < entries; i++) {
    if (await withdraw.evaluate((el) => el === document.activeElement)) break;
    await page.keyboard.press("ArrowDown");
  }
  await expect(withdraw).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(menu).toBeVisible();
  await expectAccessible(page);
  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(more).toBeFocused();
  await expect(state(page, "En attente")).toBeVisible();
});

test("a proposal withdrawn by its author, then found by its author, its state and its period", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  const motive = `retrait ${browserName} ${Date.now()}`;
  const { carol, context, id } = await propose(browser, slug, "198.51.100.40", motive);
  const author = await identityOf(carol);
  try {
    // Carol withdraws hers: nothing asked, said once done; nothing more to decide.
    await carol.goto(`/changesets/${id}`);
    await expect(state(carol, "En attente")).toBeVisible();
    await actions(carol).getByRole("button", { name: "Retirer" }).click();
    await expect(state(carol, "Retiré")).toBeVisible();
    await expect(carol.getByRole("status").filter({ hasText: `Retiré : ${motive}` })).toBeVisible();
    await expect(actions(carol).getByRole("button", { name: "Retirer" })).toHaveCount(0);
    await expect(actions(carol).getByRole("button", { name: /^Valider/ })).toHaveCount(0);
    await expectAccessible(carol);
  } finally {
    await context.close();
  }

  // Alice chooses its author, by the identifier the list shows: her own changesets gone.
  await page.goto("/changesets");
  const grid = page.getByRole("grid", { name: "Changesets" });
  const row = grid.getByRole("row").filter({ hasText: motive });
  await expect(grid.getByRole("row").filter({ hasText: "vous" }).first()).toBeVisible();
  await addFilter(page, /^Auteur/, author);
  await expect(page).toHaveURL(new RegExp(`[?&]author=${author}`));
  await expect(row).toContainText("Retiré");
  await expect(row).toContainText(author);
  await expect(grid.getByRole("row").filter({ hasText: "vous" })).toHaveCount(0);

  // Then its state, from the closed list: her pending proposals gone.
  await addFilter(page, /^État/, "Ret", /^Retiré/);
  await expect(page).toHaveURL(/[?&]state=withdrawn/);
  await expect(row).toContainText("Retiré");
  await expect(grid.getByRole("row").filter({ hasText: "En attente" })).toHaveCount(0);

  // Then its period: since today, it stays; up to yesterday, nothing, said and cleared.
  const today = await day(page);
  await addFilter(page, /^Soumis depuis/, today);
  await expect(page).toHaveURL(new RegExp(`[?&]from=${today}`));
  await expect(row).toHaveCount(1);
  await expectAccessible(page);
  await addFilter(page, /^Soumis jusqu'au/, await day(page, -1));
  await expect(page.getByRole("main")).toContainText("Aucun objet ne répond à ces filtres");
  await page.getByRole("main").getByRole("button", { name: "Retirer les filtres" }).click();
  await expect(page).toHaveURL(/\/changesets$/);
  await expect(row).toHaveCount(1);
});

test("a sensitive value masked in the differential to a reader without its right", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const {
    prefixes: [prefix],
  } = await ownedDomain(page, unique(browserName));
  // A sensitive property of the test's own (one per browser: both run at once).
  const key = `custom.code_${browserName}_${Date.now().toString(36)}`;
  const declared = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end sensitive property",
    operations: [
      {
        op: "declare_property",
        local: 0,
        key,
        type: "text",
        max_len: 32,
        cardinality: "single",
        carriers: ["prefix"],
      },
      { op: "set_sensitivity", property: { local: 0 }, sensitive: true },
    ],
  });
  expect(declared.status, JSON.stringify(declared.body)).toBe(201);
  const declarations = (await apiCall(page, "GET", "/v1/property-declarations")).body as {
    items: { id: string; key: string }[];
  };
  const property = declarations.items.find((d) => d.key === key)!.id;
  const value = "code-e2e-7f3a";
  const motive = `sensible ${browserName} ${Date.now()}`;
  const assigned = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive,
    operations: [
      {
        op: "assign_property",
        object: { id: prefix },
        kind: "prefix",
        property: { id: property },
        values: [value],
      },
    ],
  });
  expect(assigned.status, JSON.stringify(assigned.body)).toBe(201);
  const id = (assigned.body as { changeset: { id: string } }).changeset.id;

  // Alice, who may read it, sees its value in the differential.
  await page.goto(`/changesets/${id}`);
  await expect(state(page, "Appliqué")).toBeVisible();
  const main = page.getByRole("main");
  await expect(main).toContainText(key);
  await expect(main).toContainText(value);

  // Carol reads the changesets, not this property (`prop.<key>.read`): « masqué », never
  // the value.
  const context = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  try {
    const carol = await context.newPage();
    await loginIn(carol, "carol");
    await carol.goto(`/changesets/${id}`);
    await expect(state(carol, "Appliqué")).toBeVisible();
    const read = carol.getByRole("main");
    await expect(read).toContainText(key);
    await expect(read).toContainText("masqué");
    await expect(read).not.toContainText(value);
    await expectAccessible(carol);
  } finally {
    await context.close();
  }
});

test("the violations of a refused changeset, each by its rule", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  const domains = (await apiCall(page, "GET", `/v1/domains?slug=${slug}`)).body as {
    items: { id: string }[];
  };
  // The network of the domain created again: a duplicate, refused and journaled.
  const motive = `refus ${browserName} ${Date.now()}`;
  const refused = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive,
    operations: [
      {
        op: "create_prefix",
        domain: { id: domains.items[0]!.id },
        prefix: "198.51.100.0/24",
        nature: "network",
      },
    ],
  });
  expect(refused.status, JSON.stringify(refused.body)).toBe(422);
  const problem = refused.body as {
    changeset: { id: string };
    violations: { rule: string; severity: string }[];
  };
  const id = problem.changeset.id;
  const errors = problem.violations.filter((v) => v.severity === "error").length;
  expect(problem.violations.map((v) => v.rule)).toContain("core.prefix.duplicate");

  // Its record: « Refusé », the violations it was refused by counted on their tab, each
  // by its rule, the errors apart.
  await page.goto(`/changesets/${id}`);
  await expect(state(page, "Refusé")).toBeVisible();
  const violations = page.getByRole("tab", { name: /Violations/ });
  await expect(violations).toContainText(String(problem.violations.length));
  await violations.click();
  await expect(page).toHaveURL(/[?&]tab=violations/);
  const panel = page.getByRole("tabpanel", { name: /Violations/ });
  await expect(panel).toContainText(errors === 1 ? "1 erreur" : `${errors} erreurs`);
  for (const { rule } of problem.violations) await expect(panel).toContainText(rule);
  await expectAccessible(page);
  // Nothing to decide on a refused changeset.
  await expect(actions(page).getByRole("button", { name: /^Valider/ })).toHaveCount(0);
  await expect(actions(page).getByRole("button", { name: "Retirer" })).toHaveCount(0);
});
