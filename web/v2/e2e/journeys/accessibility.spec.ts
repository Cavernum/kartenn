import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Locator, Page } from "@playwright/test";
import {
  apiCall,
  atRest,
  DATA,
  expect,
  expectAccessible,
  ownedDomain,
  test,
} from "../fixtures/journeys.ts";

// Accessibility end to end of the new shell (spec 021, task 21a; REQ-021-29), ported from
// web/e2e/journeys/accessibility.spec.ts and the guards of web/e2e/journeys/smoke.spec.ts:
// every journey runs axe (checked by the coverage test below), every screen of the shell
// passes it, one journey goes through the tree, a table, an action menu and a dialog by the
// keyboard alone; and the guards of the fixtures fail a journey, axe by its violation, the
// CSP by an inline script. Task 21b adds the rest of REQ-021-29: an in-place edit, the
// draft panel, the palette and the collapsed sidebar by the keyboard alone, and every
// screen of consultation read at 320 px without scrolling sideways.

const JOURNEYS = fileURLToPath(new URL(".", import.meta.url));
const SPACES = fileURLToPath(new URL("../../shell/src/frame/spaces.ts", import.meta.url));

/** The journeys among `files` (name → source) that never call axe. */
export function uncovered(files: Record<string, string>): string[] {
  return Object.entries(files)
    .filter(([, source]) => !/await expectAccessible\(/.test(source))
    .map(([name]) => name)
    .sort();
}

test("every journey runs axe", () => {
  const files = Object.fromEntries(
    readdirSync(JOURNEYS)
      .filter((name) => name.endsWith(".spec.ts"))
      .map((name) => [name, readFileSync(`${JOURNEYS}/${name}`, "utf8")]),
  );
  expect(Object.keys(files).length).toBeGreaterThan(10);
  expect(uncovered(files)).toEqual([]);
  // The check itself: a journey without axe is named.
  expect(uncovered({ "bare.spec.ts": 'test("x", async ({ page }) => page.goto("/"));' })).toEqual([
    "bare.spec.ts",
  ]);
});

/**
 * The entries of the spaces of the core (shell/src/frame/spaces.ts), by their path: read
 * from the source, so that an entry added is a screen checked. Each entry also has its
 * icon, the only `icon: "…"` of the source: counted apart, an entry whose path is not a
 * literal the expression reads fails the journey instead of escaping it.
 */
function entries(): string[] {
  const source = readFileSync(SPACES, "utf8");
  const paths = [...source.matchAll(/\bto: "(\/[^"]*)"/g)].map((m) => m[1]!);
  const icons = [...source.matchAll(/\bicon: "[^"]+"/g)].length;
  expect(paths.length, "the entries of spaces.ts, by their paths and by their icons").toBe(icons);
  expect(new Set(paths).size, "an entry of spaces.ts per path").toBe(paths.length);
  return paths;
}

/**
 * The records whose first object is checked: the collection of the API, the route of its
 * record in the shell.
 */
const RECORDS = [
  ["domains", "/domains"],
  ["prefixes", "/prefixes"],
  ["ranges", "/ranges"],
  ["addresses", "/addresses"],
  ["entities", "/entities"],
  ["sites", "/sites"],
  ["vlan-groups", "/vlan-groups"],
  ["vlans", "/vlans"],
  ["changesets", "/changesets"],
  ["groups", "/admin/groups"],
] as const;

/** The record of the first object of `collection`, `null` if it holds none. */
async function firstRecord(page: Page, collection: string, route: string): Promise<string | null> {
  const listed = await apiCall(page, "GET", `/v1/${collection}?page_size=1`);
  expect(listed.status, `/v1/${collection}`).toBe(200);
  const id = (listed.body as { items: { id: string }[] }).items[0]?.id;
  return id === undefined ? null : `${route}/${id}`;
}

/** A draft of the page's user, empty, for its record; dropped by `dropDraft`. */
async function newDraft(page: Page, title: string): Promise<string> {
  const created = await apiCall(page, "POST", "/v1/drafts", { title, operations: [] });
  expect(created.status, JSON.stringify(created.body)).toBe(201);
  return (created.body as { id: string }).id;
}

/** Deletes the draft `id` (under `If-Match`: its ETag read first). */
async function dropDraft(page: Page, id: string): Promise<void> {
  const status = await page.evaluate(async (id) => {
    const read = await fetch(`/v1/drafts/${id}`);
    const response = await fetch(`/v1/drafts/${id}`, {
      method: "DELETE",
      headers: {
        "If-Match": read.headers.get("ETag") ?? "",
        "Idempotency-Key": crypto.randomUUID(),
      },
    });
    return response.status;
  }, id);
  expect(status).toBe(204);
}

/** The screen shown drawn: its title, nothing left loading in the content. */
async function drawn(page: Page): Promise<void> {
  const main = page.getByRole("main");
  await expect(main.locator("h1").first()).toBeVisible();
  await expect(main.locator(".k-skeleton-fill, [aria-busy='true']")).toHaveCount(0);
}

test("every screen of the shell passes axe", async ({ page, login, context, browserName }) => {
  test.setTimeout(300_000);
  await login("alice");
  const records = (
    await Promise.all(RECORDS.map(([collection, route]) => firstRecord(page, collection, route)))
  ).filter((record) => record !== null);
  // The lists of the referential (the old journey `accessibility` was their only proof).
  expect(entries()).toEqual(expect.arrayContaining(["/ranges", "/entities", "/sites"]));
  expect(records.length).toBeGreaterThanOrEqual(5);
  const draft = await newDraft(page, `axe ${browserName} ${Date.now()}`);
  const screens = [
    ...entries(),
    ...records,
    `/drafts/${draft}`,
    // The view of a plugin, in the frame.
    "/x/example/overview",
    "/help/shortcuts",
    "/about",
  ];
  for (const screen of screens) {
    await test.step(screen, async () => {
      await page.goto(screen);
      // Its plugins loaded before the next one (`atRest`: Firefox and the loads cut short).
      await atRest(page);
      await drawn(page);
      await expectAccessible(page);
    });
  }
  await dropDraft(page, draft);
  await context.clearCookies();
  await page.goto("/login");
  await expect(page.getByRole("heading", { level: 1, name: "Connexion" })).toBeVisible();
  await expectAccessible(page);
});

/**
 * What scrolls sideways in the page at its width (WCAG 1.4.10): the document, or a region
 * that scrolls in its own box, the grids excepted (data tables, note 2 of 1.4.10; each is a
 * named region that scrolls, design 020 « Adaptation à la largeur »). And what is lost
 * there: a control that can take the focus, shown, past the window or past a box that
 * cuts (`overflow: hidden` or `clip`, the frame's own included) — content cut away. A text
 * cut by an ellipsis inside its own box is not counted (its tooltip says it whole).
 */
async function sideways(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const out: string[] = [];
    const name = (element: Element) =>
      [
        element.tagName.toLowerCase(),
        element.id ? `#${element.id}` : "",
        ...[...element.classList].map((c) => `.${c}`),
        element.getAttribute("aria-label") ? `[${element.getAttribute("aria-label")}]` : "",
      ].join("");
    const root = document.documentElement;
    if (root.scrollWidth > root.clientWidth + 1) {
      out.push(`the page: ${root.scrollWidth} > ${root.clientWidth}`);
    }
    const grids = "[role='grid'], [role='treegrid']";
    for (const element of document.querySelectorAll<HTMLElement>("body *")) {
      if (element.closest(grids) || element.clientWidth === 0) continue;
      if (!/auto|scroll/.test(getComputedStyle(element).overflowX)) continue;
      if (element.scrollWidth <= element.clientWidth + 1) continue;
      out.push(`${name(element)}: ${element.scrollWidth} > ${element.clientWidth}`);
    }
    const focusable =
      "a[href], button, input, select, textarea, summary, [tabindex]:not([tabindex='-1'])";
    for (const control of document.querySelectorAll<HTMLElement>(focusable)) {
      if (control.closest(`${grids}, [inert], [hidden]`)) continue;
      const box = control.getBoundingClientRect();
      // Hidden off screen until focused (the skip link) or not shown at all.
      if (box.width <= 1 || box.height <= 1) continue;
      if (getComputedStyle(control).visibility === "hidden") continue;
      if (box.left < -1 || box.right > window.innerWidth + 1) {
        out.push(
          `${name(control)} past the window: ${Math.round(box.left)}…${Math.round(box.right)}`,
        );
        continue;
      }
      for (let parent = control.parentElement; parent; parent = parent.parentElement) {
        const overflow = getComputedStyle(parent).overflowX;
        // A region that scrolls is counted above; past it, nothing cuts this control.
        if (/auto|scroll/.test(overflow)) break;
        if (!/hidden|clip/.test(overflow)) continue;
        const cut = parent.getBoundingClientRect();
        if (box.left < cut.left - 1 || box.right > cut.right + 1) {
          out.push(`${name(control)} cut by ${name(parent)}`);
          break;
        }
      }
    }
    return out;
  });
}

