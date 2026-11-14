import type { Locator, Page } from "@playwright/test";
import {
  apiCall,
  DATA,
  expect,
  expectAccessible,
  loginIn,
  ownedDomain,
  test,
  USERS,
} from "../fixtures/journeys.ts";

// Lists and records of the new shell (spec 021, task 11; REQ-021-9, 18, 22), ported from
// web/e2e/journeys/records.spec.ts: from a domain to a prefix to an address, its inherited
// property explained, its history; a long list virtualised, its next page read ahead and a
// row opened at the keyboard; past ten pages, each page read shown and none read ahead of
// the view (021-22f); a refused cursor starts the list again; a filtered link
// copied opens the same list; the filter of a column is the bar's; the history of a network
// renumbered, each version with its motive and its author (task 11p5).

const navigation = (page: Page) => page.getByRole("navigation", { name: "Navigation principale" });
const tabs = (page: Page) => page.getByRole("tablist", { name: "Sections de la fiche" });
/** The title of a record: its kind in words (the dictionary's), then the object's name. */
const titled = (name: string) => new RegExp(`\\s${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);
/** To the end of a list: the grid scrolls in itself once virtualised, the surface before. */
const toEnd = (grid: Locator) =>
  grid.evaluate((element) => {
    element.scrollTo(0, element.scrollHeight);
    document.getElementById("k-surface")?.scrollTo(0, 1e9);
  });
const toStart = (grid: Locator) =>
  grid.evaluate((element) => {
    element.scrollTo(0, 0);
    document.getElementById("k-surface")?.scrollTo(0, 0);
  });

test("from a domain to a prefix to an address, its property explained, its history", async ({
  page,
  login,
}) => {
  await login("alice");
  await navigation(page).getByRole("link", { name: "Domaines" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Domaines" })).toBeVisible();
  const domains = page.getByRole("grid", { name: "Domaines" });
  await expect(domains.getByRole("link", { name: DATA.domain, exact: true })).toBeVisible();
  await expectAccessible(page);
  await domains.getByRole("link", { name: DATA.domain, exact: true }).click();

  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.domain) })).toBeVisible();
  // The breadcrumb names the object once read, not its identifier.
  await expect(page.getByRole("navigation", { name: /fil d'Ariane/i })).toContainText(DATA.domain);
  await tabs(page).getByRole("tab", { name: "Contenu" }).click();
  const gaps = page.waitForResponse(/\/v1\/property-gaps\?object=[0-9a-f-]+$/);
  await page.getByRole("link", { name: DATA.prefix }).click();

  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.prefix) })).toBeVisible();
  await expect(page.getByRole("region", { name: DATA.prefix })).toContainText("sur 254");
  // Its owner inherited from the domain, said in the column « Source » with the domain named,
  // a link to it (task 021-11p4); the prefix lacks no expected value: no notice of gaps.
  const owner = page.getByRole("row", { name: /core\.owner/ });
  await expect(owner.getByRole("gridcell").filter({ hasText: "Héritée de" })).toContainText(
    DATA.domain,
  );
  await expect(owner.getByRole("link", { name: DATA.domain })).toBeVisible();
  expect((await gaps).status()).toBe(200);
  await expect(page.getByRole("status").filter({ hasText: "Une propriété attendue" })).toHaveCount(
    0,
  );
  await expectAccessible(page);
  await tabs(page).getByRole("tab", { name: "Contenu" }).click();
  await page.getByRole("link", { name: DATA.address }).click();

  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.address) })).toBeVisible();
  // The properties first: core.owner, inherited from the domain through the prefix.
  await page
    .getByRole("row", { name: /core\.owner/ })
    .getByRole("button", { name: /^Expliquer la valeur/ })
    .click();
  const explanation = page.getByRole("dialog", { name: /d'où vient la valeur/ });
  await expect(explanation).toContainText("Héritée de");
  // The origin, named in the sentence, then again at the end of the chain.
  await expect(explanation.getByRole("link", { name: DATA.domain }).first()).toBeVisible();
  const chain = explanation.getByRole("listitem");
  await expect(chain.first()).toContainText(DATA.address);
  await expect(chain.filter({ hasText: DATA.prefix })).toHaveCount(1);
  await expect(chain.last()).toContainText(DATA.domain);
  await expectAccessible(page);
  await page.keyboard.press("Escape");
  await expect(explanation).toBeHidden();

  await tabs(page).getByRole("tab", { name: "Historique" }).click();
  await expect(page).toHaveURL(/[?&]tab=history/);
  const history = page.getByRole("grid", { name: `Versions de ${DATA.address}` });
  await expect(history.getByRole("row")).toHaveCount(2);
  await expect(history.getByRole("gridcell", { name: DATA.address })).toBeVisible();
  await expectAccessible(page);
});

test("a long list keeps a few dozen rows, reads ahead, and opens a row by keyboard", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto(`/addresses?within=${encodeURIComponent(DATA.bulk.prefix)}`);
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("link", { name: "2001:db8:2::1", exact: true })).toBeVisible();
  await expectAccessible(page);

  // Scroll to the end, page after page, until the whole list is read.
  await expect(async () => {
    await toEnd(grid);
    // The count in the words of the language (« 1 000 », a narrow space in French).
    await expect(page.getByText(/^1\s000 éléments$/)).toBeVisible({
      timeout: 500,
    });
  }).toPass({ timeout: 30_000 });
  expect(await grid.getAttribute("aria-rowcount")).toBe(String(DATA.bulk.count + 1));
  const rendered = await grid.locator("[role='row'][aria-rowindex]").count();
  expect(rendered).toBeGreaterThan(5);
  expect(rendered).toBeLessThan(150);

  // Keyboard: back to the first row, down, Enter into its link, Enter follows it.
  await toStart(grid);
  // The usage of the first row (a plain cell), down, then Home to the key of the second.
  await grid.getByRole("row").nth(1).getByRole("gridcell").nth(1).click();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Home");
  await page.keyboard.press("Enter");
  await expect(grid.getByRole("link", { name: "2001:db8:2::2", exact: true })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses\/[0-9a-f-]+$/);
  await expect(
    page.getByRole("heading", { level: 1, name: titled("2001:db8:2::2") }),
  ).toBeVisible();
});

test("past ten pages: each page read is shown, none read ahead of the view, the end reached", async ({
  page,
  login,
}) => {
  // The networks of the domain, more than 1 100 (the wide container's children): twelve
  // pages, the cache keeping ten (task 021-22f). The eleventh lands with the first
  // dropped and the rows in view kept in place: the window the grid gave was at the end of
  // the rows before, and read as it stood it read the twelfth at once, under the view.
  await login("alice");
  // The pages read, each once: the list read again (a change of another journey
  // invalidates it, each page under a new cursor) brings no page.
  const pages: { url: string; ids: string[]; next: boolean }[] = [];
  const firsts = new Set<string>();
  page.on("response", (response) => {
    const url = new URL(response.url());
    if (url.pathname !== "/v1/prefixes" || response.request().method() !== "GET") return;
    void response
      .json()
      .then((body: { items?: { id: string }[]; links?: { next?: string } }) => {
        const ids = (body.items ?? []).map((item) => item.id);
        if (ids.length === 0 || firsts.has(ids[0]!)) return;
        firsts.add(ids[0]!);
        pages.push({ url: url.search, ids, next: !!body.links?.next });
      })
      .catch(() => undefined);
  });
  // The reads of the list the page has started, counted in the page as it sends them: the
  // scroll below is judged on them in the same turn, never on the `request` events of the
  // test, which may come after the page read its rows (task 021-11p9). Before the shell
  // starts: it keeps the `fetch` it finds then (`activityFetch`).
  await page.addInitScript(() => {
    const shell = window as unknown as { __reads: number };
    shell.__reads = 0;
    const sent = window.fetch.bind(window);
    window.fetch = (input, init) => {
      const url = input instanceof Request ? input.url : String(input);
      if (new URL(url, location.href).pathname === "/v1/prefixes") shell.__reads++;
      return sent(input, init);
    };
  });
  await page.goto(`/prefixes?domain=${DATA.domain}`);
  const grid = page.getByRole("grid", { name: "Préfixes" });
  await expect(grid.locator("[role='gridcell']").first()).toBeVisible();
  // Every row in the document once (the keys of its cells).
  await grid.evaluate((el) => {
    const seen = new Set<string>();
    (window as unknown as { __rows: Set<string> }).__rows = seen;
    const scan = () => {
      for (const cell of el.querySelectorAll<HTMLElement>("[data-row]"))
        if (cell.dataset.row) seen.add(cell.dataset.row);
    };
    scan();
    new MutationObserver(scan).observe(el, { subtree: true, childList: true, attributes: true });
  });
  const reads = () => page.evaluate(() => (window as unknown as { __reads: number }).__reads);
  /**
   * To the end of the rows, unless the list was read since `asked` reads: a page read lands
   * under the rows in view, and a scroll after it would ask for the page after (task
   * 021-11p9; the guard judged in the test let a second scroll pass while a page landed).
   */
  const toEndUnlessRead = (asked: number) =>
    grid.evaluate((element, asked) => {
      if ((window as unknown as { __reads: number }).__reads !== asked) return;
      element.scrollTo(0, element.scrollHeight);
      document.getElementById("k-surface")?.scrollTo(0, 1e9);
    }, asked);
  // The last row wholly in view: its key and the top of its box in the grid.
  const lastInView = () =>
    grid.evaluate((el) => {
      const box = el.getBoundingClientRect();
      let last: { key: string; top: number } | null = null;
      for (const row of el.querySelectorAll<HTMLElement>("[role='row'][data-index]")) {
        const r = row.getBoundingClientRect();
        const key = row.querySelector<HTMLElement>("[data-row]")?.dataset.row;
        if (key && r.top >= box.top && r.bottom <= box.top + el.clientHeight + 1)
          last = { key, top: Math.round(r.top - box.top) };
      }
      return last;
    });
  // To the end of the rows kept until one page more is read, then that page in.
  // No more scroll once a read has started (its response parsed a moment later): one
  // scroll at least, the list read again meanwhile reading the next page after it.
  const readOneMore = async () => {
    const before = pages.length;
    const asked = await reads();
    await toEnd(grid);
    await expect(async () => {
      await toEndUnlessRead(asked);
      expect(pages.length).toBeGreaterThan(before);
    }).toPass({ timeout: 15_000 });
    const read = pages.at(-1)!;
    try {
      await expect
        // A read cancelled by the list read again is read anew once that is done.
        .poll(() => shownOf(read.ids), {
          message: `page ${pages.length} shown`,
          timeout: 15_000,
        })
        .toBe(true);
    } catch (error) {
      // What the grid holds then: the pages read (first or next, their rows), its window.
      console.log(
        `INFO a page read not shown: pages ${pages.length} started ${await reads()} ${pages.map((p) => `${p.url.includes("cursor") ? "c" : "f"}${p.ids.length}`).join(",")} ${await grid.evaluate(
          (el) => {
            const idx = [...el.querySelectorAll<HTMLElement>("[role='row'][data-index]")].map((r) =>
              Number(r.dataset.index),
            );
            return `top ${el.scrollTop} h ${el.scrollHeight} rows ${Math.min(...idx)}..${Math.max(...idx)} count ${el.getAttribute("aria-rowcount")}`;
          },
        )}`,
      );
      throw error;
    }
  };
  /** Whether a row of `ids` was in the document. */
  const shownOf = (ids: string[]) =>
    page.evaluate(
      (keys) => keys.some((key) => (window as unknown as { __rows: Set<string> }).__rows.has(key)),
      ids,
    );
  for (let n = 2; n <= 10; n++) await readOneMore();
  expect(pages).toHaveLength(10);
  // The count the scrolls are judged on sees the reads of the shell.
  expect(await reads()).toBeGreaterThanOrEqual(10);
  // The eleventh: the view stays on its rows, nothing more read without a scroll.
  // The eleventh in, the first dropped: the last row in view is still the tenth's last.
  const asked = await reads();
  await toEnd(grid);
  await expect(async () => {
    await toEndUnlessRead(asked);
    expect(pages).toHaveLength(11);
  }).toPass({ timeout: 15_000 });
  await expect.poll(async () => (await lastInView())?.key).toBe(pages[9]!.ids.at(-1));
  const before = (await lastInView())!;
  await page.waitForTimeout(1_000);
  expect(pages).toHaveLength(11);
  expect(await lastInView()).toEqual(before);
  // On to the end of the list: each page read shown, the last one without a next.
  await expect(async () => {
    await toEnd(grid);
    expect(pages.at(-1)!.next).toBe(false);
  }).toPass({ timeout: 30_000 });
  await toEnd(grid);
  await expect(grid.locator(`[data-row="${pages.at(-1)!.ids.at(-1)}"]`).first()).toBeVisible();
  const shown = await page.evaluate(() => [
    ...(window as unknown as { __rows: Set<string> }).__rows,
  ]);
  for (const read of pages)
    expect(
      read.ids.some((id) => shown.includes(id)),
      read.url,
    ).toBe(true);
});

test("a refused cursor starts the list again from its first page", async ({ page, login }) => {
  await login("alice");
  let refused = 0;
  await page.route(/\/v1\/addresses\?.*cursor=/, async (route) => {
    if (refused > 0) return route.continue();
    refused += 1;
    await route.fulfill({
      status: 400,
      contentType: "application/problem+json",
      body: JSON.stringify({
        type: "urn:kartenn:problem:invalid-cursor",
        title: "Bad Request",
        status: 400,
        detail: "the cursor has expired",
      }),
    });
  });
  await page.goto(`/addresses?within=${encodeURIComponent(DATA.bulk.prefix)}`);
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("link", { name: "2001:db8:2::1", exact: true })).toBeVisible();
  await expect(async () => {
    await toEnd(grid);
    await expect(
      page.getByRole("status").filter({ hasText: "la liste repart du début" }),
    ).toBeVisible({ timeout: 500 });
  }).toPass({ timeout: 20_000 });
  expect(refused).toBe(1);
  await toStart(grid);
  await expect(grid.getByRole("link", { name: "2001:db8:2::1", exact: true })).toBeVisible();
  await expectAccessible(page);
});

test("a filtered link copied opens the same list, its chips and its rows", async ({
  page,
  login,
  browser,
}) => {
  await login("alice");
  await page.goto("/addresses");
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("link", { name: DATA.address, exact: true })).toBeVisible();

  // The filter of the « Valeur » column is the bar's (020 decision A7): Alt+Down on its
  // header opens the picker on that property.
  await grid.getByRole("columnheader", { name: /Valeur/ }).click();
  await page.keyboard.press("Alt+ArrowDown");
  const picker = page.getByRole("dialog", { name: /filtre/i });
  await expect(picker).toBeVisible();
  await picker.getByRole("combobox", { name: /Opérateur/ }).click();
  await page.getByRole("option", { name: "dans" }).click();
  await picker.getByRole("textbox", { name: /Valeur/ }).fill(DATA.prefix);
  await picker.getByRole("button", { name: "Appliquer" }).click();
  await expect(page).toHaveURL(/[?&]within=192\.0\.2\.0(%2F|\/)24/);
  await expect(grid.getByRole("link", { name: DATA.address, exact: true })).toBeVisible();
  await expect(grid.getByRole("link", { name: "2001:db8:2::1", exact: true })).toHaveCount(0);
  await expectAccessible(page);

  // The link, opened by another session: the same chips, the same rows.
  const link = page.url();
  const bar = page.getByRole("group", { name: "Filtres : Adresses" });
  const chips = await bar.getByRole("button").allTextContents();
  const other = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  try {
    const second = await other.newPage();
    await loginIn(second, "alice");
    await second.goto(link);
    const again = second.getByRole("grid", { name: "Adresses" });
    await expect(again.getByRole("link", { name: DATA.address, exact: true })).toBeVisible();
    await expect(again.getByRole("link", { name: "2001:db8:2::1", exact: true })).toHaveCount(0);
    expect(
      await second
        .getByRole("group", { name: "Filtres : Adresses" })
        .getByRole("button")
        .allTextContents(),
    ).toEqual(chips);
  } finally {
    await other.close();
  }
});

// REQ-021-18 acceptance: `site = <site>` and `core.owner = <entity>`, each value chosen among
// the suggestions of the API, the link copied opening the same view, its chips named by
// their objects. On the prefixes: the site of a network is `core.site`, carried by
// prefixes and not inherited (REQ-002-20), so that an address has none.
test("prefixes filtered by their site and their owner, chosen among the suggestions; the link copied opens the same view", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const tag = `zz-site-${browserName}-${Date.now().toString(36)}`;
  const anchors = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end typed filters",
    operations: [
      { op: "create_entity", local: 0, slug: `${tag}-e` },
      { op: "create_site", local: 1, slug: `${tag}-s` },
    ],
  });
  expect(anchors.status, JSON.stringify(anchors.body)).toBe(201);
  const idOf = async (path: string) =>
    ((await apiCall(page, "GET", path)).body as { items: { id: string }[] }).items[0]!.id;
  const entity = await idOf(`/v1/entities?slug=${tag}-e`);
  const site = await idOf(`/v1/sites?slug=${tag}-s`);
  const declarations = (await apiCall(page, "GET", "/v1/property-declarations")).body as {
    items: { id: string; key: string }[];
  };
  const property = (key: string) => ({ id: declarations.items.find((d) => d.key === key)!.id });
  // A domain of that entity: one network on the site, another without one (RFC 5737).
  const made = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end typed filters",
    operations: [
      { op: "create_domain", local: 0, slug: tag },
      {
        op: "assign_property",
        object: { local: 0 },
        kind: "domain",
        property: property("core.owner"),
        values: [entity],
      },
      {
        op: "create_prefix",
        local: 1,
        domain: { local: 0 },
        prefix: "198.51.100.0/25",
        nature: "network",
      },
      { op: "create_prefix", domain: { local: 0 }, prefix: "198.51.100.128/25", nature: "network" },
      {
        op: "assign_property",
        object: { local: 1 },
        kind: "prefix",
        property: property("core.site"),
        values: [site],
      },
    ],
  });
  expect(made.status, JSON.stringify(made.body)).toBe(201);

  // The declarations the screen reads from here on (task 021-11p7): those of the prefixes
  // (`carrier=prefix`), never the whole catalog.
  const declarationReads = (target: Page) => {
    const seen: URL[] = [];
    target.on("request", (request) => {
      const url = new URL(request.url());
      if (url.pathname === "/v1/property-declarations") seen.push(url);
    });
    return seen;
  };
  const read = declarationReads(page);

  // Properties filter the objects of one domain (`kartenn-rest`, `filter::parse`).
  await page.goto(`/prefixes?domain=${tag}`);
  const grid = page.getByRole("grid", { name: "Préfixes" });
  await expect(grid.getByRole("row")).toHaveCount(3);
  const bar = page.getByRole("group", { name: "Filtres : Préfixes" });
  const picker = page.getByRole("dialog", { name: /filtre/i });
  for (const [label, slug] of [
    [/^Site/, `${tag}-s`],
    [/^Entité propriétaire/, `${tag}-e`],
  ] as const) {
    await bar.getByRole("button", { name: "Ajouter un filtre" }).click();
    await page.getByRole("option", { name: label }).click();
    await picker.getByRole("combobox", { name: /Valeur/ }).fill(slug);
    await page.getByRole("option", { name: new RegExp(slug) }).click();
    await picker.getByRole("button", { name: "Appliquer" }).click();
    await expect(picker).toHaveCount(0);
  }
  await expect(page).toHaveURL(new RegExp(`[?&]prop\\.core\\.site=${site}`));
  await expect(page).toHaveURL(new RegExp(`[?&]prop\\.core\\.owner=${entity}`));
  await expect(grid.getByRole("link", { name: "198.51.100.0/25", exact: true })).toBeVisible();
  await expect(grid.getByRole("row")).toHaveCount(2);
  // Each chip says its value by the slug of its object, never its identifier.
  const named = [`Site est ${tag}-s`, `Entité propriétaire est ${tag}-e`];
  for (const name of named) {
    await expect(bar.getByRole("button", { name, exact: true })).toBeVisible();
  }
  const chips = await bar.getByRole("button").allTextContents();
  await expectAccessible(page);
  /** Each declaration read of a page filtered by `carrier=prefix`, its cursor apart. */
  const byPrefix = (seen: readonly URL[]) => {
    expect(seen.length).toBeGreaterThan(0);
    for (const url of seen) {
      if (!url.searchParams.has("cursor")) expect(url.search).toBe("?carrier=prefix");
    }
  };
  byPrefix(read);

  // The link, opened by another session: the same chips, named by their objects, the
  // same rows.
  const link = page.url();
  const other = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  try {
    const second = await other.newPage();
    await loginIn(second, "alice");
    const readAgain = declarationReads(second);
    await second.goto(link);
    const again = second.getByRole("grid", { name: "Préfixes" });
    await expect(again.getByRole("link", { name: "198.51.100.0/25", exact: true })).toBeVisible();
    await expect(again.getByRole("row")).toHaveCount(2);
    const bar2 = second.getByRole("group", { name: "Filtres : Préfixes" });
    for (const name of named) {
      await expect(bar2.getByRole("button", { name, exact: true })).toBeVisible();
    }
    expect(await bar2.getByRole("button").allTextContents()).toEqual(chips);
    byPrefix(readAgain);
  } finally {
    await other.close();
  }
});

test("a record hovered then opened shows without a skeleton (its reads prefetched)", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto("/addresses");
  const link = page.getByRole("grid", { name: "Adresses" }).getByRole("link", {
    name: DATA.address,
    exact: true,
  });
  // The pointer held on the link past its delay (60 ms): the record and its properties read.
  const read = page.waitForResponse(/\/v1\/addresses\/[0-9a-f-]+\/properties/);
  await link.hover();
  await read;
  await link.click();
  await expect(page).toHaveURL(/\/addresses\/[0-9a-f-]+$/);
  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.address) })).toBeVisible();
  // Neither the record nor its properties wait: no skeleton in the main panel at once.
  const main = page.getByRole("region", { name: DATA.address });
  expect(await main.locator(".k-skeleton-fill, [aria-busy='true']").count()).toBe(0);
});

// Task 021-22g: the VLAN column of the list of prefixes (REQ-026-6) reads the properties of
// each network in view, under the key of the record's route; hovered, a network reads its
// record alone, and opens in cache. A container carries no VLAN: its hover reads both.
test("a network of the list hovered reads its record alone, its properties read by its row", async ({
  page,
  login,
}) => {
  await login("alice");
  const reads: string[] = [];
  page.on("request", (request) => {
    const path = new URL(request.url()).pathname;
    if (path.startsWith("/v1/prefixes/")) reads.push(path);
  });
  const grid = page.getByRole("grid", { name: "Préfixes" });
  const idOf = async (name: string) => {
    const href = await grid.getByRole("link", { name, exact: true }).getAttribute("href");
    return href!.split("/").at(-1)!;
  };
  await page.goto(`/prefixes?domain=${DATA.domain}`);
  const [net, container] = [await idOf(DATA.prefix), await idOf(DATA.wide.prefix)];
  // Before any hover: the properties of the network read by its row, none of the container.
  await expect.poll(() => reads).toContain(`/v1/prefixes/${net}/properties`);
  expect(reads).not.toContain(`/v1/prefixes/${container}/properties`);

  /** The reads of the prefix `id` that the hover of `name` sends, its record answered. */
  const hovered = async (name: string, id: string) => {
    const from = reads.length;
    const record = page.waitForResponse((r) => new URL(r.url()).pathname === `/v1/prefixes/${id}`);
    await grid.getByRole("link", { name, exact: true }).hover();
    await record;
    // The VLAN cells of the other rows in view may still be reading theirs.
    return reads.slice(from).filter((path) => path.startsWith(`/v1/prefixes/${id}`));
  };
  // The reads of a target go out together (`prefetch`): the record answered, none is left.
  expect(await hovered(DATA.wide.prefix, container)).toEqual(
    expect.arrayContaining([`/v1/prefixes/${container}`, `/v1/prefixes/${container}/properties`]),
  );
  expect(await hovered(DATA.prefix, net)).toEqual([`/v1/prefixes/${net}`]);
  const from = reads.length;
  await grid.getByRole("link", { name: DATA.prefix, exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/prefixes/${net}$`));
  await expect(page.getByRole("heading", { level: 1, name: titled(DATA.prefix) })).toBeVisible();
  // In cache: the screen reads neither the record nor its properties again (its panels
  // beside, the occupancy and the plugins', are no reads of its route).
  const again = [`/v1/prefixes/${net}`, `/v1/prefixes/${net}/properties`];
  expect(reads.slice(from).filter((path) => again.includes(path))).toEqual([]);
});

