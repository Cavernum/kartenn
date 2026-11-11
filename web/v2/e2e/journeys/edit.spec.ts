import type { Locator, Page } from "@playwright/test";
import { DATA, expect, expectAccessible, loginIn, test } from "../fixtures/journeys.ts";

// In-place editing and quick edit of the new shell (spec 021, task 12; REQ-021-15, 16, 36,
// 37), rewritten from web/e2e/journeys/edit.spec.ts: a reader has no editor and is told
// why; an author edits a prefix at the keyboard alone, to the draft (counter at 1); under
// an approval policy, Ctrl+Enter with its motive leaves the row « en revue », never
// « appliqué »; two tabs on the same prefix, the second gets 412 and keeps its input
// beside the new value, then applies it on the new version. The same in a cell of the
// tree of the topography (task 12a), at the version its page served; Enter on a node opens
// its record, `c` adds in it (021/A20). Each test writes in a domain of its own (RFC 5737,
// TEST-NET-2).

/** A call of the API by the page itself, under its session (same origin: CSRF). */
async function call(
  page: Page,
  method: string,
  path: string,
  body?: unknown,
): Promise<{ status: number; body: unknown }> {
  return page.evaluate(
    async ({ method, path, body }) => {
      const response = await fetch(path, {
        method,
        headers: { "Content-Type": "application/json", "Idempotency-Key": crypto.randomUUID() },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      });
      const text = await response.text();
      return { status: response.status, body: text ? JSON.parse(text) : null };
    },
    { method, path, body },
  );
}

/** A slug unique to this run and this browser. */
// After `production` in the order of the slugs: the topography opens on the first domain,
// which the journey `tree` expects to be the data set's.
const unique = (browser: string) => `zz-edit-${browser}-${Date.now().toString(36)}`;

/**
 * A domain owned by the entity of the data set, holding the network 198.51.100.0/24, under
 * an approval policy if asked: its slug (the list of the prefixes filters by it).
 */
async function newDomain(page: Page, slug: string, policy = false): Promise<string> {
  const declarations = (await call(page, "GET", "/v1/property-declarations")).body as {
    items: { id: string; key: string }[];
  };
  const owner = declarations.items.find((d) => d.key === "core.owner")!.id;
  const entities = (await call(page, "GET", `/v1/entities?slug=${DATA.entity}`)).body as {
    items: { id: string }[];
  };
  const created = await call(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end in-place edit",
    operations: [
      { op: "create_domain", local: 0, slug },
      {
        op: "assign_property",
        object: { local: 0 },
        kind: "domain",
        property: { id: owner },
        values: [entities.items[0]!.id],
      },
      { op: "create_prefix", domain: { local: 0 }, prefix: "198.51.100.0/24", nature: "network" },
    ],
  });
  expect(created.status).toBe(201);
  const domains = (await call(page, "GET", `/v1/domains?slug=${slug}`)).body as {
    items: { id: string }[];
  };
  const id = domains.items[0]!.id;
  if (policy) {
    const held = await call(page, "POST", "/v1/changesets?approval=self", {
      motive: "end-to-end approval",
      operations: [{ op: "set_approval_policy", name: slug, domains: [id], enabled: true }],
    });
    expect(held.status).toBe(201);
  }
  return slug;
}

const prefixes = (page: Page) => page.getByRole("grid", { name: "Préfixes" });
/** The cell `column` of the only row, reached by the keyboard from the header of the grid. */
async function toCell(page: Page, column: "value" | "nature"): Promise<void> {
  await prefixes(page)
    .getByRole("columnheader", { name: /^Valeur/ })
    .click({ position: { x: 4, y: 4 } });
  await page.keyboard.press("ArrowDown");
  if (column === "nature") await page.keyboard.press("ArrowRight");
}
const cellOf = (page: Page, column: string) =>
  prefixes(page).locator(`[role="gridcell"][data-column="${column}"]`).first();

