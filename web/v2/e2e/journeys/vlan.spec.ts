import type { Page } from "@playwright/test";
import { DATA, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// The VLAN, screens of the core in the new shell (spec 021, task 17; spec 026, REQ-026-6;
// REQ-008-12 to 21), rewritten from web/e2e/journeys/vlan.spec.ts (the plugin of the old
// shell): a group created from its list under « Réseau » by the edit panel, its record with
// its occupancy and its next free VID; a VLAN created from the group, its VID proposed as
// the next free one; set on a network by the API, then seen in the VLAN tab of the network
// (its removal to the draft, once), in the column of the list of prefixes and in the
// networks of its record; the list of the VLAN filtered by the URL. Each run writes in a domain of its own (RFC 5737, TEST-NET-3);
// axe-core without violation at each step.

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

/** A call of the API by the page itself, under its session (same origin: CSRF). */
async function call(page: Page, method: string, path: string, body?: unknown): Promise<unknown> {
  return page.evaluate(
    async ({ method, path, body }) => {
      const response = await fetch(path, {
        method,
        headers: { "Content-Type": "application/json", "Idempotency-Key": crypto.randomUUID() },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      });
      const text = await response.text();
      if (!response.ok) throw new Error(`${method} ${path}: ${response.status} ${text}`);
      return text ? JSON.parse(text) : null;
    },
    { method, path, body },
  );
}

const NETWORK = "203.0.113.64/26";

/** A domain of this run, owned by the entity of the data set, holding the network NETWORK. */
async function newDomain(page: Page, slug: string): Promise<void> {
  const declarations = (await call(page, "GET", "/v1/property-declarations")) as {
    items: { id: string; key: string }[];
  };
  const owner = declarations.items.find((d) => d.key === "core.owner")!.id;
  const entities = (await call(page, "GET", `/v1/entities?slug=${DATA.entity}`)) as {
    items: { id: string }[];
  };
  await call(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end VLAN",
    operations: [
      { op: "create_domain", local: 0, slug },
      {
        op: "assign_property",
        object: { local: 0 },
        kind: "domain",
        property: { id: owner },
        values: [entities.items[0]!.id],
      },
      { op: "create_prefix", domain: { local: 0 }, prefix: NETWORK, nature: "network" },
    ],
  });
}

/** Sets the VLAN 100 of `group` on the network `prefix` (property `core.vlan`), at once. */
async function setVlan(page: Page, prefix: string, group: string): Promise<void> {
  const declarations = (await call(page, "GET", "/v1/property-declarations")) as {
    items: { id: string; key: string }[];
  };
  const property = declarations.items.find((d) => d.key === "core.vlan")!.id;
  const groups = (await call(page, "GET", `/v1/vlan-groups?slug=${group}`)) as {
    items: { id: string }[];
  };
  const vlans = (await call(page, "GET", `/v1/vlans?group=${groups.items[0]!.id}&vid=100`)) as {
    items: { id: string }[];
  };
  await call(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end VLAN of a network",
    operations: [
      {
        op: "assign_property",
        kind: "prefix",
        object: { id: prefix },
        property: { id: property },
        values: [vlans.items[0]!.id],
      },
    ],
  });
}

const toast = (page: Page, text: RegExp) =>
  page.getByRole("status").filter({ hasText: text }).first();

