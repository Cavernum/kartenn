import type { Locator, Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the labelled native fields (spec 020, design « Inventaire » line 4,
// « Accessibilité »; REQ-020-16, 18, 28): TextField and FormField on their catalogue
// pages. The error is linked by `aria-describedby` (before the help) and said: it comes
// into a polite live region that was there, empty, before it (a region inserted with its
// text is not announced).

/**
 * Marks the live region of a field, so that it can be told later whether the error came
 * into the same element (said) or into a new one (silent).
 */
async function markRegion(field: Locator): Promise<void> {
  await field.evaluate((el) => {
    const region = el.closest(".k-form-field")!.querySelector("[aria-live]")!;
    (window as unknown as { kRegion: Element }).kRegion = region;
  });
}

/** The marked region holds the error now, and is still the same element. */
async function sameRegionSays(page: Page, field: Locator, text: string): Promise<void> {
  const same = await field.evaluate((el) => {
    const region = el.closest(".k-form-field")!.querySelector("[aria-live]");
    return region === (window as unknown as { kRegion: Element }).kRegion;
  });
  expect(same).toBe(true);
  await expect(
    page.locator(".k-form-field__live[aria-live='polite']").filter({ hasText: text }),
  ).toHaveCount(1);
}

test.describe("TextField", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "TextField", "keyboard");
  });

  test("its label names it; the help describes it; Tab goes from field to field", async ({
    page,
  }) => {
    const prefix = page.getByRole("textbox", { name: "Préfixe" });
    await expect(prefix).toHaveAccessibleDescription("Notation CIDR.");
    await expect(prefix).not.toHaveAttribute("aria-invalid");
    await prefix.focus();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("textbox", { name: "Nom" })).toBeFocused();
  });

  test("an error comes when the field is left: invalid, linked before the help, said", async ({
    page,
  }) => {
    const prefix = page.getByRole("textbox", { name: "Préfixe" });
    await markRegion(prefix);
    await prefix.focus();
    await page.keyboard.type("192.0.2.300/24");
    await page.keyboard.press("Tab");
    await expect(prefix).toHaveAttribute("aria-invalid", "true");
    await expect(prefix).toHaveAccessibleDescription(
      "Adresse hors de l'espace IPv4. Notation CIDR.",
    );
    await sameRegionSays(page, prefix, "Adresse hors de l'espace IPv4.");
    // Back in the field, typed right and left again: the error goes, the help stays.
    await prefix.focus();
    await prefix.fill("");
    await page.keyboard.type("192.0.2.0/24");
    await page.keyboard.press("Tab");
    await expect(prefix).not.toHaveAttribute("aria-invalid");
    await expect(prefix).toHaveAccessibleDescription("Notation CIDR.");
    await expect(page.locator(".k-form-field__live").first()).toBeEmpty();
  });

  test("another error replaces the first in the same region", async ({ page }) => {
    const prefix = page.getByRole("textbox", { name: "Préfixe" });
    await markRegion(prefix);
    await prefix.fill("192.0.2.300/24");
    await prefix.press("Tab");
    await prefix.fill("réseau");
    await prefix.press("Tab");
    await sameRegionSays(page, prefix, "Notation CIDR attendue (192.0.2.0/24).");
  });

  test("the snapshot of its tree, invalid", async ({ page }) => {
    const prefix = page.getByRole("textbox", { name: "Préfixe" });
    await prefix.fill("192.0.2.300/24");
    await prefix.press("Tab");
    await expect(page.locator(".k-cat-state__render")).toMatchAriaSnapshot(`
      - text: Préfixe
      - textbox "Préfixe": 192.0.2.300/24
      - paragraph: Adresse hors de l'espace IPv4.
      - paragraph: Notation CIDR.
      - text: Nom
      - textbox "Nom"
    `);
  });
});

test.describe("TextField: states", () => {
  test("read-only stays in the tabs and refuses typing; disabled leaves them", async ({ page }) => {
    await openState(page, "TextField", "readonly");
    const id = page.getByRole("textbox", { name: "Identifiant" });
    await id.focus();
    await expect(id).toBeFocused();
    await page.keyboard.type("x");
    await expect(id).toHaveValue("01JD4K7Q9V2M8X3N5P6R7S8T9W");
    await expect(id).toHaveAttribute("readonly", "");
    await openState(page, "TextField", "disabled");
    await expect(page.getByRole("textbox", { name: "Nom" })).toBeDisabled();
  });

  test("pending: busy, and said in its description", async ({ page }) => {
    await openState(page, "TextField", "pending");
    const prefix = page.getByRole("textbox", { name: "Préfixe" });
    await expect(prefix).toHaveAttribute("aria-busy", "true");
    await expect(prefix).toHaveAccessibleDescription("Chargement");
  });

  test("required and empty: required in its name, the error before the help", async ({ page }) => {
    await openState(page, "TextField", "missing");
    const name = page.getByRole("textbox", { name: /^Nom/ });
    await expect(name).toHaveAccessibleName(/^Nom\s*\(obligatoire\)$/);
    await expect(name).toHaveAttribute("required", "");
    await expect(name).toHaveAccessibleDescription(
      "Le nom est obligatoire. Visible dans les listes.",
    );
  });
});

test.describe("FormField: a group", () => {
  test("named by its label, described by its help; each box in it by Tab", async ({ page }) => {
    await openState(page, "FormField", "group");
    const group = page.getByRole("group", { name: /^Colonnes de l'export/ });
    await expect(group).toHaveAccessibleName(/^Colonnes de l'export\s*\(obligatoire\)$/);
    await expect(group).toHaveAccessibleDescription("Dans l'ordre de la liste.");
    await group.getByRole("checkbox", { name: "Préfixe" }).focus();
    await page.keyboard.press("Tab");
    await expect(group.getByRole("checkbox", { name: "Usage" })).toBeFocused();
  });

  test("the last box left unchecked: the error comes, said and linked to the group", async ({
    page,
  }) => {
    await openState(page, "FormField", "group");
    const group = page.getByRole("group", { name: /^Colonnes de l'export/ });
    const prefix = group.getByRole("checkbox", { name: "Préfixe" });
    await markRegion(prefix);
    await prefix.focus();
    await page.keyboard.press(" ");
    await page.keyboard.press("Tab");
    await page.keyboard.press(" ");
    await expect(group.getByRole("checkbox", { checked: true })).toHaveCount(0);
    await expect(group).toHaveAccessibleDescription(
      "Choisissez au moins une colonne. Dans l'ordre de la liste.",
    );
    await sameRegionSays(page, prefix, "Choisissez au moins une colonne.");
    // One box again: the error goes.
    await page.keyboard.press(" ");
    await expect(group).toHaveAccessibleDescription("Dans l'ordre de la liste.");
  });
});