// Reflow (REQ-021-29, WCAG 1.4.10): every screen of consultation reads at 320 px wide (the
// compact class) without scrolling sideways, outside its grids.
test("every screen of consultation reads at 320 px without scrolling sideways", async ({
  page,
  login,
  context,
  browserName,
}) => {
  test.setTimeout(300_000);
  await page.setViewportSize({ width: 320, height: 900 });
  // Alice reads every space, the administration included.
  await login("alice");
  const records = (
    await Promise.all(RECORDS.map(([collection, route]) => firstRecord(page, collection, route)))
  ).filter((record) => record !== null);
  expect(records.length).toBeGreaterThanOrEqual(5);
  const found: Record<string, string[]> = {};
  for (const screen of [
    ...entries(),
    ...records,
    "/x/example/overview",
    "/help/shortcuts",
    "/about",
  ]) {
    await test.step(screen, async () => {
      await page.goto(screen);
      await atRest(page);
      await drawn(page);
      const wide = await sideways(page);
      if (wide.length > 0) found[screen] = wide;
    });
  }
  // And the page that leads to them all, out of the session.
  await context.clearCookies();
  await page.goto("/login");
  await expect(page.getByRole("heading", { level: 1, name: "Connexion" })).toBeVisible();
  const login320 = await sideways(page);
  if (login320.length > 0) found["/login"] = login320;
  expect(found, `at 320 px under ${browserName}`).toEqual({});

  // The check itself: a button cut by a box that hides its overflow, and one pushed past
  // the window, are named (styles by CSSOM, as the CSP allows).
  await page.evaluate(() => {
    const box = document.createElement("div");
    box.style.overflow = "hidden";
    box.style.inlineSize = "100px";
    const cut = document.createElement("button");
    cut.textContent = "cut";
    cut.style.minInlineSize = "300px";
    const past = document.createElement("button");
    past.textContent = "past";
    past.style.marginInlineStart = "400px";
    box.append(cut);
    document.querySelector("main")?.append(box, past);
  });
  expect(await sideways(page)).toEqual(
    expect.arrayContaining([
      expect.stringMatching(/^button cut by div$/),
      expect.stringMatching(/^button past the window/),
    ]),
  );
});