test("a group, then a VLAN created for a network, seen in its tab, the prefix list and its record", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const run = `${browserName}-${Date.now().toString(36)}`;
  // After `production` in the order of the slugs (the topography opens on the first domain).
  const domain = `zz-vlan-${run}`;
  const group = `vlan-${run}`;
  await newDomain(page, domain);

  // The groups, under « Réseau »: a group of VIDs 100 to 199 anchored on the domain, created
  // by the edit panel (`c`) and applied at once.
  await navigation(page).getByRole("button", { name: "Réseau" }).click();
  await navigation(page).getByRole("link", { name: "Groupes de VLAN" }).click();
  await expect(page).toHaveURL(/\/vlan-groups$/);
  await expect(page.getByRole("grid", { name: "Groupes de VLAN" })).toBeVisible();
  await page.keyboard.press("c");
  const form = page.getByRole("form", { name: "Créer : Groupe de VLAN" });
  await form.getByRole("textbox", { name: "Clé" }).fill(group);
  await form.getByRole("combobox", { name: /^Ancre/ }).click();
  await page.getByRole("option", { name: new RegExp(`^${domain}`) }).click();
  await form.getByRole("textbox", { name: "Premier VID" }).fill("100");
  await form.getByRole("textbox", { name: "Dernier VID" }).fill("199");
  await form.getByRole("button", { name: "Choisir l'action" }).click();
  await page.getByRole("menuitem", { name: "Appliquer" }).click();
  await form.getByRole("textbox", { name: "Dernier VID" }).press("Control+Enter");
  await expect(toast(page, new RegExp(`Appliqué : Groupe de VLAN ${group}`))).toBeVisible();
  await expect(form).toHaveCount(0);

  // Its record: the anchor, the range, all of it free, the next free VID the first.
  await page.goto(`/vlan-groups?slug=${group}`);
  await page
    .getByRole("grid", { name: "Groupes de VLAN" })
    .getByRole("link", { name: group, exact: true })
    .click();
  await expect(
    page.getByRole("heading", { level: 1, name: `Groupe de VLAN ${group}` }),
  ).toBeVisible();
  const occupancy = page.getByRole("table", { name: `Occupation du groupe ${group}` });
  await expect(occupancy.getByRole("row", { name: /Libres/ })).toContainText("100");
  await expect(page.getByRole("definition").filter({ hasText: /^100$/ }).first()).toBeVisible();
  await page
    .getByRole("tablist", { name: "Sections de la fiche" })
    .getByRole("tab", { name: "VLAN" })
    .click();
  await expect(page.getByText("Aucun VLAN dans ce groupe")).toBeVisible();
  await expectAccessible(page);

  // « Créer un VLAN » of the group: the edit panel on the group, the VID proposed the next
  // free one, applied at once.
  await page.getByRole("button", { name: "Créer un VLAN" }).click();
  const create = page.getByRole("form", { name: "Créer : VLAN" });
  const vid = create.getByRole("textbox", { name: "VID" });
  await expect(vid).toHaveValue("100");
  await expect(vid).toHaveAccessibleDescription(`Prochain VID libre du groupe ${group}.`);
  await create.getByRole("textbox", { name: "Nom" }).fill("impression");
  await create.getByRole("button", { name: "Choisir l'action" }).click();
  await page.getByRole("menuitem", { name: "Appliquer" }).click();
  await expectAccessible(page);
  await create.getByRole("textbox", { name: "Nom" }).press("Control+Enter");
  await expect(toast(page, /Appliqué : VLAN 100 · impression/)).toBeVisible();
  await expect(create).toHaveCount(0);

  // The network: its VLAN tab says it carries none; set by the API (setting it from the
  // shell waits for an arbitration), it says the VLAN, and its removal goes to the draft
  // once.
  await page.goto(`/prefixes?domain=${domain}`);
  await page.getByRole("grid", { name: "Préfixes" }).getByRole("link", { name: NETWORK }).click();
  const tabs = page.getByRole("tablist", { name: "Sections de la fiche" });
  await tabs.getByRole("tab", { name: "VLAN" }).click();
  await expect(page.getByText("Ce réseau ne porte aucun VLAN")).toBeVisible();
  await setVlan(page, page.url().split("/").pop()!.split("?")[0]!, group);
  await page.reload();
  await expect(page.getByText("Ce réseau porte le VLAN 100 · impression.")).toBeVisible();
  await expectAccessible(page);
  const remove = page.getByRole("button", { name: "Retirer le VLAN" });
  await remove.click();
  await expect(toast(page, /Ajouté au brouillon : retrait du VLAN 100 · impression/)).toBeVisible();
  await expect(remove).toHaveAccessibleDescription(/déjà dans votre brouillon/);

  // The column of the list of prefixes: the VLAN of the network, a link to its record.
  await page.goto(`/prefixes?domain=${domain}`);
  const prefixes = page.getByRole("grid", { name: "Préfixes" });
  await expect(prefixes.getByRole("columnheader", { name: /^VLAN/ })).toBeAttached();
  await prefixes.getByRole("link", { name: "100 · impression" }).click();

  // The record of the VLAN: its group, the network that carries it.
  await expect(
    page.getByRole("heading", { level: 1, name: "VLAN 100 · impression" }),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: group, exact: true })).toBeVisible();
  await tabs.getByRole("tab", { name: "Réseaux" }).click();
  await expect(
    page
      .getByRole("grid", { name: "Réseaux qui portent le VLAN 100 · impression" })
      .getByRole("link", { name: NETWORK }),
  ).toBeVisible();
  await expectAccessible(page);

  // The list of the VLAN, filtered by the URL: the one of the data set, its status in words.
  await page.goto(`/vlans?vid=${DATA.vlan.vid}`);
  const vlans = page.getByRole("grid", { name: "VLAN" });
  // The link is the VID alone, the name in its own column.
  await expect(vlans.getByRole("link", { name: String(DATA.vlan.vid), exact: true })).toBeVisible();
  await expect(vlans.getByRole("gridcell", { name: DATA.vlan.name })).toBeVisible();
  await expect(vlans.getByText("Actif").first()).toBeVisible();
  await expect(vlans.getByRole("link", { name: DATA.vlan.group })).toBeVisible();
  await expectAccessible(page);
});
