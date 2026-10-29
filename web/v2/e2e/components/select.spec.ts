import { expect, openState, test } from "../fixtures/index.ts";

// Keyboard script of the APG Select-Only Combobox pattern (spec 020, design « Inventaire »
// line 5, « Accessibilité »; REQ-020-16, 18, 28), on Reka's Select: a `combobox` button
// named by its label and showing its value; Down, Up, Enter and Space open the `listbox`
// on the chosen option, which takes the focus; the arrows, Home, End and typing move;
// Enter chooses and closes; Escape closes without choosing; the focus goes back to the
// button. Read-only never opens. The targets are 24 px at least.

test.describe("Select (APG Select-Only Combobox)", () => {
  test.beforeEach(async ({ page }) => {
    await openState(page, "Select", "keyboard");
  });

  test("named by its label, its value shown, closed", async ({ page }) => {
    const usage = page.getByRole("combobox", { name: "Usage" });
    await expect(usage).toHaveText("Hôte");
    await expect(usage).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("listbox")).toHaveCount(0);
  });

  test("Down opens on the chosen option; Down and Enter choose; the focus comes back", async ({
    page,
  }) => {
    const usage = page.getByRole("combobox", { name: "Usage" });
    await usage.focus();
    await page.keyboard.press("ArrowDown");
    const list = page.getByRole("listbox", { name: "Usage" });
    await expect(list).toBeVisible();
    // Open, the rest of the page is hidden from the tree (Reka): the button by its class.
    await expect(page.locator(".k-select__trigger")).toHaveAttribute("aria-expanded", "true");
    await expect(list.getByRole("option", { name: "Hôte" })).toBeFocused();
    await expect(list.getByRole("option", { name: "Hôte" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await page.keyboard.press("ArrowDown");
    await expect(list.getByRole("option", { name: "Passerelle" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(usage).toBeFocused();
    await expect(usage).toHaveText("Passerelle");
    await expect(page.getByRole("status")).toHaveText("Usage choisi : Passerelle");
  });

  test("Escape closes without choosing", async ({ page }) => {
    const usage = page.getByRole("combobox", { name: "Usage" });
    await usage.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("listbox")).toBeVisible();
    await page.keyboard.press("End");
    await expect(page.getByRole("option", { name: "DHCP" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(usage).toBeFocused();
    await expect(usage).toHaveText("Hôte");
  });

  test("Space opens; Home, End and typing move; Enter chooses", async ({ page }) => {
    const usage = page.getByRole("combobox", { name: "Usage" });
    await usage.focus();
    await page.keyboard.press(" ");
    await expect(page.getByRole("listbox")).toBeVisible();
    await page.keyboard.press("End");
    await expect(page.getByRole("option", { name: "DHCP" })).toBeFocused();
    await page.keyboard.press("Home");
    await expect(page.getByRole("option", { name: "Hôte" })).toBeFocused();
    await page.keyboard.press("r");
    await expect(page.getByRole("option", { name: "Réservée" })).toBeFocused();
    // Right after a typed letter, Space continues the search (« r » then « ré… »).
    await page.keyboard.press("Enter");
    await expect(usage).toHaveText("Réservée");
    await expect(usage).toBeFocused();
  });

  test("Tab goes from the button to the next field", async ({ page }) => {
    await page.getByRole("combobox", { name: "Usage" }).focus();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("textbox", { name: "Propriétaire" })).toBeFocused();
  });

  test("Tab in the open list takes the focused option and goes on", async ({ page }) => {
    const usage = page.getByRole("combobox", { name: "Usage" });
    await usage.focus();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: "Hôte" })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    // Reka moves the focus a task later.
    await expect(page.getByRole("option", { name: "Passerelle" })).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("listbox")).toHaveCount(0);
    await expect(page.getByRole("textbox", { name: "Propriétaire" })).toBeFocused();
    await expect(usage).toHaveText("Passerelle");
  });

  test("the snapshot of its tree", async ({ page }) => {
    await expect(page.locator(".k-cat-state__render")).toMatchAriaSnapshot(`
      - text: Usage
      - combobox "Usage": Hôte
      - status: "Usage choisi : Hôte"
      - text: Propriétaire
      - textbox "Propriétaire"
    `);
  });
});

test("Select: read-only keeps its value and never opens", async ({ page }) => {
  await openState(page, "Select", "readonly");
  const usage = page.getByRole("combobox", { name: "Usage" });
  await expect(usage).toHaveAttribute("aria-readonly", "true");
  await usage.focus();
  for (const key of ["ArrowDown", "Enter", " ", "h"]) await page.keyboard.press(key);
  await usage.click();
  await expect(page.getByRole("listbox")).toHaveCount(0);
  await expect(usage).toHaveText("Passerelle");
});

test("Select: the error is linked before the help; disabled is out of the tabs", async ({
  page,
}) => {
  await openState(page, "Select", "invalid");
  const usage = page.getByRole("combobox", { name: /Usage/ });
  await expect(usage).toHaveAttribute("aria-invalid", "true");
  await expect(usage).toHaveAccessibleDescription(
    "L'usage est obligatoire. Liste fermée du contrat.",
  );
  await openState(page, "Select", "disabled");
  await expect(page.getByRole("combobox", { name: "Usage" })).toBeDisabled();
});

test("Select: in a toolbar, named by its label, the label written before the value", async ({
  page,
}) => {
  await openState(page, "Select", "toolbar");
  const flow = page.getByRole("combobox", { name: "Type de flux" });
  await expect(flow).toHaveText(/Type de flux\s*:\s*Interne/);
  await expect(flow).toHaveAccessibleName("Type de flux");
});

for (const densite of ["compacte", "confortable"] as const) {
  test(`Select and Combobox: fields of 24 px at least, ${densite}`, async ({ page }) => {
    for (const component of ["Select", "Combobox"]) {
      await page.goto(`/_ui/c/${component}?densite=${densite}`);
      await page.locator("[data-state-name]").first().waitFor();
      const small = await page.evaluate(() =>
        [...document.querySelectorAll(".k-cat-state__render .k-field-frame")]
          .map((el) => el.getBoundingClientRect())
          .filter((r) => r.height < 23.5)
          .map((r) => `${Math.round(r.width)} × ${Math.round(r.height)}`),
      );
      expect(small, component).toEqual([]);
    }
  });
}
