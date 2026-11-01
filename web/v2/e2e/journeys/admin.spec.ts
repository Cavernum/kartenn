import type { Page } from "@playwright/test";
import { apiCall, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// The administration of the rights by groups in the new shell (spec 021, task 15a;
// REQ-021-12, REQ-025-12; maquette E7), rewritten from web/e2e/journeys/admin.spec.ts (its
// roles and bindings, now the groups): a group chosen in its list opens in the right panel,
// its members and its roles on scopes; a member removed and a role added go to the draft,
// marked on their rows, the removal cancelled from its row, the draft applied; the
// delivered group `secours` says at its top why it is read only; a role lists its
// permissions under their served name, and the groups bound to it; a reader reaches no
// screen of the administration. The services accounts, the identities (15b), the audit and
// the imports (15c) extend this journey.

const panel = (page: Page) => page.getByRole("complementary", { name: "Panneau droit" });
const counter = (page: Page) => page.getByRole("button", { name: /^Brouillon \d+ opération/ });
const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

/** A group of its own for the journey, with a group of the IdP and the role `reader`. */
async function ownGroup(page: Page, key: string): Promise<string> {
  const created = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end administration",
    operations: [
      { op: "set_group", key, labels: { fr: `Équipe ${key}` } },
      { op: "add_member", group: key, member: { kind: "idp_group", issuer: "dev", group: key } },
      { op: "bind", group: key, role: "reader", scope: { kind: "global" } },
    ],
  });
  expect(created.status, JSON.stringify(created.body)).toBe(201);
  const groups = (await apiCall(page, "GET", "/v1/groups?page_size=500")).body as {
    items: { id: string; key: string }[];
  };
  return groups.items.find((g) => g.key === key)!.id;
}

test("a group: its members and roles in the panel, written to the draft, then applied", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const key = `zz-admin-${browserName}-${Date.now().toString(36)}`;
  const id = await ownGroup(page, key);

  await navigation(page).getByRole("button", { name: "Administration" }).click();
  await navigation(page).getByRole("link", { name: "Groupes" }).click();
  await expect(page).toHaveURL(/\/admin\/groups$/);
  const grid = page.getByRole("grid", { name: "Groupes" });
  await grid.getByRole("gridcell", { name: new RegExp(key) }).click();
  await expect(page).toHaveURL(new RegExp(`/admin/groups/${id}$`));
  await expect(page.getByRole("navigation", { name: "Fil d'Ariane" })).toContainText(key);

  const detail = panel(page);
  await expect(detail.getByRole("heading", { level: 2 })).toContainText(key);
  const members = detail.getByRole("list", { name: `Membres de ${key}` });
  const roles = detail.getByRole("list", { name: `Rôles sur périmètres de ${key}` });
  await expect(members.getByRole("listitem")).toHaveText([/dev \/ zz-admin.*groupe de l'IdP/]);
  await expect(roles.getByRole("listitem")).toHaveText([/reader.*Toute l'organisation/]);
  await expectAccessible(page);

  // The member removed: still there, struck, its action now cancels the removal.
  const idp = `dev / ${key}`;
  await members.getByRole("button", { name: `Retirer ${idp} du groupe` }).click();
  await expect(members.getByRole("listitem").first()).toHaveAttribute("data-pending", "removed");
  await expect(detail.getByRole("heading", { name: /^Membres · 1/ })).toContainText(
    "(brouillon : −1, +0)",
  );

  // A role added on the whole organisation, at the end of its list.
  await detail.getByRole("button", { name: "Ajouter un rôle" }).click();
  const dialog = page.getByRole("dialog", { name: `Ajouter un rôle à ${key}` });
  await dialog.getByRole("combobox", { name: /^Rôle/ }).click();
  await page.getByRole("option", { name: /redacteur/ }).click();
  await dialog.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(dialog).toHaveCount(0);
  await expect(roles.getByRole("listitem")).toHaveCount(2);
  await expect(roles.getByRole("listitem").last()).toHaveAttribute("data-pending", "added");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 2 opération/);
  await expectAccessible(page);

  // The removal cancelled from its row: out of the draft.
  await members.getByRole("button", { name: `Annuler le retrait de ${idp}` }).click();
  await expect(members.getByRole("listitem").first()).not.toHaveAttribute("data-pending");
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);

  // Applied from the draft tab (the counter would close the open panel): the role is the
  // group's.
  await detail.getByRole("tab", { name: /^Brouillon/ }).click();
  await expect(detail.getByRole("status").filter({ hasText: "Simulation valide" })).toBeVisible();
  await detail.locator(".k-split-button__main").click();
  await expect(page.getByRole("status").filter({ hasText: "Appliqué :" }).first()).toBeVisible();
  await detail.getByRole("tab", { name: "Détail" }).click();
  await expect(roles.getByRole("listitem")).toHaveCount(2);
  await expect(roles.getByRole("listitem").last()).not.toHaveAttribute("data-pending");
  await expect(detail.getByRole("heading", { name: /^Rôles sur périmètres · 2/ })).toBeVisible();
});

test("the delivered group says at its top why it is read only", async ({ page, login }) => {
  await login("alice");
  await page.goto("/admin/groups");
  await page
    .getByRole("grid", { name: "Groupes" })
    .getByRole("gridcell", { name: /secours/ })
    .click();
  const detail = panel(page);
  const notice = detail.getByRole("status").filter({ hasText: "Groupe livré : seuls ses membres" });
  await expect(notice).toBeVisible();
  await expect(notice.getByRole("button", { name: /core\.group\.builtin/ })).toBeVisible();
  // Its actions stay, refused with their reason; its members are a global administrator's.
  const edit = detail.getByRole("toolbar", { name: "Actions du groupe" }).getByRole("button", {
    name: /^Modifier/,
  });
  await expect(edit).toHaveAttribute("aria-disabled", "true");
  await expect(detail.getByRole("button", { name: "Ajouter un rôle" })).toHaveAttribute(
    "aria-disabled",
    "true",
  );
  await expect(detail.getByRole("button", { name: "Ajouter un membre" })).not.toHaveAttribute(
    "aria-disabled",
  );
  await expectAccessible(page);
});

test("a role: its permissions under their served name, its groups", async ({ page, login }) => {
  await login("alice");
  await page.goto("/admin/roles");
  const grid = page.getByRole("grid", { name: "Rôles" });
  await expect(grid).toContainText("administrator");
  await grid.getByRole("gridcell", { name: /redacteur/ }).click();
  await expect(page).toHaveURL(/\/admin\/roles\/redacteur$/);
  const detail = panel(page);
  const permissions = detail.getByRole("list", { name: "Permissions de redacteur" });
  await expect(permissions.getByRole("listitem")).toHaveCount(10);
  // Each by its served name, copiable.
  await expect(permissions.getByRole("button", { name: "Copier core.prefix.edit" })).toBeVisible();
  await expect(
    detail.getByRole("list", { name: "Groupes liés à redacteur" }).getByRole("listitem"),
  ).toContainText(["redacteurs"]);
  await expectAccessible(page);
});

test.describe("a reader", () => {
  test.use({ toleratedErrors: [/status of 403/] });

  test("reaches no screen of the administration", async ({ page, login }) => {
    await login("bob");
    await expect(navigation(page).getByRole("button", { name: "Administration" })).toHaveCount(0);
    await page.goto("/admin/groups");
    await expect(page.getByRole("main")).toContainText("Les groupes n'ont pas pu être lus");
  });
});
