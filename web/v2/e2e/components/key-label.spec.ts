import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Script of KeyLabel and EnvChip (spec 020, design « Inventaire » line 23; REQ-020-8, 21):
// the label in the language, else the key in fixed pitch (`prod`: « Production » in
// French, `prod` in English); the key in the bubble under the focus, in the description,
// and copied by Enter, said by a polite region; EnvChip the same (forced colours:
// forced-colors.spec.ts).

const bubble = (page: Page) => page.locator(".k-tooltip:not([hidden])");
const clipboard = (page: Page) => page.evaluate(() => navigator.clipboard.readText());

test("prod: its label in French, its key in English; copyable in both", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/_ui/c/KeyLabel?etat=fallback");
  const production = page.getByRole("button", { name: "Production", exact: true });
  await expect(production).toHaveAccessibleDescription(
    "Clé : prod. Clic, Entrée ou Espace : copier la clé.",
  );
  await production.focus();
  await expect(bubble(page)).toContainText("Clé : prod");
  await page.keyboard.press("Enter");
  await expect(page.locator(".k-key-label").first().getByRole("status")).toHaveText("Clé copiée");
  await expect(bubble(page)).toContainText("Clé copiée");
  expect(await clipboard(page)).toBe("prod");

  await page.goto("/_ui/c/KeyLabel?etat=fallback&langue=en");
  const key = page.getByRole("button", { name: "prod", exact: true });
  // No label in English: the key, in fixed pitch, is the name; nothing is empty.
  await expect(key.locator("code")).toHaveText("prod");
  await expect(key).toHaveAccessibleDescription("Click, Enter or Space: copy the key.");
  await page.evaluate(() => navigator.clipboard.writeText(""));
  await key.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator(".k-key-label").first().getByRole("status")).toHaveText("Key copied");
  expect(await clipboard(page)).toBe("prod");
});

test("the bubble under the keyboard focus; Escape closes it, the focus stays", async ({ page }) => {
  await openState(page, "KeyLabel", "labelled");
  const label = page.getByRole("button", { name: "Centre de données nord" });
  await label.focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  await expect(label).toBeFocused();
  await expect(bubble(page)).toContainText("Clé : dc-nord");
  await expect(bubble(page)).toContainText("Clic, Entrée ou Espace : copier la clé.");
  await page.keyboard.press("Escape");
  await expect(bubble(page)).toHaveCount(0);
  await expect(label).toBeFocused();
});

test("plain: the text alone, no button", async ({ page }) => {
  await openState(page, "KeyLabel", "plain");
  await expect(page.locator("[data-state-name] .k-key-label")).toHaveText("Centre de données nord");
  await expect(page.locator("[data-state-name] .k-key-label button")).toHaveCount(0);
});

test("EnvChip: a button that copies its key", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openState(page, "EnvChip", "labelled");
  const chip = page.getByRole("button", { name: "Production" });
  await expect(chip).toHaveAccessibleDescription(
    "Clé : prod. Clic, Entrée ou Espace : copier la clé.",
  );
  await chip.focus();
  await page.keyboard.press("Space");
  await expect(page.locator(".k-env-chip-host").getByRole("status")).toHaveText("Clé copiée");
  expect(await clipboard(page)).toBe("prod");
});
