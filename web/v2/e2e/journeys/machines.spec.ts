import type { Page } from "@playwright/test";
import {
  apiCall,
  DATA,
  expect,
  expectAccessible,
  ownedDomain,
  test,
} from "../fixtures/journeys.ts";

// The plugin of the machines and interfaces module on the SDK 1.0 (spec 021, task 18a;
// spec 009, REQ-009-20), rewritten from web/e2e/journeys/machines.spec.ts (the plugin of the
// old shell), built and installed as an operator would (serve.sh). Read: a cluster, a machine
// member of it with an interface in access on the VLAN of the data set and an address
// attached, written by the API; seen in the views of the space « Machines » (list, record of
// the machine, of its interface, of the cluster; the search of the interfaces by MAC), the
// links followed without reloading the page; in the « Interfaces » tab of the address, of its
// network and of the VLAN; in the machine column of the addresses. Created in the draft: a
// machine from its list, an operation added and no changeset sent. Each browser writes in a
// domain of its own (RFC 5737, TEST-NET-3); axe-core without violation at each step.

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

const NETWORK = "203.0.113.128/26";
const ADDRESS = "203.0.113.130";
/** An address of the network attached to nothing, for the action. */
const FREE = "203.0.113.131";

/** The identifier of the first item of a list read from the API. */
async function first(page: Page, path: string): Promise<string> {
  const listed = await apiCall(page, "GET", path);
  expect(listed.status, JSON.stringify(listed.body)).toBe(200);
  return (listed.body as { items: { id: string }[] }).items[0]!.id;
}

interface Written {
  machine: string;
  cluster: string;
  address: string;
  /** The address FREE, attached to nothing. */
  free: string;
  network: string;
  vlan: string;
}

/**
 * The objects of the journey, applied at once: a domain of this run holding NETWORK, an
 * address of it, a cluster, a machine member of the cluster, its interface `eth0` in access
 * on the VLAN of the data set, the address attached to it.
 */
async function write(page: Page, tag: string, mac: string): Promise<Written> {
  const domain = await ownedDomain(page, tag, [NETWORK]);
  const entity = await first(page, `/v1/entities?slug=${DATA.entity}`);
  const group = await first(page, `/v1/vlan-groups?slug=${DATA.vlan.group}`);
  const vlan = await first(page, `/v1/vlans?group=${group}&vid=${DATA.vlan.vid}`);
  const domainId = await first(page, `/v1/domains?slug=${tag}`);
  const applied = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end machines",
    operations: [
      { op: "create_address", local: 0, domain: { id: domainId }, address: ADDRESS, usage: "host" },
      { op: "create_address", domain: { id: domainId }, address: FREE, usage: "host" },
      { op: "machines.create_cluster", local: 1, slug: tag, owner: { id: entity } },
      {
        op: "machines.create_machine",
        local: 2,
        name: tag,
        kind: "physical",
        owner: { id: entity },
        member_of: { local: 1 },
      },
      {
        op: "machines.create_interface",
        local: 3,
        machine: { local: 2 },
        name: "eth0",
        mac,
        vlan: { mode: "access", access: { id: vlan } },
      },
      {
        op: "machines.attach_address",
        interface: { local: 3 },
        address: { local: 0 },
        primary: true,
      },
    ],
  });
  expect(applied.status, JSON.stringify(applied.body)).toBe(201);
  return {
    machine: await first(page, `/v1/machines?name=${tag}`),
    cluster: await first(page, `/v1/clusters?slug=${tag}`),
    address: await first(page, `/v1/addresses?domain=${tag}&value=${ADDRESS}`),
    free: await first(page, `/v1/addresses?domain=${tag}&value=${FREE}`),
    network: domain.prefixes[0]!,
    vlan,
  };
}

