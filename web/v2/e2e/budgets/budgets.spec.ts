import { readFileSync } from "node:fs";
import { expect, test, type CDPSession, type Locator, type Page } from "@playwright/test";
import { benchPage, framed, heap, quantile, settle, timed } from "../bench/measure.ts";
import { BASE_URL, STATE } from "../fixtures/env.ts";
import { loginIn } from "../fixtures/journeys.ts";

// The budgets of the browser for the new shell (spec 021, REQ-021-30, design « Budgets »),
// on the dataset of bench-core and a container of 10⁵ networks, Chromium, the CPU slowed
// down four times through CDP. The protocol of the prototype D-1 (bench/measure.ts): a
// latency runs from the `timeStamp` of its event to the first animation frame after the
// expected state; dropped frames are 1 − frames observed ÷ (duration ÷ 16.67 ms); a long
// task is one the browser reports (> 50 ms); the heap is read after a garbage collection.
// Each measure prints `PASS|FAIL <budget>: <measured> (limit <limit>)`, or `INFO` for a
// figure given for reading; one budget exceeded fails the bench, every one gets its verdict.

/** Rows of the container scrolled, and the rows loaded before (a window more). */
const ROWS = 10_000;
/** Presses of Arrow down, read at their 75th percentile. */
const PRESSES = 50;
/** Repetitions of each interaction whose INP is read at its 75th percentile. */
const REPEATS = 20;
/** Navigations before the heap is read. */
const NAVIGATIONS = 50;
/** Navigations to a target prefetched on hover, and whose layout shifts are read. */
const CACHED = 10;
/** Pages of a list after the first, each to the rows it draws. */
const PAGES = 20;
/** Where the first screen is useful: the tiles of the core on the home page. */
const USEFUL = "main .k-home-tiles";
/** The window after a navigation whose layout shifts count for it. */
const SHIFT_WINDOW = 1_000;

/** The ids `serve.sh budgets` got from `kartenn-bench seed`. */
function seeded(): { domain: string; container: string } {
  const line = readFileSync(`${STATE}budgets-v2.env`, "utf8");
  const domain = /domain=([0-9a-f-]{36})/.exec(line)?.[1];
  const container = /container=([0-9a-f-]{36})/.exec(line)?.[1];
  if (!domain || !container) throw new Error(`no seeded ids in ${STATE}budgets-v2.env`);
  return { domain, container };
}

/** What the page records from its first script: interactions, shifts, clicks, first screen. */
interface Probe {
  /** Event Timing entries of interactions (`interactionId` > 0), 16 ms and longer. */
  events: { id: number; start: number; duration: number }[];
  /** Layout shifts without a recent input. */
  shifts: { start: number; value: number }[];
  /** The `timeStamp` of each click. */
  clicks: number[];
  /** Mutations of the element watched by `watch`, each at the frame after it. */
  drawn: number[];
  /** The first frame where `USEFUL` is in the document. */
  useful?: number;
}

declare global {
  interface Window {
    __kProbe: Probe;
  }
}

function probe({ origin, useful }: { origin: string; useful: string }): void {
  const p: Probe = { events: [], shifts: [], clicks: [], drawn: [] };
  window.__kProbe = p;
  performance.setResourceTimingBufferSize(100_000);
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries() as (PerformanceEntry & { interactionId?: number })[])
      if (entry.interactionId)
        p.events.push({
          id: entry.interactionId,
          start: entry.startTime,
          duration: entry.duration,
        });
  }).observe({ type: "event", buffered: true, durationThreshold: 16 } as PerformanceObserverInit);
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries() as (PerformanceEntry & {
      value: number;
      hadRecentInput: boolean;
    })[])
      if (!entry.hadRecentInput) p.shifts.push({ start: entry.startTime, value: entry.value });
  }).observe({ type: "layout-shift", buffered: true });
  window.addEventListener("click", (event) => p.clicks.push(event.timeStamp), true);
  if (location.origin !== origin) return;
  const observer = new MutationObserver(() => {
    if (!document.querySelector(useful)) return;
    observer.disconnect();
    requestAnimationFrame(() => (p.useful ??= performance.now()));
  });
  observer.observe(document, { subtree: true, childList: true });
}

/** A page of the bench: its runtime, the probe, and the CDP session. */
async function benchOf(page: Page): Promise<CDPSession> {
  const cdp = await benchPage(page);
  await page.addInitScript(probe, { origin: BASE_URL, useful: USEFUL });
  return cdp;
}

const slowDown = (cdp: CDPSession) => cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
const fullSpeed = (cdp: CDPSession) => cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });

