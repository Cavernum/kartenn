import type { Locator, Page } from "@playwright/test";
import { BASE_URL } from "../fixtures/env.ts";
import { DATA, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// The example plugin of web/v2/plugins in the new shell (spec 021, task 19; REQ-021-25, 27;
// REQ-007-21), built and installed as an operator would (serve.sh): written without a
// framework on the SDK 1.0, its elements in their shadow root with the sheet of
// @kartenn/ui linked by `handle.styles()`, then its own; its view reads the IPv4 prefixes
// through the shared cache and follows its invalidations — a prefix created elsewhere shows
// up without a reload, in the same element. The screens of the referential (task 11) place
// its tile, column, tab, panel and action.

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

  // Taller than the window (the prefixes of every journey, a window 256 px high: 1.4.10),
  // the surface scrolls: a tab stop, scrolled by the arrows, whatever its page holds
  // (2.1.1; task 029-5).
  const size = page.viewportSize()!;
  await page.setViewportSize({ width: size.width, height: 256 });
  const main = page.locator("#k-surface");
  await expect.poll(() => main.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
  await expect(main).toHaveAttribute("tabindex", "0");
  await expectAccessible(page);
  await page.setViewportSize(size);

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
  // The element and its heading, kept to be found again after the write.
  await view.evaluate((element) => {
    const kept = window as unknown as { __example?: unknown[] };
    kept.__example = [element, element.shadowRoot!.querySelector("h1")];
  });

  // An IPv4 prefix of documentation (RFC 5737) in the domain of the data set, a range per
  // browser (the projects run side by side), written by the API beside the page (the
  // session of the same browser); another value if one is already taken.
  const range = test.info().project.name === "firefox" ? "203.0.113" : "198.51.100";
  const listed = await page.request.get(`/v1/prefixes?value=${encodeURIComponent(DATA.prefix)}`);
  const domain = ((await listed.json()) as { items: { domain: { id: string } }[] }).items[0]!.domain
    .id;
  let value = "";
  let status = 0;
  for (let attempt = 0; attempt < 5 && ![200, 201].includes(status); attempt++) {
    value = `${range}.${4 * Math.floor(Math.random() * 64)}/30`;
    const written = await page.request.post("/v1/changesets?approval=self", {
      headers: { Origin: BASE_URL },
      data: {
        motive: "example plugin journey",
        operations: [
          { op: "create_prefix", domain: { id: domain }, prefix: value, nature: "network" },
        ],
      },
    });
    status = written.status();
  }
  expect([200, 201]).toContain(status);

  // The event of the stream invalidates the list; the observer of the plugin reads it again,
  // in the same element, its page kept around the list.
  await expect(row(view, value)).toBeVisible();
  await expect(row(view, DATA.prefix)).toBeVisible();
  const same = await view.evaluate((element) => {
    const [kept, heading] = (window as unknown as { __example: unknown[] }).__example;
    return element === kept && element.shadowRoot!.querySelector("h1") === heading;
  });
  expect(same).toBe(true);
});

test("the screens of the referential place its tile, column, tab, panel and action", async ({
  page,
  login,
}) => {
  // Alice may edit addresses: the action shows in the menu of the record.
  await login("alice");

  // The tile of the home page: the counters of the referential, read by the shared cache.
  await expect(page.locator("example-counts")).toContainText("Préfixes");

  // The column of the addresses: an element of the plugin in each row, given the row.
  await page.goto("/addresses");
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("columnheader", { name: /Usage \(exemple\)/ })).toBeAttached();
  await expect(grid.locator("example-usage").first()).toHaveText(/^(réservée|autre)$/);

  // The panel and the tab of a prefix, each given the object.
  await page.goto("/prefixes");
  await page
    .getByRole("grid", { name: "Préfixes" })
    .getByRole("link", { name: DATA.prefix })
    .click();
  await expect(
    page.getByRole("heading", {
      level: 1,
      name: new RegExp(`\\s${DATA.prefix.replaceAll(".", "\\.")}$`),
    }),
  ).toBeVisible();
  const id = page.url().split("/").pop()!;
  await expect(page.locator("example-summary")).toContainText(id);
  await page
    .getByRole("tablist", { name: "Sections de la fiche" })
    .getByRole("tab", { name: "Exemple" })
    .click();
  const details = page.locator("example-details");
  await expect(details).toContainText(DATA.prefix);
  await expect(details).toContainText(DATA.domain);
  await expectAccessible(page);

  // The action: its form in the dialog of the shell, an operation added to a draft of the
  // server, no changeset sent.
  const changesets: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().includes("/v1/changesets")) {
      changesets.push(request.url());
    }
  });
  await page.getByRole("button", { name: "Actions" }).click();
  await page.getByRole("menuitem", { name: "Ajouter une adresse au brouillon" }).click();
  const dialog = page.getByRole("dialog", { name: "Ajouter une adresse au brouillon" });
  const form = dialog.locator("example-draft-address");
  await form.getByRole("textbox", { name: /^Adresse/ }).fill("192.0.2.77");
  await form.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(form.getByText("Opération ajoutée ; le brouillon en compte 1.")).toBeVisible();
  await expectAccessible(page);
  expect(changesets).toEqual([]);
  // Its own button closes it (`kartenn-close`), beside the cross of the dialog.
  await form.getByRole("button", { name: "Fermer" }).click();
  await expect(dialog).toHaveCount(0);
});
