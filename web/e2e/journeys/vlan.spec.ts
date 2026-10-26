import type { Page } from "@playwright/test";
import { DATA } from "../fixtures/data.ts";
import { expect, expectAccessible, test } from "../fixtures/index.ts";

// The plugin of the VLAN module (spec 008, REQ-008-24), built and installed as an operator
// would (serve.sh): a group created in its view, a VLAN created for a network by the action
// of its record and set on it, then seen in the VLAN tab and in the column of the prefix
// list; axe-core without violation at each step.

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

/** The network of this browser, created from the page if it does not exist yet. */
async function network(page: Page, value: string): Promise<string> {
  return page.evaluate(
    async ({ value, domain }) => {
      const find = async () => {
        const response = await fetch(`/v1/prefixes?value=${encodeURIComponent(value)}`);
        return ((await response.json()) as { items: { id: string }[] }).items[0]?.id;
      };
      const found = await find();
      if (found) return found;
      const domains = await fetch(`/v1/domains?slug=${domain}`);
      const id = ((await domains.json()) as { items: { id: string }[] }).items[0]!.id;
      const created = await fetch("/v1/changesets?approval=self", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          motive: "end-to-end journeys",
          operations: [{ op: "create_prefix", domain: { id }, prefix: value, nature: "network" }],
        }),
      });
      if (created.status !== 201) throw new Error(`create_prefix: ${created.status}`);
      return (await find())!;
    },
    { value, domain: DATA.domain },
  );
}

test("a group, then a VLAN created for a network, seen in its tab and the prefix list", async ({
  page,
  login,
}, info) => {
  await login("alice");
  // One network and one group per browser: the journeys of both browsers run side by side.
  const value = info.project.name === "firefox" ? "10.5.2.0/24" : "10.5.1.0/24";
  const prefix = await network(page, value);
  const slug = `e2e-${info.project.name}-${Date.now() % 1_000_000}`;

  // The view of the groups, from the navigation: a group of VIDs 100 to 199.
  await navigation(page).getByRole("link", { name: "Groupes de VLAN" }).click();
  await expect(page).toHaveURL(/\/x\/vlan\/groups$/);
  const view = page.locator("vlan-groups");
  await view.getByRole("textbox", { name: "Identifiant" }).fill(slug);
  await view
    .getByRole("combobox", { name: "Entité d'ancrage" })
    .selectOption({ label: DATA.entity });
  await view.getByRole("spinbutton", { name: "Premier VID" }).fill("100");
  await view.getByRole("spinbutton", { name: "Dernier VID" }).fill("199");
  await view.getByRole("textbox", { name: "Motif" }).fill("journey");
  await view.getByRole("button", { name: "Créer le groupe" }).click();
  await expect(view.getByRole("cell", { name: slug, exact: true })).toBeVisible();
  await view.getByRole("button", { name: `Occupation de ${slug}` }).click();
  await expect(view.getByText("0 VID utilisés, 0 réservés, 100 libres")).toBeVisible();
  await expectAccessible(page);

  // The action of the network: the VID proposed is the next free one of the group.
  await page.goto(`/prefixes/${prefix}`);
  await page.getByRole("button", { name: "Actions" }).click();
  await page.getByRole("menuitem", { name: "Créer un VLAN pour ce réseau" }).click();
  const dialog = page.getByRole("dialog", { name: "Créer un VLAN pour ce réseau" });
  await dialog.getByRole("combobox", { name: "Groupe" }).selectOption({ label: slug });
  await expect(dialog.getByRole("spinbutton", { name: "VID" })).toHaveValue("100");
  await dialog.getByRole("textbox", { name: "Nom" }).fill("serveurs");
  await dialog.getByRole("textbox", { name: "Motif" }).fill("journey");
  await dialog.getByRole("button", { name: "Créer et poser" }).click();
  await expect(dialog.getByText("VLAN 100 · serveurs créé et posé sur le réseau.")).toBeVisible();
  await expectAccessible(page);
  await dialog.getByRole("button", { name: "Fermer" }).click();
  await expect(dialog).toHaveCount(0);

  // The VLAN tab of the network.
  await page.reload();
  const tabs = page.getByRole("tablist", { name: "Sections de la fiche" });
  await tabs.getByRole("tab", { name: "VLAN" }).click();
  await expect(page.getByText("Ce réseau porte le VLAN 100 · serveurs.")).toBeVisible();
  await expectAccessible(page);

  // The column of the prefix list.
  await page.goto(`/prefixes?value=${encodeURIComponent(value)}`);
  const table = page.getByRole("table");
  await expect(table.getByRole("columnheader", { name: "VLAN" })).toBeVisible();
  await expect(table.getByText("VLAN 100 · serveurs")).toBeVisible();
  await expectAccessible(page);
});
