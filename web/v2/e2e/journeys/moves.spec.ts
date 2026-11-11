import type { Locator, Page } from "@playwright/test";
import { DATA, expect, expectAccessible, test } from "../fixtures/journeys.ts";

// Moves and the invariant of the trees of the new shell (spec 021, task 24; REQ-021-17, 32,
// 33, 34): the tree of the prefixes offers no move, a prefix changes place by its value —
// in the draft, the tree at the present unchanged and the node marked; once applied, the
// tree read again shows its new place; an entity changes parent by « Changer de parent… »,
// the valid targets only, never shown at its new place before the answer; and a property
// test: a fixed seed, 100 steps of changes of the values of prefixes and ranges, through
// the draft or applied at once, accepted or refused — each row of the tree rendered within
// the row of its parent, its parent the one the API gives; and its twin on a tree whose
// parent is declared, the hosting of the machines: 100 moves of hosts, accepted or refused,
// each row under the host the API gives, a host that may hold it. Each test writes in a
// domain or under a tag of its own (RFC 5737, TEST-NET-2 and TEST-NET-3).

/**
 * A call of the API by the page itself, under its session (same origin: CSRF); a 429 (the
 * quota of writes, 10 a second) waits as `Retry-After` says, then tries again.
 */
async function call(
  page: Page,
  method: string,
  path: string,
  body?: unknown,
  headers: Record<string, string> = {},
): Promise<{ status: number; body: unknown; etag: string | null }> {
  return page.evaluate(
    async ({ method, path, body, headers }) => {
      // The same idempotency key on each try: a write replayed is not written twice.
      const key = crypto.randomUUID();
      for (let attempt = 0; attempt <= 10; attempt++) {
        let response: Response;
        let text: string;
        try {
          response = await fetch(path, {
            method,
            headers: { "Content-Type": "application/json", "Idempotency-Key": key, ...headers },
            ...(body === undefined ? {} : { body: JSON.stringify(body) }),
          });
          text = await response.text();
        } catch (error) {
          // Said with what was asked: an answer the browser cannot read is a failure.
          throw new Error(`${method} ${path} ${JSON.stringify(body)}: ${String(error)}`);
        }
        if (response.status === 429) {
          const wait = Number(response.headers.get("Retry-After") ?? "1") || 1;
          await new Promise((done) => setTimeout(done, wait * 1000));
          continue;
        }
        return {
          status: response.status,
          body: text ? JSON.parse(text) : null,
          etag: response.headers.get("ETag"),
        };
      }
      throw new Error(`${method} ${path}: still 429 after ten tries`);
    },
    { method, path, body, headers },
  );
}

/** A slug unique to this run and this browser, after `production` (the journey `tree`). */
const unique = (what: string, browser: string) =>
  `zz-${what}-${browser}-${Date.now().toString(36)}`;

/** A domain owned by the entity of the data set, holding `operations`' objects: its id. */
async function newDomain(page: Page, slug: string, operations: object[]): Promise<string> {
  const declarations = (await call(page, "GET", "/v1/property-declarations")).body as {
    items: { id: string; key: string }[];
  };
  const owner = declarations.items.find((d) => d.key === "core.owner")!.id;
  const entities = (await call(page, "GET", `/v1/entities?slug=${DATA.entity}`)).body as {
    items: { id: string }[];
  };
  const domain = { local: 0 };
  const created = await call(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end moves",
    operations: [
      { op: "create_domain", local: 0, slug },
      {
        op: "assign_property",
        object: domain,
        kind: "domain",
        property: { id: owner },
        values: [entities.items[0]!.id],
      },
      ...operations.map((op) => ({ ...op, domain })),
    ],
  });
  expect(created.status, JSON.stringify(created.body)).toBe(201);
  const domains = (await call(page, "GET", `/v1/domains?slug=${slug}`)).body as {
    items: { id: string }[];
  };
  return domains.items[0]!.id;
}

/**
 * Applies the active draft of the tab, as its foot would: its motive written (a JSON Merge
 * Patch, task 14a), then submitted under `approval.self`.
 */
async function applyActive(page: Page, summary: string): Promise<void> {
  const active = await page.evaluate(() => sessionStorage.getItem("kartenn.draft.active"));
  const read = await call(page, "GET", `/v1/drafts/${active}`);
  const draft = await call(
    page,
    "PATCH",
    `/v1/drafts/${active}`,
    { reason: { summary } },
    { "Content-Type": "application/merge-patch+json", "If-Match": read.etag! },
  );
  expect(draft.status, JSON.stringify(draft.body)).toBe(200);
  const submitted = await call(
    page,
    "POST",
    `/v1/drafts/${active}/submission?approval=self`,
    {},
    { "If-Match": draft.etag! },
  );
  expect(submitted.status, JSON.stringify(submitted.body)).toBe(201);
}

const prefix = (value: string, nature: "network" | "container") => ({
  op: "create_prefix",
  prefix: value,
  nature,
});
const range = (first: string, last: string) => ({
  op: "create_range",
  first,
  last,
  usage: "static",
});

/** The tree of the topography of the domain `slug`. */
const treeOf = (page: Page, slug: string) =>
  page.getByRole("treegrid", { name: `Arbre des préfixes du domaine ${slug}` });
/** The rows of the tree served; those of the preview of the draft are keyed `draft:` (24b). */
const SERVED = "[role='row'][data-node]:not([data-node^='draft:'])";
const PREVIEWED = "[role='row'][data-node^='draft:']";
const labelled = (tree: Locator, rows: string, label: string) =>
  tree.locator(rows).filter({
    has: tree.page().locator(".k-tree-grid__label", {
      hasText: new RegExp(`^${label.replaceAll(".", "\\.")}$`),
    }),
  });
