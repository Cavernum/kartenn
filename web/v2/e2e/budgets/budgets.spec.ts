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
  /**
   * Event Timing entries of interactions (`interactionId` > 0), 16 ms and longer: the type
   * of the event and what it reached (tag, role, accessible name or text, cut short).
   */
  events: { id: number; start: number; duration: number; name: string; target: string }[];
  /** Every layout shift, `recent` when the browser flags an input in the 500 ms before. */
  shifts: { start: number; value: number; recent: boolean }[];
  /** The `timeStamp` of each click. */
  clicks: number[];
  /** The first frame each `data-row` was in the document, once `watchRows` runs. */
  rows: Record<string, number>;
  /** The first frame where `USEFUL` is in the document. */
  useful?: number;
}

declare global {
  interface Window {
    __kProbe: Probe;
  }
}

function probe({ origin, useful }: { origin: string; useful: string }): void {
  const p: Probe = { events: [], shifts: [], clicks: [], rows: {} };
  window.__kProbe = p;
  performance.setResourceTimingBufferSize(100_000);
  const describe = (node: Node | null): string => {
    if (!(node instanceof Element)) return node ? node.nodeName : "none";
    const named = node.closest("[role],button,a,input,[aria-label]") ?? node;
    const role = named.getAttribute("role") ?? named.tagName.toLowerCase();
    const name = (named.getAttribute("aria-label") ?? named.textContent ?? "").trim();
    return `${role} « ${name.slice(0, 40)} »`;
  };
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries() as (PerformanceEntry & {
      interactionId?: number;
      target?: Node | null;
    })[])
      if (entry.interactionId)
        p.events.push({
          id: entry.interactionId,
          start: entry.startTime,
          duration: entry.duration,
          name: entry.name,
          target: describe(entry.target ?? null),
        });
  }).observe({ type: "event", buffered: true, durationThreshold: 16 } as PerformanceObserverInit);
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries() as (PerformanceEntry & {
      value: number;
      hadRecentInput: boolean;
    })[])
      p.shifts.push({ start: entry.startTime, value: entry.value, recent: entry.hadRecentInput });
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

/** The median and the worst of `samples`, printed for reading. */
function spread(name: string, samples: number[], unit: "ms" | ""): void {
  const shown = (v: number) => (unit === "ms" ? `${v.toFixed(1)} ms` : v.toFixed(3));
  console.log(
    `INFO ${name}: median ${shown(quantile(samples, 0.5))}, worst ${shown(Math.max(...samples))} (${samples.length} samples)`,
  );
}

/** `PRESSES` presses of Arrow down, each until the focus moves: their latencies. */
async function arrowDown(page: Page): Promise<number[]> {
  const latencies: number[] = [];
  for (let i = 0; i < PRESSES; i++) {
    const { latency } = await timed(page, { on: ["keydown"], until: { focusMoved: true } }, () =>
      page.keyboard.press("ArrowDown"),
    );
    latencies.push(latency);
  }
  return latencies;
}

/** The longest interaction of a run: its duration, its event and what it reached. */
interface Longest {
  value: number;
  name: string;
  target: string;
}

/**
 * The INP of what `act` does (Event Timing): of each interaction that started during it,
 * its longest entry; the longest of them. An interaction under 16 ms reports nothing: 0.
 */
async function inp(page: Page, act: () => Promise<unknown>): Promise<Longest> {
  const from = await page.evaluate(() => performance.now());
  await act();
  await settle(page);
  // The entries come after the next paint.
  await page.waitForTimeout(150);
  return page.evaluate((since) => {
    let longest = { value: 0, name: "none", target: "none (under 16 ms)" };
    for (const e of window.__kProbe.events)
      if (e.start >= since && e.duration > longest.value)
        longest = { value: e.duration, name: e.name, target: e.target };
    return longest;
  }, from);
}

/**
 * `REPEATS` runs of `act`, after `before` each time: the 75th percentile of their INP, the
 * INP of each, and the longest interaction of all.
 */
async function interaction(
  page: Page,
  act: (run: number) => Promise<unknown>,
  before: (run: number) => Promise<unknown> = async () => undefined,
): Promise<{ p75: number; samples: number[]; longest: Longest }> {
  const runs: Longest[] = [];
  for (let run = 0; run < REPEATS; run++) {
    await before(run);
    runs.push(await inp(page, () => act(run)));
  }
  const samples = runs.map((r) => r.value);
  const longest = runs.reduce((a, b) => (b.value > a.value ? b : a));
  return { p75: quantile(samples, 0.75), samples, longest };
}

/** The `k:view` marks after the last click, and that click (clock of the page). */
const viewAfterClick = (page: Page) =>
  page.evaluate(() => {
    const click = window.__kProbe.clicks.at(-1) ?? 0;
    const marks = performance.getEntriesByName("k:view", "mark").filter((m) => m.startTime > click);
    return { click, view: marks[0]?.startTime };
  });

