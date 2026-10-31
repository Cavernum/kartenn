import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the typed filters (spec 020, task 19; design « Inventaire » line 6;
// REQ-020-20, 21): a filter added, edited and removed with the keyboard alone, in sequence
// (property, operator, value; APG Button then Combobox, in a non-modal popover); a value
// that does not fit its type refused before it is applied, its reason linked to the field;
// two Escapes (the list, then the popover); a filter left half written kept as a draft,
// out of the URL; the URL written after each change, in the grammar of the list route.

const bar = (page: Page) => page.getByRole("group", { name: "Filtres des adresses" });
const add = (page: Page) => bar(page).getByRole("button", { name: "Ajouter un filtre" });
// Open, a list of Reka hides the rest of the page from the tree: the popover by its class.
const popover = (page: Page) => page.locator(".k-filter-picker__popover");
const property = (page: Page) => popover(page).locator("[data-filter-step='property'] input");
const operator = (page: Page) => popover(page).locator(".k-select__trigger");
const value = (page: Page) => popover(page).locator("[data-filter-step='value'] input").first();
const query = (page: Page) => page.locator("[data-query]");
const chips = (page: Page) => bar(page).locator(".k-filter-chip");
/** The button of a chip, by its sentence (its remove button is « Retirer le filtre … »). */
const chip = (page: Page, sentence: string) =>
  bar(page).getByRole("button", { name: sentence, exact: true });

/**
 * Types in the focused field, then takes the suggestion once it is the only one (the list
 * of the text, not the one before it), and waits for the list to be gone: a list fading out
 * is still the top layer, which an Escape at once would close again.
 */
async function choose(page: Page, text: string, option: RegExp): Promise<void> {
  await page.keyboard.type(text);
  const list = page.getByRole("listbox");
  await expect(list.getByRole("option")).toHaveCount(1);
  await expect(list.getByRole("option", { name: option })).toHaveCount(1);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("listbox")).toHaveCount(0);
}