test("a reader sees no editor and why; an author edits a prefix at the keyboard, to the draft", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const domain = await newDomain(page, unique(browserName));

  // Bob reads: the cell offers no editor, and the foot of the list says which right is missing.
  const other = await browser.newContext({ ignoreHTTPSErrors: true });
  const reader = await other.newPage();
  await loginIn(reader, "bob");
  await reader.goto(`/prefixes?domain=${domain}`);
  await expect(cellOf(reader, "value")).toContainText("198.51.100.0/24");
  await toCell(reader, "value");
  await reader.keyboard.press("F2");
  await expect(prefixes(reader).getByRole("textbox")).toHaveCount(0);
  await expect(cellOf(reader, "value")).toHaveAttribute("aria-readonly", "true");
  await expect(cellOf(reader, "value")).toHaveAccessibleDescription(
    "Lecture seule : il faut core.prefix.edit",
  );
  await expect(reader.locator(".k-keyboard-hint")).toContainText("Lecture seule : il faut");
  await expect(reader.getByRole("button", { name: /core\.prefix\.edit/ })).toBeVisible();
  await other.close();

  // Alice, at the keyboard alone: Enter opens, Enter adds to the draft.
  await page.goto(`/prefixes?domain=${domain}`);
  await expect(cellOf(page, "value")).toContainText("198.51.100.0/24");
  await toCell(page, "value");
  await page.keyboard.press("Enter");
  const editor = prefixes(page).getByRole("textbox", { name: /^Valeur — / });
  await expect(editor).toBeFocused();
  await page.keyboard.press("Control+a");
  await page.keyboard.type("198.51.100.0/25");
  await page.keyboard.press("Enter");
  await expect(cellOf(page, "value")).toHaveAttribute("data-state", "draft");
  await expect(cellOf(page, "value")).toContainText("198.51.100.0/25");
  await expect(page.getByRole("button", { name: /^Brouillon 1 opération$/ })).toBeVisible();
  await expectAccessible(page);
});

test("under an approval policy, Ctrl+Enter with its motive: « en revue », never applied, its changeset linked", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const domain = await newDomain(page, unique(browserName), true);
  await page.goto(`/prefixes?domain=${domain}`);
  await expect(cellOf(page, "nature")).toContainText("Réseau");
  await toCell(page, "nature");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Control+a");
  await page.keyboard.type("container");
  await page.keyboard.press("Control+Enter");
  // The motive asked in line, prefilled, the focus on it.
  const line = page.getByRole("form", { name: /^Appliquer maintenant : Nature de / });
  const motive = line.getByRole("textbox", { name: "Motif" });
  await expect(motive).toBeFocused();
  await expect(motive).toHaveValue(/^Modification rapide : Nature de 198\.51\.100\.0\/24$/);
  await expectAccessible(page);
  const held = page.waitForResponse((r) => r.request().method() === "PATCH" && r.status() === 202);
  await page.keyboard.press("Enter");
  // The changeset held, as the answer names it (`Link`, rel changeset).
  const link = (await held).headers()["link"] ?? "";
  const changeset = /^<\/v1\/changesets\/([0-9a-f-]{36})>/.exec(link)?.[1];
  expect(changeset, link).toBeDefined();
  const proposed = page.getByRole("status").filter({ hasText: "Proposé, en revue" }).first();
  await expect(proposed).toBeVisible();
  await expect(cellOf(page, "nature")).toHaveAttribute("data-state", "review");
  // The cell says why it is held, the reason of the answer (REQ-021-15).
  await expect(cellOf(page, "nature")).toHaveAccessibleDescription(
    "Une politique d'approbation retient la modification : elle attend une revue.",
  );
  // Seen too: the foot of the list says it for the cell of the focus, back on it.
  await expect(cellOf(page, "nature")).toBeFocused();
  await expect(
    page.locator(".k-keyboard-hint").filter({ hasText: "Une politique d'approbation retient" }),
  ).toBeVisible();
  // The value read stays in the cell: the one proposed is not the object's yet.
  await expect(cellOf(page, "nature")).toContainText("Réseau");
  await expect(page.getByText(/Appliqué :/)).toHaveCount(0);
  // The toast leads to that changeset, pending, with the motive.
  await page.getByRole("button", { name: "Voir la proposition" }).click();
  await expect(page).toHaveURL(new RegExp(`/changesets/${changeset}$`));
  await expect(
    page.getByRole("main").getByText("Modification rapide : Nature de 198.51.100.0/24").first(),
  ).toBeVisible();
});

