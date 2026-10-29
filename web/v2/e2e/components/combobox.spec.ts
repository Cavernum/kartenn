import type { Locator, Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Combobox pattern, list autocomplete with manual selection
// (spec 020, design « Inventaire » line 5, « Accessibilité »; REQ-020-16, 18, 28): the
// focus stays in the field, the highlighted suggestion is its `aria-activedescendant`;
// the suggestions come from the host's function once the typing pauses (150 ms); Alt+Down
// opens without moving, Down goes to the first suggestion, Enter takes the highlighted
// one, Escape closes and leaves the next Escape to the layer around.

const field = (page: Page) => page.getByRole("combobox", { name: "Groupe" });
const list = (page: Page) => page.getByRole("listbox", { name: "Groupe" });

/** The option the field points to by `aria-activedescendant`, or null. */
async function active(input: Locator): Promise<string | null> {
  return input.evaluate((el) => {
    const id = el.getAttribute("aria-activedescendant");
    return id ? (document.getElementById(id)?.textContent?.trim() ?? `#${id} (absent)`) : null;
  });
}

test.describe("Combobox (APG Combobox)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Combobox", "keyboard");
  });

  test("named by its label, closed at rest, a list autocomplete", async ({ page }) => {
    const input = field(page);
    await expect(input).toHaveAttribute("aria-autocomplete", "list");
    await expect(input).toHaveAttribute("aria-expanded", "false");
    await expect(input).not.toHaveAttribute("aria-activedescendant");
    await expect(page.getByRole("listbox")).toHaveCount(0);
  });

  test("typing waits for a pause: one call, the suggestions, none highlighted", async ({
    page,
  }) => {
    const input = field(page);
    await input.focus();
    await input.pressSequentially("camp", { delay: 20 });
    await expect(list(page).getByRole("option")).toHaveCount(3);
    await expect(page.locator("[data-calls]")).toHaveText("Appels : 1");
    await expect(input).toHaveAttribute("aria-expanded", "true");
    await expect(input).toHaveAttribute("aria-controls", /.+/);
    expect(await active(input)).toBeNull();
    await expect(page.getByRole("status").first()).toHaveText("3 suggestions");
    await expect(input).toBeFocused();
  });

  test("Alt+Down opens without moving; Down then goes to the first", async ({ page }) => {
    const input = field(page);
    await input.focus();
    await page.keyboard.press("Alt+ArrowDown");
    await expect(list(page).getByRole("option")).toHaveCount(4);
    await expect(input).toHaveAttribute("aria-expanded", "true");
    expect(await active(input)).toBeNull();
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => active(input)).toContain("campus-ouest");
    await expect(input).toBeFocused();
  });

  test("Down on a closed field opens it on its first suggestion; Up and Down move", async ({
    page,
  }) => {
    const input = field(page);
    await input.focus();
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => active(input)).toContain("campus-ouest");
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => active(input)).toContain("campus-est");
    // The highlighted suggestion is the selected one of the list, and it alone.
    const options = list(page).getByRole("option");
    await expect(options.filter({ hasText: "campus-est" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(options.and(page.locator("[aria-selected='true']"))).toHaveCount(1);
    await page.keyboard.press("ArrowUp");
    await expect.poll(() => active(input)).toContain("campus-ouest");
    await expect(input).toBeFocused();
  });

  test("Enter takes the highlighted suggestion and closes the list", async ({ page }) => {
    const input = field(page);
    await input.focus();
    await input.pressSequentially("camp", { delay: 20 });
    await expect(list(page).getByRole("option")).toHaveCount(3);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => active(input)).toContain("campus-est");
    await page.keyboard.press("Enter");
    await expect(input).toHaveValue("campus-est");
    await expect(page.getByRole("status").last()).toHaveText("Groupe choisi : campus-est");
    await expect(input).toHaveAttribute("aria-expanded", "false");
    await expect.poll(() => active(input)).toBeNull();
    await expect(input).toBeFocused();
  });

  test("Enter without a highlighted suggestion keeps the text (the host's)", async ({ page }) => {
    const input = field(page);
    await input.focus();
    await input.pressSequentially("camp", { delay: 20 });
    await expect(list(page).getByRole("option")).toHaveCount(3);
    await page.keyboard.press("Enter");
    await expect(page.getByRole("status").last()).toHaveText("Texte gardé : camp");
    await expect(input).toHaveValue("camp");
    await expect(input).toHaveAttribute("aria-expanded", "false");
  });

  test("Escape closes and keeps the text; the next one goes to the layer around", async ({
    page,
  }) => {
    const input = field(page);
    await input.focus();
    await input.pressSequentially("camp", { delay: 20 });
    await expect(list(page).getByRole("option")).toHaveCount(3);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Escape");
    await expect(input).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(input).toHaveValue("camp");
    await expect.poll(() => active(input)).toBeNull();
    await page.evaluate(() =>
      window.addEventListener("keydown", (event) => {
        (window as unknown as { kEscape: boolean }).kEscape = event.defaultPrevented;
      }),
    );
    await page.keyboard.press("Escape");
    expect(await page.evaluate(() => (window as unknown as { kEscape: boolean }).kEscape)).toBe(
      false,
    );
  });

  test("Home and End stay in the text", async ({ page }) => {
    const input = field(page);
    await input.focus();
    await input.pressSequentially("campus", { delay: 20 });
    await expect(list(page).getByRole("option")).toHaveCount(3);
    await page.keyboard.press("Home");
    expect(await input.evaluate((el: HTMLInputElement) => el.selectionStart)).toBe(0);
    await page.keyboard.press("End");
    expect(await input.evaluate((el: HTMLInputElement) => el.selectionStart)).toBe(6);
    expect(await active(input)).toBeNull();
  });

  test("Tab leaves the field and closes the list", async ({ page }) => {
    const input = field(page);
    await input.focus();
    await page.keyboard.press("Alt+ArrowDown");
    await expect(list(page)).toBeVisible();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("textbox", { name: "Propriétaire" })).toBeFocused();
    await expect(page.getByRole("listbox")).toHaveCount(0);
  });

  test("the snapshot of its tree, list open", async ({ page }) => {
    await field(page).focus();
    await page.keyboard.press("Alt+ArrowDown");
    await expect(list(page).getByRole("option")).toHaveCount(4);
    await expect(page.locator("main")).toMatchAriaSnapshot(`
      - listbox "Groupe":
        - option "campus-ouest 100–199"
        - option "campus-est 200–299"
        - option "campus-nord 300–399"
        - option "labo 900–949"
      - region "Combobox":
        - heading "Combobox" [level=1]
        - paragraph
        - figure:
          - text: Groupe
          - combobox "Groupe" [expanded]
          - status: 4 suggestions
          - paragraph: "Appels : 1"
          - status: Aucun groupe choisi
          - text: Propriétaire
          - textbox "Propriétaire"
          - code: keyboard
          - text: /Au clavier/
    `);
  });
});

for (const [state, said] of [
  ["loading", "Recherche des suggestions"],
  ["empty", "Aucun résultat"],
  ["failed", "Suggestions indisponibles"],
] as const) {
  test(`Combobox: a list without suggestion says it (${state})`, async ({ page }) => {
    await openState(page, "Combobox", state);
    const input = field(page);
    await expect(input).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("status")).toHaveText(said);
    await expect(list(page).getByRole("option")).toHaveCount(0);
    await expect(page.locator(".k-list__note")).toHaveText(said);
  });
}

test("Combobox: the error is linked before the help; disabled is out of the tabs", async ({
  page,
}) => {
  await openState(page, "Combobox", "invalid");
  const input = field(page);
  await expect(input).toHaveAttribute("aria-invalid", "true");
  await expect(input).toHaveAccessibleDescription(
    "Aucun groupe dmz sur ce site. Groupes ancrés sur le site du réseau.",
  );
  await openState(page, "Combobox", "disabled");
  await expect(field(page)).toBeDisabled();
});
