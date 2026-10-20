import { expect, expectAccessible, test } from "../fixtures/index.ts";

// Frame of the shell (spec 007, task 13): navigation by rights (REQ-007-20), language and
// theme from the user menu (REQ-007-29, 30), the about page.

test("a reader does not see the administration", async ({ page, login }) => {
  await login("bob");
  const navigation = page.getByRole("navigation", { name: "Navigation principale" });
  await expect(navigation.getByRole("link", { name: "À propos" })).toBeVisible();
  await expect(navigation.getByRole("link", { name: "Administration" })).toHaveCount(0);
  await expectAccessible(page);
});

test("an administrator reaches the administration", async ({ page, login }) => {
  await login("alice");
  const navigation = page.getByRole("navigation", { name: "Navigation principale" });
  await navigation.getByRole("link", { name: "Administration" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Administration" })).toBeVisible();
  await expectAccessible(page);
});

test("language and theme are chosen from the user menu and kept", async ({ page, login }) => {
  await login("bob");
  await page.getByRole("button", { name: "Bob Exemple, menu de l'utilisateur" }).click();
  await expectAccessible(page);
  await page.getByRole("menuitemradio", { name: "English" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Prefix tree" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("lang", "en");

  await page.getByRole("button", { name: "Bob Exemple, user menu" }).click();
  await page.getByRole("menuitemradio", { name: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expectAccessible(page);

  await page.reload();
  await expect(page.getByRole("heading", { level: 1, name: "Prefix tree" })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});

test("the about page names the versions and the time zone", async ({ page, login }) => {
  await login("bob");
  await page.goto("/about");
  await expect(page.getByRole("heading", { level: 1, name: "À propos" })).toBeVisible();
  await expect(page.getByText("Fuseau horaire affiché")).toBeVisible();
  await expectAccessible(page);
});