test("a refusal of the server (403) keeps the input and says the alert", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const slug = await newDomain(page, unique(browserName));
  const listed = (await call(page, "GET", `/v1/prefixes?domain=${slug}`)).body as {
    items: { id: string }[];
  };
  await page.goto(`/prefixes/${listed.items[0]!.id}`);
  // The right withdrawn between the read and the write: the shell, read before, shows the
  // form; the server, the judge, refuses. Its answer given by the route, as kartenn-server
  // words a 403: urn:kartenn:problem:forbidden, its Request-Id a UUID v7 (request_id.rs),
  // also its `instance` (urn:uuid:…).
  const requestId = "01934f2a-7c3e-7b8a-9c1d-000000000403";
  await page.route(/\/v1\/prefixes\/[0-9a-f-]{36}\?approval=self$/, (route) =>
    route.request().method() === "PATCH"
      ? route.fulfill({
          status: 403,
          contentType: "application/problem+json",
          headers: { "Request-Id": requestId },
          body: JSON.stringify({
            type: "urn:kartenn:problem:forbidden",
            title: "The action is not permitted",
            status: 403,
            detail: "core.prefix.edit is not granted on this domain",
            instance: `urn:uuid:${requestId}`,
          }),
        })
      : route.fallback(),
  );
  await page.getByRole("main").getByRole("button", { name: "Modifier" }).click();
  const form = page.getByRole("form", { name: "Modifier : Préfixe 198.51.100.0/24" });
  await form.getByRole("button", { name: "Choisir l'action" }).click();
  await page.getByRole("menuitem", { name: "Appliquer" }).click();
  const motive = form.getByRole("textbox", { name: "Motif" });
  await motive.fill("bout en bout : refus");
  const value = form.getByRole("textbox", { name: "Valeur" });
  await value.fill("198.51.100.0/25");
  const sent = page.waitForResponse((r) => r.request().method() === "PATCH");
  await page.keyboard.press("Control+Enter");
  expect((await sent).status()).toBe(403);
  const alert = form
    .getByRole("alert")
    .filter({ hasText: "Vous n'avez pas le droit de faire cela" });
  await expect(alert).toBeVisible();
  await expect(alert).toContainText("Votre saisie est gardée.");
  await expect(alert.getByRole("button", { name: `Copier ${requestId}` })).toBeVisible();
  // The panel stays, the input and the motive in their fields; nothing said as applied.
  await expect(value).toHaveValue("198.51.100.0/25");
  await expect(motive).toHaveValue("bout en bout : refus");
  await expect(page.getByText(/Appliqué :/)).toHaveCount(0);
  await expectAccessible(page);
});

test("two tabs on the same prefix: the second gets 412 and keeps its input, then applies", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const domain = await newDomain(page, unique(browserName));
  const list = `/prefixes?domain=${domain}`;
  const second = await page.context().newPage();
  for (const tab of [page, second]) {
    await tab.goto(list);
    await expect(cellOf(tab, "value")).toContainText("198.51.100.0/24");
  }

  /** In `tab`: the value of the prefix typed, then Ctrl+Enter and the motive by Enter. */
  async function ask(tab: Page, value: string): Promise<void> {
    await toCell(tab, "value");
    await tab.keyboard.press("Enter");
    await tab.keyboard.press("Control+a");
    await tab.keyboard.type(value);
    await tab.keyboard.press("Control+Enter");
    await expect(tab.getByRole("textbox", { name: "Motif" })).toBeFocused();
  }

  // The second tab asks first, on the version both read; the first one then applies.
  await ask(second, "198.51.100.0/26");
  await ask(page, "198.51.100.0/25");
  await page.keyboard.press("Enter");
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: /Appliqué : Valeur de/ })
      .first(),
  ).toBeVisible();
  await expect(cellOf(page, "value")).toContainText("198.51.100.0/25");

  // The second still holds the version read before: 412, the input kept beside the new value.
  await second.keyboard.press("Enter");
  const line = second.getByRole("form", { name: /^Appliquer maintenant : Valeur de / });
  await expect(
    line.getByRole("status").filter({ hasText: "L'objet a changé depuis votre lecture" }),
  ).toBeVisible();
  await expect(line).toContainText("maintenant 198.51.100.0/25, votre saisie 198.51.100.0/26");
  await expect(line.getByRole("textbox", { name: "Valeur" })).toHaveValue("198.51.100.0/26");
  await expectAccessible(second);
  // Applied again, on the new version.
  await line.getByRole("button", { name: "Appliquer sur la nouvelle version" }).click();
  await expect(
    second
      .getByRole("status")
      .filter({ hasText: /Appliqué : Valeur de/ })
      .first(),
  ).toBeVisible();
  await expect(cellOf(second, "value")).toContainText("198.51.100.0/26");
  await second.close();
});

