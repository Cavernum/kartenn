import { expect, openState, test } from "../fixtures/index.ts";

// Script of InlineAlert (spec 020, design « Inventaire » line 26; REQ-020-7, 22, 28): an
// error is an `alert`, the other tones a `status`; an alert that shows is said: its region
// comes first, its content hidden (out of the accessibility tree), then shown a frame
// later in the same element (a region inserted with its text is not announced).

test("its region comes before its content, which then shows in it", async ({ page }) => {
  await openState(page, "InlineAlert", "announce");
  // Every alert added to the page, as it was at its insertion.
  await page.evaluate(() => {
    const seen: { waiting: string | null; text: string }[] = [];
    (window as unknown as { kSeen: typeof seen }).kSeen = seen;
    new MutationObserver((records) => {
      for (const record of records)
        for (const node of record.addedNodes)
          if (node instanceof HTMLElement && node.matches(".k-inline-alert"))
            seen.push({
              waiting: node.getAttribute("data-state"),
              text: node.innerText.trim(),
            });
    }).observe(document.body, { childList: true, subtree: true });
  });
  await page.getByRole("button", { name: "Appliquer le changeset" }).click();
  const alert = page.getByRole("alert");
  await expect(alert).toContainText("Le changeset est refusé.");
  await expect(alert).toContainText("prefix.overlaps-sibling");
  await expect(alert).not.toHaveAttribute("data-state");
  // Inserted waiting, its text hidden (innerText leaves out what is not rendered).
  expect(await page.evaluate(() => (window as unknown as { kSeen: unknown }).kSeen)).toEqual([
    { waiting: "waiting", text: "" },
  ]);
  // Its code is copyable, by the keyboard.
  await page.keyboard.press("Tab");
  await expect(alert.getByRole("button", { name: /prefix\.overlaps-sibling/ })).toBeFocused();
});

test("roles: alert for an error, status for the other tones, the icon one included", async ({
  page,
}) => {
  await openState(page, "InlineAlert", "ko");
  await expect(page.getByRole("alert")).toContainText("Le changeset est refusé.");
  await openState(page, "InlineAlert", "icon");
  await expect(page.getByRole("status")).toContainText("Groupe livré : lecture seule.");
  await expect(page.getByRole("alert")).toHaveCount(0);
});
