import type { Page } from "@playwright/test";
import { DATA, expect, expectAccessible, ownedDomain, test } from "../fixtures/journeys.ts";

// The VLAN, screens of the core in the new shell (spec 021, tasks 17 and 17b; spec 026,
// REQ-026-6; REQ-008-12 to 21), rewritten from web/e2e/journeys/vlan.spec.ts (the plugin of
// the old shell): a group created from its list under « Réseau » by the edit panel, its
// record with its occupancy and its next free VID; a VLAN created from the group, its VID
// proposed as the next free one, a VID taken or out of the range refused on its field with its
// code; set on a network from the VLAN tab of the network (an
// existing VLAN, to the draft, applied), then seen there (its removal to the draft, once),
// in the column of the list of prefixes and in the networks of its record; the list of the
// VLAN filtered by the URL. « Créer un VLAN pour ce réseau » (021/A13): the creation and the
// setting of `core.vlan` by a local name, one changeset — to the draft then applied, or
// applied at once, a refusal of the setting said on the field of the network. Each run
// writes in a domain of its own (RFC 5737, TEST-NET-3); axe-core without violation at each
// step.

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
const SECOND = "203.0.113.128/26";

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

const toast = (page: Page, text: RegExp) =>
  page.getByRole("status").filter({ hasText: text }).first();
const counter = (page: Page) => page.getByRole("button", { name: /^Brouillon \d+ opération/ });
const panel = (page: Page) => page.getByRole("complementary", { name: "Panneau droit" });
const escaped = (text: string) => text.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");

/** The active draft applied from its panel, Ctrl+Entrée with the summary prefilled. */
async function applyDraft(page: Page, operations: number): Promise<void> {
  await expect(counter(page)).toHaveAccessibleName(
    new RegExp(`^Brouillon ${operations} opération`),
  );
  // The counter opens the panel on the draft, or closes it whatever its tab: the panel still
  // open (the edit panel just closed in its Détail tab), its tab Brouillon.
  const draftTab = panel(page).getByRole("tab", { name: /Brouillon/ });
  if (await draftTab.isVisible()) await draftTab.click();
  else await counter(page).click();
  await expect(draftTab).toHaveAttribute("aria-selected", "true");
  await expect(
    panel(page).getByRole("status").filter({ hasText: "Simulation valide" }),
  ).toBeVisible();
  await panel(page)
    .getByRole("textbox", { name: /^Résumé/ })
    .press("Control+Enter");
  await expect(toast(page, /Appliqué :/)).toBeVisible();
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 0 opération/);
}

/** The operations of the changeset the last toast links to: one changeset, its operations. */
async function changesetOperations(page: Page): Promise<string[]> {
  await page.getByRole("button", { name: "Voir le changeset" }).first().click();
  await expect(page).toHaveURL(/\/changesets\/[0-9a-f-]+$/);
  await page.getByRole("tab", { name: /Opérations/ }).click();
  const list = page.getByRole("list", { name: "Opérations" });
  await expect(list).toBeVisible();
  return (await list.getByRole("listitem").allInnerTexts()).map((text) => text.split(/\s/)[0]!);
}

