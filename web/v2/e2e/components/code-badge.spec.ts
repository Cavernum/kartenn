import type { Page } from "@playwright/test";
import { expect, openState, test } from "../fixtures/index.ts";

// Script of CodeBadge (spec 020, design « Inventaire » line 22; REQ-020-22): a code is
// copied from the keyboard (Enter, Space), the copy said by a polite `status` region and
// shown in the bubble of the button; without the asynchronous Clipboard API (or refused),
// the copy command of the document takes over; both refused, the code is selected and a
// visible message says how to copy it by hand. A code cut by the width shows whole in the
// bubble under the keyboard focus.

const CODE = "urn:kartenn:problem:precondition-failed";
const REQUEST = "req_7f3a9c2e5b1d4068";

/** Any open bubble (decorative: out of the accessibility tree). */
const bubble = (page: Page) => page.locator(".k-tooltip:not([hidden])");
const clipboard = (page: Page) => page.evaluate(() => navigator.clipboard.readText());

/** The asynchronous Clipboard API refuses; with `command`, the copy command too. */
async function refuse(page: Page, command: boolean): Promise<void> {
  await page.addInitScript((both) => {
    Object.defineProperty(navigator.clipboard, "writeText", {
      value: () => Promise.reject(new DOMException("Refused", "NotAllowedError")),
    });
    if (both) Object.defineProperty(document, "execCommand", { value: () => false });
  }, command);
}

test("copied from the keyboard, said and shown; Space too", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openState(page, "CodeBadge", "keyboard");
  const code = page.getByRole("button", { name: `Copier ${CODE}` });
  const request = page.getByRole("button", { name: `Copier Request-Id ${REQUEST}` });
  await code.focus();
  await page.keyboard.press("Enter");
  // The region next to the button says it (polite), and the bubble shows it.
  const said = page.locator(".k-code-badge").first().getByRole("status");
  await expect(said).toHaveText("Copié");
  await expect(bubble(page)).toContainText("Copié");
  expect(await clipboard(page)).toBe(CODE);
  await page.keyboard.press("Tab");
  await expect(request).toBeFocused();
  await page.keyboard.press("Space");
  await expect(page.locator(".k-code-badge").nth(1).getByRole("status")).toHaveText("Copié");
  expect(await clipboard(page)).toBe(REQUEST);
  // « Copié » goes after a while.
  await expect(bubble(page)).toHaveCount(0, { timeout: 4000 });
});

test("without the Clipboard API, the copy command takes over", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await refuse(page, false);
  await openState(page, "CodeBadge", "keyboard");
  await page.getByRole("button", { name: `Copier ${CODE}` }).focus();
  await page.keyboard.press("Enter");
  await expect(page.locator(".k-code-badge").first().getByRole("status")).toHaveText("Copié");
  expect(await clipboard(page)).toBe(CODE);
});

test("both refused: the code selected, the way to copy it shown and said", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await refuse(page, true);
  await openState(page, "CodeBadge", "keyboard");
  const code = page.getByRole("button", { name: `Copier ${CODE}` });
  await code.focus();
  await page.keyboard.press("Enter");
  const message =
    "Copie impossible : le texte est sélectionné, copiez-le au clavier (Ctrl+C, ⌘C sur Mac).";
  await expect(page.locator(".k-code-badge").first().getByRole("status")).toHaveText(message);
  await expect(bubble(page)).toHaveText(message);
  await expect(code).toHaveAttribute("data-state", "failed");
  expect(await page.evaluate(() => window.getSelection()?.toString())).toBe(CODE);
  // The keys of the system copy the selection, the focus still on the button.
  await page.keyboard.press("ControlOrMeta+C");
  expect(await clipboard(page)).toBe(CODE);
  // Escape closes the message (WCAG 1.4.13), the focus stays.
  await page.keyboard.press("Escape");
  await expect(bubble(page)).toHaveCount(0);
  await expect(code).toBeFocused();
  await expect(code).not.toHaveAttribute("data-state", "failed");
});

test("a cut code shows whole in its bubble under the focus; Escape closes it", async ({ page }) => {
  await openState(page, "CodeBadge", "cut");
  const code = page.getByRole("button", { name: `Copier ${CODE}` });
  await code.focus();
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Tab");
  await expect(code).toBeFocused();
  await expect(bubble(page)).toHaveText(CODE);
  await page.keyboard.press("Escape");
  await expect(bubble(page)).toHaveCount(0);
  await expect(code).toBeFocused();
});

test("an error 412: its sentence said, its code and request identifier copied by keys", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openState(page, "ErrorState", "precondition");
  const state = page.locator("[data-state-name='precondition']");
  // The sentence is the alert, shown once its region is in the tree.
  await expect(state.getByRole("alert")).toHaveText(/Ce préfixe a changé depuis votre lecture/);
  await expect(state.locator(".k-error-state__title")).not.toHaveAttribute("data-state");
  const code = state.getByRole("button", { name: `Copier type ${CODE}` });
  const request = state.getByRole("button", { name: `Copier Request-Id ${REQUEST}` });
  await expect(code).toBeVisible();
  await expect(request).toBeVisible();
  await code.focus();
  await page.keyboard.press("Enter");
  await expect(state.locator(".k-code-badge").first().getByRole("status")).toHaveText("Copié");
  expect(await clipboard(page)).toBe(CODE);
  await page.keyboard.press("Tab");
  await expect(request).toBeFocused();
  await page.keyboard.press("Enter");
  expect(await clipboard(page)).toBe(REQUEST);
  await page.keyboard.press("Tab");
  await expect(state.getByRole("button", { name: "Relire" })).toBeFocused();
});

test("a whole code has no bubble at rest", async ({ page }) => {
  await openState(page, "CodeBadge", "keyboard");
  await page.getByRole("button", { name: `Copier ${CODE}` }).focus();
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: `Copier Request-Id ${REQUEST}` })).toBeFocused();
  await expect(bubble(page)).toHaveCount(0);
});