/**
 * The topography of the domain `slug` (identifier `id`), its tree, the column « Nature »
 * shown: of priority 2 (021/A21), it hides in the width of the tree, the chooser shows it.
 */
async function topologyOf(tab: Page, slug: string, id: string): Promise<Locator> {
  await tab.goto(`/topology?domain=${id}`);
  const tree = tab.getByRole("treegrid", { name: `Arbre des préfixes du domaine ${slug}` });
  await expect(tree.getByText("198.51.100.0/24", { exact: true })).toBeVisible();
  // No node chosen yet: the chooser of the tree is the only one.
  const chooser = tab.getByRole("button", { name: "Colonnes" });
  if (await chooser.isVisible()) {
    await chooser.click();
    const nature = tab.getByRole("menuitemcheckbox", { name: "Nature" });
    if ((await nature.getAttribute("aria-checked")) !== "true") await nature.click();
    await tab.keyboard.press("Escape");
  }
  await expect(treeCell(tree, "detail")).toBeVisible();
  return tree;
}
/** The cell `column` of the network in the tree. */
const treeCell = (tree: Locator, column: string) =>
  tree.locator(`[role="gridcell"][data-column="${column}"]`).first();

test("a cell of the tree: a reader is told why, an author edits it to the draft, the node opens its record", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const domain = await newDomain(page, unique(browserName));

  // Bob reads: no editor, the foot of the tree says the right missing; « Ajouter » refused.
  const other = await browser.newContext({ ignoreHTTPSErrors: true });
  const reader = await other.newPage();
  await loginIn(reader, "bob");
  const id = await domainId(page, domain);
  const read = await topologyOf(reader, domain, id);
  await treeCell(read, "detail").click();
  await reader.keyboard.press("F2");
  await expect(read.getByRole("textbox")).toHaveCount(0);
  await expect(treeCell(read, "detail")).toHaveAttribute("aria-readonly", "true");
  await expect(reader.locator(".k-keyboard-hint")).toContainText("Lecture seule : il faut");
  await expect(reader.getByRole("button", { name: /core\.prefix\.edit/ })).toBeVisible();
  // The « + » of the node, reached by Tab from it, and « Ajouter » of the content: both
  // visible, refused, their permission said (REQ-021-37).
  await read.getByText("198.51.100.0/24", { exact: true }).click();
  await reader.keyboard.press("Tab");
  const plus = read.getByRole("button", { name: "Ajouter dans 198.51.100.0/24" });
  await expect(plus).toBeFocused();
  await expect(plus).toHaveAttribute("aria-disabled", "true");
  await expect(plus).toHaveAccessibleDescription(/core\.address\.edit/);
  const add = reader
    .getByRole("region", { name: "198.51.100.0/24" })
    .getByRole("button", { name: /^Ajouter dans 198\.51\.100\.0\/24/ });
  await expect(add).toHaveAttribute("aria-disabled", "true");
  await expect(add).toHaveAccessibleDescription(/core\.address\.edit/);
  await other.close();

  // Alice, at the keyboard: Enter opens the editor of the nature, its words stand for its key.
  const tree = await topologyOf(page, domain, id);
  await treeCell(tree, "detail").click();
  await page.keyboard.press("Enter");
  const editor = tree.getByRole("textbox", { name: /^Nature, usage — 198\.51\.100\.0\/24$/ });
  await expect(editor).toBeFocused();
  await page.keyboard.press("Control+a");
  await page.keyboard.type("Conteneur");
  await page.keyboard.press("Enter");
  await expect(treeCell(tree, "detail")).toHaveAttribute("data-state", "draft");
  await expect(treeCell(tree, "detail")).toContainText("Conteneur");
  await expect(page.getByRole("button", { name: /^Brouillon 1 opération$/ })).toBeVisible();
  await expectAccessible(page);

  // Enter on the node opens its record (021/A20), not its « + ».
  await tree.getByText("198.51.100.0/24", { exact: true }).click();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/prefixes\/[0-9a-f-]{36}$/);
});