// « Clore… » of a record (task 11g, 021/A31): the closure in the right panel, its simulation
// said first; to the draft by Ctrl+Enter, then applied from the draft; refused by a rule, the
// code said before and after it is sent. Each test closes in a domain of its own (RFC 5737).
const unique = (browser: string) => `zz-close-${browser}-${Date.now().toString(36)}`;
const counter = (page: Page) => page.getByRole("button", { name: /^Brouillon \d+ opération/ });
const panel = (page: Page) => page.getByRole("complementary", { name: "Panneau droit" });
const escaped = (text: string) => text.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");

/** A domain of its own, its network 198.51.100.0/24 holding the address `address`. */
async function closable(page: Page, browser: string, address: string): Promise<string> {
  const { slug } = await ownedDomain(page, unique(browser));
  const domains = (await apiCall(page, "GET", `/v1/domains?slug=${slug}`)).body as {
    items: { id: string }[];
  };
  const created = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end closure",
    operations: [
      { op: "create_address", domain: { id: domains.items[0]!.id }, address, usage: "host" },
    ],
  });
  expect(created.status).toBe(201);
  return slug;
}

/** « Clore… » of the record shown, from its menu of actions: the close panel. */
async function closeFromRecord(page: Page, name: string): Promise<Locator> {
  await expect(page.getByRole("heading", { level: 1, name: titled(name) })).toBeVisible();
  await page.getByRole("main").getByRole("button", { name: "Actions" }).click();
  await page.getByRole("menuitem", { name: "Clore…" }).click();
  const form = page.getByRole("form", { name: new RegExp(`^Clore : .* ${escaped(name)}$`) });
  await expect(form).toBeVisible();
  return form;
}