test.describe("FilterPicker (typed filters)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "FilterPicker", "keyboard");
  });

  test("the filters read from the URL, as chips; the other parameters kept", async ({ page }) => {
    await expect(chips(page)).toHaveCount(2);
    await expect(chip(page, "Domaine est Centre de données nord")).toBeVisible();
    await expect(chip(page, "Adresse dans 192.0.2.0/24")).toHaveAttribute(
      "aria-haspopup",
      "dialog",
    );
    await expect(add(page)).toHaveAttribute("aria-expanded", "false");
    await expect(add(page)).toHaveAttribute("aria-keyshortcuts", "/");
    await expect(query(page)).toHaveText(
      "?valid_at=2026-01-15T10:00:00+01:00&domain=dc-nord&within=192.0.2.0/24",
    );
  });

  test("the keys behind the labels of a chip: its description, its tooltip under the focus", async ({
    page,
  }) => {
    const domain = chip(page, "Domaine est Centre de données nord");
    await expect(domain).toHaveAccessibleDescription("Paramètre : domain=dc-nord");
    await domain.focus();
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Tab");
    await expect(domain).toBeFocused();
    await expect(page.getByRole("tooltip")).toHaveText("Paramètre : domain=dc-nord");
    // Open, the picker is not covered by it.
    await page.keyboard.press("Enter");
    await expect(popover(page)).toBeVisible();
    await expect(page.getByRole("tooltip")).toBeHidden();
  });

  test("add with the keyboard alone: the property list open, labels and keys, values of a closed list", async ({
    page,
  }) => {
    await add(page).focus();
    await page.keyboard.press("Enter");
    await expect(popover(page)).toHaveAttribute("role", "dialog");
    await expect(popover(page)).toHaveAccessibleName("Ajouter un filtre");
    await expect(property(page)).toBeFocused();
    // Its list is open as it opens: each property, its label then its key (REQ-020-21).
    const properties = page.getByRole("listbox", { name: "Propriété" });
    await expect(properties.getByRole("option")).toHaveCount(8);
    await expect(properties.getByRole("option", { name: "Usage usage" })).toBeVisible();
    await expect(properties.getByRole("option", { name: "prop.dns.ttl" })).toBeVisible();
    await choose(page, "usa", /^Usage/);
    // One operator only: on to the value, a closed list with autocompletion.
    await expect(value(page)).toBeFocused();
    await expect(operator(page)).toHaveText(/est/);
    await choose(page, "hô", /^Hôte/);
    await choose(page, "vi", /^VIP/);
    const chosen = popover(page).getByRole("list", { name: "Valeurs choisies" });
    await expect(chosen.getByRole("listitem")).toHaveText(["Hôte", "VIP"]);
    // Being written, the filter is a draft chip, out of the URL.
    await expect(page.locator(".k-filter-chip__pill[data-state='draft']")).toHaveCount(1);
    await expect(query(page)).not.toContainText("usage");
    await page.keyboard.press("Enter");
    await expect(popover(page)).toHaveCount(0);
    await expect(chip(page, "Usage est l'un de Hôte, VIP")).toBeFocused();
    await expect(page.getByRole("status").first()).toHaveText(
      "Filtre appliqué : Usage est l'un de Hôte, VIP",
    );
    await expect(query(page)).toContainText("&usage=host&usage=vip");
  });

  test("a value out of its type is refused before it is applied, its reason linked", async ({
    page,
  }) => {
    await add(page).focus();
    await page.keyboard.press("Enter");
    await choose(page, "ttl", /^prop\.dns\.ttl/);
    // Several operators: the choice of the operator comes next.
    await expect(operator(page)).toBeFocused();
    await expect(operator(page)).toHaveText(/est/);
    await page.keyboard.press("Tab");
    await expect(value(page)).toBeFocused();
    await page.keyboard.type("soixante");
    await page.keyboard.press("Enter");
    await expect(popover(page)).toBeVisible();
    const field = popover(page).getByRole("textbox", { name: "Valeur" });
    await expect(field).toBeFocused();
    await expect(field).toHaveAttribute("aria-invalid", "true");
    await expect(field).toHaveAccessibleDescription(/^Nombre attendu, par exemple 42 ou 3\.5\./);
    await expect(query(page)).not.toContainText("ttl");
    await page.keyboard.press("ControlOrMeta+a");
    await page.keyboard.type("60");
    await page.keyboard.press("Enter");
    await expect(popover(page)).toHaveCount(0);
    await expect(chip(page, "prop.dns.ttl est 60")).toBeFocused();
    await expect(query(page)).toContainText("&prop.dns.ttl=60");
  });

  test("edit with the keyboard: the value field first; host bits refused with the prefix", async ({
    page,
  }) => {
    const address = chip(page, "Adresse dans 192.0.2.0/24");
    await address.focus();
    await page.keyboard.press("Enter");
    await expect(popover(page)).toHaveAccessibleName("Modifier le filtre");
    await expect(address).toHaveAttribute("aria-expanded", "true");
    await expect(value(page)).toBeFocused();
    await expect(value(page)).toHaveValue("192.0.2.0/24");
    await page.keyboard.press("ControlOrMeta+a");
    await page.keyboard.type("198.51.100.1/24");
    await page.keyboard.press("Enter");
    await expect(
      popover(page).getByRole("textbox", { name: "Valeur" }),
    ).toHaveAccessibleDescription(/^Bits d'hôte non nuls : le préfixe est 198\.51\.100\.0\/24\./);
    await page.keyboard.press("ControlOrMeta+a");
    await page.keyboard.type("198.51.100.0/24");
    await page.keyboard.press("Enter");
    await expect(popover(page)).toHaveCount(0);
    const edited = chip(page, "Adresse dans 198.51.100.0/24");
    await expect(edited).toBeFocused();
    await expect(edited).toHaveAttribute("aria-expanded", "false");
    await expect(chips(page)).toHaveCount(2);
    await expect(query(page)).toContainText("&within=198.51.100.0/24");
  });

  test("remove with the keyboard: said, the focus on the next chip, out of the URL", async ({
    page,
  }) => {
    const remove = bar(page).getByRole("button", {
      name: "Retirer le filtre Domaine est Centre de données nord",
    });
    await remove.focus();
    await page.keyboard.press("Enter");
    await expect(chips(page)).toHaveCount(1);
    await expect(chip(page, "Adresse dans 192.0.2.0/24")).toBeFocused();
    await expect(page.getByRole("status").first()).toHaveText(
      "Filtre retiré : Domaine est Centre de données nord",
    );
    await expect(query(page)).not.toContainText("domain");
    // The last one: the focus goes to « Ajouter un filtre ».
    await page.keyboard.press("Tab");
    await page.keyboard.press("Enter");
    await expect(chips(page)).toHaveCount(0);
    await expect(add(page)).toBeFocused();
  });

  test("two Escapes: the list, then the popover; the focus back on the button", async ({
    page,
  }) => {
    await add(page).focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("listbox", { name: "Propriété" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(popover(page)).toBeVisible();
    await expect(property(page)).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(popover(page)).toHaveCount(0);
    await expect(add(page)).toBeFocused();
    await expect(chips(page)).toHaveCount(2);
  });

  test("a filter left half written stays a draft; « Annuler » drops it", async ({ page }) => {
    await add(page).focus();
    await page.keyboard.press("Enter");
    await choose(page, "propri", /^Propriétaire/);
    await expect(operator(page)).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(popover(page)).toHaveCount(0);
    const draft = chip(page, "Propriétaire est (en brouillon)");
    await expect(draft).toBeVisible();
    await expect(query(page)).not.toContainText("owner");
    await draft.focus();
    await page.keyboard.press("Enter");
    await expect(popover(page)).toHaveAccessibleName("Ajouter un filtre");
    await expect(value(page)).toBeFocused();
    await popover(page).getByRole("button", { name: "Annuler" }).focus();
    await page.keyboard.press("Enter");
    await expect(popover(page)).toHaveCount(0);
    await expect(chips(page)).toHaveCount(2);
    await expect(add(page)).toBeFocused();
  });

  test("an operator without value; one filter per parameter, the new one replaces", async ({
    page,
  }) => {
    await add(page).focus();
    await page.keyboard.press("Enter");
    await choose(page, "envi", /^Environnement/);
    await expect(operator(page)).toBeFocused();
    // The list opens on « est »; Down goes to « est renseigné », Enter takes it.
    await page.keyboard.press("Enter");
    await expect(page.getByRole("option", { name: "est", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: "est renseigné" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(operator(page)).toBeFocused();
    await expect(operator(page)).toHaveText(/est renseigné/);
    await expect(popover(page).getByText("Cet opérateur ne prend pas de valeur.")).toBeVisible();
    // « Annuler », then « Appliquer » (the primary one last).
    await page.keyboard.press("Tab");
    await expect(popover(page).getByRole("button", { name: "Annuler" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(popover(page).getByRole("button", { name: "Appliquer" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(chip(page, "Environnement est renseigné")).toBeFocused();
    await expect(query(page)).toContainText("&prop.policy.environment:exists=true");
    // A second domain filter replaces the first one (repeated, `domain=` would be « one of »).
    await add(page).focus();
    await page.keyboard.press("Enter");
    await choose(page, "doma", /^Domaine/);
    await choose(page, "sud", /^Centre de données sud/);
    await page.keyboard.press("Enter");
    await expect(chips(page)).toHaveCount(3);
    await expect(query(page)).toContainText("domain=dc-sud");
    await expect(query(page)).not.toContainText("dc-nord");
  });

  test("Tab in the operator list takes it and goes on to the fields it brings", async ({
    page,
  }) => {
    await add(page).focus();
    await page.keyboard.press("Enter");
    await choose(page, "ttl", /^prop\.dns\.ttl/);
    await expect(operator(page)).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("option", { name: "est", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: "entre" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(operator(page)).toHaveText(/entre/);
    await expect(popover(page).getByRole("textbox", { name: "De" })).toBeFocused();
    await page.keyboard.type("60");
    await page.keyboard.press("Enter");
    await expect(chip(page, "prop.dns.ttl à partir de 60")).toBeFocused();
    await expect(query(page)).toContainText("&prop.dns.ttl:between=60..");
  });

  test("a chosen value removed with the keyboard, said; the field after", async ({ page }) => {
    await add(page).focus();
    await page.keyboard.press("Enter");
    await choose(page, "usa", /^Usage/);
    await choose(page, "hô", /^Hôte/);
    const said = popover(page).locator(".k-filter-picker__said");
    await expect(said).toHaveRole("status");
    await expect(said).toHaveText("Valeur ajoutée : Hôte");
    await choose(page, "vi", /^VIP/);
    await page.keyboard.press("Tab");
    const remove = popover(page).getByRole("button", { name: "Retirer Hôte" });
    await expect(remove).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(said).toHaveText("Valeur retirée : Hôte");
    await expect(popover(page).getByRole("button", { name: "Retirer VIP" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(value(page)).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(chip(page, "Usage est VIP")).toBeFocused();
    await expect(query(page)).toContainText("&usage=vip");
  });

  test("the same opener twice closes the picker; another one takes it over", async ({ page }) => {
    await add(page).click();
    await expect(popover(page)).toBeVisible();
    await expect(add(page)).toHaveAttribute("aria-expanded", "true");
    const address = chip(page, "Adresse dans 192.0.2.0/24");
    await address.click();
    await expect(popover(page)).toHaveAccessibleName("Modifier le filtre");
    await expect(address).toHaveAttribute("aria-expanded", "true");
    await expect(add(page)).toHaveAttribute("aria-expanded", "false");
    await expect(value(page)).toBeFocused();
    await address.click();
    await expect(popover(page)).toHaveCount(0);
    await expect(address).toHaveAttribute("aria-expanded", "false");
  });

  test("Tab leaves the bar for the next field", async ({ page }) => {
    await add(page).focus();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("textbox", { name: "Note" })).toBeFocused();
  });
});
