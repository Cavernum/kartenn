import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { DATA } from "../fixtures/data.ts";
import { expect, expectAccessible, test } from "../fixtures/index.ts";

// Administration (spec 007, task 22; REQ-007-19): a service account and its secret shown
// once, the audit search that finds its creation, the export saved as NDJSON; an import
// followed, resumed and cancelled.

test("a service account, its secret shown once, its creation found in the audit and exported", async ({
  page,
  login,
}, info) => {
  const name = `svc-${info.project.name}-${Date.now()}`;
  await login("alice");
  await page
    .getByRole("navigation", { name: "Navigation principale" })
    .getByRole("link", { name: "Administration" })
    .click();
  await page.getByRole("link", { name: "Comptes de service" }).click();
  await expectAccessible(page);

  const form = page.getByRole("form", { name: "Créer un compte de service" });
  await form.getByLabel("Nom").fill(name);
  await form.getByLabel("Motif").fill("end-to-end service account");
  await form.getByRole("button", { name: "Créer" }).click();

  // Shown once, in a dialog.
  const dialog = page.getByRole("dialog", { name: "Nouveau secret" });
  await expect(dialog).toBeVisible();
  const secret = (await dialog.getByLabel("Valeur du secret").textContent())?.trim() ?? "";
  expect(secret.length).toBeGreaterThan(20);
  await expectAccessible(page);
  await dialog.getByRole("button", { name: "J'ai copié le secret" }).click();
  await expect(dialog).toHaveCount(0);

  // Never again: neither on the list nor on the account.
  await page.getByRole("link", { name }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: `Compte de service ${name}` }),
  ).toBeVisible();
  await expect(page.getByRole("table", { name: "Secrets" }).getByRole("row")).toHaveCount(2);
  expect(await page.content()).not.toContain(secret);
  await page.reload();
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  expect(await page.content()).not.toContain(secret);

  // The audit finds the creation (written by the relay: searched again until it is there).
  await page.goto("/admin/audit");
  const search = page.getByRole("search", { name: "Recherche dans l'audit" });
  await search.getByLabel("Événement").fill("account.service.changed");
  const events = page.getByRole("table", { name: "Audit" });
  await expect(async () => {
    await search.getByRole("button", { name: "Filtrer" }).click();
    await expect(events.getByRole("row").nth(1)).toContainText("account.service.changed", {
      timeout: 1_000,
    });
  }).toPass({ timeout: 20_000 });
  await expectAccessible(page);

  // The export of the same search, saved as a file.
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    search.getByRole("button", { name: "Exporter (NDJSON)" }).click(),
  ]);
  expect(download.suggestedFilename()).toMatch(/^kartenn-audit-.*\.ndjson$/);
  const lines = readFileSync(await download.path(), "utf8")
    .trim()
    .split("\n");
  expect(lines.length).toBeGreaterThan(0);
  for (const line of lines) expect(JSON.parse(line).name).toBe("account.service.changed");
  await expect(page.getByRole("status").filter({ hasText: "Export enregistré" })).toBeVisible();
});

/** Opens, through the API from the page, an import that pauses on its first batch. */
async function pausingImport(page: Page, source: string): Promise<string> {
  return page.evaluate(
    async ({ source, domain, address }) => {
      const json = { "content-type": "application/json" };
      const domains = await (await fetch(`/v1/domains?slug=${domain}`)).json();
      const opened = await fetch("/v1/imports", {
        method: "POST",
        headers: json,
        body: JSON.stringify({ source }),
      });
      const location = opened.headers.get("location")!;
      // An address that already exists: the batch is refused, the import pauses.
      await fetch(`${location}/chunks/0`, {
        method: "PUT",
        headers: json,
        body: JSON.stringify({
          elements: [
            {
              external_id: "e2e-duplicate",
              operation: {
                op: "create_address",
                domain: { id: domains.items[0].id },
                address,
                usage: "host",
              },
            },
          ],
        }),
      });
      await fetch(`${location}/start`, { method: "POST" });
      return location.split("/").pop()!;
    },
    { source, domain: DATA.domain, address: DATA.addresses[0]! },
  );
}

test("an import followed, resumed, then cancelled", async ({ page, login }, info) => {
  const source = `e2e-${info.project.name}-${Date.now()}`;
  await login("alice");
  const id = await pausingImport(page, source);

  await page
    .getByRole("navigation", { name: "Navigation principale" })
    .getByRole("link", { name: "Imports" })
    .click();
  await page.getByRole("link", { name: source }).click();
  await expect(page).toHaveURL(new RegExp(`/imports/${id}$`));
  const state = page.getByRole("status").filter({ hasText: "État" });
  await expect(state).toHaveText("État : En pause", { timeout: 20_000 });
  await expect(page.getByRole("heading", { name: "Rapport" })).toBeVisible();
  await expectAccessible(page);

  const [resumed] = await Promise.all([
    page.waitForResponse((r) => r.url().endsWith(`/imports/${id}/resume`)),
    page.getByRole("button", { name: "Reprendre" }).click(),
  ]);
  expect(resumed.status()).toBe(202);
  // Its batch refused again, it pauses again.
  await expect(state).toHaveText("État : En pause", { timeout: 20_000 });

  await page.getByRole("button", { name: "Annuler l'import" }).click();
  await expect(state).toHaveText("État : Annulé");
  await expect(page.getByRole("button", { name: "Reprendre" })).toHaveCount(0);
});

test("roles, bindings, issuers and identities, listed and accessible", async ({ page, login }) => {
  await login("alice");
  await page.goto("/admin/roles");
  await expect(page.getByRole("table", { name: "Rôles" })).toContainText("contributor");
  await expectAccessible(page);
  await page.goto("/admin/bindings");
  await expect(page.getByRole("table", { name: "Liaisons" })).toContainText("kartenn-admins");
  await expectAccessible(page);
  await page.goto("/admin/issuers");
  await expect(page.getByRole("table", { name: "Émetteurs" }).getByRole("row")).not.toHaveCount(1);
  await expectAccessible(page);
  await page.goto("/admin/identities");
  const identities = page.getByRole("table", { name: "Identités" });
  await identities.getByRole("link", { name: "Alice Exemple" }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "Identité Alice Exemple" }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Pseudonymiser" })).toBeDisabled();
  await expectAccessible(page);
});

test("a reader reaches no section of the administration", async ({ page, login }) => {
  await login("bob");
  await page.goto("/admin");
  await expect(page.getByRole("main").getByRole("link")).toHaveText(["Imports"]);
  await page.goto("/admin/roles");
  await expect(page.getByRole("alert")).toContainText("droit");
});