/** One verdict, printed, and a soft expectation: the test goes on to its other measures. */
function check(name: string, measured: number, limit: number, unit: "ms" | "MB" | "%" | ""): void {
  const shown = (v: number) =>
    unit === "%"
      ? `${(v * 100).toFixed(2)} %`
      : unit === ""
        ? Number.isInteger(v)
          ? String(v)
          : v.toFixed(3)
        : `${v.toFixed(1)} ${unit}`;
  console.log(
    `${measured <= limit ? "PASS" : "FAIL"} ${name}: ${shown(measured)} (limit ${shown(limit)})`,
  );
  expect.soft(measured, name).toBeLessThanOrEqual(limit);
}

/** The slug of a domain of bench-core (not the one of the wide container). */
async function benchDomain(page: Page): Promise<string> {
  const response = await page.request.get("/v1/domains", { headers: { Origin: BASE_URL } });
  expect(response.ok(), `GET /v1/domains: ${response.status()}`).toBe(true);
  const { items } = (await response.json()) as { items: { slug: string }[] };
  const slug = items.find((d) => d.slug !== "bench-web")?.slug;
  if (!slug) throw new Error("no domain of bench-core");
  return slug;
}

const tree = (page: Page) =>
  page.getByRole("treegrid", { name: "Arbre des préfixes du domaine bench-web" });

/** The container of 10⁵ networks open in the topography, its children shown. */
async function openContainer(page: Page): Promise<Locator> {
  const { domain } = seeded();
  await page.goto(`/topology?domain=${domain}`);
  const root = tree(page).locator("[role='row'][data-node]").first();
  await root.click();
  await page.keyboard.press("ArrowRight");
  await expect(root).toHaveAttribute("aria-expanded", "true");
  await expect(tree(page).locator("[role='row'][aria-level='2']").first()).toBeVisible();
  return root;
}

/** The 75th percentile of `PRESSES` presses of Arrow down, each until the focus moves. */
async function arrowDown(page: Page): Promise<number> {
  const latencies: number[] = [];
  for (let i = 0; i < PRESSES; i++) {
    const { latency } = await timed(page, { on: ["keydown"], until: { focusMoved: true } }, () =>
      page.keyboard.press("ArrowDown"),
    );
    latencies.push(latency);
  }
  return quantile(latencies, 0.75);
}

/**
 * The INP of what `act` does (Event Timing): of each interaction that started during it,
 * its longest entry; the longest of them. An interaction under 16 ms reports nothing: 0.
 */
async function inp(page: Page, act: () => Promise<unknown>): Promise<number> {
  const from = await page.evaluate(() => performance.now());
  await act();
  await settle(page);
  // The entries come after the next paint.
  await page.waitForTimeout(150);
  return page.evaluate((since) => {
    const worst = new Map<number, number>();
    for (const e of window.__kProbe.events)
      if (e.start >= since) worst.set(e.id, Math.max(worst.get(e.id) ?? 0, e.duration));
    return Math.max(0, ...worst.values());
  }, from);
}

/** The 75th percentile of the INP of `REPEATS` runs of `act`, after `before` each time. */
async function interaction(
  page: Page,
  act: (run: number) => Promise<unknown>,
  before: (run: number) => Promise<unknown> = async () => undefined,
): Promise<number> {
  const values: number[] = [];
  for (let run = 0; run < REPEATS; run++) {
    await before(run);
    values.push(await inp(page, () => act(run)));
  }
  return quantile(values, 0.75);
}

/** The `k:view` marks after the last click, and that click (clock of the page). */
const viewAfterClick = (page: Page) =>
  page.evaluate(() => {
    const click = window.__kProbe.clicks.at(-1) ?? 0;
    const marks = performance.getEntriesByName("k:view", "mark").filter((m) => m.startTime > click);
    return { click, view: marks[0]?.startTime };
  });

/** Clicks `link` and waits for the `k:view` of the navigation; its delay from the click. */
async function navigate(page: Page, link: Locator): Promise<{ click: number; delay: number }> {
  await link.click();
  await expect
    .poll(async () => (await viewAfterClick(page)).view, { timeout: 30_000 })
    .toBeDefined();
  const { click, view } = await viewAfterClick(page);
  return { click, delay: view! - click };
}

/** The sum of the layout shifts between `from` and `to` (clock of the page). */
const shiftsBetween = (page: Page, from: number, to: number) =>
  page.evaluate(
    ([f, t]) =>
      window.__kProbe.shifts
        .filter((s) => s.start >= f && s.start <= t)
        .reduce((sum, s) => sum + s.value, 0),
    [from, to] as const,
  );