const rowOf = (tree: Locator, label: string) => labelled(tree, SERVED, label);
/** A row of the preview of the draft: an object at its place after the draft. */
const previewOf = (tree: Locator, label: string) => labelled(tree, PREVIEWED, label);
/** The words of the states of the draft on a row (24d), never told by the mark alone. */
const MOVED_WORDS = "(déplacé dans le brouillon)";
const PREVIEW_WORDS = "(aperçu du brouillon)";
const DRAFT_WORDS = "(en brouillon)";
/** The row at its present place of an object the draft moves, and its row of the preview. */
async function expectMoved(present: Locator, after: Locator): Promise<void> {
  await expect(present).toHaveAttribute("data-state", "moved");
  await expect(present).toContainText(MOVED_WORDS);
  await expect(after).toHaveAttribute("data-state", "preview");
  await expect(after).toContainText(PREVIEW_WORDS);
  for (const [row, words] of [
    [present, [PREVIEW_WORDS, DRAFT_WORDS]],
    [after, [MOVED_WORDS, DRAFT_WORDS]],
  ] as const)
    for (const other of words) await expect(row).not.toContainText(other);
}

/** Opens every node of the tree, one chevron at a time, each level read before the next. */
async function expandAll(tree: Locator): Promise<void> {
  for (let i = 0; i < 100; i++) {
    // The children of a node opened are read: no row that reads a page, no node busy.
    await expect(tree.locator("[data-more], [aria-busy='true']")).toHaveCount(0);
    const closed = tree.locator("[role='row'][aria-expanded='false']").first();
    if ((await closed.count()) === 0) return;
    await closed.locator(".k-tree-grid__twisty").click();
  }
}

/** A row as rendered, or as the API lists it: its level, its label, its parent's label. */
interface Line {
  level: number;
  label: string;
  parent: string | null;
}

/**
 * The rows of the tree served rendered, depth first, each with its visual parent (the row
 * of level − 1 above); the preview of a draft, never above a row served, left out.
 */
async function rendered(tree: Locator): Promise<Line[]> {
  const rows = await tree.locator(`${SERVED}:not([data-more])`).evaluateAll((all) =>
    all.map((row) => ({
      level: Number(row.getAttribute("aria-level")),
      label: row.querySelector(".k-tree-grid__label")?.textContent?.trim() ?? "",
    })),
  );
  const stack: string[] = [];
  return rows.map(({ level, label }) => {
    stack.length = level - 1;
    const parent = level === 1 ? null : (stack[level - 2] ?? null);
    stack[level - 1] = label;
    return { level, label, parent };
  });
}

type TreeNode =
  | { kind: "prefix"; id: string; value: string }
  | { kind: "range"; id: string; first: string; last: string }
  | { kind: "address"; id: string; value: string };
const labelOf = (node: TreeNode) =>
  node.kind === "range" ? `${node.first} – ${node.last}` : node.value;

/** The tree as the API gives it, depth first: the first level, then each prefix's children. */
async function served(page: Page, domain: string): Promise<Line[]> {
  const lines: Line[] = [];
  const walk = async (url: string, level: number, parent: string | null) => {
    for (let next: string | null = url; next;) {
      const page_ = (await call(page, "GET", next)).body as {
        items: TreeNode[];
        links: { next?: string | null };
      };
      for (const node of page_.items) {
        lines.push({ level, label: labelOf(node), parent });
        if (node.kind === "prefix") {
          await walk(`/v1/prefixes/${node.id}/children`, level + 1, labelOf(node));
        }
      }
      next = page_.links.next ?? null;
    }
  };
  await walk(`/v1/domains/${domain}/roots`, 1, null);
  return lines;
}

/** The first and last addresses of a label: a CIDR, a range « a – b », an address (IPv4). */
function bounds(label: string): [bigint, bigint] {
  const ip = (text: string) =>
    text.split(".").reduce((n, part) => (n << 8n) + BigInt(Number(part)), 0n);
  if (label.includes(" – ")) {
    const [first, last] = label.split(" – ");
    return [ip(first!), ip(last!)];
  }
  const [address, length] = label.split("/");
  const start = ip(address!);
  const size = length === undefined ? 1n : 1n << BigInt(32 - Number(length));
  return [start, start + size - 1n];
}

/** REQ-021-34: each row within the row of its parent (comparison of the values shown). */
function expectContained(lines: readonly Line[]): void {
  for (const line of lines) {
    if (line.parent === null) continue;
    const [first, last] = bounds(line.label);
    const [low, high] = bounds(line.parent);
    expect(
      low <= first && last <= high,
      `${line.label} shown under ${line.parent}, which does not contain it`,
    ).toBe(true);
  }
}

/**
 * One changeset of `operations` for a property test, at once or through a draft of its
 * own (not the active draft of the tab): whether it was applied.
 */
async function submit(page: Page, operations: object[], viaDraft: boolean): Promise<boolean> {
  if (!viaDraft) {
    const direct = await call(page, "POST", "/v1/changesets?approval=self", {
      motive: "property test",
      operations,
    });
    expect([201, 422], JSON.stringify(direct.body)).toContain(direct.status);
    return direct.status === 201;
  }
  const draft = await call(page, "POST", "/v1/drafts", {
    title: "property test",
    reason: { summary: "property test" },
    operations,
  });
  expect(draft.status, JSON.stringify(draft.body)).toBe(201);
  const id = (draft.body as { id: string }).id;
  const submitted = await call(
    page,
    "POST",
    `/v1/drafts/${id}/submission?approval=self`,
    {},
    { "If-Match": draft.etag! },
  );
  expect([201, 422], JSON.stringify(submitted.body)).toContain(submitted.status);
  if (submitted.status === 201) return true;
  // Refused, the draft stays: taken out, so that none accumulates.
  const left = await call(page, "GET", `/v1/drafts/${id}`);
  await call(page, "DELETE", `/v1/drafts/${id}`, undefined, { "If-Match": left.etag! });
  return false;
}

