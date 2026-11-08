import type { Page } from "@playwright/test";
import { apiCall, expect, expectAccessible, ownedDomain, test } from "../fixtures/journeys.ts";

// Anomalies and declarations of the new shell (spec 021, task 11i; REQ-021-9), the rows of
// the old shell that no journey proved (web/e2e only visited /anomalies and /properties
// for axe): the Anomalies tab of a record, a domain's with their object and a network's
// own; /anomalies filtered by the bar, one acknowledged with its motive, the gaps a tab of
// their own in the URL; /property-declarations, each its cardinality, and the gaps of an
// expected key at hand; a gap read (task 11j), opened by a key of the test's own. Each test
// in a domain of its own: its network 198.51.100.0/24, in a documentation block (RFC 5737),
// opens the warning `core.addr.documentation` (REQ-002-35).

const RULE = "core.addr.documentation";
const NETWORK = "198.51.100.0/24";
const unique = (what: string, browser: string) =>
  `zz-${what}-${browser}-${Date.now().toString(36)}`;
const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });
const tabs = (page: Page) => page.getByRole("tablist", { name: "Sections de la fiche" });
/** The title of a record: its kind in words (the dictionary's), then the object's name. */
const titled = (name: string) => new RegExp(`\\s${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);
// Open, a list of Reka hides the rest of the page from the tree: the popover by its class.
const popover = (page: Page) => page.locator(".k-filter-picker__popover");

/** Types in the focused field, then takes the suggestion once it is the only one. */
async function choose(page: Page, text: string, option: RegExp): Promise<void> {
  await page.keyboard.type(text);
  const list = page.getByRole("listbox");
  await expect(list.getByRole("option")).toHaveCount(1);
  await expect(list.getByRole("option", { name: option })).toHaveCount(1);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("listbox")).toHaveCount(0);
}

test("the anomalies of a record in its tab: a domain's with their object, a network's own", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique("anomalies", browserName));
  const domains = (await apiCall(page, "GET", `/v1/domains?slug=${slug}`)).body as {
    items: { id: string }[];
  };
  await page.goto(`/domains/${domains.items[0]!.id}`);
  await expect(page.getByRole("heading", { level: 1, name: titled(slug) })).toBeVisible();
  await tabs(page).getByRole("tab", { name: "Anomalies" }).click();
  await expect(page).toHaveURL(/[?&]tab=anomalies/);
  // Those of the objects of the domain, each with its object: the network's warning, its code.
  const ofDomain = page.getByRole("grid", { name: `Anomalies de ${slug}` });
  const warning = ofDomain.getByRole("row").filter({ hasText: RULE });
  await expect(warning).toHaveCount(1);
  await expect(warning.getByRole("button", { name: /^Acquitter…/ })).toBeVisible();
  await expectAccessible(page);
  await warning.getByRole("link", { name: NETWORK }).click();

  // The network's own, without the column of the object: it is the record's.
  await expect(page.getByRole("heading", { level: 1, name: titled(NETWORK) })).toBeVisible();
  await tabs(page).getByRole("tab", { name: "Anomalies" }).click();
  const own = page.getByRole("grid", { name: `Anomalies de ${NETWORK}` });
  await expect(own.getByRole("row").filter({ hasText: RULE })).toHaveCount(1);
  await expect(own.getByRole("columnheader", { name: "Objet" })).toHaveCount(0);
  await expectAccessible(page);
});

test("the open anomalies filtered by domain, one acknowledged with its motive, then the gaps", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique("acquit", browserName));
  await navigation(page).getByRole("link", { name: "Anomalies", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Anomalies" })).toBeVisible();
  await expect(page.getByRole("grid", { name: "Anomalies" })).toBeVisible();

  // The domain by the bar, at the keyboard: the list keeps its anomalies, in the URL.
  const bar = page.getByRole("group", { name: "Filtres : Anomalies" });
  await bar.getByRole("button", { name: "Ajouter un filtre" }).click();
  await choose(page, "Domaine", /^Domaine/);
  await choose(page, slug, new RegExp(slug));
  await page.keyboard.press("Enter");
  await expect(popover(page)).toHaveCount(0);
  await expect(bar.getByRole("button", { name: `Domaine est ${slug}`, exact: true })).toBeFocused();
  await expect(page).toHaveURL(/[?&]domain=[0-9a-f-]{36}/);
  const grid = page.getByRole("grid", { name: "Anomalies" });
  await expect(grid.getByRole("row").filter({ hasText: RULE })).toHaveCount(1);
  await expect(grid.getByRole("link", { name: NETWORK })).toBeVisible();
  await expectAccessible(page);

  // Acknowledged with a motive: refused without one, said on the field; then closed.
  await grid.getByRole("button", { name: /^Acquitter…/ }).click();
  const dialog = page.getByRole("dialog", { name: "Acquitter une anomalie" });
  await expect(dialog).toContainText(RULE);
  await dialog.getByRole("button", { name: "Acquitter", exact: true }).click();
  const motive = dialog.getByRole("textbox", { name: /^Motif/ });
  await expect(motive).toHaveAttribute("aria-invalid", "true");
  await expect(motive).toBeFocused();
  await expect(dialog).toContainText("Écrivez le motif de l'acquittement.");
  await expectAccessible(page);
  // Written where the focus was put back, sent by Enter (the form's own submission).
  await page.keyboard.type("Réseau de documentation, voulu par le parcours");
  await page.keyboard.press("Enter");
  await expect(dialog).toHaveCount(0);
  await expect(
    page
      .getByRole("status")
      .filter({ hasText: `Anomalie ${RULE} acquittée.` })
      .first(),
  ).toBeVisible();
  // The last one gone, the grid with it: the focus on the empty state, never on the body.
  await expect(page.getByText("Aucune anomalie ouverte")).toBeVisible();
  await expect(page.locator(":focus")).toContainText("Aucune anomalie ouverte");
  // Closed for good: the server lists none in the domain any more.
  const domain = new URL(page.url()).searchParams.get("domain");
  const listed = (await apiCall(page, "GET", `/v1/anomalies?domain=${domain}`)).body as {
    items: unknown[];
  };
  expect(listed.items).toEqual([]);

  // The gaps, a tab of their own in the URL: the filters of the anomalies left behind.
  await page
    .getByRole("tablist", { name: "Anomalies et lacunes" })
    .getByRole("tab", { name: "Lacunes" })
    .click();
  await expect(page).toHaveURL(/[?&]tab=gaps/);
  await expect(page).not.toHaveURL(/[?&]domain=/);
  await expect(page.getByRole("group", { name: "Filtres : Lacunes" })).toBeVisible();
  await expect(
    page.getByRole("grid", { name: "Lacunes" }).or(page.getByText("Aucune lacune")),
  ).toBeVisible();
  await expectAccessible(page);
});

test("a gap read: its key, its object and the deviation, from a source of the build", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique("gap", browserName), []);
  const domains = (await apiCall(page, "GET", `/v1/domains?slug=${slug}`)).body as {
    items: { id: string }[];
  };
  const domain = domains.items[0]!.id;
  // The lightest source of the build: a key of the test's own (one per browser: both run
  // at once), checked against the inference of the policy (REQ-003-24, REQ-010-40), one
  // rule of it on that key, in the test's domain alone. The write of a network then
  // compares, with no task in the background.
  const stamp = `${browserName}-${Date.now().toString(36)}`;
  const key = `custom.gap_${stamp.replaceAll("-", "_")}`;
  const source = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end expected source",
    operations: [
      {
        op: "declare_property",
        local: 0,
        key,
        type: "enum",
        cardinality: "single",
        carriers: ["prefix"],
        expected: { source: "policy.inference", mode: "check" },
        labels: { fr: "Zone du parcours" },
      },
      { op: "add_enum_value", property: { local: 0 }, key: "prod", labels: { fr: "Production" } },
      { op: "policy.axis.designate", property: key, carrier: "tree", inference: true },
      {
        op: "policy.inference_rule.create",
        key: `gap-${stamp}`,
        axis: key,
        criteria: [{ domain: [domain] }],
        values: ["prod"],
      },
    ],
  });
  expect(source.status, JSON.stringify(source.body)).toBe(201);
  // A network written without the value expected: the gap opened by the write.
  const network = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end gap",
    operations: [
      { op: "create_prefix", domain: { id: domain }, prefix: NETWORK, nature: "network" },
    ],
  });
  expect(network.status, JSON.stringify(network.body)).toBe(201);

  // Its row in the tab of the gaps filtered by the key: the key and its label, the network,
  // the deviation composed by the screen (task 11m, A36): the value expected by its label,
  // none resolved, the source and its revision; never the server's sentence.
  await page.goto(`/anomalies?tab=gaps&property=${key}`);
  await expect(page.getByRole("heading", { level: 1, name: "Anomalies" })).toBeVisible();
  const grid = page.getByRole("grid", { name: "Lacunes" });
  const gap = grid.getByRole("row").filter({ hasText: NETWORK });
  await expect(gap).toHaveCount(1);
  await expect(gap).toContainText("Zone du parcours");
  await expect(gap).toContainText(key);
  await expect(gap).toContainText("Production");
  await expect(gap).toContainText("non renseigné");
  await expect(gap).toContainText("policy.inference");
  await expect(gap).toContainText(/révision \d+/);
  await expect(gap).not.toContainText("resolved");
  await expect(gap.getByRole("link", { name: NETWORK })).toBeVisible();
  await expectAccessible(page);
});

test("the declared properties, each its cardinality; the gaps of an expected key at hand", async ({
  page,
  login,
}) => {
  await login("alice");
  await navigation(page).getByRole("link", { name: "Propriétés", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Propriétés" })).toBeVisible();
  const grid = page.getByRole("grid", { name: "Propriétés" });
  // In words, the contract's value behind them: one value, inherited along the addressing.
  const owner = grid.getByRole("row", { name: /core\.owner/ });
  await expect(owner).toContainText("Une valeur");
  await expect(owner).toContainText("Adressage");
  await expectAccessible(page);

  // An expected key (policy.environment, expected from the inference of the policy): its
  // gaps at hand, the tab of the gaps filtered by it.
  const environment = grid.getByRole("row", { name: /policy\.environment\b/ });
  await expect(environment).toContainText("attendue");
  await environment.getByRole("button", { name: "Voir les lacunes de policy.environment" }).click();
  await expect(page).toHaveURL(/\/anomalies\?/);
  await expect(page).toHaveURL(/[?&]tab=gaps/);
  await expect(page).toHaveURL(/[?&]property=policy\.environment(&|$)/);
  await expect(page.getByRole("tab", { name: "Lacunes" })).toHaveAttribute("aria-selected", "true");
  const bar = page.getByRole("group", { name: "Filtres : Lacunes" });
  await expect(bar.getByRole("button", { name: /^Propriété est / })).toHaveCount(1);
  await expect(
    page.getByRole("grid", { name: "Lacunes" }).or(page.getByText("Aucune lacune")),
  ).toBeVisible();
  await expectAccessible(page);
});