/**
 * Clicks `link` and waits for the `k:view` of the navigation: the click, the mark (clock
 * of the page) and the delay between them.
 */
async function navigate(
  page: Page,
  link: Locator,
): Promise<{ click: number; view: number; delay: number }> {
  await link.click();
  await expect
    .poll(async () => (await viewAfterClick(page)).view, { timeout: 30_000 })
    .toBeDefined();
  const { click, view } = await viewAfterClick(page);
  return { click, view: view!, delay: view! - click };
}

/**
 * The sum of every layout shift between `from` and `to` (clock of the page), those the
 * browser flags as after an input included: the click of a navigation counts as an input
 * for 500 ms (Layout Instability, `hadRecentInput`), which would leave out the shifts of
 * the screen it draws.
 */
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
      // A load: CLS as browsers report it, the shifts after an input left out.
      shifts: window.__kProbe.shifts.filter((s) => !s.recent).reduce((sum, s) => sum + s.value, 0),
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
  // The first frame each row of the grid is in the document: a row is its cells'
  // `data-row`, the key of its object (`row-key="id"`); those already drawn are not new.
  await grid.evaluate((el) => {
    const seen = window.__kProbe.rows;
    const scan = () => {
      const now = performance.now();
      for (const cell of el.querySelectorAll<HTMLElement>("[data-row]")) {
        const key = cell.dataset.row;
        if (key && !(key in seen)) seen[key] = now;
      }
    };
    scan();
    let pending = false;
    new MutationObserver(() => {
      if (pending) return;
      pending = true;
      requestAnimationFrame(() => {
        pending = false;
        scan();
      });
    }).observe(el, { subtree: true, childList: true, attributes: true });
  });
  // One page at a time: the end of the grid in the window, the read it sets off, then the
  // frame where the first row of that page is in the document, from the end of its own
  // response (its resource timing, by its URL).
  // The first row of each page the grid reads, by the URL of its read (cursor included).
  const firstRow = new Map<string, string>();
  page.on("response", (r) => {
    if (new URL(r.url()).pathname !== "/v1/addresses" || r.request().method() !== "GET") return;
    void r
      .json()
      .then((body: { items?: { id: string }[] }) => {
        const id = body.items?.[0]?.id;
        if (id) firstRow.set(r.url(), id);
      })
      .catch(() => undefined);
  });
  const reads = () =>
    page.evaluate(() =>
      performance
        .getEntriesByType("resource")
        .filter((e) => new URL(e.name).pathname === "/v1/addresses")
        .map((e) => e.name),
    );
  const delays: number[] = [];
  let undrawn = 0;
  let withoutRows = 0;
  let measured = new Set(await reads());
  while (delays.length + undrawn < PAGES && withoutRows < PAGES) {
    // The grid scrolls once virtualised, the surface of the frame before (records.spec.ts);
    // out of the end a moment, then at the end, so that the approach of the end is seen.
    const before = measured.size;
    await expect
      .poll(
        async () => {
          await grid.evaluate((el) => {
            el.scrollTo(0, Math.max(0, el.scrollHeight - 3 * el.clientHeight));
            el.scrollTo(0, el.scrollHeight);
            document.getElementById("k-surface")?.scrollTo(0, 1e9);
          });
          return (await reads()).length;
        },
        { timeout: 30_000, intervals: [500] },
      )
      .toBeGreaterThan(before);
    const now = await reads();
    for (const url of now.filter((u) => !measured.has(u))) {
      // A read without rows (a revalidation answered 304, an empty page) is no page drawn.
      const deadline = Date.now() + 10_000;
      while (!firstRow.has(url) && Date.now() < deadline) await page.waitForTimeout(100);
      if (!firstRow.has(url)) {
        withoutRows++;
        continue;
      }
      const delay = await page.evaluate(
        async ([u, key]) => {
          const deadline = performance.now() + 5_000;
          while (performance.now() < deadline) {
            const timing = performance.getEntriesByName(u).at(-1) as
              PerformanceResourceTiming | undefined;
            const drawn = window.__kProbe.rows[key];
            if (timing && drawn !== undefined) return Math.max(0, drawn - timing.responseEnd);
            await new Promise((resolve) => setTimeout(resolve, 50));
          }
          return null;
        },
        [url, firstRow.get(url)!] as const,
      );
      if (delay === null) undrawn++;
      else delays.push(delay);
    }
    measured = new Set(now);
    await settle(page);
    await page.waitForTimeout(300);
  }
  console.log(
    `INFO reads of /v1/addresses without rows: ${withoutRows}; pages whose first row was not` +
      ` drawn within 5 s of its response: ${undrawn}`,
  );
  expect(delays.length).toBeGreaterThanOrEqual(PAGES - 2);
  check(`page of 100 rows, p95 of ${delays.length}`, quantile(delays, 0.95), 100, "ms");
  spread("page of 100 rows", delays, "ms");
});