test("the views of the machines, their links, the tabs and the column of the core", async ({
  page,
  login,
}, info) => {
  await login("alice");
  const firefox = info.project.name === "firefox";
  // `zz-` as the other journeys: the domain of the run sorts after the data set's, which the
  // topology opens on (journey `tree`).
  const tag = `zz-machines-${info.project.name}-${Date.now() % 1_000_000}`;
  // RFC 7042 documentation addresses, one per browser.
  const mac = firefox ? "00:00:5e:00:53:21" : "00:00:5e:00:53:20";
  const written = await write(page, tag, mac);

  // The list, from the navigation under the space « Machines ».
  await navigation(page).getByRole("button", { name: "Machines" }).click();
  await navigation(page).getByRole("link", { name: "Machines", exact: true }).click();
  await expect(page).toHaveURL(/\/x\/machines\/machines$/);
  const view = page.locator("machines-machines");
  await expect(view.getByRole("heading", { level: 1, name: "Machines" })).toBeVisible();
  const list = view.getByRole("table", { name: "Machines", exact: true });
  await expect(list.getByRole("row").filter({ hasText: tag })).toContainText("physique");
  await expectAccessible(page);

  // Its record, by its link: followed by the shell, the page not reloaded.
  await page.evaluate(() => ((window as unknown as { __kept: boolean }).__kept = true));
  await list.getByRole("link", { name: tag, exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/x/machines/machines/${written.machine}$`));
  const record = page.locator("machines-machines");
  // The focus on the heading of the new page, the link clicked being gone.
  await expect(record.getByRole("heading", { level: 1, name: tag })).toBeFocused();
  await expect(record.getByRole("definition").filter({ hasText: DATA.entity })).toBeVisible();
  await expect(record.getByRole("link", { name: tag, exact: true })).toHaveAttribute(
    "href",
    `/x/machines/clusters/${written.cluster}`,
  );
  const interfaces = record.getByRole("table", { name: "Interfaces", exact: true });
  const eth0 = interfaces.getByRole("row", { name: /eth0/ });
  await expect(eth0).toContainText(mac);
  await expect(eth0).toContainText(`accès ${DATA.vlan.vid}`);
  await expect(eth0).toContainText(ADDRESS);
  expect(await page.evaluate(() => (window as unknown as { __kept?: boolean }).__kept)).toBe(true);
  await expectAccessible(page);

  // The record of the interface, then of the cluster.
  await eth0.getByRole("link", { name: "eth0" }).click();
  await expect(page).toHaveURL(/\/x\/machines\/interfaces\/[0-9a-f-]+$/);
  const one = page.locator("machines-interfaces");
  await expect(one.getByRole("heading", { level: 1, name: "eth0" })).toBeVisible();
  await expect(
    one.getByRole("table", { name: "Adresses rattachées" }).getByRole("link", { name: ADDRESS }),
  ).toBeVisible();
  await expectAccessible(page);
  await one.getByRole("link", { name: tag, exact: true }).click();
  await record.getByRole("link", { name: tag, exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/x/machines/clusters/${written.cluster}$`));
  const cluster = page.locator("machines-clusters");
  await expect(cluster.getByRole("heading", { level: 1, name: tag })).toBeVisible();
  await expect(
    cluster
      .getByRole("table", { name: "Machines du cluster" })
      .getByRole("row", { name: /membre/ }),
  ).toContainText(tag);
  await expectAccessible(page);

  // The search of the interfaces by MAC (incident response), its criteria in the address.
  await navigation(page).getByRole("link", { name: "Interfaces", exact: true }).click();
  const search = page.locator("machines-interfaces");
  await search.getByRole("textbox", { name: "MAC" }).fill(mac);
  await search.getByRole("button", { name: "Rechercher" }).click();
  await expect(
    search.getByRole("status").filter({ hasText: "Interfaces trouvées : 1." }),
  ).toHaveCount(1);
  await expect(
    search.getByRole("table", { name: "Interfaces trouvées" }).getByRole("row", { name: /eth0/ }),
  ).toContainText(tag);
  await expect(page).toHaveURL(/[?&]mac=/);
  await expectAccessible(page);
  // A result opened, then back: the search as it was, its criteria in the field.
  const searched = page.url();
  await search
    .getByRole("table", { name: "Interfaces trouvées" })
    .getByRole("link", { name: "eth0" })
    .click();
  await expect(page).toHaveURL(/\/x\/machines\/interfaces\/[0-9a-f-]+$/);
  await page.goBack();
  await expect(page).toHaveURL(searched);
  await expect(search.getByRole("textbox", { name: "MAC" })).toHaveValue(mac);
  await expect(
    search.getByRole("table", { name: "Interfaces trouvées" }).getByRole("row", { name: /eth0/ }),
  ).toContainText(tag);

  // The « Interfaces » tab of the address, of its network and of the VLAN.
  const tab = async (path: string, expected: string[]) => {
    await page.goto(path);
    await page
      .getByRole("tablist", { name: "Sections de la fiche" })
      .getByRole("tab", { name: "Interfaces" })
      .click();
    // The VLAN of the data set carries the interfaces of both browsers: this one's by its tag.
    const row = page.locator("machines-tab").getByRole("row").filter({ hasText: tag });
    await expect(row).toContainText("eth0");
    for (const text of expected) await expect(row).toContainText(text);
    await expectAccessible(page);
  };
  await tab(`/addresses/${written.address}`, [mac]);
  await tab(`/prefixes/${written.network}`, [ADDRESS]);
  await tab(`/vlans/${written.vlan}`, [mac]);

  // The machine column of the list of addresses, the row given.
  await page.goto(`/addresses?value=${ADDRESS}`);
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("columnheader", { name: /Machine/ })).toBeAttached();
  await expect(grid.locator("machines-column").getByText(tag)).toBeVisible();
  await expectAccessible(page);

  // The action of an address not attached yet: attached to eth0 of the machine, in the draft
  // (no changeset sent), its dialog closed by its own button.
  const changesets: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().includes("/v1/changesets")) {
      changesets.push(request.url());
    }
  });
  await page.goto(`/addresses/${written.free}`);
  await page.getByRole("button", { name: "Actions" }).click();
  await page.getByRole("menuitem", { name: "Rattacher à une interface" }).click();
  const dialog = page.getByRole("dialog", { name: "Rattacher à une interface" });
  const attach = dialog.locator("machines-attach");
  await attach.getByRole("combobox", { name: /^Machine/ }).selectOption({ label: tag });
  await expect(attach.getByRole("combobox", { name: /^Interface/ })).toHaveValue(/.+/);
  await attach.getByRole("checkbox", { name: "Adresse principale de sa famille" }).check();
  await attach.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(
    attach.getByText(/^Opération ajoutée ; le brouillon en compte \d+\.$/),
  ).toBeVisible();
  await expectAccessible(page);
  expect(changesets).toEqual([]);
  await attach.getByRole("button", { name: "Fermer" }).click();
  await expect(dialog).toHaveCount(0);
});

