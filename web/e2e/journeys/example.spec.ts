import type { Page } from "@playwright/test";
import { expect, expectAccessible, test } from "../fixtures/index.ts";

// The example plugin of web/plugins/example (spec 007, task 23; REQ-007-6, 11, 21), built
// and installed as an operator would (serve.sh): each extension point shows where its
// manifest declares it, the elements follow the dark mode through the design tokens, and
// the action adds an operation to the draft.

/** The identifier of the object of `collection` whose value is `value`, read by the page. */
async function idOf(page: Page, collection: string, value: string): Promise<string> {
  const items = await page.evaluate(
    async (url) => {
      const response = await fetch(url);
      return ((await response.json()) as { items: { id: string }[] }).items;
    },
    `/v1/${collection}?value=${encodeURIComponent(value)}`,
  );
  expect(items).toHaveLength(1);
  return items[0]!.id;
}

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

test("each extension point shows where the manifest declares it", async ({ page, login }) => {
  await login("bob");
  await page.goto("/about");
  await expect(
    page.getByRole("main").getByRole("listitem").filter({ hasText: "example 1.0.0" }),
  ).toContainText("chargé");

  // The view, from the navigation.
  await navigation(page).getByRole("link", { name: "Exemple" }).click();
  await expect(page).toHaveURL(/\/x\/example\/overview$/);
  await expect(page.getByRole("heading", { level: 1, name: "Plugin d'exemple" })).toBeVisible();
  await expect(page.getByText("Vous regardez le référentiel au présent.")).toBeVisible();
  await expectAccessible(page);

  // The tab and the panel of a prefix; the tab reads it through the SDK client.
  const prefix = await idOf(page, "prefixes", "10.1.0.0/24");
  await page.goto(`/prefixes/${prefix}`);
  const tabs = page.getByRole("tablist", { name: "Sections de la fiche" });
  await tabs.getByRole("tab", { name: "Exemple" }).click();
  await expect(page.getByText(/^Préfixe 10\.1\.0\.0\/24, domaine /)).toBeVisible();
  await expect(page.getByRole("heading", { name: "Résumé de l'exemple" })).toBeVisible();
  await expectAccessible(page);

  // The panel also shows on an address, the tab does not.
  const address = await idOf(page, "addresses", "10.1.0.10");
  await page.goto(`/addresses/${address}`);
  await expect(page.getByRole("heading", { name: "Résumé de l'exemple" })).toBeVisible();
  await expect(tabs.getByRole("tab", { name: "Exemple" })).toHaveCount(0);

  // The column, in the list of addresses only.
  await page.goto("/addresses");
  const table = page.getByRole("table");
  await expect(table.getByRole("columnheader", { name: "Usage (exemple)" })).toBeVisible();
  await expect(table.locator("example-usage").first()).toBeAttached();
  await expectAccessible(page);
  await page.goto("/prefixes");
  await expect(page.getByRole("columnheader", { name: "Usage (exemple)" })).toHaveCount(0);
});

test("its elements follow the dark mode through the design tokens", async ({ page, login }) => {
  await login("bob");
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/x/example/overview");
  await expect(page.getByRole("heading", { level: 1, name: "Plugin d'exemple" })).toBeVisible();
  const colors = async () =>
    page.evaluate(() => {
      const card = document.querySelector("example-overview")?.shadowRoot?.querySelector(".card");
      if (!card) return null;
      const style = getComputedStyle(card);
      return { background: style.backgroundColor, text: style.color };
    });
  // The sheet comes through styles(), with its digest: the dark tokens of the shell.
  await expect.poll(colors).toEqual({ background: "rgb(27, 32, 39)", text: "rgb(232, 234, 237)" });
  await expectAccessible(page);

  await page.emulateMedia({ colorScheme: "light" });
  await expect.poll(colors).toEqual({ background: "rgb(244, 245, 247)", text: "rgb(27, 31, 36)" });
});

test("its action adds an operation to the draft without sending it", async ({ page, login }) => {
  await login("alice");
  const prefix = await idOf(page, "prefixes", "10.1.0.0/24");
  await page.goto(`/prefixes/${prefix}`);
  await page.getByRole("button", { name: "Actions" }).click();
  await page.getByRole("menuitem", { name: "Ajouter une adresse au brouillon" }).click();
  const dialog = page.getByRole("dialog", { name: "Ajouter une adresse au brouillon" });
  const writes: string[] = [];
  // Simulations of the draft may leave (REQ-007-26), never a submission.
  page.on("request", (request) => {
    if (request.method() !== "GET" && !request.url().includes("simulate=true"))
      writes.push(`${request.method()} ${request.url()}`);
  });
  await dialog.getByRole("textbox", { name: "Adresse" }).fill("10.1.0.200");
  await dialog.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(dialog.getByText("Opération ajoutée ; le brouillon en compte 1.")).toBeVisible();
  await expectAccessible(page);
  expect(writes).toEqual([]);
  await dialog.getByRole("button", { name: "Fermer" }).click();
  await expect(dialog).toHaveCount(0);

  // The draft of the SDK is shared: the view of the plugin counts it.
  await navigation(page).getByRole("link", { name: "Exemple" }).click();
  await expect(page.getByText("Opérations dans le brouillon : 1")).toBeVisible();
});
