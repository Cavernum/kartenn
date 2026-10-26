import type { Page } from "@playwright/test";
import { DATA } from "../fixtures/data.ts";
import { expect, expectAccessible, loginIn, test } from "../fixtures/index.ts";

// Changesets (spec 007, task 21; REQ-007-18): a proposal of Bob, validated by Alice, then
// undone; each step visible in the list.

/** Submits, from the page and its session, a changeset; its status and body. */
async function submit(
  page: Page,
  body: object,
): Promise<{ status: number; body: { changeset: { id: string }; kind?: string } }> {
  return page.evaluate(async (payload) => {
    const response = await fetch("/v1/changesets?approval=self", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    return {
      status: response.status,
      body: (await response.json()) as { changeset: { id: string }; kind?: string },
    };
  }, body);
}

async function domainId(page: Page): Promise<string> {
  return page.evaluate(async (slug) => {
    const response = await fetch(`/v1/domains?slug=${slug}`);
    return ((await response.json()) as { items: { id: string }[] }).items[0]!.id;
  }, DATA.domain);
}

test("a proposal of Bob, validated by Alice, then undone, each step in the list", async ({
  page,
  login,
  browser,
}, info) => {
  // One address per run and browser: the journeys of both browsers run side by side.
  const address = `10.1.0.${100 + ((Date.now() + info.parallelIndex * 37) % 150)}`;
  const motive = `proposal ${info.project.name} ${address} ${Date.now()}`;

  await login("bob");
  const proposed = await submit(page, {
    motive,
    operations: [
      { op: "create_address", domain: { id: await domainId(page) }, address, usage: "host" },
    ],
  });
  expect(proposed.status, JSON.stringify(proposed.body)).toBe(202);
  const id = proposed.body.changeset.id;

  // Bob sees his proposal pending, and may only withdraw it.
  await page
    .getByRole("navigation", { name: "Navigation principale" })
    .getByRole("link", { name: "Changesets" })
    .click();
  await page.getByRole("button", { name: "Mes changesets" }).click();
  const list = page.getByRole("table", { name: "Changesets" });
  const row = list.getByRole("row").filter({ hasText: motive });
  await expect(row).toContainText("En attente");
  await expect(row).toContainText("vous");
  await expectAccessible(page);
  await row.getByRole("link", { name: motive }).click();
  await expect(page.getByRole("status").filter({ hasText: "État : En attente" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Retirer" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Valider" })).toHaveCount(0);

  // Alice finds it among the proposals to validate, and validates it.
  const other = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  try {
    const alice = await other.newPage();
    await loginIn(alice, "alice");
    await alice.goto("/proposals");
    const proposals = alice.getByRole("table", { name: "Propositions à valider" });
    await expect(proposals.getByRole("row").filter({ hasText: motive })).toContainText(
      "core.address.edit",
    );
    await expectAccessible(alice);
    await proposals.getByRole("link", { name: motive }).click();
    await expect(alice).toHaveURL(new RegExp(`/changesets/${id}$`));
    await alice.getByRole("button", { name: "Valider" }).click();
    await expect(alice.getByRole("status").filter({ hasText: "État : Appliqué" })).toBeVisible();
    await expectAccessible(alice);

    // Then undoes it: a new changeset, applied, that closes the address.
    await alice.getByRole("button", { name: "Annuler ce changeset" }).click();
    const undo = alice.getByRole("status").filter({ hasText: "Changeset d'annulation soumis" });
    await expect(undo).toBeVisible();
    await undo.getByRole("link", { name: "Appliqué" }).click();
    await expect(alice.getByRole("heading", { level: 1 })).toContainText("Changeset");
    await expect(alice.getByRole("listitem").filter({ hasText: "close" }).first()).toBeVisible();
  } finally {
    await other.close();
  }

  // Back in Bob's list: his proposal applied.
  await page.goto("/changesets");
  await page.getByRole("button", { name: "Mes changesets" }).click();
  await expect(list.getByRole("row").filter({ hasText: motive })).toContainText("Appliqué");
});
