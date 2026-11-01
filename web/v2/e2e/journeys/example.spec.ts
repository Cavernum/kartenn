import { expect, test } from "../fixtures/journeys.ts";

// The example plugin of web/v2/plugins in the new shell (spec 021, task 16): built, served
// by kartenn-server and loaded by the host, listed loaded in « À propos ». Its elements,
// its shadow root with the sheet of @kartenn/ui (`handle.styles()`) and its reads by the
// `QueryClient` come with its port to the SDK 1.0 (task 19), and this journey with them.

test("the example plugin is loaded by the shell, said in « À propos »", async ({ page, login }) => {
  await login("bob");
  await page.goto("/about");
  const example = page.getByRole("main").getByRole("listitem").filter({ hasText: "example 1.0.0" });
  await expect(example).toContainText("Chargé");
  await expect(example).not.toContainText("Écarté");
});