test("first useful screen within 2 s of the callback, without shift; a slow plugin set aside", async ({
  page,
}) => {
  const cdp = await benchOf(page);
  await slowDown(cdp);
  await loginIn(page, "alice");
  // From the start of the fetch of the shell, after the redirects of the callback, to the
  // first frame with the tiles of the home page.
  await expect.poll(() => page.evaluate(() => window.__kProbe.useful)).toBeDefined();
  await page.waitForTimeout(SHIFT_WINDOW);
  const { elapsed, shifts } = await page.evaluate(() => {
    const navigation = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming;
    return {
      elapsed: window.__kProbe.useful! - navigation.fetchStart,
      shifts: window.__kProbe.shifts.reduce((sum, s) => sum + s.value, 0),
    };
  });
  check("first useful screen", elapsed, 2_000, "ms");
  check("CLS of the first screen", shifts, 0.05, "");

  // The plugin that defines itself after 6 s is set aside at 5 s.
  await page.goto("/about");
  await expect(
    page.getByRole("main").getByRole("listitem").filter({ hasText: "slow 1.0.0" }),
  ).toContainText("aucune extension définie dans le délai", { timeout: 15_000 });
});

test("a page of 100 rows drawn within 100 ms of its response", async ({ page }) => {
  const cdp = await benchOf(page);
  await loginIn(page, "alice");
  await slowDown(cdp);
  await page.goto("/addresses");
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.locator("[role='gridcell']").first()).toBeVisible();
  await grid.evaluate((el) => {
    new MutationObserver(() =>
      requestAnimationFrame(() => window.__kProbe.drawn.push(performance.now())),
    ).observe(el, { subtree: true, childList: true, characterData: true });
  });
  // One page at a time: the end of the grid in the window, then its read, then a pause, so
  // that what the grid draws after a response is that page.
  const pages = () =>
    page.evaluate(
      () =>
        performance
          .getEntriesByType("resource")
          .filter((entry) => new URL(entry.name).pathname === "/v1/addresses").length,
    );
  for (let read = await pages(); read <= PAGES;) {
    // The grid scrolls once virtualised, the surface of the frame before (records.spec.ts).
    await grid.evaluate((el) => {
      el.scrollTo(0, el.scrollHeight);
      document.getElementById("k-surface")?.scrollTo(0, 1e9);
    });
    await expect.poll(pages, { timeout: 30_000 }).toBeGreaterThan(read);
    read = await pages();
    await settle(page);
    await page.waitForTimeout(300);
  }
  const delays = await page.evaluate(() => {
    const ends = performance
      .getEntriesByType("resource")
      .filter((entry) => new URL(entry.name).pathname === "/v1/addresses")
      .map((entry) => (entry as PerformanceResourceTiming).responseEnd);
    const drawn = window.__kProbe.drawn;
    // Each page after the first (the warm-up of the view), to the first frame after it.
    return ends.slice(1).flatMap((end) => {
      const at = drawn.find((t) => t >= end);
      return at === undefined ? [] : [at - end];
    });
  });
  expect(delays.length).toBeGreaterThanOrEqual(PAGES);
  check(`page of 100 rows, p95 of ${delays.length}`, quantile(delays, 0.95), 100, "ms");
  console.log(`INFO page of 100 rows, median: ${quantile(delays, 0.5).toFixed(1)} ms`);
});