test("an address closed from its record to the draft, at the keyboard, then applied", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const address = "198.51.100.10";
  const slug = await closable(page, browserName, address);
  await page.goto(`/addresses?domain=${slug}`);
  await page.getByRole("grid", { name: "Adresses" }).getByRole("link", { name: address }).click();
  const form = await closeFromRecord(page, address);
  // Its simulation alone, said before anything is sent: it passes, the address alone.
  await expect(form.getByText("La clôture passe les règles")).toBeVisible();
  await expect(form).toContainText("Seul cet objet se clôt.");
  const action = form.locator(".k-split-button__main");
  await expect(action).toHaveText(/^Clore au brouillon/);
  await expect(action).toBeFocused();
  await expectAccessible(page);
  await page.keyboard.press("Control+Enter");
  await expect(form).toHaveCount(0);
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 1 opération/);

  // The draft, simulated, applied with its summary. The counter opens the panel on the draft,
  // or closes it whatever its tab: the panel still open (the close panel just closed in its
  // Détail tab), its tab Brouillon.
  const draftTab = panel(page).getByRole("tab", { name: /Brouillon/ });
  if (await draftTab.isVisible()) await draftTab.click();
  else await counter(page).click();
  await expect(draftTab).toHaveAttribute("aria-selected", "true");
  const operations = panel(page).getByRole("list", { name: /^Opérations du brouillon/ });
  await expect(operations).toContainText("close");
  await expect(
    panel(page).getByRole("status").filter({ hasText: "Simulation valide" }),
  ).toBeVisible();
  const summary = panel(page).getByRole("textbox", { name: /^Résumé/ });
  await summary.focus();
  await page.keyboard.press("Control+Enter");
  await expect(page.getByRole("status").filter({ hasText: "Appliqué :" }).first()).toBeVisible();
  await expect(counter(page)).toHaveAccessibleName(/^Brouillon 0 opération/);
  // Closed: no longer in the list of the domain at the present.
  await page.goto(`/addresses?domain=${slug}`);
  await expect(page.getByText(/^Aucun objet/).first()).toBeVisible();
  await expect(page.getByRole("link", { name: address, exact: true })).toHaveCount(0);
});