test("a group, then a VLAN created and set on a network, seen in its tab, the prefix list and its record", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const run = `${browserName}-${Date.now().toString(36)}`;
  // After `production` in the order of the slugs (the topography opens on the first domain).
  const domain = `zz-vlan-${run}`;
  const group = `vlan-${run}`;
  // Its own name: the suggestions of « Poser un VLAN existant » span every group.
  const vlan = `impression-${run}`;
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
  // The anchor found by its text among the entities, sites and domains (`q`, task 12j).
  await form.getByRole("combobox", { name: /^Ancre/ }).fill(domain);
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
  await create.getByRole("textbox", { name: "Nom" }).fill(vlan);
  await create.getByRole("button", { name: "Choisir l'action" }).click();
  await page.getByRole("menuitem", { name: "Appliquer" }).click();
  await expectAccessible(page);
  await create.getByRole("textbox", { name: "Nom" }).press("Control+Enter");
  await expect(toast(page, new RegExp(`Appliqué : VLAN 100 · ${vlan}`))).toBeVisible();
  await expect(create).toHaveCount(0);
  // The same « Créer un VLAN », now in the bar above the VLAN of the group, has the focus back.
  const again = page.getByRole("button", { name: "Créer un VLAN" });
  await expect(again).toBeFocused();
  await expect(
    page.getByRole("grid", { name: `VLAN du groupe ${group}` }).getByRole("link", { name: "100" }),
  ).toBeVisible();

  // The refusals of the server on the field of the VID, with their code (design § « VLAN,
  // écrans du cœur »), sent from the field of the name: a VID taken in the group, then one
  // outside its range, each said in place of the other, the focus on the VID; abandoned,
  // nothing written.
  await again.click();
  await expect(vid).toHaveValue("101");
  const named = create.getByRole("textbox", { name: "Nom" });
  await named.fill(`refus-${run}`);
  await create.getByRole("button", { name: "Choisir l'action" }).click();
  await page.getByRole("menuitem", { name: "Appliquer" }).click();
  await vid.fill("100");
  await named.press("Control+Enter");
  await expect(vid).toHaveAccessibleDescription(/core\.vlan\.vid_duplicate/);
  await expect(vid).toHaveAttribute("aria-invalid", "true");
  await expect(vid).toBeFocused();
  await expectAccessible(page);
  await vid.fill("200");
  await named.press("Control+Enter");
  await expect(vid).toHaveAccessibleDescription(/core\.vlan\.vid_out_of_range/);
  await expect(vid).not.toHaveAccessibleDescription(/core\.vlan\.vid_duplicate/);
  await expect(vid).toBeFocused();
  await page.keyboard.press("Escape");
  await page
    .getByRole("alertdialog", { name: "Abandonner les modifications ?" })
    .getByRole("button", { name: "Abandonner" })
    .click();
  await expect(create).toHaveCount(0);
  const ofGroup = (await call(page, "GET", `/v1/vlans?name=refus-${run}`)) as { items: unknown[] };
  expect(ofGroup.items).toEqual([]);

  // The network: its VLAN tab says it carries none; « Poser un VLAN existant » takes the VLAN
  // among the suggestions, its setting to the draft, once; the draft applied, it says the VLAN.
  await page.goto(`/prefixes?domain=${domain}`);
  await page.getByRole("grid", { name: "Préfixes" }).getByRole("link", { name: NETWORK }).click();
  const tabs = page.getByRole("tablist", { name: "Sections de la fiche" });
  await tabs.getByRole("tab", { name: "VLAN" }).click();
  await expect(page.getByText("Ce réseau ne porte aucun VLAN")).toBeVisible();
  const setting = page.getByRole("form", { name: "Poser un VLAN existant" });
  const set = setting.getByRole("button", { name: "Poser le VLAN" });
  // A text typed and no VLAN taken: nothing is guessed, said on the field.
  const chosen = setting.getByRole("combobox", { name: /^VLAN/ });
  await chosen.fill(vlan);
  await set.click();
  await expect(chosen).toHaveAccessibleDescription(/Choisissez un VLAN dans la liste/);
  await expect(chosen).toBeFocused();
  await chosen.fill(vlan);
  await page.getByRole("option", { name: `100 · ${vlan}` }).click();
  await expectAccessible(page);
  await set.click();
  await expect(
    toast(
      page,
      new RegExp(`Ajouté au brouillon : pose du VLAN 100 · ${vlan} sur ${escaped(NETWORK)}`),
    ),
  ).toBeVisible();
  await expect(set).toHaveAccessibleDescription(/déjà dans votre brouillon/);
  await applyDraft(page, 1);
  await expect(page.getByText(`Ce réseau porte le VLAN 100 · ${vlan}.`)).toBeVisible();
  await expectAccessible(page);
  const remove = page.getByRole("button", { name: "Retirer le VLAN" });
  await remove.click();
  await expect(
    toast(page, new RegExp(`Ajouté au brouillon : retrait du VLAN 100 · ${vlan}`)),
  ).toBeVisible();
  await expect(remove).toHaveAccessibleDescription(/déjà dans votre brouillon/);

  // The column of the list of prefixes: the VLAN of the network, a link to its record.
  await page.goto(`/prefixes?domain=${domain}`);
  const prefixes = page.getByRole("grid", { name: "Préfixes" });
  await expect(prefixes.getByRole("columnheader", { name: /^VLAN/ })).toBeAttached();
  await prefixes.getByRole("link", { name: `100 · ${vlan}` }).click();

  // The record of the VLAN: its group, the network that carries it.
  await expect(page.getByRole("heading", { level: 1, name: `VLAN 100 · ${vlan}` })).toBeVisible();
  // In its attributes (the tab « Propriétés » names the group too, in its grid).
  await expect(
    page.getByRole("definition").getByRole("link", { name: group, exact: true }),
  ).toBeVisible();
  await tabs.getByRole("tab", { name: "Réseaux" }).click();
  await expect(
    page
      .getByRole("grid", { name: `Réseaux qui portent le VLAN 100 · ${vlan}` })
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

test("a VLAN created for a network: to the draft then applied, or applied at once, one changeset", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const run = `${browserName}-${Date.now().toString(36)}`;
  const { slug, prefixes } = await ownedDomain(page, `zz-vlan-for-${run}`, [NETWORK, SECOND]);
  const group = `vlan-for-${run}`;
  const domains = (await call(page, "GET", `/v1/domains?slug=${slug}`)) as {
    items: { id: string }[];
  };
  await call(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end VLAN for a network",
    operations: [
      {
        op: "create_vlan_group",
        slug: group,
        anchor: { domain: { id: domains.items[0]!.id } },
        vid_min: 100,
        vid_max: 199,
      },
    ],
  });

  /** « Créer un VLAN pour ce réseau » from the VLAN tab of `prefix`: the panel, filled. */
  async function createFor(prefix: string, network: string, vid: string, name: string) {
    await page.goto(`/prefixes/${prefix}?tab=vlan`);
    await expect(page.getByText("Ce réseau ne porte aucun VLAN")).toBeVisible();
    await page.getByRole("button", { name: "Créer un VLAN pour ce réseau" }).click();
    const form = page.getByRole("form", { name: `Créer un VLAN pour ${network}` });
    await form.getByRole("combobox", { name: /^Groupe/ }).fill(group);
    await page.getByRole("option", { name: group, exact: true }).click();
    await expect(form.getByRole("textbox", { name: "VID" })).toHaveValue(vid);
    await form.getByRole("textbox", { name: "Nom" }).fill(name);
    await expect(form.getByRole("textbox", { name: "Réseau" })).toHaveValue(network);
    await expect(form.getByText("Deux opérations, un seul changeset")).toBeVisible();
    return form;
  }

  // To the draft (the default action, Ctrl+Entrée): two operations, nothing set before the
  // draft is applied; the draft applied, one changeset holds both.
  const first = `serveurs-${run}`;
  const draft = await createFor(prefixes[0]!, NETWORK, "100", first);
  await expectAccessible(page);
  await draft.getByRole("textbox", { name: "Nom" }).press("Control+Enter");
  await expect(
    toast(page, new RegExp(`Ajouté au brouillon : VLAN 100 · ${first} pour ${escaped(NETWORK)}`)),
  ).toBeVisible();
  await expect(draft).toHaveCount(0);
  await expect(page.getByText("Ce réseau ne porte aucun VLAN")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Créer un VLAN pour ce réseau" }),
  ).toHaveAccessibleDescription(/déjà dans votre brouillon/);
  await applyDraft(page, 2);
  await expect(page.getByText(`Ce réseau porte le VLAN 100 · ${first}.`)).toBeVisible();
  expect(await changesetOperations(page)).toEqual(["create_vlan", "assign_property"]);

  // Applied at once, its motive prefilled; a refusal of the setting (the answer rewritten
  // once, as the server says a local name no earlier operation defines) is said on the field
  // of the network with its code; applied again, one changeset holds both.
  const second = `postes-${run}`;
  const now = await createFor(prefixes[1]!, SECOND, "101", second);
  await now.getByRole("button", { name: "Choisir l'action" }).click();
  await page.getByRole("menuitem", { name: "Appliquer" }).click();
  await expect(now.getByRole("textbox", { name: "Motif" })).toHaveValue(
    `Création d'un VLAN pour ${SECOND}`,
  );
  await page.route(
    "**/v1/changesets?approval=self",
    (route) =>
      route.fulfill({
        status: 422,
        contentType: "application/problem+json",
        body: JSON.stringify({
          type: "about:blank",
          title: "Unprocessable Content",
          status: 422,
          violations: [
            {
              rule: "prop.assignment.unknown_local",
              detail: "$0 is defined by no earlier operation",
              operation: 1,
              pointer: null,
              severity: "error",
              subjects: [],
            },
          ],
        }),
      }),
    { times: 1 },
  );
  await now.getByRole("textbox", { name: "Nom" }).press("Control+Enter");
  const network = now.getByRole("textbox", { name: "Réseau" });
  await expect(network).toHaveAccessibleDescription(/prop\.assignment\.unknown_local/);
  await expect(network).toBeFocused();
  await expectAccessible(page);
  await page.keyboard.press("Control+Enter");
  await expect(
    toast(page, new RegExp(`Appliqué : VLAN 101 · ${second} pour ${escaped(SECOND)}`)),
  ).toBeVisible();
  await expect(now).toHaveCount(0);
  await expect(page.getByText(`Ce réseau porte le VLAN 101 · ${second}.`)).toBeVisible();
  expect(await changesetOperations(page)).toEqual(["create_vlan", "assign_property"]);
});