test("two tabs, the tree and the content: Ctrl+Enter applies one value, the other gets 412 and keeps its own", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const domain = await newDomain(page, unique(browserName));
  const id = await domainId(page, domain);
  const added = await call(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end range",
    operations: [
      {
        op: "create_range",
        domain: { id },
        first: "198.51.100.10",
        last: "198.51.100.20",
        usage: "static",
      },
    ],
  });
  expect(added.status).toBe(201);
  const RANGE = "198.51.100.10 – 198.51.100.20";

  // The first tab: the range in the tree, under its network opened.
  const tree = await topologyOf(page, domain, id);
  await tree.getByText("198.51.100.0/24", { exact: true }).click();
  await page.keyboard.press("ArrowRight");
  const rangeRow = tree.locator("[role='row'][data-node]").filter({
    has: page.getByText(RANGE, { exact: true }),
  });
  const inTree = rangeRow.locator('[role="gridcell"][data-column="detail"]');
  await expect(inTree).toContainText("Statique");
  const network = new URL(page.url()).searchParams.get("node")!;

  // The second tab: the same range in the grid of the content of its network.
  const second = await page.context().newPage();
  await second.goto(`/topology?domain=${id}&node=${network}`);
  const content = second.getByRole("grid", { name: "Contenu de 198.51.100.0/24" });
  await expect(content.getByText(RANGE)).toBeVisible();
  const inContent = content.locator('[role="gridcell"][data-column="detail"]').first();
  if (!(await inContent.isVisible())) {
    await second
      .getByRole("region", { name: "198.51.100.0/24" })
      .getByRole("button", { name: "Colonnes" })
      .click();
    await second.getByRole("menuitemcheckbox", { name: "Nature, usage" }).click();
    await second.keyboard.press("Escape");
  }
  await expect(inContent).toContainText("Statique");

  /** In `tab`, from `cell`: the usage typed, then Ctrl+Enter, the motive asked. */
  async function ask(tab: Page, cell: Locator, value: string): Promise<void> {
    await cell.click();
    await tab.keyboard.press("Enter");
    await tab.keyboard.press("Control+a");
    await tab.keyboard.type(value);
    await tab.keyboard.press("Control+Enter");
    await expect(tab.getByRole("textbox", { name: "Motif" })).toBeFocused();
  }

  // The second tab asks first, on the version both read (« Réservée »); the first one then
  // applies another value (« DHCP »).
  await ask(second, inContent, "Réservée");
  await ask(page, inTree, "DHCP");
  const written = page.waitForResponse((r) => r.request().method() === "PATCH");
  await page.keyboard.press("Enter");
  // A modification applied at once: 200 (REQ-021-15), the changeset in its body.
  expect((await written).status()).toBe(200);
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: /Appliqué : Usage de/ })
      .first(),
  ).toBeVisible();
  await expect(inTree).toContainText("DHCP");

  // The second still holds the version its page served: 412; the input kept is its own
  // (« reserved »), beside the value read again (« dhcp »), never replaced by it.
  await second.keyboard.press("Enter");
  const line = second.getByRole("form", { name: /^Appliquer maintenant : Usage de / });
  await expect(
    line.getByRole("status").filter({ hasText: "L'objet a changé depuis votre lecture" }),
  ).toBeVisible();
  await expect(line).toContainText("maintenant dhcp, votre saisie reserved");
  await expect(line.getByRole("combobox", { name: /^Usage/ })).toContainText("Réservée");
  // The grid read again: the value now, the input waiting in the line.
  await expect(inContent).toContainText("DHCP");
  await expectAccessible(second);
  await line.getByRole("button", { name: "Appliquer sur la nouvelle version" }).click();
  await expect(
    second
      .getByRole("status")
      .filter({ hasText: /Appliqué : Usage de/ })
      .first(),
  ).toBeVisible();
  // The focus back to its cell, the value applied.
  await expect(inContent).toBeFocused();
  await expect(inContent).toContainText("Réservée");
  await second.close();
});

/** The identifier of a domain, by its slug. */
async function domainId(page: Page, slug: string): Promise<string> {
  const domains = (await call(page, "GET", `/v1/domains?slug=${slug}`)).body as {
    items: { id: string }[];
  };
  return domains.items[0]!.id;
}
/** The names of the fields of an edit panel, in order. */
const fieldsOf = (form: Locator) =>
  form
    .locator(".k-edit-panel__field")
    .evaluateAll((fields) =>
      fields.map((f) => f.querySelector("label")?.textContent?.trim() ?? f.textContent?.trim()),
    );