test("a network closed alone is refused by a rule, its code said; the cascade counted", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const network = "198.51.100.0/24";
  const slug = await closable(page, browserName, "198.51.100.20");
  await page.goto(`/prefixes?domain=${slug}`);
  await page.getByRole("grid", { name: "Préfixes" }).getByRole("link", { name: network }).click();
  const form = await closeFromRecord(page, network);
  // Before it is sent: the rule that refuses it, with its code.
  await expect(form.getByText("Clôture refusée par les règles")).toBeVisible();
  await expect(form).toContainText("core.address.outside_network");
  await expectAccessible(page);

  // Applied anyway: the server refuses it, on the action, with the same code.
  await form.getByRole("button", { name: "Choisir l'action" }).click();
  await page.getByRole("menuitem", { name: "Appliquer la clôture" }).click();
  const motive = form.getByRole("textbox", { name: /^Motif/ });
  await expect(motive).toHaveValue(new RegExp(`^Clôture : .* ${escaped(network)}$`));
  await motive.press("Control+Enter");
  const failed = form.locator(".k-inline-alert").filter({ hasText: "La clôture n'a pas abouti" });
  await expect(failed).toBeVisible();
  await expect(failed).toContainText("core.address.outside_network");
  await expectAccessible(page);

  // With the cascade: it passes, the address closed with it, counted.
  await form.getByRole("checkbox", { name: "Clore aussi ce qui en dépend" }).check();
  await expect(form.getByText("La clôture passe les règles")).toBeVisible();
  await expect(form).toContainText(/Se closent avec lui : .* · 1\./);
});