test("a container of 10⁵ scrolled over 10⁴ rows, Arrow down in the tree, the heap after 50 navigations", async ({
  page,
}) => {
  const cdp = await benchOf(page);
  await loginIn(page, "alice");
  await openContainer(page);
  // Every row the tree will hold, read first at full speed: the end of the tree in the
  // window until no page of the container has been read for 10 s. A page is 100 children
  // (the server's default, the tree names no `limit`); the cache keeps 10 pages of a list
  // (`MAX_PAGES`), dropping the first when an eleventh comes; and the tree asks for a page
  // again only when its count of children has changed (`treePages.ts`): once ten pages
  // are kept, the eleventh leaves the count as it was and the reads stop. About 10³
  // children, never 10⁴, can be in memory.
  const { container } = seeded();
  const children = `/v1/prefixes/${container}/children`;
  const loading = await tree(page).evaluate(async (el, path) => {
    let scroller = el as HTMLElement | null;
    while (scroller && !(scroller.scrollHeight > scroller.clientHeight))
      scroller = scroller.parentElement;
    if (!scroller) throw new Error("nothing scrolls");
    const row = el.querySelector("[data-node]")!.getBoundingClientRect().height;
    const reads = () =>
      performance
        .getEntriesByType("resource")
        .filter((e) => new URL(e.name).pathname === path) as PerformanceResourceTiming[];
    const deadline = performance.now() + 600_000;
    let count = -1;
    let changed = performance.now();
    // The last child reached: the greatest `aria-posinset` of the second level drawn.
    let reached = 0;
    const position = () => {
      for (const r of el.querySelectorAll("[role='row'][aria-level='2'][aria-posinset]"))
        reached = Math.max(reached, Number(r.getAttribute("aria-posinset")));
    };
    while (performance.now() - changed < 10_000 && performance.now() < deadline) {
      position();
      const now = reads().length;
      if (now !== count) {
        count = now;
        changed = performance.now();
      }
      // The row that reads the next page asks when it comes into view: out of view a
      // moment, then in again at the end.
      scroller.scrollTop = Math.max(0, scroller.scrollHeight - 3 * scroller.clientHeight);
      await new Promise((resolve) => setTimeout(resolve, 100));
      scroller.scrollTop = scroller.scrollHeight;
      await new Promise((resolve) => setTimeout(resolve, 300));
    }
    const durations = reads().map((e) => e.responseEnd - e.startTime);
    return {
      rows: Math.floor(scroller.scrollHeight / row) - 1,
      reached,
      durations,
      more: el.querySelector("[data-more]") !== null,
    };
  }, children);
  const sorted = [...loading.durations].sort((a, b) => a - b);
  console.log(
    `INFO rows the tree holds (its height): ${loading.rows}; last child reached: ${loading.reached};` +
      ` after ${sorted.length} pages read` +
      ` (median ${(sorted[Math.floor(sorted.length / 2)] ?? 0).toFixed(0)} ms, slowest` +
      ` ${(sorted.at(-1) ?? 0).toFixed(0)} ms each), a row for more: ${loading.more}`,
  );
  await settle(page);
  await slowDown(cdp);
  // 10⁴ rows in 10 s: down and up the rows in memory, the position by the clock (the
  // protocol of the prototype: a dropped frame does not lengthen the run), short of the
  // last rows, so that the row which reads the next page never comes into view: the window
  // measures the drawing alone, and the reads it would set off are counted.
  let span: [number, number] = [0, 0];
  const frames = await framed(page, async () => {
    span = await tree(page).evaluate(
      async (el, [rows, duration]) => {
        let scroller = el as HTMLElement;
        while (!(scroller.scrollHeight > scroller.clientHeight)) scroller = scroller.parentElement!;
        const row = el.querySelector("[data-node]")!.getBoundingClientRect().height;
        const extent = scroller.scrollHeight - scroller.clientHeight - 4 * row;
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
        return [start, performance.now()] as [number, number];
      },
      [ROWS, 10_000] as const,
    );
    return [];
  });
  const during = await page.evaluate(
    ([from, to]) =>
      performance
        .getEntriesByType("resource")
        .filter((e) => new URL(e.name).pathname.startsWith("/v1/"))
        .filter((e) => e.startTime >= from && e.startTime <= to)
        .map((e) => new URL(e.name).pathname),
    span,
  );
  const childrenDuring = during.filter((p) => p.endsWith("/children")).length;
  console.log(
    `INFO reads while scrolling: ${childrenDuring} /children, ${during.length} under /v1/ in all`,
  );
  check("reads of children while scrolling", childrenDuring, 0, "");
  check(`dropped frames over ${ROWS} rows in 10 s`, frames.dropped, 0.02, "%");
  check("long tasks while scrolling", frames.longTasks, 0, "");
  console.log(`INFO longest task while scrolling: ${frames.longest.toFixed(1)} ms`);

  await tree(page).locator("[role='row'][aria-level='2'] [role='gridcell']").first().click();
  await settle(page);
  const treeArrows = await arrowDown(page);
  check(`Arrow down in the tree, p75 of ${PRESSES}`, quantile(treeArrows, 0.75), 100, "ms");
  spread("Arrow down in the tree", treeArrows, "ms");

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
  // The shifts of a navigation: every one, from the frame of its `k:view` (the screen
  // drawn) to a second after, none left out for the input of the click (`shiftsBetween`).
  const navigationShifts = async (i: number, witness = false) => {
    const link = records.nth(i);
    await link.scrollIntoViewIfNeeded();
    // Its reads on intent (60 ms of hover), awaited before the click: without them, the
    // navigation is not one in cache, and the measure fails.
    const read = page
      .waitForResponse((r) => /\/v1\/prefixes\/[0-9a-f-]+\/properties/.test(r.url()), {
        timeout: 10_000,
      })
      .then(() => true)
      .catch(() => false);
    await link.hover();
    const prefetched = await read;
    if (!prefetched) console.log(`FAIL navigation in cache: record ${i} not prefetched on hover`);
    expect.soft(prefetched, `record ${i} prefetched on hover`).toBe(true);
    await settle(page);
    if (witness) {
      // The witness: 200 ms after the screen is drawn, a block of 200 px pushes the content
      // of `main` down — within the 500 ms the click counts as an input.
      await page.evaluate(() => {
        const observer = new PerformanceObserver((list) => {
          if (list.getEntriesByName("k:view").length === 0) return;
          observer.disconnect();
          setTimeout(() => {
            const block = document.createElement("div");
            block.style.height = "200px";
            document.querySelector("main")?.prepend(block);
          }, 200);
        });
        observer.observe({ type: "mark" });
      });
    }
    const { view, delay } = await navigate(page, link);
    await page.waitForTimeout(SHIFT_WINDOW + 200);
    const shift = await shiftsBetween(page, view, view + SHIFT_WINDOW);
    await page.goBack();
    await expect(records.nth(i + 1)).toBeVisible();
    return { delay, shift };
  };
  const delays: number[] = [];
  const shifts: number[] = [];
  for (let i = 0; i < CACHED; i++) {
    const { delay, shift } = await navigationShifts(i);
    delays.push(delay);
    shifts.push(shift);
  }
  check(`navigation in cache to k:view, worst of ${CACHED}`, Math.max(...delays), 100, "ms");
  spread("navigation in cache", delays, "ms");
  check(`CLS of a navigation, worst of ${CACHED}`, Math.max(...shifts), 0.05, "");
  spread("CLS of a navigation", shifts, "");
  // The method sees a shift provoked on purpose, which a measure of the shifts without a
  // recent input would leave out.
  const witness = await navigationShifts(CACHED, true);
  const flagged = await page.evaluate(() => window.__kProbe.shifts.some((s) => s.recent));
  console.log(
    `INFO witness: a block of 200 px 200 ms after k:view seen as a CLS of ${witness.shift.toFixed(3)}` +
      ` (shifts flagged after an input seen: ${flagged})`,
  );
  expect(witness.shift, "the shift of the witness is seen").toBeGreaterThan(0.05);
});

test("INP of the scripted interactions within 200 ms at their 75th percentile", async ({
  page,
}) => {
  const cdp = await benchOf(page);
  await loginIn(page, "alice");
  const domain = await benchDomain(page);
  await slowDown(cdp);
  /** The verdict of a scenario, its spread, and its longest interaction and its target. */
  const report = (
    what: string,
    { p75, samples, longest }: Awaited<ReturnType<typeof interaction>>,
  ) => {
    check(`INP, ${what}, p75 of ${REPEATS}`, p75, 200, "ms");
    spread(`INP, ${what}`, samples, "ms");
    console.log(
      `INFO INP, ${what}, longest: ${longest.value.toFixed(0)} ms, ${longest.name} on ${longest.target}`,
    );
  };

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
  report("open a node", open);

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
  report("filter a list", filter);

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
  report("edit a cell", edit);

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
  report("change of tab", tab);

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
  report("open the palette", opened);
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
  const gridArrows = await arrowDown(page);
  check(`Arrow down in a grid, p75 of ${PRESSES}`, quantile(gridArrows, 0.75), 100, "ms");
  spread("Arrow down in a grid", gridArrows, "ms");
});