/** A generator of numbers in [0, 1), from a fixed seed (mulberry32). */
function seeded(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

test("the tree of the prefixes offers no move: a prefix changes place by its value", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const slug = unique("derived", browserName);
  const domain = await newDomain(page, slug, [
    prefix("198.51.100.0/24", "container"),
    prefix("198.51.100.0/25", "container"),
    prefix("198.51.100.64/26", "network"),
  ]);
  await page.goto(`/topology?domain=${domain}`);
  const tree = treeOf(page, slug);
  await expect(rowOf(tree, "198.51.100.0/24")).toBeVisible();
  await expandAll(tree);
  const moved = rowOf(tree, "198.51.100.64/26");
  await expect(moved).toHaveAttribute("aria-level", "3");

  // No drag, no cut and paste, no « Déplacer vers… » (REQ-021-17).
  await expect(tree.locator("[draggable='true']")).toHaveCount(0);
  await moved.locator(".k-tree-grid__label").click();
  await page.keyboard.press("Control+x");
  await rowOf(tree, "198.51.100.0/24").locator(".k-tree-grid__label").click();
  await page.keyboard.press("Control+v");
  await expect(tree.locator("[data-move]")).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Déplacer vers/ })).toHaveCount(0);
  await expect(page.getByRole("menuitem", { name: /Déplacer vers/ })).toHaveCount(0);
  expect(await rendered(tree)).toEqual(await served(page, domain));

  // Its value changed in the draft: the tree at the present unchanged, the node marked.
  await moved.locator(".k-tree-grid__label").click();
  await page.keyboard.press("e");
  const form = page.getByRole("form", { name: /^Modifier : Préfixe 198\.51\.100\.64\/26$/ });
  await form.getByRole("textbox", { name: "Valeur" }).fill("198.51.100.128/26");
  await form.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(page.getByRole("button", { name: /^Brouillon 1 opération$/ })).toBeVisible();
  await expect(moved).toHaveAttribute("aria-level", "3");
  // The preview of its simulation (24b): the node again at its place after, under the /24;
  // at its present place moved in the draft, at its place after a row of the preview (24d).
  const after = previewOf(tree, "198.51.100.128/26");
  await expect(after).toHaveAttribute("aria-level", "2");
  await expectMoved(moved, after);
  expect(await rendered(tree)).toEqual(await served(page, domain));
  await expectAccessible(page);

  // Applied (the draft submitted): the tree read again shows its new place, under the /24.
  await applyActive(page, "end-to-end moves");
  // Read again on the event of the stream that names the prefix (022).
  const now = rowOf(tree, "198.51.100.128/26");
  await expect(now).toHaveAttribute("aria-level", "2", { timeout: 15_000 });
  await expect(rowOf(tree, "198.51.100.64/26")).toHaveCount(0);
  await expect(tree.locator(PREVIEWED)).toHaveCount(0);
  await expandAll(tree);
  const lines = await rendered(tree);
  expect(lines).toEqual(await served(page, domain));
  expectContained(lines);
});

test("a prefix inserted in the draft shows its future children under it before it is applied", async ({
  page,
  login,
  browserName,
}) => {
  await login("alice");
  const slug = unique("preview", browserName);
  const domain = await newDomain(page, slug, [
    prefix("198.51.100.0/24", "container"),
    prefix("198.51.100.0/26", "network"),
    prefix("198.51.100.64/26", "network"),
  ]);
  await page.goto(`/topology?domain=${domain}`);
  const tree = treeOf(page, slug);
  await expect(rowOf(tree, "198.51.100.0/24")).toBeVisible();
  await expandAll(tree);
  const before = await served(page, domain);

  // A /25 inserted between the /24 and its children, from the « + » of the /24, in the draft.
  await rowOf(tree, "198.51.100.0/24").locator(".k-tree-grid__label").click();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Enter");
  const form = page.getByRole("form", { name: "Ajouter dans 198.51.100.0/24" });
  await form.getByRole("textbox", { name: "Valeur" }).fill("198.51.100.0/25");
  // A container: a network holds no prefix (core.prefix.network_not_leaf).
  await form.getByRole("combobox", { name: /^Nature/ }).click();
  await page.getByRole("option", { name: /^Conteneur/ }).click();
  await form.getByRole("button", { name: "Ajouter au brouillon" }).click();
  await expect(page.getByRole("button", { name: /^Brouillon 1 opération$/ })).toBeVisible();

  // The preview: the /25 under the /24, its future children under it, rows of the preview;
  // the children moved in the draft at their present place (24d); the tree served unchanged.
  const inserted = previewOf(tree, "198.51.100.0/25");
  await expect(inserted).toHaveAttribute("aria-level", "2");
  await expect(inserted).toHaveAttribute("data-state", "preview");
  await expect(inserted).toContainText(PREVIEW_WORDS);
  await expect(page.getByRole("status").filter({ hasText: "Aperçu du brouillon" })).toContainText(
    "3 nœuds apparaissent à leur place après application",
  );
  await inserted.locator("[role='gridcell']").first().click();
  await page.keyboard.press("ArrowRight");
  for (const child of ["198.51.100.0/26", "198.51.100.64/26"]) {
    await expect(previewOf(tree, child)).toHaveAttribute("aria-level", "3");
    await expect(rowOf(tree, child)).toHaveAttribute("aria-level", "2");
    await expectMoved(rowOf(tree, child), previewOf(tree, child));
  }
  // A row of the preview chosen stays selected, its object's content beside the tree.
  await previewOf(tree, "198.51.100.0/26").locator(".k-tree-grid__label").click();
  await expect(previewOf(tree, "198.51.100.0/26")).toHaveAttribute("aria-selected", "true");
  await expect(page).toHaveURL(/[?&]node=[0-9a-f-]{36}/);
  expect(await rendered(tree)).toEqual(before);
  await expectAccessible(page);

  // Applied: the preview gone, the tree read again shows the /25 and its children.
  await applyActive(page, "end-to-end preview");
  await expect(rowOf(tree, "198.51.100.0/25")).toHaveAttribute("aria-level", "2", {
    timeout: 15_000,
  });
  await expect(tree.locator(PREVIEWED)).toHaveCount(0);
  await expandAll(tree);
  const lines = await rendered(tree);
  expect(lines).toEqual(await served(page, domain));
  expect(lines.filter((line) => line.parent === "198.51.100.0/25").map((l) => l.label)).toEqual([
    "198.51.100.0/26",
    "198.51.100.64/26",
  ]);
});

