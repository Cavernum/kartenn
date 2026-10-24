import type { Page } from "@playwright/test";
import { expect, expectAccessible, loginIn, test } from "../fixtures/index.ts";
import { appliedByAuthor, DATA } from "../fixtures/data.ts";
import { ISSUER } from "../fixtures/env.ts";

// The draft (spec 007, task 20; REQ-007-26, 27): operations gathered in the browser,
// simulated after each change, submitted as one changeset; a lost answer retried without a
// second changeset; kept across an expired session, erased at logout and for another user.

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
    { method, path: appliedByAuthor(method, path), body },
  );
}

async function idOf(page: Page, path: string): Promise<string> {
  const answer = (await call(page, "GET", path)).body as { items: { id: string }[] };
  return answer.items[0]!.id;
}

/** A new domain owned by the entity of the data set, unique to this run and browser. */
async function newDomain(page: Page, browser: string): Promise<string> {
  const slug = `e2e-draft-${browser}-${Date.now().toString(36)}`;
  const declarations = (await call(page, "GET", "/v1/property-declarations")).body as {
    items: { id: string; key: string }[];
  };
  const owner = declarations.items.find((d) => d.key === "core.owner")!.id;
  const entity = await idOf(page, `/v1/entities?slug=${DATA.entity}`);
  const answer = await call(page, "POST", "/v1/changesets", {
    motive: "end-to-end draft",
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

/** The changesets whose motive is `motive`. */
async function changesetsWith(page: Page, motive: string): Promise<number> {
  const answer = (await call(page, "GET", "/v1/changesets?page_size=200")).body as {
    items: { motive: string }[];
  };
  return answer.items.filter((item) => item.motive === motive).length;
}

async function openDraft(page: Page, count: number): Promise<void> {
  await page.getByRole("button", { name: `Brouillon (${count})` }).click();
  await expect(page.getByRole("heading", { level: 2, name: "Brouillon" })).toBeVisible();
}

async function addOperation(page: Page, operation: object): Promise<void> {
  await page.getByLabel("Nouvelle opération (JSON du contrat)").fill(JSON.stringify(operation));
  await page.getByRole("button", { name: "Ajouter au brouillon" }).click();
}

const draftOf = (page: Page) => page.locator("aside.k-draft");

/** The operations of every draft stored in IndexedDB. */
function storedOperations(page: Page): Promise<number> {
  return page.evaluate(
    () =>
      new Promise<number>((resolve) => {
        const request = indexedDB.open("kartenn-drafts");
        request.onsuccess = () => {
          const db = request.result;
          if (!db.objectStoreNames.contains("drafts")) {
            db.close();
            return resolve(0);
          }
          const all = db.transaction("drafts").objectStore("drafts").getAll();
          all.onsuccess = () => {
            db.close();
            resolve(
              (all.result as { operations: unknown[] }[]).reduce(
                (sum, draft) => sum + draft.operations.length,
                0,
              ),
            );
          };
        };
        request.onerror = () => resolve(-1);
      }),
  );
}

test("three operations, one invalid: the simulation shows it, corrected, one changeset", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const domain = { id: await newDomain(page, browserName) };
  const motive = `bout en bout : brouillon ${browserName} ${Date.now()}`;
  await openDraft(page, 0);
  await addOperation(page, {
    op: "create_prefix",
    domain,
    prefix: "192.0.2.0/24",
    nature: "network",
  });
  await addOperation(page, { op: "create_address", domain, address: "192.0.2.10", usage: "host" });
  // The third one duplicates the second address.
  await addOperation(page, { op: "create_address", domain, address: "192.0.2.10", usage: "host" });
  await expect(page.getByRole("button", { name: "Brouillon (3)" })).toBeVisible();
  await page.getByLabel("Motif").fill(motive);
  const draft = draftOf(page);
  await expect(draft.getByText(/Simulation invalide/)).toBeVisible();
  const third = draft.getByRole("listitem").filter({ hasText: "Opération 3" });
  await expect(third.locator(".k-error").first()).toBeVisible();
  await expect(draft.getByRole("button", { name: "Soumettre le brouillon" })).toBeDisabled();
  await expectAccessible(page);

  // Corrected: another address; the simulation becomes valid.
  await third
    .getByLabel("Opération 3")
    .fill(JSON.stringify({ op: "create_address", domain, address: "192.0.2.11", usage: "host" }));
  await third.getByLabel("Opération 3").blur();
  await expect(draft.getByText(/Simulation valide/)).toBeVisible();
  await draft.getByRole("button", { name: "Soumettre le brouillon" }).click();
  await expect(draft.getByText("Changeset appliqué.")).toBeVisible();
  await expect(draft.getByRole("link", { name: "Voir le changeset" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Brouillon (0)" })).toBeVisible();
  expect(await changesetsWith(page, motive)).toBe(1);
});

test("an answer lost at the submission, then « Réessayer »: one changeset", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const domain = { id: await newDomain(page, browserName) };
  const motive = `bout en bout : réponse perdue ${browserName} ${Date.now()}`;
  await openDraft(page, 0);
  await addOperation(page, {
    op: "create_prefix",
    domain,
    prefix: "198.51.100.0/24",
    nature: "network",
  });
  await page.getByLabel("Motif").fill(motive);
  const draft = draftOf(page);
  await expect(draft.getByText(/Simulation valide/)).toBeVisible();
  // The server applies the changeset, the answer never reaches the page.
  let lost = 0;
  await page.route("**/v1/changesets?apply=own", async (route) => {
    if (route.request().method() !== "POST" || lost > 0) return route.fallback();
    lost += 1;
    await route.fetch();
    await route.abort("connectionreset");
  });
  await draft.getByRole("button", { name: "Soumettre le brouillon" }).click();
  await expect(draft.getByText(/Réseau indisponible/)).toBeVisible();
  await page.unroute("**/v1/changesets?apply=own");
  await draft.getByRole("button", { name: "Réessayer" }).click();
  await expect(draft.getByText("Changeset appliqué.")).toBeVisible();
  expect(lost).toBe(1);
  expect(await changesetsWith(page, motive)).toBe(1);
});

test("an expired session keeps the draft; another user does not see it; logout erases it", async ({
  page,
  login,
  context,
}) => {
  await login("alice");
  await openDraft(page, 0);
  await addOperation(page, { op: "create_domain", local: 0, slug: "brouillon-garde" });
  await expect(page.getByRole("button", { name: "Brouillon (1)" })).toBeVisible();
  // Written to IndexedDB, as a page left right after would need it.
  await expect.poll(() => storedOperations(page)).toBe(1);

  // The session expires; the same user logs in again: the draft is there.
  await context.clearCookies();
  await loginIn(page, "alice");
  await expect(page.getByRole("button", { name: "Brouillon (1)" })).toBeVisible();

  // Another user in the same browser: the draft of Alice is erased, unread.
  await context.clearCookies();
  await loginIn(page, "bob");
  await expect(page.getByRole("button", { name: "Brouillon (0)" })).toBeVisible();
  await context.clearCookies();
  await loginIn(page, "alice");
  await expect(page.getByRole("button", { name: "Brouillon (0)" })).toBeVisible();

  // Alice drafts again, then logs out: nothing of it remains in IndexedDB.
  await openDraft(page, 0);
  await addOperation(page, { op: "create_domain", local: 0, slug: "brouillon-efface" });
  await expect(page.getByRole("button", { name: "Brouillon (1)" })).toBeVisible();
  await expect.poll(() => storedOperations(page)).toBe(1);
  await page.getByRole("button", { name: /menu de l'utilisateur/ }).click();
  await page.getByRole("menuitem", { name: "Se déconnecter" }).click();
  await page.waitForURL(`${ISSUER}/end-session?**`);
  await loginIn(page, "alice");
  await expect(page.getByRole("button", { name: "Brouillon (0)" })).toBeVisible();
});