// The history of an object modified (task 021-11p5, REQ-021-9; maquette E2): each version
// with the motive of the changeset that wrote it, a link to it, and its author as the
// server names it, in a domain of its own (RFC 5737).
test("a network renumbered: its history says each motive and its author", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const slug = `zz-history-${browserName}-${Date.now().toString(36)}`;
  const { prefixes } = await ownedDomain(page, slug);
  const renumbered = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end renumbering",
    operations: [{ op: "update_prefix", id: { id: prefixes[0] }, prefix: "198.51.100.0/25" }],
  });
  expect(renumbered.status, JSON.stringify(renumbered.body)).toBe(201);

  await page.goto(`/prefixes/${prefixes[0]}?tab=history`);
  await expect(
    page.getByRole("heading", { level: 1, name: titled("198.51.100.0/25") }),
  ).toBeVisible();
  const history = page.getByRole("grid", { name: "Versions de 198.51.100.0/25" });
  // The header, then three versions on both axes: the new value, the old one closed by the
  // renumbering, the old one as it was asserted at its creation. Each motive is a link to
  // its changeset.
  const rows = history.getByRole("row");
  await expect(rows).toHaveCount(4);
  const renumbering = rows
    .filter({ hasText: "198.51.100.0/25" })
    .getByRole("link", { name: "end-to-end renumbering" });
  await expect(renumbering).toHaveAttribute("href", /^\/changesets\/[0-9a-f-]+$/);
  const created = rows
    .filter({ hasText: "198.51.100.0/24" })
    .getByRole("link", { name: "end-to-end journey" });
  await expect(created.first()).toBeVisible();
  // The author of every version, as the identities name him now.
  await expect(history.getByRole("gridcell", { name: USERS.alice, exact: true })).toHaveCount(3);
  await expectAccessible(page);
});