test.describe("a slowed answer", () => {
  test("an entity changes parent by its menu, the valid targets only, never before the answer", async ({
    page,
    login,
    browserName,
  }) => {
    await login("alice");
    const slug = unique("node", browserName);
    const created = await call(page, "POST", "/v1/changesets?approval=self", {
      motive: "end-to-end moves",
      operations: [
        { op: "create_entity", local: 0, slug: `${slug}-a` },
        { op: "create_entity", local: 1, slug: `${slug}-b`, parent: { local: 0 } },
        { op: "create_entity", local: 2, slug: `${slug}-c` },
      ],
    });
    expect(created.status, JSON.stringify(created.body)).toBe(201);
    const a = (
      (await call(page, "GET", `/v1/entities?slug=${slug}-a`)).body as {
        items: { id: string }[];
      }
    ).items[0]!.id;

    await page.goto(`/entities/${a}`);
    // At the keyboard: the menu, its first entry, the panel's first field.
    const actions = page.getByRole("button", { name: "Actions" });
    await actions.press("Enter");
    const entry = page.getByRole("menuitem", { name: "Changer de parent…" });
    await expect(entry).toBeFocused();
    await page.keyboard.press("Enter");
    const form = page.getByRole("form", { name: `Changer de parent : Entité ${slug}-a` });
    const parent = form.getByRole("combobox", { name: /^Parent/ });
    await expect(parent).toBeFocused();
    await expect(parent).toHaveValue("Aucun");
    await expectAccessible(page);
    // Typed: the valid targets that match, neither itself nor its child b (a cycle).
    await page.keyboard.press("Control+a");
    await page.keyboard.type(slug);
    await expect(page.getByRole("option", { name: `${slug}-c` })).toBeVisible();
    await expect(page.getByRole("option", { name: `${slug}-a`, exact: true })).toHaveCount(0);
    await expect(page.getByRole("option", { name: `${slug}-b` })).toHaveCount(0);
    // A root among them.
    await page.keyboard.press("Control+a");
    await page.keyboard.type("Auc");
    await expect(page.getByRole("option", { name: "Aucun" })).toBeVisible();
    await page.keyboard.press("Control+a");
    await page.keyboard.type(`${slug}-c`);
    const only = page.getByRole("option", { name: `${slug}-c` });
    await expect(only).toBeVisible();
    await page.keyboard.press("ArrowDown");
    // The highlighted suggestion is the field's `aria-activedescendant`.
    await expect(parent).toHaveAttribute("aria-activedescendant", (await only.getAttribute("id"))!);
    await page.keyboard.press("Enter");
    await expect(parent).toHaveValue(`${slug}-c`);

    // Applied now, its answer slowed: the record keeps its parent until it is read again.
    let release!: () => void;
    const held = new Promise<void>((done) => (release = done));
    const submission = (url: URL) =>
      url.pathname === "/v1/changesets" && url.search === "?approval=self";
    await page.route(submission, async (route) => {
      await held;
      await route.continue();
    });
    await form.getByRole("button", { name: "Choisir l'action" }).click();
    await page.getByRole("menuitem", { name: "Appliquer" }).click();
    const motive = form.getByRole("textbox", { name: "Motif" });
    await expect(motive).toHaveValue(`Changement de parent : Entité ${slug}-a → ${slug}-c`);
    const sent = page.waitForRequest((r) => submission(new URL(r.url())));
    await motive.press("Control+Enter");
    await sent;
    const attributes = page.locator(".k-attribute-list").first();
    await expect(attributes).not.toContainText(`${slug}-c`);
    release();
    await expect(
      page
        .getByRole("status")
        .filter({ hasText: new RegExp(`Appliqué : Entité ${slug}-a`) })
        .first(),
    ).toBeVisible();
    await expect(attributes).toContainText(`${slug}-c`);
    // The panel closed, the focus back on the button of the menu that opened it.
    await expect(form).toHaveCount(0);
    await expect(actions).toBeFocused();
    await page.unroute(submission);
  });
});