test("a container of 10⁵ scrolled over 10⁴ rows, Arrow down in the tree, the heap after 50 navigations", async ({
  page,
}) => {
  const cdp = await benchOf(page);
  await loginIn(page, "alice");
  await openContainer(page);
  // The rows loaded first, at full speed: the end of the tree in the window until its height
  // holds the rows to scroll, or grows no more for 3 s — the cache keeps 10 pages of a list
  // at most (`MAX_PAGES`, design « Cache »): about 10³ children in memory, a window that
  // slides as further pages come.
  const loaded = await tree(page).evaluate(async (el, rows) => {
    let scroller = el as HTMLElement | null;
    while (scroller && !(scroller.scrollHeight > scroller.clientHeight))
      scroller = scroller.parentElement;
    if (!scroller) throw new Error("nothing scrolls");
    const row = el.querySelector("[data-node]")!.getBoundingClientRect().height;
    const deadline = performance.now() + 600_000;
    let height = 0;
    let grown = performance.now();
    while (scroller.scrollHeight < (rows + 50) * row && performance.now() - grown < 3_000) {
      if (performance.now() > deadline) break;
      if (scroller.scrollHeight > height) {
        height = scroller.scrollHeight;
        grown = performance.now();
      }
      scroller.scrollTop = scroller.scrollHeight;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return Math.floor(scroller.scrollHeight / row);
  }, ROWS);
  console.log(`INFO rows of the container in memory: ${loaded}`);
  await settle(page);
  await slowDown(cdp);
  // 10⁴ rows in 10 s: down and up the rows in memory, the position by the clock (the
  // protocol of the prototype: a dropped frame does not lengthen the run).
  const frames = await framed(page, async () => {
    await tree(page).evaluate(
      async (el, [rows, duration]) => {
        let scroller = el as HTMLElement;
        while (!(scroller.scrollHeight > scroller.clientHeight)) scroller = scroller.parentElement!;
        const row = el.querySelector("[data-node]")!.getBoundingClientRect().height;
        const extent = scroller.scrollHeight - scroller.clientHeight;
        const distance = rows * row;
        scroller.scrollTop = 0;
        await new Promise((resolve) => requestAnimationFrame(resolve));
        const start = performance.now();
        await new Promise<void>((resolve) => {
          const tick = () => {
            const done = Math.min(1, (performance.now() - start) / duration);
            const travelled = (done * distance) % (2 * extent);
            scroller.scrollTop = travelled <= extent ? travelled : 2 * extent - travelled;
            if (done < 1) requestAnimationFrame(tick);
            else resolve();
          };
          requestAnimationFrame(tick);
        });
      },
      [ROWS, 10_000] as const,
    );
    return [];
  });
  check(`dropped frames over ${ROWS} rows in 10 s`, frames.dropped, 0.02, "%");
  check("long tasks while scrolling", frames.longTasks, 0, "");
  console.log(`INFO longest task while scrolling: ${frames.longest.toFixed(1)} ms`);

  await tree(page).locator("[role='row'][aria-level='2'] [role='gridcell']").first().click();
  await settle(page);
  check(`Arrow down in the tree, p75 of ${PRESSES}`, await arrowDown(page), 100, "ms");

  // Fifty navigations through the entries of the frame, one after the other.
  const links = page
    .getByRole("navigation", { name: "Navigation principale" })
    .locator("a[href^='/']:visible");
  const count = await links.count();
  expect(count).toBeGreaterThan(1);
  for (let i = 0; i < NAVIGATIONS; i++) await navigate(page, links.nth(i % count));
  await fullSpeed(cdp);
  check(`JS heap after ${ROWS} rows and ${NAVIGATIONS} navigations`, await heap(cdp), 64, "MB");
});

test("navigations to a prefetched target within 100 ms, without layout shift", async ({ page }) => {
  const cdp = await benchOf(page);
  await loginIn(page, "alice");
  const domain = await benchDomain(page);
  await slowDown(cdp);
  const list = `/prefixes?domain=${encodeURIComponent(domain)}`;
  await page.goto(list);
  const grid = page.getByRole("grid", { name: "Préfixes" });
  const records = grid.locator("[role='gridcell'][data-column='value'] a");
  await expect(records.first()).toBeVisible();
  const delays: number[] = [];
  let worstShift = 0;
  for (let i = 0; i < CACHED; i++) {
    const link = records.nth(i);
    await link.scrollIntoViewIfNeeded();
    // Its reads on intent (60 ms of hover), awaited before the click.
    const read = page
      .waitForResponse((r) => /\/v1\/prefixes\/[0-9a-f-]+\/properties/.test(r.url()), {
        timeout: 10_000,
      })
      .catch(() => undefined);
    await link.hover();
    await read;
    await settle(page);
    const { click, delay } = await navigate(page, link);
    delays.push(delay);
    await page.waitForTimeout(SHIFT_WINDOW);
    worstShift = Math.max(worstShift, await shiftsBetween(page, click, click + SHIFT_WINDOW));
    await page.goBack();
    await expect(records.nth(i + 1)).toBeVisible();
  }
  check(`navigation in cache to k:view, p75 of ${CACHED}`, quantile(delays, 0.75), 100, "ms");
  console.log(`INFO navigation in cache, worst of ${CACHED}: ${Math.max(...delays).toFixed(1)} ms`);
  check(`CLS of a navigation, worst of ${CACHED}`, worstShift, 0.05, "");
});

test("INP of the scripted interactions within 200 ms at their 75th percentile", async ({
  page,
}) => {
  const cdp = await benchOf(page);
  await loginIn(page, "alice");
  const domain = await benchDomain(page);
  await slowDown(cdp);
  const limit = 200;
  const name = (what: string) => `INP, ${what}, p75 of ${REPEATS}`;

  // Open a node: the container of 10⁵, closed again between two runs.
  const root = await openContainer(page);
  await page.keyboard.press("ArrowLeft");
  await expect(root).toHaveAttribute("aria-expanded", "false");
  const open = await interaction(
    page,
    async () => {
      await page.keyboard.press("ArrowRight");
      await expect(root).toHaveAttribute("aria-expanded", "true");
    },
    async (run) => {
      if (run === 0) return;
      await page.keyboard.press("ArrowLeft");
      await expect(root).toHaveAttribute("aria-expanded", "false");
      await settle(page);
    },
  );
  check(name("open a node"), open, limit, "ms");

  // Filter: the value of the addresses within a prefix, from the header of the column.
  const addresses = page.getByRole("grid", { name: "Adresses" });
  const filter = await interaction(
    page,
    async (run) => {
      const within = run % 2 === 0 ? "192.0.2.0/24" : "198.51.100.0/24";
      await addresses.getByRole("columnheader", { name: /Valeur/ }).click();
      await page.keyboard.press("Alt+ArrowDown");
      const picker = page.getByRole("dialog", { name: /filtre/i });
      await picker.getByRole("combobox", { name: /Opérateur/ }).click();
      await page.getByRole("option", { name: "dans" }).click();
      await picker.getByRole("textbox", { name: /Valeur/ }).fill(within);
      await picker.getByRole("button", { name: "Appliquer" }).click();
      await expect(page).toHaveURL(/[?&]within=/);
      await expect(picker).toBeHidden();
    },
    async () => {
      await page.goto("/addresses");
      await expect(addresses.locator("[role='gridcell']").first()).toBeVisible();
      await settle(page);
    },
  );
  check(name("filter a list"), filter, limit, "ms");

  // Edit a cell: the value of the first prefix, into the draft.
  const prefixes = page.getByRole("grid", { name: "Préfixes" });
  await page.goto(`/prefixes?domain=${encodeURIComponent(domain)}`);
  const cell = prefixes.locator("[role='gridcell'][data-column='value']").first();
  await expect(cell).toBeVisible();
  await prefixes.getByRole("columnheader", { name: /^Valeur/ }).click({ position: { x: 4, y: 4 } });
  await page.keyboard.press("ArrowDown");
  const edit = await interaction(page, async (run) => {
    const value = `198.51.100.${(run % 2) * 128}/25`;
    await page.keyboard.press("Enter");
    await expect(prefixes.getByRole("textbox", { name: /^Valeur — / })).toBeFocused();
    await page.keyboard.press("Control+a");
    await page.keyboard.type(value);
    await page.keyboard.press("Enter");
    await expect(cell).toHaveAttribute("data-state", "draft");
    await expect(cell).toContainText(value);
  });
  check(name("edit a cell"), edit, limit, "ms");

  // Change the tab of a record.
  await prefixes.locator("[role='gridcell'][data-column='value'] a").nth(1).click();
  const tabs = page.getByRole("tablist", { name: "Sections de la fiche" });
  await expect(tabs).toBeVisible();
  const tab = await interaction(page, async (run) => {
    const target =
      run % 2 === 0 ? tabs.getByRole("tab", { name: "Historique" }) : tabs.getByRole("tab").first();
    await target.click();
    await expect(target).toHaveAttribute("aria-selected", "true");
  });
  check(name("change of tab"), tab, limit, "ms");

  // Open the command palette, closed again between two runs.
  const palette = page.getByRole("dialog", { name: "Palette de commandes" });
  const opened = await interaction(
    page,
    async () => {
      await page.keyboard.press("Control+k");
      await expect(palette).toBeVisible();
    },
    async (run) => {
      if (run === 0) return;
      await page.keyboard.press("Escape");
      await expect(palette).toBeHidden();
      await settle(page);
    },
  );
  check(name("open the palette"), opened, limit, "ms");
});

test("Arrow down in a grid within 100 ms at its 75th percentile", async ({ page }) => {
  const cdp = await benchOf(page);
  await loginIn(page, "alice");
  await slowDown(cdp);
  await page.goto("/addresses");
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.locator("[role='gridcell']").first()).toBeVisible();
  await grid.getByRole("columnheader", { name: /Valeur/ }).click({ position: { x: 4, y: 4 } });
  await page.keyboard.press("ArrowDown");
  await settle(page);
  check(`Arrow down in a grid, p75 of ${PRESSES}`, await arrowDown(page), 100, "ms");
});