/** Whether the focus is on `target` or inside it. */
const holdsFocus = (target: Locator) =>
  target.evaluate((element) => element.contains(document.activeElement));

/** Presses `key` (Tab by default) until the focus is on or inside `target`, `max` at most. */
async function tabTo(page: Page, target: Locator, key = "Tab", max = 80): Promise<void> {
  for (let i = 0; i < max; i += 1) {
    if (await holdsFocus(target)) return;
    await page.keyboard.press(key);
  }
  await expect(target).toBeFocused();
}

/** The row of a node of the tree, by its value. */
const node = (tree: Locator, value: string) =>
  tree.locator("[role='row'][data-node]").filter({
    has: tree.page().getByText(value, { exact: true }),
  });

/** A title that ends with the name of the object (its kind in words comes first). */
const titled = (name: string) => new RegExp(`\\s${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);

test("the tree, a table, an action menu and a dialog by the keyboard alone", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.goto("/topology");
  await atRest(page);
  const tree = page.getByRole("treegrid", { name: `Arbre des préfixes du domaine ${DATA.domain}` });
  await expect(node(tree, DATA.prefix)).toBeVisible();

  // The skip link first, then the main content.
  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: "Aller au contenu" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("main")).toBeFocused();

  // The tree, one tab stop: the first node, down to the wide container, Right opens it,
  // Down enters its first child, Enter opens its record.
  await tabTo(page, tree);
  await page.keyboard.press("Control+Home");
  await expect(node(tree, DATA.prefix).locator("[role='gridcell']").first()).toBeFocused();
  const wide = node(tree, DATA.wide.prefix);
  for (let i = 0; i < 20 && !(await holdsFocus(wide)); i += 1) {
    await page.keyboard.press("ArrowDown");
  }
  expect(await holdsFocus(wide)).toBe(true);
  await page.keyboard.press("ArrowRight");
  await expect(wide).toHaveAttribute("aria-expanded", "true");
  const child = node(tree, "2001:db8:3::/64");
  await expect(child).toBeVisible();
  await page.keyboard.press("ArrowDown");
  expect(await holdsFocus(child)).toBe(true);
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/prefixes\/[0-9a-f-]{36}$/);
  await expect(
    page.getByRole("heading", { level: 1, name: titled("2001:db8:3::/64") }),
  ).toBeVisible();

  // The action menu: Enter opens it, the arrows move, Enter opens the plugin's dialog.
  const actions = page.getByRole("main").getByRole("button", { name: "Actions" });
  await tabTo(page, actions);
  await page.keyboard.press("Enter");
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  const item = menu.getByRole("menuitem", { name: "Ajouter une adresse au brouillon" });
  for (let i = 0; i < 10 && !(await holdsFocus(item)); i += 1) {
    await page.keyboard.press("ArrowDown");
  }
  await expect(item).toBeFocused();
  await expectAccessible(page);
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Ajouter une adresse au brouillon" });
  await expect(dialog).toBeVisible();
  // The focus is inside the dialog; Escape closes it, back on the menu's button.
  await expect.poll(() => holdsFocus(dialog)).toBe(true);
  await expectAccessible(page);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(actions).toBeFocused();

  // A table: back to the navigation, its link to the addresses; the grid is one tab stop,
  // on its first header; down to the first row, Enter into its link, Enter follows it.
  const navigation = page.getByRole("navigation", { name: "Navigation principale" });
  await tabTo(page, navigation.getByRole("link", { name: "Adresses" }), "Shift+Tab");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses$/);
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("row").nth(1)).toBeVisible();
  await tabTo(page, grid);
  await expect(page.locator(":focus")).toHaveAttribute("role", "columnheader");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  const link = grid.getByRole("row").nth(1).getByRole("link").first();
  await expect(link).toBeFocused();
  const address = (await link.textContent())!.trim();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/addresses\/[0-9a-f-]{36}$/);
  await expect(page.getByRole("heading", { level: 1, name: titled(address) })).toBeVisible();
  await expectAccessible(page);
});

/** The id of the draft active in the page's tab (the SDK keeps it), `null` if none. */
const activeDraft = (page: Page) =>
  page.evaluate(() => {
    const key = "kartenn.draft.active";
    return sessionStorage.getItem(key) ?? localStorage.getItem(key);
  });

// The rest of REQ-021-29 at the keyboard alone (task 21b): an in-place edit, the draft
// panel it fills, the palette and the collapsed sidebar, axe at each step. The edit goes to
// the draft, removed from the panel then dropped: nothing is written; its domain is its own
// (RFC 5737, TEST-NET-2), after `production` in the order of the slugs.
test("an in-place edit, the draft panel, the palette and the collapsed sidebar by the keyboard alone", async ({
  page,
  login,
  browserName,
}) => {
  test.setTimeout(120_000);
  await login("alice");
  const { slug } = await ownedDomain(page, `zz-a11y-${browserName}-${Date.now().toString(36)}`);

  // In place: the grid is one tab stop, on its first header; down to the row, Enter opens
  // the editor of its value, Enter adds the new value to the draft.
  await page.goto(`/prefixes?domain=${slug}`);
  await atRest(page);
  const grid = page.getByRole("grid", { name: "Préfixes" });
  const value = grid.locator('[role="gridcell"][data-column="value"]').first();
  await expect(value).toContainText("198.51.100.0/24");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: "Aller au contenu" })).toBeFocused();
  await page.keyboard.press("Enter");
  await tabTo(page, grid);
  await expect(page.locator(":focus")).toHaveAttribute("role", "columnheader");
  await page.keyboard.press("ArrowDown");
  expect(await holdsFocus(value)).toBe(true);
  await page.keyboard.press("Enter");
  const editor = grid.getByRole("textbox", { name: /^Valeur — / });
  await expect(editor).toBeFocused();
  await expectAccessible(page);
  await page.keyboard.press("Control+a");
  await page.keyboard.type("198.51.100.0/25");
  await page.keyboard.press("Enter");
  await expect(value).toHaveAttribute("data-state", "draft");
  // The focus stays on the cell, back from its editor.
  expect(await holdsFocus(value)).toBe(true);
  // Said by the polite status of the list.
  await expect(
    page
      .getByRole("main")
      .getByRole("status")
      .filter({ hasText: /Ajouté au brouillon : / }),
  ).toHaveText(/198\.51\.100\.0\/2[45]/);
  const draft = (await activeDraft(page))!;
  expect(draft, "the draft the edit went to").not.toBeNull();
  // Dropped whatever happens next.
  try {
    // The draft panel: back to its counter, Enter opens the panel on its tab; the operation's
    // remove button reached by Tab, Enter removes it; Escape closes the panel, the focus back
    // on the counter.
    const counter = page.getByRole("button", { name: /^Brouillon\s*\d+\s*opérations?$/ });
    await expect(counter).toHaveAccessibleName(/^Brouillon\s*1\s*opération$/);
    await tabTo(page, counter, "Shift+Tab");
    await page.keyboard.press("Enter");
    const panel = page.getByRole("complementary", { name: "Panneau droit" });
    await expect(panel.getByRole("tab", { name: /^Brouillon/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(counter).toHaveAttribute("aria-expanded", "true");
    const remove = panel.getByRole("button", { name: /^Retirer l'opération 1 : / });
    await tabTo(page, remove);
    await expectAccessible(page);
    await page.keyboard.press("Enter");
    // Its button gone, the focus goes to the sentence that says the draft empty.
    await expect(
      panel.locator("p[tabindex='-1']", { hasText: "Le brouillon est vide" }),
    ).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(panel.getByRole("tab")).toHaveCount(0);
    await expect(counter).toBeFocused();
    await expect(counter).toHaveAccessibleName(/^Brouillon\s*0\s*opérations?$/);
    await expect(value).not.toHaveAttribute("data-state", "draft");

    // The palette: Ctrl+K puts the focus in its field, Escape gives it back; open again, a
    // page by its name, the arrows move the choice, Enter follows it.
    await page.keyboard.press("Control+k");
    const palette = page.getByRole("dialog", { name: "Palette de commandes" });
    const field = palette.getByRole("combobox");
    await expect(field).toBeFocused();
    await expectAccessible(page);
    await page.keyboard.press("Escape");
    await expect(palette).toBeHidden();
    await expect(counter).toBeFocused();
    await page.keyboard.press("Control+k");
    await page.keyboard.type("adresses");
    const option = palette.getByRole("option", { name: /Adresses/ }).first();
    await expect(option).toHaveAttribute("aria-selected", "true");
    // The count of the list, said by its polite region.
    await expect(palette.getByRole("status")).toHaveText(/^\d+ résultats?$/);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowUp");
    await expect(option).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Enter");
    await expect(palette).toBeHidden();
    await expect(page).toHaveURL(/\/addresses$/);
    await expect(page.getByRole("heading", { level: 1, name: "Adresses" })).toBeVisible();

    // The sidebar collapsed by its toggle, at the keyboard (`aria-expanded` says its state):
    // its links keep their names, an icon each; one reached by Shift+Tab, Enter follows it;
    // the toggle, reached by Tab, opens it again.
    const navigation = page.getByRole("navigation", { name: "Navigation principale" });
    const toggle = navigation.getByRole("button", { name: "Barre latérale" });
    await tabTo(page, toggle, "Shift+Tab");
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await page.keyboard.press("Enter");
    await expect(navigation).toHaveAttribute("data-state", "collapsed");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(toggle).toBeFocused();
    const prefixes = navigation.getByRole("link", { name: "Préfixes" });
    await expect(prefixes).toBeVisible();
    await tabTo(page, prefixes, "Shift+Tab");
    await expectAccessible(page);
    await page.keyboard.press("Enter");
    await expect(page).toHaveURL(/\/prefixes$/);
    await expect(page.getByRole("heading", { level: 1, name: "Préfixes" })).toBeVisible();
    await expect(navigation).toHaveAttribute("data-state", "collapsed");
    await tabTo(page, toggle);
    await page.keyboard.press("Enter");
    await expect(navigation).toHaveAttribute("data-state", "expanded");
  } finally {
    await dropDraft(page, draft);
  }
});

const NARROW = "Modification à partir de 1024 px de large : élargissez la fenêtre pour modifier.";

// Editing from 1024 px wide (task 21c; REQ-021-29, Q-021-12): under it, at 768 px (narrow)
// and 390 px (compact), a cell of the list says why it has no editor, `e` and the palette's
// « Modifier » say it by the status of the list, no edit panel opens; at 1024 px editing
// comes back; an editor opened there and the window narrowed keeps its text and the focus,
// its text refused by the width until the window is wide again. The edit goes to a draft,
// dropped at the end; its domain is its own (RFC 5737, TEST-NET-2).
test("under 1024 px a cell, `e` and the palette say the width; at 1024 px editing comes back", async ({
  page,
  login,
  browserName,
}) => {
  test.setTimeout(120_000);
  await login("alice");
  const { slug } = await ownedDomain(page, `zz-narrow-${browserName}-${Date.now().toString(36)}`);
  const grid = page.getByRole("grid", { name: "Préfixes" });
  const value = grid.locator('[role="gridcell"][data-column="value"]').first();
  const status = page.getByRole("main").getByRole("status").filter({ hasText: NARROW });
  const editPanel = page.getByRole("complementary", { name: "Panneau droit" });
  /** The list at `width`, the focus on the value of its first row by the keyboard. */
  async function onValue(width: number): Promise<void> {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(`/prefixes?domain=${slug}`);
    await atRest(page);
    await expect(value).toContainText("198.51.100.0/24");
    // Its first header (narrower, Firefox stops first on the grid's own scrolling box).
    await tabTo(page, grid.getByRole("columnheader").first());
    await page.keyboard.press("ArrowDown");
    expect(await holdsFocus(value)).toBe(true);
  }

  for (const width of [768, 390]) {
    await onValue(width);
    // The cell: read only, the width as its description, and at the foot of the list.
    await expect(value).toHaveAttribute("aria-readonly", "true");
    await expect(value).toHaveAccessibleDescription(NARROW);
    await expect(page.locator(".k-keyboard-hint").filter({ hasText: NARROW })).toBeVisible();
    // Enter opens no editor.
    await page.keyboard.press("Enter");
    await expect(grid.getByRole("textbox")).toHaveCount(0);
    // `e` says the width by the status of the list; no edit panel opens.
    await page.keyboard.press("e");
    await expect(status).toHaveText(NARROW);
    await expect(editPanel).toHaveCount(0);
    expect(await holdsFocus(value)).toBe(true);
    await expectAccessible(page);
    // The palette keeps « Modifier la ligne : Préfixe »; run, it says the width the same way.
    await page.keyboard.press("Control+k");
    const palette = page.getByRole("dialog", { name: "Palette de commandes" });
    await page.keyboard.type("Modifier la ligne");
    const command = palette.getByRole("option", { name: /^Modifier la ligne : Préfixe/ });
    await expect(command).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("Enter");
    await expect(palette).toBeHidden();
    await expect(status).toHaveText(NARROW);
    await expect(editPanel).toHaveCount(0);
  }

  // At 1024 px: the cell edits again; the window narrowed with its editor open keeps it.
  await onValue(1024);
  await expect(value).not.toHaveAttribute("aria-readonly", "true");
  await page.keyboard.press("Enter");
  const editor = grid.getByRole("textbox", { name: /^Valeur — / });
  await expect(editor).toBeFocused();
  await page.keyboard.press("Control+a");
  await page.keyboard.type("198.51.100.0/25");
  await page.setViewportSize({ width: 768, height: 900 });
  await expect(page.locator("[data-width='narrow']")).toHaveCount(1);
  await expect(editor).toBeFocused();
  await expect(editor).toHaveValue("198.51.100.0/25");
  // Enter: refused by the width, said in the alert of the grid; the editor stays.
  await page.keyboard.press("Enter");
  await expect(page.getByRole("main").getByRole("alert").filter({ hasText: NARROW })).toContainText(
    "Échap annule la saisie.",
  );
  await expect(editor).toBeFocused();
  await expect(editor).toHaveValue("198.51.100.0/25");
  await expect(value).not.toHaveAttribute("data-state", "draft");
  // Wide again, Enter adds it to the draft.
  await page.setViewportSize({ width: 1024, height: 900 });
  await expect(page.locator("[data-width='standard']")).toHaveCount(1);
  await page.keyboard.press("Enter");
  await expect(value).toHaveAttribute("data-state", "draft");
  const draft = (await activeDraft(page))!;
  expect(draft, "the draft the edit went to").not.toBeNull();
  await dropDraft(page, draft);
});

// The draft under 1024 px (task 21d; REQ-021-29): at 768 px its page is read, its
// operations listed, « Appliquer » refused with the width, reached and pressed at the
// keyboard without submitting anything, as the removal of an operation; at 1024 px it
// applies again. The draft changes one of the prefixes of its own domain, dropped at the end.
test("under 1024 px « Appliquer » of the draft says the width; at 1024 px it comes back", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { prefixes } = await ownedDomain(
    page,
    `zz-narrow-draft-${browserName}-${Date.now().toString(36)}`,
  );
  const created = await apiCall(page, "POST", "/v1/drafts", {
    title: `étroit ${browserName} ${Date.now()}`,
    operations: [{ op: "update_prefix", id: { id: prefixes[0]! }, nature: "container" }],
  });
  expect(created.status, JSON.stringify(created.body)).toBe(201);
  const id = (created.body as { id: string }).id;
  try {
    await page.setViewportSize({ width: 768, height: 900 });
    await page.goto(`/drafts/${id}`);
    await atRest(page);
    const main = page.getByRole("main");
    const apply = main.locator(".k-split-button__main");
    await expect(main.locator(".k-draft-submit__result")).toContainText("Simulation valide");
    await expect(apply).toHaveText(/^Appliquer/);
    await expect(apply).toHaveAttribute("aria-disabled", "true");
    await expect(apply).toHaveAccessibleDescription(NARROW);
    const remove = main.getByRole("button", { name: /^Retirer l'opération 1/ });
    await expect(remove).toHaveAccessibleDescription(NARROW);
    await expectAccessible(page);
    // At the keyboard: reached, pressed, nothing submitted.
    await tabTo(page, apply);
    await page.keyboard.press("Enter");
    await expect(apply).toBeFocused();
    expect((await apiCall(page, "GET", `/v1/drafts/${id}`)).status).toBe(200);

    await page.setViewportSize({ width: 1024, height: 900 });
    await expect(page.locator("[data-width='standard']")).toHaveCount(1);
    await expect(apply).not.toHaveAttribute("aria-disabled", "true");
  } finally {
    await dropDraft(page, id);
  }
});

// The administration under 1024 px (task 21e; REQ-021-29): at 768 px the groups are read,
// « Créer » refused with the width, reached and pressed at the keyboard without opening
// anything, `c` says the width; at 1024 px it opens its dialog again. Nothing is written.
test("under 1024 px « Créer » of the groups says the width; at 1024 px it comes back", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.setViewportSize({ width: 768, height: 900 });
  await page.goto("/admin/groups");
  await atRest(page);
  const main = page.getByRole("main");
  await expect(main.getByRole("grid", { name: "Groupes" })).toBeVisible();
  const create = main.getByRole("button", { name: /^Créer/ });
  await expect(create).toHaveAttribute("aria-disabled", "true");
  await expect(create).toHaveAccessibleDescription(NARROW);
  await expectAccessible(page);
  // At the keyboard: reached, pressed, no dialog; `c` says the width politely.
  await tabTo(page, create);
  await page.keyboard.press("Enter");
  await expect(create).toBeFocused();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.keyboard.press("c");
  await expect(page.getByRole("status").filter({ hasText: NARROW })).toHaveCount(1);
  await expect(page.getByRole("dialog")).toHaveCount(0);

  await page.setViewportSize({ width: 1024, height: 900 });
  await expect(page.locator("[data-width='standard']")).toHaveCount(1);
  await expect(create).not.toHaveAttribute("aria-disabled", "true");
  await create.press("Enter");
  await expect(page.getByRole("dialog", { name: "Créer un groupe" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

// The service accounts under 1024 px (task 21e2; REQ-021-29): at 768 px « Créer » is
// refused with the width, reached and pressed at the keyboard without opening anything,
// `c` says the width; at 1024 px it opens its dialog again. No account nor secret is made.
test("under 1024 px « Créer » of the service accounts says the width; at 1024 px it comes back", async ({
  page,
  login,
}) => {
  await login("alice");
  await page.setViewportSize({ width: 768, height: 900 });
  await page.goto("/admin/accounts");
  await atRest(page);
  const main = page.getByRole("main");
  // The list or its empty state: whichever the journeys before left.
  const toolbar = main.getByRole("toolbar", { name: "Actions des comptes de service" });
  await expect(toolbar).toBeVisible();
  const create = toolbar.getByRole("button", { name: /^Créer/ });
  await expect(create).toHaveAttribute("aria-disabled", "true");
  await expect(create).toHaveAccessibleDescription(NARROW);
  await expectAccessible(page);
  await tabTo(page, create);
  await page.keyboard.press("Enter");
  await expect(create).toBeFocused();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.keyboard.press("c");
  await expect(page.getByRole("status").filter({ hasText: NARROW })).toHaveCount(1);
  await expect(page.getByRole("dialog")).toHaveCount(0);

  await page.setViewportSize({ width: 1024, height: 900 });
  await expect(page.locator("[data-width='standard']")).toHaveCount(1);
  await expect(create).not.toHaveAttribute("aria-disabled", "true");
  await create.press("Enter");
  await expect(page.getByRole("dialog", { name: "Créer un compte de service" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

// The record and the tree under 1024 px (task 21f; REQ-021-29): at 768 px « Modifier » of a
// record says the width, reached and pressed at the keyboard it opens no panel; in the
// Topography a cell of the tree and the « + » of its node say it too; at 1024 px «
// Modifier » and the « + » come back. Nothing is written; the domain is its own (RFC 5737).
test("under 1024 px « Modifier » of a record and a cell of the tree say the width", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const { slug, prefixes } = await ownedDomain(
    page,
    `zz-narrow-record-${browserName}-${Date.now().toString(36)}`,
  );
  const editPanel = page.getByRole("complementary", { name: "Panneau droit" });
  const modify = page.getByRole("main").getByRole("button", { name: /^Modifier/ });
  await page.setViewportSize({ width: 768, height: 900 });
  await page.goto(`/prefixes/${prefixes[0]!}`);
  await atRest(page);
  await expect(modify).toHaveAttribute("aria-disabled", "true");
  await expect(modify).toHaveAccessibleDescription(NARROW);
  await tabTo(page, modify);
  await page.keyboard.press("Enter");
  await expect(modify).toBeFocused();
  await expect(editPanel).toHaveCount(0);
  await expectAccessible(page);

  // The tree of the domain, its column « Nature » shown by the chooser if the width hid it.
  const domains = (await apiCall(page, "GET", `/v1/domains?slug=${slug}`)).body as {
    items: { id: string }[];
  };
  await page.goto(`/topology?domain=${domains.items[0]!.id}`);
  await atRest(page);
  const tree = page.getByRole("treegrid", { name: `Arbre des préfixes du domaine ${slug}` });
  await expect(tree.getByText("198.51.100.0/24", { exact: true })).toBeVisible();
  const chooser = page.getByRole("button", { name: "Colonnes" });
  if (await chooser.isVisible()) {
    await chooser.click();
    const nature = page.getByRole("menuitemcheckbox", { name: "Nature" });
    if ((await nature.getAttribute("aria-checked")) !== "true") await nature.click();
    await page.keyboard.press("Escape");
  }
  const detail = tree.locator('[role="gridcell"][data-column="detail"]').first();
  await expect(detail).toHaveAttribute("aria-readonly", "true");
  await expect(detail).toHaveAccessibleDescription(NARROW);
  const plus = tree.getByRole("button", { name: "Ajouter dans 198.51.100.0/24" });
  await expect(plus).toHaveAttribute("aria-disabled", "true");
  await expect(plus).toHaveAccessibleDescription(NARROW);
  await expectAccessible(page);

  await page.setViewportSize({ width: 1024, height: 900 });
  await expect(page.locator("[data-width='standard']")).toHaveCount(1);
  await expect(plus).not.toHaveAttribute("aria-disabled", "true");
  await expect(detail).not.toHaveAttribute("aria-readonly", "true");
  await page.goto(`/prefixes/${prefixes[0]!}`);
  await atRest(page);
  await expect(modify).not.toHaveAttribute("aria-disabled", "true");
});

// The guards of every journey (fixtures/journeys.ts and fixtures/index.ts): each must make
// a journey fail.
test.describe("guards of every journey", () => {
  test("an axe-core violation fails the journey", async ({ page, login }) => {
    await login("bob");
    await expectAccessible(page);
    await page.evaluate(() => {
      const image = document.createElement("img");
      image.src = "data:image/gif;base64,R0lGODlhAQABAAAAACw=";
      document.querySelector("main")?.append(image);
    });
    // The call every journey makes throws, on the rule and the element.
    await expect(expectAccessible(page)).rejects.toThrow(/image-alt: [^\n]*img/);
  });

  test.describe("the CSP", () => {
    // The browsers also say the refusal on their console, which the guard of the console
    // errors would catch: tolerated here, so that the guard of the CSP alone fails.
    test.use({ toleratedErrors: [/^console: .*Content.Security.Policy/] });

    // The guard of the CSP runs after the journey, as the fixture `page` ends: only an
    // expected failure can show it. Declared once the script is known refused and its
    // violation recorded, so that any other failure (a login broken, a CSP that lets the
    // script run) fails the test for real.
    test("an inline script fails the journey", async ({ page, login }) => {
      await login("bob");
      await page.evaluate(() => {
        const script = document.createElement("script");
        script.textContent = "window.__kartennInjected = true;";
        document.body.append(script);
      });
      await expect
        .poll(() => page.evaluate(() => window.__kartennCsp ?? []))
        .toEqual([expect.stringMatching(/^script-src(-elem)? inline$/)]);
      expect(
        await page.evaluate(() => "__kartennInjected" in window),
        "the inline script ran",
      ).toBe(false);
      test.fail();
    });
  });
});