// The hosting of a machine (task 18b; plugin Machines, in Vue since the 18cb and the 18cd):
// its host changed from its record, or in the tree « Hébergement et clusters » (a
// `TreeGrid` whose parent is declared: Ctrl+X then Ctrl+V), each host checked by `canMove`
// of the SDK, the move to the draft by `draft.move` and its simulation, never a changeset.
test.describe("the host of a machine", () => {
  /**
   * Two sites; a physical host on the first with its virtual guest; a virtual machine on the
   * second; a cluster without a site.
   */
  async function hosting(page: Page, tag: string) {
    const entity = (
      (await call(page, "GET", `/v1/entities?slug=${DATA.entity}`)).body as {
        items: { id: string }[];
      }
    ).items[0]!.id;
    const created = await call(page, "POST", "/v1/changesets?approval=self", {
      motive: "end-to-end hosting",
      operations: [
        { op: "create_site", local: 0, slug: `${tag}-a` },
        { op: "create_site", local: 1, slug: `${tag}-b` },
        {
          op: "machines.create_machine",
          local: 2,
          name: `${tag}-host`,
          kind: "physical",
          owner: { id: entity },
          site: { local: 0 },
        },
        {
          op: "machines.create_machine",
          name: `${tag}-guest`,
          kind: "virtual",
          owner: { id: entity },
          site: { local: 0 },
          host: { machine: { local: 2 } },
        },
        {
          op: "machines.create_machine",
          name: `${tag}-away`,
          kind: "virtual",
          owner: { id: entity },
          site: { local: 1 },
        },
        { op: "machines.create_cluster", slug: `${tag}-c`, owner: { id: entity } },
      ],
    });
    expect(created.status, JSON.stringify(created.body)).toBe(201);
    const id = async (name: string) =>
      (
        (await call(page, "GET", `/v1/machines?name=${name}`)).body as {
          items: { id: string }[];
        }
      ).items[0]!.id;
    const clusters = (await call(page, "GET", `/v1/clusters?slug=${tag}-c`)).body as {
      items: { id: string }[];
    };
    return {
      host: await id(`${tag}-host`),
      guest: await id(`${tag}-guest`),
      away: await id(`${tag}-away`),
      cluster: clusters.items[0]!.id,
    };
  }

  /** The tree of the hosting, its rows by their label, the focus moved by the arrows only. */
  function hostingTree(page: Page) {
    const view = page.locator("machines-clusters");
    const tree = view.getByRole("treegrid", { name: "Hébergement et clusters" });
    const row = (label: string) =>
      tree.locator("[role='row'][data-node]").filter({
        has: page.locator(".k-tree-grid__label", { hasText: new RegExp(`^${label}$`) }),
      });
    /** Up or Down from the row of the focus to the row of `label`. */
    async function arrowTo(label: string): Promise<void> {
      const keys = await tree
        .locator("[role='row'][data-node]")
        .evaluateAll((rows) => rows.map((r) => (r as HTMLElement).dataset.node));
      const from = await page.evaluate(
        () => (document.activeElement?.closest("[role='row']") as HTMLElement | null)?.dataset.node,
      );
      const to = await row(label).getAttribute("data-node");
      const delta = keys.indexOf(to!) - keys.indexOf(from);
      for (let i = 0; i < Math.abs(delta); i++) {
        await page.keyboard.press(delta > 0 ? "ArrowDown" : "ArrowUp");
      }
      await expect(row(label).locator("[data-column='']")).toBeFocused();
    }
    /** What the view or the tree says in a status region. */
    const says = (text: string) => view.getByRole("status").filter({ hasText: text });
    return { view, tree, row, arrowTo, says };
  }

  test("in the tree, to a cluster at the keyboard: « en attente » at its place, then in the draft; no changeset", async ({
    page,
    login,
    browserName,
  }) => {
    await login("alice");
    const tag = unique("tree", browserName);
    const { away, cluster } = await hosting(page, tag);
    const changesets: string[] = [];
    page.on("request", (request) => {
      if (request.method() === "POST" && request.url().includes("/v1/changesets")) {
        changesets.push(request.url());
      }
    });
    let release!: () => void;
    const held = new Promise<void>((done) => (release = done));
    const simulation = (url: URL) => /^\/v1\/drafts\/[0-9a-f-]+\/simulation$/.test(url.pathname);
    await page.route(simulation, async (route) => {
      await held;
      await route.continue();
    });
    await page.goto("/x/machines/clusters");
    const { tree, row, arrowTo, says } = hostingTree(page);
    await expect(row(`${tag}-away`)).toHaveAttribute("aria-level", "1");
    await expectAccessible(page);
    // The focus put on the machine once; from there, the keyboard only.
    await row(`${tag}-away`).locator("[data-column='']").click();
    await page.keyboard.press("Control+x");
    await expect(row(`${tag}-away`)).toHaveAttribute("data-move", "cut");
    await arrowTo(`${tag}-c`);
    const sent = page.waitForRequest((r) => simulation(new URL(r.url())));
    await page.keyboard.press("Control+v");
    await sent;
    // Its simulation held: the move waits at its place.
    await expect(row(`${tag}-away`)).toHaveAttribute("data-move", "pending");
    await expect(row(`${tag}-away`)).toContainText("en attente");
    await expect(says(`Déplacement de ${tag}-away dans ${tag}-c demandé`)).toHaveCount(1);
    await expect(row(`${tag}-away`)).toHaveAttribute("aria-level", "1");
    release();
    await expect(
      says(`Déplacement de ${tag}-away vers ${tag}-c au brouillon, tenu par sa simulation`),
    ).toHaveCount(1);
    // At its place, marked as moved in the draft; nothing under the cluster yet, its guests
    // not read (closed): no preview.
    const present = tree.locator(`[role='row'][data-node='${away}']`);
    const preview = tree.locator(`[role='row'][data-node='preview:${away}']`);
    await expect(present).toHaveAttribute("data-state", "moved");
    await expect(present).toContainText("déplacé dans le brouillon");
    await expect(present).toHaveAttribute("aria-level", "1");
    await expect(present).toHaveCount(1);
    await expect(preview).toHaveCount(0);
    await expect(says(`Son aperçu se montre sous ${tag}-c une fois ouvert.`)).toHaveCount(1);
    await expectAccessible(page);
    // The cluster opened (the focus on it since Ctrl+V): its guests read, the preview of the
    // machine under it, told apart from its row at its place by its state (021/A27).
    await expect(row(`${tag}-c`).locator("[data-column='']")).toBeFocused();
    await page.keyboard.press("ArrowRight");
    await expect(preview).toHaveAttribute("data-state", "preview");
    await expect(preview).toHaveAttribute("aria-level", "2");
    await expect(preview).toContainText("aperçu du brouillon");
    await expect(present).toHaveAttribute("data-state", "moved");
    await expect(present).toHaveAttribute("aria-level", "1");
    await expectAccessible(page);
    await page.unroute(simulation);
    expect(changesets).toEqual([]);
    const drafts = (await call(page, "GET", "/v1/drafts")).body as { items: { id: string }[] };
    const operations = await Promise.all(
      drafts.items.map(
        async (d) =>
          ((await call(page, "GET", `/v1/drafts/${d.id}`)).body as { operations: unknown[] })
            .operations,
      ),
    );
    expect(operations.flat()).toContainEqual({
      op: "machines.update_machine",
      id: { id: away },
      host: { cluster: { id: cluster } },
    });
  });

  test("in the tree, a host under its guest refused by the client; to a host on another site, the server warns", async ({
    page,
    login,
    browserName,
  }) => {
    await login("alice");
    const tag = unique("treesite", browserName);
    const { away } = await hosting(page, tag);
    const writes: string[] = [];
    page.on("request", (request) => {
      if (request.method() !== "GET" && request.url().includes("/v1/")) writes.push(request.url());
    });
    await page.goto("/x/machines/clusters");
    const { tree, row, arrowTo, says } = hostingTree(page);
    // The host opened: its guest under it.
    await row(`${tag}-host`).locator("[data-column='']").click();
    await page.keyboard.press("ArrowRight");
    await expect(row(`${tag}-guest`)).toHaveAttribute("aria-level", "2");
    // Cut, then pasted into its own guest: refused by `canMove`, with the rule; nothing sent.
    await page.keyboard.press("Control+x");
    await arrowTo(`${tag}-guest`);
    await page.keyboard.press("Control+v");
    await expect(says("elle l'héberge, ou c'est elle (machines.host.invalid)")).toHaveCount(1);
    await expect(row(`${tag}-host`)).toHaveAttribute("data-move", "cut");
    await page.keyboard.press("Escape");
    await expect(row(`${tag}-host`)).not.toHaveAttribute("data-move");
    expect(writes).toEqual([]);
    // The machine of the other site, to the host: held by the simulation, the site warned of.
    await arrowTo(`${tag}-away`);
    await page.keyboard.press("Control+x");
    await arrowTo(`${tag}-host`);
    await page.keyboard.press("Control+v");
    await expect(
      says(`Déplacement de ${tag}-away vers ${tag}-host au brouillon, tenu par sa simulation`),
    ).toHaveCount(1);
    await expect(says("machines.host.site_mismatch")).toHaveCount(1);
    // The host open, its guests read: the machine moved at its place, its preview under the
    // host, told apart by their state (021/A27).
    await expect(tree.locator(`[role='row'][data-node='${away}']`)).toHaveAttribute(
      "aria-level",
      "1",
    );
    await expect(tree.locator(`[role='row'][data-node='${away}']`)).toHaveAttribute(
      "data-state",
      "moved",
    );
    const preview = tree.locator(`[role='row'][data-node='preview:${away}']`);
    await expect(preview).toHaveAttribute("data-state", "preview");
    await expect(preview).toHaveAttribute("aria-level", "2");
    await expectAccessible(page);
    expect(writes.filter((url) => url.includes("/v1/changesets"))).toEqual([]);
  });

  test("a host cannot go under itself nor its guest: refused by the client, with the rule", async ({
    page,
    login,
    browserName,
  }) => {
    await login("alice");
    const tag = unique("hosting", browserName);
    const { host } = await hosting(page, tag);
    await page.goto(`/x/machines/machines/${host}`);
    const record = page.locator("machines-machines");
    const form = record.getByRole("region", { name: "Hébergement" });
    const field = form.getByRole("combobox", { name: /^Nouvel hébergeur/ });
    // Each refused host said in its name, with its reason and the code of the server rule.
    const option = (name: string) => page.getByRole("option").filter({ hasText: name });
    const refused = (name: string) =>
      option(`${name} — refusé : elle l'héberge, ou c'est elle (machines.host.invalid)`);
    await field.fill(tag);
    await expect(refused(`${tag}-host`)).toHaveCount(1);
    await expect(refused(`${tag}-guest`)).toHaveCount(1);
    await expect(option(`${tag}-away`)).not.toContainText("refusé");
    await expectAccessible(page);
    await field.fill("Aucun");
    await expect(option("Aucun (non hébergée) — hébergeur actuel")).toHaveCount(1);
    // Taken and sent at the keyboard all the same: refused on the field, nothing in the draft.
    const writes: string[] = [];
    page.on("request", (request) => {
      if (request.method() !== "GET" && request.url().includes("/v1/")) writes.push(request.url());
    });
    await field.fill(`${tag}-guest`);
    await expect(refused(`${tag}-guest`)).toHaveCount(1);
    await field.press("ArrowDown");
    await field.press("Enter");
    await field.press("Tab");
    await page.keyboard.press("Enter");
    await expect(field).toHaveAttribute("aria-invalid", "true");
    await expect(field).toBeFocused();
    await expect(form).toContainText(
      "refusé : elle l'héberge, ou c'est elle (machines.host.invalid)",
    );
    expect(writes).toEqual([]);
  });

  test("to a host on another site: accepted by the client, « en attente » at its place, the server warns", async ({
    page,
    login,
    browserName,
  }) => {
    await login("alice");
    const tag = unique("site", browserName);
    const { host, away } = await hosting(page, tag);
    const changesets: string[] = [];
    page.on("request", (request) => {
      if (request.method() === "POST" && request.url().includes("/v1/changesets")) {
        changesets.push(request.url());
      }
    });
    await page.goto(`/x/machines/machines/${away}`);
    const record = page.locator("machines-machines");
    await expect(record.getByRole("heading", { level: 1, name: `${tag}-away` })).toBeVisible();
    const hostedBy = record.locator("dt:has-text('Hébergée par') + dd");
    await expect(hostedBy).toHaveText("—non renseigné");
    const form = record.getByRole("region", { name: "Hébergement" });
    const field = form.getByRole("combobox", { name: /^Nouvel hébergeur/ });
    await field.fill(`${tag}-host`);
    const offered = page.getByRole("option").filter({ hasText: `${tag}-host` });
    await expect(offered).toHaveCount(1);
    await expect(offered).not.toContainText("refusé");

    // Its simulation held: the move waits at its place, the host read unchanged.
    let release!: () => void;
    const held = new Promise<void>((done) => (release = done));
    const simulation = (url: URL) => /^\/v1\/drafts\/[0-9a-f-]+\/simulation$/.test(url.pathname);
    await page.route(simulation, async (route) => {
      await held;
      await route.continue();
    });
    // At the keyboard: the host taken among the suggestions, then the button after it.
    await field.press("ArrowDown");
    await field.press("Enter");
    await expect(field).toHaveValue(`${tag}-host`);
    await field.press("Tab");
    const submit = form.getByRole("button", { name: "Déplacer au brouillon" });
    await expect(submit).toBeFocused();
    const sent = page.waitForRequest((r) => simulation(new URL(r.url())));
    await page.keyboard.press("Enter");
    await sent;
    await expect(
      form
        .getByRole("status")
        .filter({ hasText: `Déplacement vers ${tag}-host en attente du serveur` }),
    ).toHaveCount(1);
    await expect(hostedBy).toHaveText("—non renseigné");
    release();
    // Held by its simulation; the other site warned of, not refused (the rule is a warning).
    await expect(
      form
        .getByRole("status")
        .filter({ hasText: `Déplacement vers ${tag}-host au brouillon, tenu` }),
    ).toHaveCount(1);
    await expect(form.locator("[data-tone=warn]")).toContainText("machines.host.site_mismatch");
    await expect(hostedBy).toHaveText("—non renseigné");
    await expectAccessible(page);
    await page.unroute(simulation);
    // In the draft, no changeset sent.
    expect(changesets).toEqual([]);
    const drafts = (await call(page, "GET", "/v1/drafts")).body as { items: { id: string }[] };
    const operations = await Promise.all(
      drafts.items.map(
        async (d) =>
          ((await call(page, "GET", `/v1/drafts/${d.id}`)).body as { operations: unknown[] })
            .operations,
      ),
    );
    expect(operations.flat()).toContainEqual({
      op: "machines.update_machine",
      id: { id: away },
      host: { machine: { id: host } },
    });
  });

  test("property: 100 moves of hosts, accepted or refused, and every row under the host the API gives", async ({
    page,
    login,
    browserName,
  }) => {
    // One browser is enough: the server's answers, not the engine's, make the trees.
    test.skip(browserName !== "chromium", "property test: one browser");
    test.setTimeout(600_000);
    await login("alice");
    const tag = unique("hostprop", browserName);
    const entity = (
      (await call(page, "GET", `/v1/entities?slug=${DATA.entity}`)).body as {
        items: { id: string }[];
      }
    ).items[0]!.id;
    // Two clusters, and machines of each kind: the physical ones host, the virtual ones
    // host containers, the containers and the appliance host nothing (009 M5). No site: the
    // moves raise no `machines.host.site_mismatch` (a warning, shown by the journeys above).
    const kinds = {
      p1: "physical",
      p2: "physical",
      v1: "virtual",
      v2: "virtual",
      v3: "virtual",
      c1: "container",
      c2: "container",
      c3: "container",
      a1: "appliance",
    } as const;
    type Kind = (typeof kinds)[keyof typeof kinds] | "cluster";
    const created = await call(page, "POST", "/v1/changesets?approval=self", {
      motive: "end-to-end hosting property",
      operations: [
        { op: "machines.create_cluster", local: 0, slug: `${tag}-k1`, owner: { id: entity } },
        { op: "machines.create_cluster", slug: `${tag}-k2`, owner: { id: entity } },
        ...Object.entries(kinds).map(([suffix, kind], i) => ({
          op: "machines.create_machine",
          local: i + 1,
          name: `${tag}-${suffix}`,
          kind,
          owner: { id: entity },
          // A first hosting: v1 on p1, c1 on v1, v2 on the first cluster.
          ...(suffix === "v1" ? { host: { machine: { local: 1 } } } : {}),
          ...(suffix === "c1" ? { host: { machine: { local: 3 } } } : {}),
          ...(suffix === "v2" ? { host: { cluster: { local: 0 } } } : {}),
        })),
      ],
    });
    expect(created.status, JSON.stringify(created.body)).toBe(201);

    type Served = {
      id: string;
      name: string;
      kind: Kind;
      host?: { machine?: { id: string }; cluster?: { id: string } } | null;
    };
    const mine = async (): Promise<Served[]> =>
      (
        (await call(page, "GET", `/v1/machines?name_prefix=${encodeURIComponent(`${tag}-`)}`))
          .body as { items: Served[] }
      ).items;
    const clusters = await Promise.all(
      ["k1", "k2"].map(async (suffix) => {
        const read = (await call(page, "GET", `/v1/clusters?slug=${tag}-${suffix}`)).body as {
          items: { id: string; slug: string }[];
        };
        return { id: read.items[0]!.id, label: read.items[0]!.slug, kind: "cluster" as const };
      }),
    );
    const machines = (await mine()).map((m) => ({ id: m.id, label: m.name, kind: m.kind }));
    expect(machines).toHaveLength(Object.keys(kinds).length);
    const nodes = [...clusters, ...machines];
    const kindOf = new Map(nodes.map((n) => [n.label, n.kind]));

    /** The hosting as the API gives it: each node of the run, its level and its host. */
    async function servedHosting(): Promise<Line[]> {
      const labelOf = new Map(nodes.map((n) => [n.id, n.label]));
      const hostOf = new Map<string, string | null>(clusters.map((c) => [c.label, null]));
      for (const m of await mine()) {
        const host = m.host?.machine?.id ?? m.host?.cluster?.id;
        hostOf.set(m.name, host ? labelOf.get(host)! : null);
      }
      const levelOf = (label: string): number => {
        const host = hostOf.get(label);
        return host ? levelOf(host) + 1 : 1;
      };
      return [...hostOf].map(([label, parent]) => ({ level: levelOf(label), label, parent }));
    }

    await page.goto("/x/machines/clusters");
    const { tree, row } = hostingTree(page);
    await expect(row(`${tag}-k1`)).toBeVisible();
    /**
     * The tree rendered, every node of the run opened: each row of the run with its visual
     * parent (the row of level − 1 above it); the rows of the other journeys, sharing the
     * first level, walked but left out, as the rows of a preview.
     */
    async function renderedHosting(): Promise<Line[]> {
      const closed = tree
        .locator("[role='row'][aria-expanded='false']")
        .filter({ has: page.locator(".k-tree-grid__label", { hasText: `${tag}-` }) });
      const more = tree.locator("[role='row'][data-more]");
      let opened = false;
      for (let i = 0; i < 100 && !opened; i++) {
        await expect(tree.locator("[aria-busy='true']")).toHaveCount(0);
        if ((await more.count()) > 0) await more.first().click();
        else if ((await closed.count()) > 0)
          await closed.first().locator(".k-tree-grid__twisty").click();
        else opened = true;
      }
      if (!opened) {
        // Never compared on a tree half open: the rows left closed are named.
        const left = await closed.locator(".k-tree-grid__label").allTextContents();
        throw new Error(`nœuds restés fermés après 100 ouvertures : ${left.join(", ")}`);
      }
      const rows = await tree
        .locator(
          "[role='row'][data-node]:not([data-node^='preview:']):not([data-more]):not(.k-tree-grid__notice)",
        )
        .evaluateAll((all) =>
          all.map((r) => ({
            level: Number(r.getAttribute("aria-level")),
            label: r.querySelector(".k-tree-grid__label")?.textContent?.trim() ?? "",
          })),
        );
      const stack: string[] = [];
      return rows
        .map(({ level, label }) => {
          stack.length = level - 1;
          const parent = level === 1 ? null : (stack[level - 2] ?? null);
          stack[level - 1] = label;
          return { level, label, parent };
        })
        .filter((line) => line.label.startsWith(`${tag}-`));
    }
    const sorted = (lines: Line[]) =>
      [...lines].sort((a, b) => (a.label < b.label ? -1 : a.label > b.label ? 1 : 0));
    /** REQ-021-34: each row under a parent that may host it (009 M5), as the server says. */
    const mayHost = (guest: Kind, host: Kind) =>
      (guest === "virtual" && (host === "physical" || host === "cluster")) ||
      (guest === "container" && (host === "physical" || host === "virtual" || host === "cluster"));
    function expectSupported(lines: readonly Line[]): void {
      for (const line of lines) {
        if (line.parent === null) continue;
        const [guest, host] = [kindOf.get(line.label)!, kindOf.get(line.parent)!];
        expect(
          mayHost(guest, host),
          `${line.label} (${guest}) shown under ${line.parent} (${host})`,
        ).toBe(true);
      }
    }

    const random = seeded(0x2124);
    const pick = <T>(list: readonly T[]): T => list[Math.floor(random() * list.length)]!;
    const outcomes = { applied: 0, refused: 0 };
    // 100 steps, the other 100 of REQ-021-34 in the property test of the values.
    for (let step = 1; step <= 100; step++) {
      const guest = pick(machines);
      // Any target, of a kind that may host or not, itself among them; or none.
      const target = random() < 0.15 ? null : pick(nodes);
      const host =
        target === null
          ? null
          : target.kind === "cluster"
            ? { cluster: { id: target.id } }
            : { machine: { id: target.id } };
      const operations = [{ op: "machines.update_machine", id: { id: guest.id }, host }];
      if (await submit(page, operations, random() < 0.5)) outcomes.applied += 1;
      else outcomes.refused += 1;
      // Within the quotas of the instance, as the property test of the values.
      await page.waitForTimeout(400);

      if (step % 10 === 0) {
        const expected = sorted(await servedHosting());
        expectSupported(expected);
        await expect
          .poll(async () => sorted(await renderedHosting()), {
            message: `step ${step}: the tree rendered is the API's`,
            timeout: 20_000,
          })
          .toEqual(expected);
        expectSupported(await renderedHosting());
      }
    }
    expect(outcomes.applied).toBeGreaterThan(10);
    expect(outcomes.refused).toBeGreaterThan(10);
  });
});

