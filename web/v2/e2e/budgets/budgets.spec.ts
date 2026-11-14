import { readFileSync } from "node:fs";
import {
  expect,
  test,
  type CDPSession,
  type Locator,
  type Page,
  type Request,
} from "@playwright/test";
import { benchPage, framed, heap, quantile, settle, timed, type Frames } from "../bench/measure.ts";
import { BASE_URL, STATE } from "../fixtures/env.ts";
import { loginIn } from "../fixtures/journeys.ts";
import { OTHER_DOMAIN } from "./global-setup.ts";

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
  /**
   * The event stream: each connection, error and event, its data cut short, and the
   * `domains` a `changes` names (none when its data does not read).
   */
  stream: { at: number; kind: string; data?: string; domains?: string[] }[];
}

declare global {
  interface Window {
    __kProbe: Probe;
  }
}

function probe({ origin, useful }: { origin: string; useful: string }): void {
  const p: Probe = { events: [], shifts: [], clicks: [], rows: {}, stream: [] };
  window.__kProbe = p;
  // What the event stream brings: a `ready` or a change invalidates reads under way.
  const Native = window.EventSource;
  const kinds = ["ready", "changes", "changeset", "labels", "draft", "stats", "reset"];
  window.EventSource = class extends Native {
    constructor(url: string | URL, init?: EventSourceInit) {
      super(url, init);
      p.stream.push({ at: performance.now(), kind: "open" });
      this.addEventListener("error", () => p.stream.push({ at: performance.now(), kind: "error" }));
      for (const kind of kinds)
        this.addEventListener(kind, (event) => {
          const data = String((event as MessageEvent).data);
          let domains: string[] | undefined;
          if (kind === "changes")
            try {
              domains = (JSON.parse(data) as { domains?: string[] }).domains ?? [];
            } catch {
              // Data that does not read: no domains.
            }
          p.stream.push({
            at: performance.now(),
            kind,
            data: data.slice(0, 200),
            ...(domains ? { domains } : {}),
          });
        });
    }
  };
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
  const slug = items.find((d) => d.slug !== "bench-web" && d.slug !== OTHER_DOMAIN)?.slug;
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

/** What the event stream brought between `from` and `to` (clock of the page), printed. */
async function streamBetween(page: Page, what: string, from: number, to: number): Promise<void> {
  const seen = await page.evaluate(
    ([f, t]) => window.__kProbe.stream.filter((s) => s.at >= f && s.at <= t),
    [from, to] as const,
  );
  const counts = new Map<string, number>();
  for (const s of seen) counts.set(s.kind, (counts.get(s.kind) ?? 0) + 1);
  const first = seen.find((s) => s.kind === "changes");
  console.log(
    `INFO event stream ${what}: ${[...counts].map(([k, n]) => `${k} ${n}`).join(", ") || "nothing"}` +
      (first ? `; first change: ${first.data}` : ""),
  );
}

/** The `changes` of the event stream between `from` and `to` (clock of the page) naming `domain`. */
const changesNaming = (page: Page, domain: string, from: number, to: number) =>
  page.evaluate(
    ([d, f, t]) =>
      window.__kProbe.stream.filter(
        (s) => s.kind === "changes" && s.at >= f && s.at <= t && (s.domains ?? []).includes(d),
      ).length,
    [domain, from, to] as const,
  );

/** The id of the domain `slug`, none if it is not there. */
async function domainId(page: Page, slug: string): Promise<string | undefined> {
  const response = await page.request.get(`/v1/domains?slug=${encodeURIComponent(slug)}`, {
    headers: { Origin: BASE_URL },
  });
  if (!response.ok()) return undefined;
  const { items } = (await response.json()) as { items: { id: string; slug: string }[] };
  return items.find((d) => d.slug === slug)?.id;
}

/** Between two changesets of the background writer, in ms. */
const WRITE_EVERY = 500;

/** What the background writer did: the changesets applied, and what stopped it, if not `stop`. */
interface Writes {
  written: number;
  failure?: string;
}

/**
 * A writer in the background (A40, task 021-22m): a changeset applied every `WRITE_EVERY`
 * ms on the domain of id `domain`, each a network /64 of its own in 2001:db8::/32 (RFC 3849),
 * through the API under the session of `page` but from outside it: the shell learns of them
 * through its event stream alone. A refused write, or a page closed (the test failed or ran
 * out of time), stops it and is told in `writes`; `stop` stops it and waits for the
 * changeset under way.
 */
function backgroundWriter(
  page: Page,
  domain: string,
): { writes: Writes; stop: () => Promise<Writes> } {
  const writes: Writes = { written: 0 };
  let running = true;
  let wake = () => {};
  // A /48 of its own for each run, from the clock: two runs on one instance never collide.
  const run = (Date.now() & 0xffff).toString(16);
  const loop = (async () => {
    while (running) {
      const prefix = `2001:db8:${run}:${(writes.written + 1).toString(16)}::/64`;
      try {
        const response = await page.request.post("/v1/changesets?approval=self", {
          headers: { Origin: BASE_URL },
          data: {
            motive: "browser bench, background writer",
            operations: [
              { op: "create_prefix", domain: { id: domain }, prefix, nature: "network" },
            ],
          },
        });
        if (!response.ok()) {
          writes.failure = `POST /v1/changesets (${prefix}): ${response.status()} ${(await response.text()).slice(0, 200)}`;
          return;
        }
        writes.written++;
      } catch (error) {
        if (running) writes.failure = String(error).slice(0, 200);
        return;
      }
      await new Promise<void>((resolve) => {
        wake = resolve;
        setTimeout(resolve, WRITE_EVERY);
      });
    }
  })();
  return {
    writes,
    stop: async () => {
      running = false;
      wake();
      await loop;
      return writes;
    },
  };
}

/**
 * `ROWS` rows of the tree in 10 s: down and up the rows in memory, the position by the
 * clock (the protocol of the prototype: a dropped frame does not lengthen the run), short
 * of the last rows, so that the row which reads the next page never comes into view. The
 * window of the scroll (clock of the page).
 */
const scrollRows = (page: Page) =>
  tree(page).evaluate(
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

/**
 * The reads under /v1/ that started between `from` and `to` (clock of the page), printed by
 * where they come from: the container's own pages (read again: a refetch) or other nodes.
 * The reads of the container's children among them.
 */
async function readsWhile(page: Page, what: string, container: string, span: [number, number]) {
  const during = await page.evaluate(
    ([from, to]) =>
      performance
        .getEntriesByType("resource")
        .filter((e) => new URL(e.name).pathname.startsWith("/v1/"))
        .filter((e) => e.startTime >= from && e.startTime <= to)
        .map((e) => {
          const url = new URL(e.name);
          return { path: url.pathname, cursor: url.searchParams.has("cursor") };
        }),
    span,
  );
  const childReads = during.filter((r) => r.path.endsWith("/children"));
  const ofContainer = childReads.filter((r) => r.path === `/v1/prefixes/${container}/children`);
  console.log(
    `INFO reads ${what}: ${childReads.length} /children (${ofContainer.length} of the` +
      ` container, ${ofContainer.filter((r) => !r.cursor).length} of them its first page;` +
      ` ${new Set(childReads.map((r) => r.path)).size} nodes), ${during.length} under /v1/ in all`,
  );
  return { children: childReads.length, ofContainer: ofContainer.length };
}

/**
 * The scroll of `scrollRows` again, the domain `OTHER_DOMAIN` written all along by the
 * background writer (A40, task 021-22m): each `changes` names that domain alone, of no tree
 * on screen, and none may read the children of the container again. The writes are first
 * seen at the stream, before the window opens, then counted at the stream during it: no
 * read of the container without them would prove nothing. Printed: the writer, the reads,
 * the stream, the frames for reading; checked: the changes seen and the reads of the
 * container.
 */
async function scrollWhileWritten(page: Page): Promise<void> {
  const { domain, container } = seeded();
  const other = await domainId(page, OTHER_DOMAIN);
  if (!other) {
    console.log(
      `FAIL changes naming ${OTHER_DOMAIN} at the stream while scrolling: no such domain`,
    );
    expect.soft(other, `the domain ${OTHER_DOMAIN} (global-setup.ts)`).toBeDefined();
    return;
  }
  const writer = backgroundWriter(page, other);
  let span: [number, number] = [0, 0];
  let frames: Frames;
  let first: number | undefined;
  let duringWindow = 0;
  let writes: Writes;
  try {
    const armed = await page.evaluate(() => performance.now());
    const deadline = Date.now() + 30_000;
    while (first === undefined && !writer.writes.failure && Date.now() < deadline) {
      if ((await changesNaming(page, other, armed, Number.MAX_SAFE_INTEGER)) > 0)
        first = (await page.evaluate(() => performance.now())) - armed;
      else await page.waitForTimeout(100);
    }
    const before = writer.writes.written;
    frames = await framed(page, async () => {
      span = await scrollRows(page);
      return [];
    });
    duringWindow = writer.writes.written - before;
  } finally {
    writes = await writer.stop();
  }
  const what = `while scrolling, ${OTHER_DOMAIN} written`;
  console.log(
    `INFO background writer on ${OTHER_DOMAIN}: ${writes.written} changesets, one every` +
      ` ${WRITE_EVERY} ms, ${duringWindow} of them during the window, the first at the stream` +
      ` ${first === undefined ? "never (waited 30 s)" : `in ${first.toFixed(0)} ms`};` +
      ` ${writes.failure ? `stopped by ${writes.failure}` : "stopped at the end"}`,
  );
  const reads = await readsWhile(page, what, container, span);
  await streamBetween(page, what, ...span);
  const seen = await changesNaming(page, other, ...span);
  const ofContainer = await changesNaming(page, domain, ...span);
  console.log(
    `INFO ${what}: ${ofContainer} changes naming the domain of the container;` +
      ` dropped frames ${(frames.dropped * 100).toFixed(2)} %, ${frames.longTasks} long` +
      ` tasks, the longest ${frames.longest.toFixed(1)} ms`,
  );
  const proven = seen > 0 && !writes.failure;
  console.log(
    `${proven ? "PASS" : "FAIL"} changes naming ${OTHER_DOMAIN} at the stream while scrolling:` +
      ` ${seen} (at least 1, ${duringWindow} written, no write refused)`,
  );
  expect.soft(proven, `changes naming ${OTHER_DOMAIN} at the stream while scrolling`).toBe(true);
  check(
    `reads of the container's children while ${OTHER_DOMAIN} is written`,
    reads.ofContainer,
    0,
    "",
  );
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

/** Under the 30 s an object read stays fresh in the cache (`FRESHNESS.object`, @kartenn/sdk). */
const FRESH = 25_000;

/**
 * Whether a path of the API is fresh in the cache of the page, whichever screen read it:
 * answered (2xx or 304) within `FRESH` ms, and no read of it under way. By path: the
 * navigations measured read at the present, without dates.
 */
function freshReads(page: Page): (path: string) => boolean {
  const pending = new Map<string, number>();
  const answered = new Map<string, number>();
  const pathOf = (request: Request) => new URL(request.url()).pathname;
  const add = (path: string, n: number) => pending.set(path, (pending.get(path) ?? 0) + n);
  page.on("request", (request) => add(pathOf(request), 1));
  page.on("requestfailed", (request) => add(pathOf(request), -1));
  page.on("requestfinished", (request) => {
    // Its status known before it leaves the reads under way.
    void request.response().then((response) => {
      if (response && response.status() < 400) answered.set(pathOf(request), Date.now());
      add(pathOf(request), -1);
    });
  });
  return (path) =>
    (pending.get(path) ?? 0) === 0 && Date.now() - (answered.get(path) ?? -Infinity) < FRESH;
}

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
  // first frame where a row of that page is in the document, from the end of its own
  // response (its resource timing, by its URL). A row of the page, whichever comes first:
  // past the tenth page the cache drops the first (`MAX_PAGES`), and the grid keeps the
  // rows in view in their place (021-22f), the new page under them, its first rows drawn
  // ahead of the window (021-22a).
  // The rows of each page the grid reads, by the URL of its read (cursor included).
  const pageRows = new Map<string, string[]>();
  page.on("response", (r) => {
    if (new URL(r.url()).pathname !== "/v1/addresses" || r.request().method() !== "GET") return;
    void r
      .json()
      .then((body: { items?: { id: string }[] }) => {
        const ids = (body.items ?? []).map((item) => item.id);
        if (ids.length > 0) pageRows.set(r.url(), ids);
      })
      .catch(() => undefined);
  });
  // The reads started: a resource timing comes only with the end of its response, and a
  // scroll to the end while the page is drawn would pass over its first row.
  let started = 0;
  page.on("request", (r) => {
    if (new URL(r.url()).pathname === "/v1/addresses" && r.method() === "GET") started++;
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
    // out of the end a moment, then at the end, so that the approach of the end is seen;
    // no more once a read has started, until its response is in.
    const before = measured.size;
    const asked = started;
    await expect
      .poll(
        async () => {
          if (started === asked)
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
      while (!pageRows.has(url) && Date.now() < deadline) await page.waitForTimeout(100);
      const ids = pageRows.get(url);
      if (!ids) {
        withoutRows++;
        continue;
      }
      const delay = await page.evaluate(
        async ([u, keys]) => {
          const deadline = performance.now() + 5_000;
          while (performance.now() < deadline) {
            const timing = performance.getEntriesByName(u).at(-1) as
              PerformanceResourceTiming | undefined;
            const drawn = keys
              .map((key) => window.__kProbe.rows[key])
              .filter((at): at is number => at !== undefined);
            if (timing && drawn.length > 0)
              return Math.max(0, Math.min(...drawn) - timing.responseEnd);
            await new Promise((resolve) => setTimeout(resolve, 50));
          }
          return null;
        },
        [url, ids] as const,
      );
      if (delay === null) undrawn++;
      else delays.push(delay);
    }
    measured = new Set(now);
    await settle(page);
    await page.waitForTimeout(300);
  }
  console.log(
    `INFO reads of /v1/addresses without rows: ${withoutRows}; pages of which no row was` +
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
  // (the server's default, the tree names no `limit`); the cache keeps 100 pages of the
  // children of a node (`CHILDREN_PAGES`, A34), and the tree stops at them with a notice
  // in place of the row that reads more: 10⁴ children of the 10⁵ in memory, all of them
  // before the window.
  const { container } = seeded();
  const children = `/v1/prefixes/${container}/children`;
  const loadFrom = await page.evaluate(() => performance.now());
  const loading = await tree(page).evaluate(async (el, path) => {
    // What scrolls the rows, found again at each turn (task 021-22l): up to 200 rows the
    // tree is not virtualised and has no height of its own, so the frame scrolls; past
    // them it is held to a list's height and scrolls itself. Found once, before the
    // second page came, it would be the frame for good, the tree never scrolled, the row
    // that reads the next page never in view.
    const scrollerOf = () => {
      let found = el as HTMLElement | null;
      while (found && !(found.scrollHeight > found.clientHeight)) found = found.parentElement;
      if (!found) throw new Error("nothing scrolls");
      return found;
    };
    let scroller = scrollerOf();
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
      scroller = scrollerOf();
      scroller.scrollTop = Math.max(0, scroller.scrollHeight - 3 * scroller.clientHeight);
      await new Promise((resolve) => setTimeout(resolve, 100));
      scroller.scrollTop = scroller.scrollHeight;
      await new Promise((resolve) => setTimeout(resolve, 300));
    }
    const durations = reads().map((e) => e.responseEnd - e.startTime);
    return {
      rows: Math.floor(scroller.scrollHeight / row) - 1,
      // The tree scrolls its rows itself at the end (virtualised), not the frame.
      own: scroller === el,
      reached,
      durations,
      // The first page read more than once: the container read again from its start.
      firsts: reads().filter((e) => !new URL(e.name).searchParams.has("cursor")).length,
      more: el.querySelector("[data-more]") !== null,
      // The count of children the tree says it holds (a stalled read: one page short).
      setsize: el.querySelector("[role='row'][aria-level='2']")?.getAttribute("aria-setsize"),
      capped: el.querySelector("[data-node^='capped:']") !== null,
    };
  }, children);
  await streamBetween(
    page,
    "while loading",
    loadFrom,
    await page.evaluate(() => performance.now()),
  );
  const sorted = [...loading.durations].sort((a, b) => a - b);
  console.log(
    `INFO rows the tree holds (its height): ${loading.rows}, scrolled by the tree itself:` +
      ` ${loading.own}; last child reached: ${loading.reached};` +
      ` after ${sorted.length} pages read, ${loading.firsts} of them the first` +
      ` (median ${(sorted[Math.floor(sorted.length / 2)] ?? 0).toFixed(0)} ms, slowest` +
      ` ${(sorted.at(-1) ?? 0).toFixed(0)} ms each), a row for more: ${loading.more},` +
      ` aria-setsize ${loading.setsize ?? "none"},` +
      ` the notice of the cap: ${loading.capped}`,
  );
  // Every row the window scrolls over is read before it.
  const loaded = loading.reached >= ROWS && !loading.more;
  console.log(
    `${loaded ? "PASS" : "FAIL"} children loaded before scrolling: ${loading.reached}` +
      ` (at least ${ROWS}, no row for more)`,
  );
  expect.soft(loaded, "children loaded before scrolling").toBe(true);
  await settle(page);
  await slowDown(cdp);
  // 10⁴ rows in 10 s (`scrollRows`), on a quiet instance: the window measures the drawing
  // alone, and the reads it would set off are counted.
  let span: [number, number] = [0, 0];
  const frames = await framed(page, async () => {
    span = await scrollRows(page);
    return [];
  });
  const reads = await readsWhile(page, "while scrolling", container, span);
  await streamBetween(page, "while scrolling", ...span);
  check("reads of children while scrolling", reads.children, 0, "");
  check(`dropped frames over ${ROWS} rows in 10 s`, frames.dropped, 0.02, "%");
  check("long tasks while scrolling", frames.longTasks, 0, "");
  console.log(`INFO longest task while scrolling: ${frames.longest.toFixed(1)} ms`);
  // A diagnosis, off by default (`K_BENCH_TREE_AB=1`, task 021-22e-c): the same scroll again
  // under sheets that take one part of the rows away each, its dropped frames printed, to
  // name what a row costs. Never a verdict: the sheets change what the rows show.
  if (process.env.K_BENCH_TREE_AB) {
    const fixed = await tree(page).evaluate(
      (el) => getComputedStyle(el.querySelector("[role='row'][data-node]")!).gridTemplateColumns,
    );
    const B = ".k-tree-grid__body";
    const variants: [string, string][] = [
      ["base", ""],
      ["no plus", `${B} .k-tree-grid__add{display:none!important}`],
      ["no svg", `${B} svg{display:none!important}`],
      ["no pointer", `${B}{pointer-events:none}`],
      ["no svg, no pointer", `${B} svg{display:none!important}\n${B}{pointer-events:none}`],
      ["no transition", `${B} .k-tree-grid__row{transition:none!important}`],
      ["fixed tracks", `${B} .k-tree-grid__row{grid-template-columns:${fixed}!important}`],
      ["block cells", `${B} .k-tree-grid__cell{display:block!important}`],
      // The spread of two identical passes.
      ["base again", ""],
    ];
    variants.push([
      "all",
      variants
        .slice(1)
        .map(([, css]) => css)
        .join("\n"),
    ]);
    for (const [name, css] of variants) {
      // A constructed sheet: the CSP refuses an injected `<style>` (REQ-020-14).
      await tree(page).evaluate((_, text) => {
        const w = window as unknown as { kAb?: CSSStyleSheet };
        if (!w.kAb) {
          w.kAb = new CSSStyleSheet();
          document.adoptedStyleSheets = [...document.adoptedStyleSheets, w.kAb];
        }
        w.kAb.replaceSync(text);
      }, css);
      await settle(page);
      const ab = await framed(page, async () => {
        await scrollRows(page);
        return [];
      });
      console.log(
        `INFO AB ${name}: dropped ${(ab.dropped * 100).toFixed(2)} %, ${ab.longTasks} long tasks`,
      );
    }
    await tree(page).evaluate(() =>
      (window as unknown as { kAb: CSSStyleSheet }).kAb.replaceSync(""),
    );
    await settle(page);
  }
  // The same scroll, another domain written all along (A40).
  await scrollWhileWritten(page);
  await settle(page);

  // From the top: the window ends near the last of the 10⁴ rows, where 50 presses would
  // reach the notice of the cap, and the focus would move no more.
  await tree(page).evaluate((el) => {
    let scroller = el as HTMLElement;
    while (!(scroller.scrollHeight > scroller.clientHeight)) scroller = scroller.parentElement!;
    scroller.scrollTop = 0;
  });
  await settle(page);
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
  // From the first screen of the list: its cells read before any hover.
  const fresh = freshReads(page);
  await page.goto(list);
  const grid = page.getByRole("grid", { name: "Préfixes" });
  const records = grid.locator("[role='gridcell'][data-column='value'] a");
  await expect(records.first()).toBeVisible();
  // The shifts of a navigation: every one, from the frame of its `k:view` (the screen
  // drawn) to a second after, none left out for the input of the click (`shiftsBetween`).
  const navigationShifts = async (i: number, witness = false) => {
    const link = records.nth(i);
    await link.scrollIntoViewIfNeeded();
    const id = new URL((await link.getAttribute("href"))!, BASE_URL).pathname.split("/").at(-1);
    // The reads of its route (`meta.queries`): the record and its properties. The hover
    // (60 ms) reads what the cache lacks: the VLAN column of the list has already read the
    // properties of a network in view (REQ-026-6), the hover then reads the record alone
    // (task 021-22g). Both fresh before the click, or the navigation is not one in cache.
    const reads = [`/v1/prefixes/${id}`, `/v1/prefixes/${id}/properties`];
    // Two moves within the link: the intent starts with a `pointermove` (021-7a), which a
    // move to where the pointer already is would not send (the link scrolled under it).
    await link.hover({ position: { x: 2, y: 2 } });
    await link.hover();
    const prefetched = await expect
      .poll(() => reads.every(fresh), { timeout: 10_000 })
      .toBe(true)
      .then(
        () => true,
        () => false,
      );
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
    return { delay, shift, prefetched };
  };
  const delays: number[] = [];
  const missed: string[] = [];
  const shifts: number[] = [];
  for (let i = 0; i < CACHED; i++) {
    const { delay, shift, prefetched } = await navigationShifts(i);
    if (prefetched) delays.push(delay);
    else missed.push(`record ${i} ${delay.toFixed(1)} ms`);
    shifts.push(shift);
  }
  // The 75th percentile of the navigations in cache alone, the worst printed below (A35);
  // one not prefetched waits for its reads, is no navigation in cache and is printed apart.
  check(
    `navigation in cache to k:view, p75 of ${delays.length} prefetched`,
    delays.length > 0 ? quantile(delays, 0.75) : Infinity,
    100,
    "ms",
  );
  if (delays.length > 0) spread("navigation in cache", delays, "ms");
  console.log(
    `INFO navigations not prefetched, out of the p75: ${missed.length > 0 ? missed.join(", ") : "none"}`,
  );
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