test("a physical machine to a cluster: offered by the client, refused by the server at its place", async ({
  page,
  login,
}, info) => {
  await login("alice");
  const firefox = info.project.name === "firefox";
  const tag = `zz-host-${info.project.name}-${Date.now() % 1_000_000}`;
  const written = await write(page, tag, firefox ? "00:00:5e:00:53:23" : "00:00:5e:00:53:22");
  await page.goto(`/x/machines/machines/${written.machine}`);
  const record = page.locator("machines-machines");
  await expect(record.getByRole("heading", { level: 1, name: tag })).toBeVisible();
  const form = record.getByRole("region", { name: "Hébergement" });
  const select = form.getByRole("combobox", { name: /^Nouvel hébergeur/ });
  // The kinds of the pair are not in the contract: the cluster is offered.
  const cluster = select.locator(`option[value="cluster:${written.cluster}"]`);
  await expect(cluster).toBeEnabled();
  await select.selectOption(`cluster:${written.cluster}`);
  await select.press("Tab");
  await expect(form.getByRole("button", { name: "Déplacer au brouillon" })).toBeFocused();
  await page.keyboard.press("Enter");
  // The simulation of the draft refuses it with the rule of the module; the host read stays.
  const alert = form.getByRole("alert");
  await expect(alert).toContainText(`Déplacement vers ${tag} refusé par le serveur`);
  await expect(alert).toContainText("machines.host.invalid");
  await expect(form).toContainText("L'opération reste au brouillon");
  await expect(record.locator("dt:has-text('Hébergée par') + dd")).toHaveText("—aucun");
  await expectAccessible(page);
});

test("a machine created in the draft from its list, no changeset sent", async ({
  page,
  login,
}, info) => {
  await login("alice");
  const tag = `zz-draft-${info.project.name}-${Date.now() % 1_000_000}`;
  await page.goto("/x/machines/machines");
  const view = page.locator("machines-machines");
  const creation = view.getByRole("region", { name: "Créer une machine" });
  await expect(creation.getByRole("combobox", { name: /Propriétaire/ })).toHaveValue(/.+/);
  const changesets: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().includes("/v1/changesets")) {
      changesets.push(request.url());
    }
  });
  // Without its name, refused on the field, nothing sent.
  await creation.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(creation.getByRole("textbox", { name: /^Nom/ })).toHaveAttribute(
    "aria-invalid",
    "true",
  );
  await creation.getByRole("textbox", { name: /^Nom/ }).fill(tag);
  await creation.getByRole("combobox", { name: /Nature/ }).selectOption({ label: "virtuelle" });
  await creation
    .getByRole("combobox", { name: /Propriétaire/ })
    .selectOption({ label: DATA.entity });
  await creation.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(
    creation.getByText(/^Opération ajoutée ; le brouillon en compte \d+\.$/),
  ).toBeVisible();
  await expectAccessible(page);
  expect(changesets).toEqual([]);
  // In the draft of the server, not in the referential.
  const drafts = (await apiCall(page, "GET", "/v1/drafts")).body as {
    items: { id: string }[];
  };
  const operations = await Promise.all(
    drafts.items.map(async (d) => {
      const read = (await apiCall(page, "GET", `/v1/drafts/${d.id}`)).body as {
        operations: { op: string; name?: string }[];
      };
      return read.operations;
    }),
  );
  expect(operations.flat()).toContainEqual(
    expect.objectContaining({ op: "machines.create_machine", name: tag, kind: "virtual" }),
  );
  const listed = (await apiCall(page, "GET", `/v1/machines?name=${tag}`)).body as {
    items: unknown[];
  };
  expect(listed.items).toEqual([]);
});