test("the same edit panel from a node of the topography and from the list of addresses", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const slug = await newDomain(page, unique(browserName));
  await page.goto(`/topology?domain=${await domainId(page, slug)}`);
  const tree = page.getByRole("treegrid", { name: `Arbre des préfixes du domaine ${slug}` });
  // The keyboard: `c` on a node adds in it (021/A20).
  await tree.getByText("198.51.100.0/24", { exact: true }).click();
  await page.keyboard.press("c");
  const fromNode = page.getByRole("form", { name: "Ajouter dans 198.51.100.0/24" });
  await expect(fromNode.getByRole("button", { name: "Adresse", pressed: true })).toBeVisible();
  await expect(fromNode.getByRole("combobox", { name: /^Domaine/ })).toContainText(slug);
  const nodeFields = await fieldsOf(fromNode);
  await expectAccessible(page);
  await fromNode.getByRole("button", { name: "Annuler" }).click();

  await page.goto("/addresses");
  await expect(page.getByRole("grid", { name: "Adresses" })).toBeVisible();
  await page.keyboard.press("c");
  const fromList = page.getByRole("form", { name: "Créer : Adresse" });
  await expect(fromList).toBeVisible();
  expect(await fieldsOf(fromList)).toEqual(nodeFields);
});

test("without approval.self, « Appliquer » stays visible, refused, its permission said at the keyboard", async ({
  page,
  login,
}) => {
  await login("carol");
  await page.goto("/addresses");
  await expect(page.getByRole("grid", { name: "Adresses" })).toBeVisible();
  await page.keyboard.press("c");
  const form = page.getByRole("form", { name: "Créer : Adresse" });
  await expect(form).toBeVisible();
  const chevron = form.getByRole("button", { name: "Choisir l'action" });
  for (let i = 0; i < 12 && !(await chevron.evaluate((el) => el === document.activeElement)); i++) {
    await page.keyboard.press("Tab");
  }
  await expect(chevron).toBeFocused();
  await page.keyboard.press("Enter");
  await page.keyboard.press("ArrowDown");
  const apply = page.getByRole("menuitem", { name: /^Appliquer/ });
  await expect(apply).toBeFocused();
  await expect(apply).toHaveAttribute("aria-disabled", "true");
  await expect(apply).toHaveAccessibleDescription(/Il faut core\.address\.approval\.self/);
  // Its tooltip beside it (decorative: the description says it).
  await expect(
    page.locator(".k-tooltip").filter({ hasText: "core.address.approval.self" }),
  ).toBeVisible();
});

test("a creation applied at once: ?approval=self, its violation on its field, then applied", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const slug = await newDomain(page, unique(browserName));
  await page.goto(`/prefixes?domain=${slug}`);
  await expect(cellOf(page, "value")).toContainText("198.51.100.0/24");
  await page.keyboard.press("c");
  const form = page.getByRole("form", { name: "Créer : Préfixe" });
  await form.getByRole("combobox", { name: /^Domaine/ }).click();
  await page.getByRole("option", { name: slug }).click();
  await form.getByRole("button", { name: "Choisir l'action" }).click();
  await page.getByRole("menuitem", { name: "Appliquer" }).click();
  await expect(form.getByRole("textbox", { name: "Motif" })).toHaveValue(/^Création : Préfixe$/);
  const value = form.getByRole("textbox", { name: "Valeur" });
  // Host bits set: refused (core.prefix.unaligned), on the field the operation stands for.
  await value.fill("203.0.113.1/24");
  const sent = page.waitForRequest(
    (r) => r.method() === "POST" && r.url().endsWith("/v1/changesets?approval=self"),
  );
  await page.keyboard.press("Control+Enter");
  await sent;
  await expect(value).toHaveAttribute("aria-invalid", "true");
  await expect(value).toBeFocused();
  await expect(value).toHaveAccessibleDescription(/core\.prefix\.unaligned/);
  // Under the fields (REQ-021-20): the code of each violation, of the problem and the
  // Request-Id, each a button that copies it.
  const refusal = form.getByRole("alert").filter({ hasText: "La modification n'est pas valide" });
  await expect(refusal.getByRole("button", { name: "Copier core.prefix.unaligned" })).toBeVisible();
  await expect(refusal.getByRole("button", { name: /^Copier urn:kartenn:problem:/ })).toBeVisible();
  await expect(refusal).toContainText("Request-Id");
  await value.fill("203.0.113.0/24");
  await page.keyboard.press("Control+Enter");
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: /Appliqué : Préfixe 203\.0\.113\.0\/24/ })
      .first(),
  ).toBeVisible();
  await expect(form).toHaveCount(0);
});
