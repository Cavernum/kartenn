import type { Locator, Page } from "@playwright/test";
import { BASE_URL } from "../fixtures/env.ts";
import { DATA, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// The example plugin of web/v2/plugins in the new shell (spec 021, task 19; REQ-021-25, 27;
// REQ-007-21), built and installed as an operator would (serve.sh): written without a
// framework on the SDK 1.0, its elements in their shadow root with the sheet of
// @kartenn/ui linked by `handle.styles()`, then its own; its view reads the prefixes
// through the shared cache and follows its invalidations — a prefix created elsewhere shows
// up without a reload. Its tab, panel, column, action and tile wait for the screens that
// place them (tasks 11, 14): defined here, drawn by plugins/example/test/example.test.ts.

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });

/** The elements its manifest declares, one of each point. */
const ELEMENTS = [
  "example-overview",
  "example-details",
  "example-summary",
  "example-usage",
  "example-draft-address",
  "example-counts",
];

/** A prefix of the list of the view, by its value. */
const row = (view: Locator, value: string) => view.getByRole("listitem").filter({ hasText: value });

test("its view opens from the sidebar, in a shadow root styled by the sheet of @kartenn/ui", async ({
  page,
  login,
}) => {
  await login("bob");
  await page.goto("/about");
  const example = page.getByRole("main").getByRole("listitem").filter({ hasText: "example 1.0.0" });
  await expect(example).toContainText("Chargé");
  await expect(example).not.toContainText("Écarté");
  const defined = await page.evaluate(
    (tags) => tags.filter((tag) => customElements.get(tag) !== undefined),
    ELEMENTS,
  );
  expect(defined).toEqual(ELEMENTS);

  const nav = navigation(page);
  await nav.getByRole("button", { name: "Référentiel" }).click();
  await nav.getByRole("link", { name: "Exemple" }).click();
  await expect(page).toHaveURL(/\/x\/example\/overview$/);
  // The element writes the one heading of the page.
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Plugin d'exemple");
  const view = page.locator("example-overview");
  await expect(view.getByText("Vous regardez le référentiel au présent.")).toBeVisible();
  await expect(row(view, DATA.prefix)).toContainText(DATA.domain);
  await expect(view.getByRole("region", { name: "Préfixes" })).toBeVisible();

  // The sheet of @kartenn/ui (the shell's link, its digest) then the plugin's, in that order;
  // a class of the library (`k-stack`) and one of the plugin both apply in the shadow root.
  const styles = await view.evaluate((element) => {
    const root = element.shadowRoot!;
    const shared = document.querySelector<HTMLLinkElement>(
      'link[data-kartenn-sheet="@kartenn/ui/style.css"]',
    );
    return {
      links: [...root.querySelectorAll("link")].map((link) => [link.href, link.integrity]),
      shared: [shared?.href, shared?.integrity],
      stack: getComputedStyle(root.querySelector(".k-stack")!).display,
      title: getComputedStyle(root.querySelector(".k-example-title")!).fontWeight,
    };
  });
  expect(styles.links).toEqual([
    styles.shared,
    [`${BASE_URL}/plugins/example/1.0.0/styles.css`, expect.stringMatching(/^sha384-/)],
  ]);
  expect(styles).toMatchObject({ stack: "flex", title: "600" });
  await expectAccessible(page);

  // The tokens cross the shadow root: the dark theme reaches the plugin's surfaces.
  const surface = () =>
    view.evaluate(
      (element) =>
        getComputedStyle(element.shadowRoot!.querySelector(".k-example-section")!).backgroundColor,
    );
  const light = await surface();
  await page.emulateMedia({ colorScheme: "dark" });
  await expect.poll(surface).not.toBe(light);
  await expectAccessible(page);
});

test("its view follows the shared cache: a prefix created elsewhere shows up without a reload", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto("/x/example/overview");
  const view = page.locator("example-overview");
  await expect(row(view, DATA.prefix)).toBeVisible();

  // A prefix of documentation (RFC 3849) that no other run has created, in the domain of
  // the data set, written by the API beside the page (the session of the same browser).
  const group = () => (0x1000 + Math.floor(Math.random() * 0xefff)).toString(16);
  const value = `2001:db8:${group()}:${group()}::/64`;
  const listed = await page.request.get(`/v1/prefixes?value=${encodeURIComponent(DATA.prefix)}`);
  const domain = ((await listed.json()) as { items: { domain: { id: string } }[] }).items[0]!.domain
    .id;
  const written = await page.request.post("/v1/changesets?approval=self", {
    headers: { Origin: BASE_URL },
    data: {
      motive: "example plugin journey",
      operations: [
        { op: "create_prefix", domain: { id: domain }, prefix: value, nature: "network" },
      ],
    },
  });
  expect([200, 201], await written.text()).toContain(written.status());

  // The event of the stream invalidates the list; the observer of the plugin reads it again.
  await expect(row(view, value)).toBeVisible();
  await expect(row(view, DATA.prefix)).toBeVisible();
});
