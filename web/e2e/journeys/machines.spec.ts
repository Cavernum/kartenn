import type { Page } from "@playwright/test";
import { appliedByAuthor, DATA } from "../fixtures/data.ts";
import { expect, expectAccessible, test } from "../fixtures/index.ts";

// The plugin of the machines and interfaces module (spec 009, REQ-009-20), built and
// installed as an operator would (serve.sh): a machine created in its view, an interface
// added, its VLAN set, an address attached by the action of its record, then seen in the
// « Machines » tab and in the column of the address list; axe-core without violation at
// each step.

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

/** Calls the API from the page (its session and its CSRF guard), failing on an error. */
async function call(page: Page, method: string, path: string, body?: unknown): Promise<unknown> {
  const text = body === undefined ? null : JSON.stringify(body);
  return page.evaluate(
    async ({ method, path, text }) => {
      const response = await fetch(path, {
        method,
        headers: text === null ? {} : { "content-type": "application/json" },
        body: text,
      });
      if (!response.ok) throw new Error(`${method} ${path}: ${response.status}`);
      return response.status === 204 ? null : ((await response.json()) as unknown);
    },
    { method, path: appliedByAuthor(method, path), text },
  );
}

/** The identifier of the first item of a list read from the API. */
async function first(page: Page, path: string): Promise<string> {
  const { items } = (await call(page, "GET", path)) as { items: { id: string }[] };
  return items[0]!.id;
}

/** A network of this browser with one address, and a VLAN of its own group. */
async function fixtures(page: Page, tag: string, network: string, address: string) {
  const domain = await first(page, `/v1/domains?slug=${DATA.domain}`);
  const entity = await first(page, `/v1/entities?slug=${DATA.entity}`);
  if (
    !((await call(page, "GET", `/v1/addresses?value=${address}`)) as { items: [] }).items.length
  ) {
    await call(page, "POST", "/v1/changesets", {
      motive: "end-to-end journeys",
      operations: [
        { op: "create_prefix", domain: { id: domain }, prefix: network, nature: "network" },
        { op: "create_address", domain: { id: domain }, address, usage: "host" },
      ],
    });
  }
  await call(page, "POST", "/v1/vlan-groups", {
    motive: "end-to-end journeys",
    slug: tag,
    anchor: { entity: { id: entity } },
    vid_min: 300,
    vid_max: 399,
  });
  const group = await first(page, `/v1/vlan-groups?slug=${tag}`);
  await call(page, "POST", "/v1/vlans", {
    motive: "end-to-end journeys",
    group: { id: group },
    vid: 310,
    name: "hotes",
  });
  return first(page, `/v1/addresses?value=${address}`);
}

test("a machine, its interface and VLAN, an address attached, seen in its tab and list", async ({
  page,
  login,
}, info) => {
  await login("alice");
  // One network, group and machine per browser: the journeys of both run side by side.
  const firefox = info.project.name === "firefox";
  const network = firefox ? "10.6.2.0/24" : "10.6.1.0/24";
  const value = firefox ? "10.6.2.10" : "10.6.1.10";
  const tag = `e2e-${info.project.name}-${Date.now() % 1_000_000}`;
  const address = await fixtures(page, tag, network, value);

  // The view of the machines, from the navigation: a machine, then its interface.
  await navigation(page).getByRole("link", { name: "Machines" }).click();
  await expect(page).toHaveURL(/\/x\/machines\/machines$/);
  const view = page.locator("machines-machines");
  await view.getByRole("textbox", { name: "Nom" }).fill(tag);
  await view.getByRole("combobox", { name: "Nature" }).selectOption({ label: "physique" });
  await view
    .getByRole("combobox", { name: "Entité propriétaire" })
    .selectOption({ label: DATA.entity });
  await view.getByRole("textbox", { name: "Motif" }).fill("journey");
  await view.getByRole("button", { name: "Créer la machine" }).click();
  await view.getByRole("button", { name: `Interfaces de ${tag}` }).click();
  await expect(view.getByRole("heading", { name: `Machine ${tag}` })).toBeVisible();
  await expectAccessible(page);
  // The innermost section of a form: the sections that hold it come first.
  const adding = view
    .locator("section", { has: page.getByRole("heading", { name: "Ajouter une interface" }) })
    .last();
  await adding.getByRole("textbox", { name: "Nom" }).fill("eth0");
  await adding.getByRole("textbox", { name: "MAC" }).fill("02:00:00:00:00:10");
  await adding.getByRole("textbox", { name: "Motif" }).fill("journey");
  await adding.getByRole("button", { name: "Ajouter l'interface" }).click();
  await expect(view.getByRole("cell", { name: "eth0", exact: true })).toBeVisible();

  // Its VLAN: access on the VLAN 310 of the group.
  const mode = view
    .locator("section", { has: page.getByRole("heading", { name: "VLAN de eth0" }) })
    .last();
  await mode.getByRole("combobox", { name: "Mode 802.1Q" }).selectOption("access");
  await mode.getByRole("combobox", { name: "VLAN d'accès" }).selectOption({ label: "310 · hotes" });
  await mode.getByRole("textbox", { name: "Motif" }).fill("journey");
  await mode.getByRole("button", { name: "Poser le mode" }).click();
  await expect(view.getByRole("cell", { name: "accès 310" })).toBeVisible();
  await expectAccessible(page);

  // The action of the address: attached to eth0 of the machine.
  await page.goto(`/addresses/${address}`);
  await page.getByRole("button", { name: "Actions" }).click();
  await page.getByRole("menuitem", { name: "Rattacher à une interface" }).click();
  const dialog = page.getByRole("dialog", { name: "Rattacher à une interface" });
  await dialog.getByRole("combobox", { name: "Machine" }).selectOption({ label: tag });
  await expect(dialog.getByRole("combobox", { name: "Interface" })).toHaveValue(/.+/);
  await dialog.getByRole("textbox", { name: "Motif" }).fill("journey");
  await dialog.getByRole("button", { name: "Rattacher" }).click();
  await expect(dialog.getByText(`Rattachée à eth0 de ${tag}.`)).toBeVisible();
  await expectAccessible(page);
  await dialog.getByRole("button", { name: "Fermer" }).click();
  await expect(dialog).toHaveCount(0);

  // The « Machines » tab of the address.
  await page.reload();
  const tabs = page.getByRole("tablist", { name: "Sections de la fiche" });
  await tabs.getByRole("tab", { name: "Machines" }).click();
  await expect(page.getByText(`${tag} par eth0`)).toBeVisible();
  await expectAccessible(page);

  // The column of the address list.
  await page.goto(`/addresses?value=${encodeURIComponent(value)}`);
  const table = page.getByRole("table");
  await expect(table.getByRole("columnheader", { name: "Machine" })).toBeVisible();
  await expect(table.getByText(tag, { exact: true })).toBeVisible();
  await expectAccessible(page);
});
