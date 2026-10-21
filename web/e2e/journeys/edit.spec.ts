import type { Page } from "@playwright/test";
import { expect, expectAccessible, test } from "../fixtures/index.ts";
import { DATA } from "../fixtures/data.ts";

// Quick edit (spec 007, task 19; REQ-007-25, 20): creation, modification and closure by
// the route of the resource, a motive and the If-Match of the read; 412, 202, 422 and
// 403 as the design « Écriture » answers them.

/** A call of the API by the page itself, under its session. */
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

/** The identifier of the only object a list filtered by `query` returns. */
async function idOf(page: Page, path: string): Promise<string> {
  const answer = (await call(page, "GET", path)).body as { items: { id: string }[] };
  return answer.items[0]!.id;
}

/** A new domain owned by the entity of the data set, unique to this run and browser. */
async function newDomain(page: Page, slug: string): Promise<string> {
  const declarations = (await call(page, "GET", "/v1/property-declarations")).body as {
    items: { id: string; key: string }[];
  };
  const owner = declarations.items.find((d) => d.key === "core.owner")!.id;
  const entity = await idOf(page, `/v1/entities?slug=${DATA.entity}`);
  const answer = await call(page, "POST", "/v1/changesets", {
    motive: "end-to-end quick edit",
    operations: [
      { op: "create_domain", local: 0, slug },
      {
        op: "assign_property",
        object: { local: 0 },
        kind: "domain",
        property: { id: owner },
        values: [entity],
      },
    ],
  });
  expect(answer.status).toBe(201);
  return idOf(page, `/v1/domains?slug=${slug}`);
}

/** A slug unique to this run and this browser. */
function unique(browser: string): string {
  return `e2e-${browser}-${Date.now().toString(36)}`;
}

test("a creation, then two tabs on the same prefix: 412, the input kept after reading again", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const domain = await newDomain(page, unique(browserName));

  // Creation by the collection: 201 leads to the record of the new prefix.
  await page.goto(`/prefixes/new?domain=${domain}`);
  await page.getByLabel("Valeur").fill("172.16.0.0/24");
  await page.getByLabel("Motif").fill("bout en bout : création");
  await expectAccessible(page);
  await page.getByRole("button", { name: "Créer" }).click();
  await expect(page).toHaveURL(/\/prefixes\/[0-9a-f-]{36}$/);
  const record = page.url();

  // Two tabs read the same version.
  const other = await page.context().newPage();
  await other.goto(record);
  for (const tab of [page, other]) {
    await tab.getByRole("button", { name: "Modifier" }).click();
  }

  // The first one saves.
  await page.getByLabel("Nature").selectOption("container");
  await page.getByLabel("Motif").fill("bout en bout : premier onglet");
  await page.getByRole("button", { name: "Enregistrer" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Changement appliqué." })).toBeVisible();

  // The second one is refused (412), sees both versions, resumes with its input.
  await other.getByLabel("Valeur").fill("172.16.0.0/25");
  await other.getByLabel("Motif").fill("bout en bout : second onglet");
  await other.getByRole("button", { name: "Enregistrer" }).click();
  const dialog = other.getByRole("dialog", { name: "L'objet a changé" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("row", { name: /^Nature/ })).toContainText("container");
  await expect(dialog.getByRole("row", { name: /^Valeur / })).toContainText("172.16.0.0/25");
  await expectAccessible(other);
  await dialog.getByRole("button", { name: "Reprendre sur la nouvelle version" }).click();
  await expect(other.getByLabel("Valeur")).toHaveValue("172.16.0.0/25");
  await expect(other.getByLabel("Nature")).toHaveValue("container");
  await other.getByRole("button", { name: "Enregistrer" }).click();
  await expect(other.getByRole("status").filter({ hasText: "Changement appliqué." })).toBeVisible();
  await expect(other.locator(".k-fields")).toContainText("172.16.0.0/25");
  await expect(other.locator(".k-fields")).toContainText("container");

  // Closure, with its motive.
  await other.getByRole("button", { name: "Clore" }).click();
  await other.getByLabel("Motif").fill("bout en bout : clôture");
  await other.getByRole("button", { name: "Clore" }).click();
  await expect(other.getByRole("status").filter({ hasText: "Changement appliqué." })).toBeVisible();
  await other.close();
});

test("under an approval policy, a modification waits (202) and links its changeset", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const slug = unique(browserName);
  const domain = await newDomain(page, slug);
  const policy = await call(page, "POST", "/v1/changesets", {
    motive: "end-to-end approval",
    operations: [{ op: "set_approval_policy", name: slug, domains: [domain], enabled: true }],
  });
  expect(policy.status).toBe(201);

  await page.goto(`/domains/${domain}`);
  await page.getByRole("button", { name: "Modifier" }).click();
  await page.getByLabel("Nom court").fill(`${slug}-renomme`);
  await page.getByLabel("Motif").fill("bout en bout : approbation");
  await page.getByRole("button", { name: "Enregistrer" }).click();
  const pending = page.getByRole("status").filter({ hasText: "en attente d'approbation" });
  await expect(pending).toBeVisible();
  await expect(pending.getByRole("link", { name: "Voir le changeset" })).toHaveAttribute(
    "href",
    /^\/changesets\/[0-9a-f-]{36}$/,
  );
  await expectAccessible(page);
});

test("a violation is placed on its field", async ({ page, login }) => {
  await login("alice");
  const entity = await idOf(page, `/v1/entities?slug=${DATA.entity}`);
  await page.goto(`/entities/${entity}`);
  await page.getByRole("button", { name: "Modifier" }).click();
  await page.getByLabel("Nom court").fill("Mauvais slug");
  await page.getByLabel("Motif").fill("bout en bout : violation");
  await page.getByRole("button", { name: "Enregistrer" }).click();
  const field = page.getByLabel("Nom court");
  await expect(field).toHaveAttribute("aria-invalid", "true");
  await expect(field).toHaveAccessibleDescription(/\S/);
  await expect(field).toHaveValue("Mauvais slug");
  await expectAccessible(page);
});

test("a refusal of the server (403) keeps the input", async ({ page, login }) => {
  // Carol may edit one entity only: the interface shows her the form, the server refuses.
  await login("carol");
  const entity = await idOf(page, `/v1/entities?slug=${DATA.entity}`);
  await page.goto(`/entities/${entity}`);
  await page.getByRole("button", { name: "Modifier" }).click();
  await page.getByLabel("Nom court").fill("exemple-carol");
  await page.getByLabel("Motif").fill("bout en bout : refus");
  await page.getByRole("button", { name: "Enregistrer" }).click();
  await expect(page.getByRole("alert")).toContainText("Vous n'avez pas le droit de faire cela.");
  await expect(page.getByLabel("Nom court")).toHaveValue("exemple-carol");
  await expect(page.getByLabel("Motif")).toHaveValue("bout en bout : refus");
  await expectAccessible(page);
});