test("property: 100 changes of values, accepted or refused, and every row within its parent", async ({
  page,
  login,
  browserName,
}) => {
  // One browser is enough: the server's answers, not the engine's, make the trees.
  test.skip(browserName !== "chromium", "property test: one browser");
  test.setTimeout(600_000);
  await login("alice");
  const slug = unique("property", browserName);
  const domain = await newDomain(page, slug, [
    prefix("198.51.100.0/24", "container"),
    prefix("198.51.100.0/25", "container"),
    prefix("198.51.100.128/25", "container"),
    prefix("198.51.100.0/26", "network"),
    prefix("198.51.100.64/27", "network"),
    prefix("198.51.100.128/26", "network"),
    prefix("198.51.100.192/26", "network"),
    prefix("203.0.113.0/24", "network"),
    range("198.51.100.10", "198.51.100.20"),
    range("198.51.100.130", "198.51.100.140"),
    range("203.0.113.5", "203.0.113.9"),
  ]);
  const random = seeded(0x2124);
  const pick = <T>(list: readonly T[]): T => list[Math.floor(random() * list.length)]!;

  // The values offered: the aligned blocks of both networks, and a few with host bits set.
  const blocks: string[] = [];
  for (const base of ["198.51.100", "203.0.113"]) {
    for (let length = 24; length <= 28; length++) {
      const size = 2 ** (32 - length);
      for (let start = 0; start < 256; start += size) blocks.push(`${base}.${start}/${length}`);
    }
  }
  const values = [...blocks, "198.51.100.5/26", "203.0.113.17/25"];

  const objects = async () => {
    const [prefixes, ranges] = await Promise.all([
      call(page, "GET", `/v1/prefixes?domain=${slug}`),
      call(page, "GET", `/v1/ranges?domain=${slug}`),
    ]);
    return {
      prefixes: (prefixes.body as { items: { id: string }[] }).items,
      ranges: (ranges.body as { items: { id: string; usage: string }[] }).items,
    };
  };

  await page.goto(`/topology?domain=${domain}`);
  const tree = treeOf(page, slug);
  await expect(rowOf(tree, "198.51.100.0/24")).toBeVisible();
  const outcomes = { applied: 0, refused: 0 };
  // 100 steps, the other 100 of REQ-021-34 in the property test of the hosting.
  for (let step = 1; step <= 100; step++) {
    const { prefixes, ranges } = await objects();
    const viaDraft = random() < 0.5;
    let operations: object[];
    if (random() < 0.6 || ranges.length === 0) {
      operations = [{ op: "update_prefix", id: { id: pick(prefixes).id }, prefix: pick(values) }];
    } else {
      // A range changes value by its replacement: closed, then created with new bounds.
      const old = pick(ranges);
      const base = random() < 0.8 ? "198.51.100" : "203.0.113";
      const [low, high] = [Math.floor(random() * 256), Math.floor(random() * 256)].sort(
        (x, y) => x - y,
      );
      const reversed = random() < 0.1;
      const [first, last] = reversed ? [high, low] : [low, high];
      operations = [
        { op: "close", kind: "range", id: { id: old.id } },
        {
          op: "create_range",
          domain: { id: domain },
          first: `${base}.${first}`,
          last: `${base}.${last}`,
          usage: old.usage,
        },
      ];
    }
    if (await submit(page, operations, viaDraft)) outcomes.applied += 1;
    else outcomes.refused += 1;
    // Within the quotas of the instance (writes 10 a second; the journeys beside this one
    // share them), the tree's own reads included.
    await page.waitForTimeout(400);

    if (step % 10 === 0) {
      // The tree rendered, read again from the stream of events, becomes the API's: each
      // row under the parent the API gives it, and within it.
      const expected = await served(page, domain);
      expectContained(expected);
      await expect
        .poll(
          async () => {
            await expandAll(tree);
            return rendered(tree);
          },
          { message: `step ${step}: the tree rendered is the API's`, timeout: 20_000 },
        )
        .toEqual(expected);
      expectContained(await rendered(tree));
    }
  }
  // Both kinds of outcome were exercised.
  expect(outcomes.applied).toBeGreaterThan(10);
  expect(outcomes.refused).toBeGreaterThan(10);
});
