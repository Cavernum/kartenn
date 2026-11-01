import type { Browser, Page } from "@playwright/test";
import {
  apiCall,
  expect,
  expectAccessible,
  loginIn,
  ownedDomain,
  test,
} from "../fixtures/journeys.ts";

// The changesets and their review in the new shell (spec 021, task 14; REQ-021-10, 37;
// maquette E4), rewritten from web/e2e/journeys/changesets.spec.ts: a proposal of Carol
// (an editor without `approval.self`) is pending; she reads it among hers, « Valider »
// refused with the permission she lacks (REQ-024-11), « Retirer » offered; Alice finds it in
// « À instruire », counted in the sidebar, validates it, then undoes it — a new changeset;
// another proposal rejected with its motive, asked in a dialog. Each test writes in a
// domain of its own (RFC 5737, TEST-NET-2).

const unique = (browser: string) => `zz-review-${browser}-${Date.now().toString(36)}`;

/** Carol, in a context of her own, proposes the creation of an address in `domain`. */
async function propose(browser: Browser, domain: string, address: string, motive: string) {
  const context = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  const carol = await context.newPage();
  await loginIn(carol, "carol");
  const domains = (await apiCall(carol, "GET", `/v1/domains?slug=${domain}`)).body as {
    items: { id: string }[];
  };
  const proposed = await apiCall(carol, "POST", "/v1/changesets?approval=self", {
    motive,
    operations: [
      { op: "create_address", domain: { id: domains.items[0]!.id }, address, usage: "host" },
    ],
  });
  expect(proposed.status, JSON.stringify(proposed.body)).toBe(202);
  return {
    carol,
    context,
    id: (proposed.body as { changeset: { id: string } }).changeset.id,
  };
}

const actions = (page: Page) => page.getByRole("toolbar", { name: "Actions de revue" });
const state = (page: Page, label: string) =>
  page.getByRole("main").locator(".k-status-badge").filter({ hasText: label });

test("a proposal of Carol, validated by Alice from « À instruire », then undone", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  const motive = `proposition ${browserName} ${Date.now()}`;
  const { carol, context, id } = await propose(browser, slug, "198.51.100.20", motive);
  try {
    // Carol reads hers: pending, the permission she lacks to apply it, « Retirer » offered.
    await carol.goto("/changesets");
    await carol.getByRole("button", { name: "Mes changesets" }).click();
    await expect(carol).toHaveURL(/author=/);
    const row = carol.getByRole("grid", { name: "Changesets" }).getByRole("row").filter({
      hasText: motive,
    });
    await expect(row).toContainText("En attente");
    await expect(row).toContainText("vous");
    await row.getByRole("link", { name: motive }).click();
    await expect(carol).toHaveURL(new RegExp(`/changesets/${id}$`));
    await expect(state(carol, "En attente")).toBeVisible();
    const validate = actions(carol).getByRole("button", { name: /^Valider/ });
    await expect(validate).toHaveAttribute("aria-disabled", "true");
    await expect(validate).toHaveAccessibleDescription(/Il faut core\.address\.approval\.self/);
    await expect(actions(carol).getByRole("button", { name: "Retirer" })).not.toHaveAttribute(
      "aria-disabled",
    );
    await expectAccessible(carol);
  } finally {
    await context.close();
  }

  // Alice finds it in « À instruire », counted in the sidebar, and validates it.
  await page.goto("/proposals");
  await expect(
    page.getByRole("navigation", { name: "Navigation principale" }).getByRole("link", {
      name: /À instruire/,
    }),
  ).toContainText(/\d/);
  const proposals = page.getByRole("grid", { name: "À instruire" });
  const proposal = proposals.getByRole("row").filter({ hasText: motive });
  // The permission its author lacks, by its served name.
  await expect(proposal).toContainText(/core\.address\./);
  await expectAccessible(page);
  await proposal.getByRole("link", { name: motive }).click();
  await expect(page).toHaveURL(new RegExp(`/changesets/${id}$`));
  await expect(page.getByRole("tab", { name: /Différentiel/ })).toBeVisible();
  await actions(page)
    .getByRole("button", { name: /^Valider/ })
    .click();
  await expect(state(page, "Appliqué")).toBeVisible();
  await expect(page.getByRole("status").filter({ hasText: "Validé et appliqué" })).toBeVisible();

  // Then undoes it, confirmed: a new changeset, which its link opens.
  await actions(page).getByRole("button", { name: "Annuler ce changeset" }).click();
  const confirm = page.getByRole("alertdialog", { name: /^Annuler « / });
  await expect(confirm).toBeVisible();
  await confirm.getByRole("button", { name: "Annuler ce changeset" }).click();
  const undone = page.getByRole("status").filter({ hasText: /Annulation de « / });
  await expect(undone).toBeVisible();
  await page.getByRole("button", { name: "Voir le changeset" }).click();
  await expect(page).not.toHaveURL(new RegExp(`/changesets/${id}$`));
  await page.getByRole("tab", { name: /Opérations/ }).click();
  await expect(page.getByRole("list", { name: "Opérations" })).toContainText("close");
});

test("a proposal rejected with its motive, asked in a dialog", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const { slug } = await ownedDomain(page, unique(browserName));
  const motive = `rejet ${browserName} ${Date.now()}`;
  const { context, id } = await propose(browser, slug, "198.51.100.30", motive);
  await context.close();

  await page.goto(`/changesets/${id}`);
  await actions(page).getByRole("button", { name: "Rejeter…" }).click();
  const dialog = page.getByRole("dialog", { name: `Rejeter « ${motive} »` });
  const reason = dialog.getByRole("textbox", { name: /Motif du rejet/ });
  await expect(reason).toBeFocused();
  // Without a motive, nothing is sent: the field says why.
  await dialog.getByRole("button", { name: /^Rejeter/ }).click();
  await expect(dialog.getByText("Le motif du rejet est obligatoire.")).toBeVisible();
  await expectAccessible(page);
  await reason.fill("Le réseau change de site d'abord.");
  await dialog.getByRole("button", { name: /^Rejeter/ }).click();
  await expect(dialog).toBeHidden();
  await expect(state(page, "Rejeté")).toBeVisible();
  await expect(page.getByRole("main")).toContainText("Le réseau change de site d'abord.");
  // Nothing more to decide.
  await expect(actions(page).getByRole("button", { name: /^Valider/ })).toHaveCount(0);
});
