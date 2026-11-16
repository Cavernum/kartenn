import { readFileSync } from "node:fs";
import {
  expect,
  test,
  type CDPSession,
  type Locator,
  type Page,
  type Request,
} from "@playwright/test";
import { loadLine, watchMachine } from "../bench/machine.ts";
import { benchPage, framed, heap, quantile, settle, timed, type Frames } from "../bench/measure.ts";
import { BASE_URL, isBatch, readsOf, STATE } from "../fixtures/env.ts";
import { atRest, loginIn } from "../fixtures/journeys.ts";
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
  /** When the body of each response was read (`Response.json`), by its URL, the last. */
  parsed: Record<string, number>;
  /** The mutation that first put each `data-row` in the document, once `watchRows` runs. */
  inserted: Record<string, number>;
  /** At each scan of `watchRows`: the rows of the grid in the document, and if virtualised. */
  mounted: { at: number; rows: number; virtual: boolean }[];
  /** The long animation frames (> 50 ms), their render phases and their longest scripts. */
  frames: LongFrame[];
}

/** A `long-animation-frame` entry, as the probe keeps it. */
interface LongFrame {
  start: number;
  duration: number;
  /** The start of its rendering (`renderStart`), then of its style and layout. */
  render: number;
  layout: number;
  scripts: { invoker: string; start: number; duration: number; layout: number }[];
}

declare global {
  interface Window {
    __kProbe: Probe;
  }
}

/**
 * Added after `probe` by the page of 100 rows alone (021-22f-e): when each body is read,
 * and the long animation frames, which `pageSpans` reads.
 */
function watchSpans(): void {
  const p = window.__kProbe;
  const json = Response.prototype.json;
  Response.prototype.json = function (this: Response) {
    const url = this.url;
    return json.call(this).then((body: unknown) => {
      p.parsed[url] = performance.now();
      return body;
    });
  };
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries() as (PerformanceEntry & {
      renderStart: number;
      styleAndLayoutStart: number;
      scripts: (PerformanceEntry & {
        invoker: string;
        sourceFunctionName: string;
        forcedStyleAndLayoutDuration: number;
      })[];
    })[])
      p.frames.push({
        start: entry.startTime,
        duration: entry.duration,
        render: entry.renderStart,
        layout: entry.styleAndLayoutStart,
        scripts: entry.scripts.map((s) => ({
          invoker: `${s.invoker}${s.sourceFunctionName ? ` ${s.sourceFunctionName}` : ""}`,
          start: s.startTime,
          duration: s.duration,
          layout: s.forcedStyleAndLayoutDuration,
        })),
      });
  }).observe({ type: "long-animation-frame", buffered: true } as PerformanceObserverInit);
}

function probe({ origin, useful }: { origin: string; useful: string }): void {
  const p: Probe = {
    events: [],
    shifts: [],
    clicks: [],
    rows: {},
    stream: [],
    parsed: {},
    inserted: {},
    mounted: [],
    frames: [],
  };
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
 * How long `scrollRows` takes over its `ROWS` rows, in ms: 500 rows/s (task 021-22e-f,
 * arbitration C), four times the speed of the prototype's protocol (021 design,
 * « Budgets »).
 */
const SCROLL_MS = 20_000;

/**
 * `ROWS` rows of the tree in `SCROLL_MS`: down and up the rows in memory, the position by
 * the clock (the protocol of the prototype: a dropped frame does not lengthen the run),
 * short of the last rows, so that the row which reads the next page never comes into view.
 * The window of the scroll (clock of the page).
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
    [ROWS, SCROLL_MS] as const,
  );

/**
 * The window of `act` watched (task 021-22e-l): its frames counted (`framed`), the machine
 * meanwhile (`watchMachine`), and the main thread of the page by CDP (`Performance`: its
 * tasks, scripts, layout and style, and the CPU time of the thread). Printed for reading in
 * one line `INFO <what>`; the frames given back for the verdicts.
 */
async function watched(
  page: Page,
  cdp: CDPSession,
  what: string,
  act: () => Promise<void>,
): Promise<Frames> {
  await cdp.send("Performance.enable");
  const metrics = async () =>
    new Map(
      (await cdp.send("Performance.getMetrics")).metrics.map((m) => [m.name, m.value] as const),
    );
  const before = await metrics();
  const machine = watchMachine();
  const frames = await framed(page, async () => {
    await act();
    return [];
  });
  const outside = machine.stop();
  const after = await metrics();
  const spent = (name: string) =>
    `${(((after.get(name) ?? 0) - (before.get(name) ?? 0)) * 1000).toFixed(0)} ms`;
  const i = frames.intervals;
  console.log(
    `INFO ${what}: dropped frames ${(frames.dropped * 100).toFixed(2)} %, ${frames.longTasks}` +
      ` long tasks (the longest ${frames.longest.toFixed(1)} ms); frame intervals` +
      ` ${i ? `median ${i.p50.toFixed(1)} ms, p99 ${i.p99.toFixed(1)} ms, longest ${i.max.toFixed(1)} ms, ${i.gaps} over 1.5 frames` : "unknown"};` +
      ` main thread of the page: tasks ${spent("TaskDuration")}, scripts ${spent("ScriptDuration")},` +
      ` layout ${spent("LayoutDuration")}, style ${spent("RecalcStyleDuration")}, CPU of the` +
      ` thread ${spent("ThreadTime")}; ${outside}`,
  );
  return frames;
}

/**
 * The witness of the frames (task 021-22e-l): `SCROLL_MS` of animation frames counted in the
 * same page, nothing scrolled, the CPU as the caller left it (slowed down, as for the
 * scroll). What the browser and the machine drop of themselves, against which the scroll's
 * dropped frames are read.
 */
async function witness(page: Page, cdp: CDPSession, what: string): Promise<Frames> {
  await settle(page);
  return watched(page, cdp, `witness, ${what}, nothing scrolled for ${SCROLL_MS / 1000} s`, () =>
    page.waitForTimeout(SCROLL_MS),
  );
}

/**
 * A Chromium trace of `act` when `K_BENCH_TRACE` names a directory (task 021-22e-l), off by
 * default: the threads of every process of the browser, its frames and its garbage
 * collections. A diagnosis, never a verdict: tracing costs the page some of its time.
 */
async function traced(page: Page, what: string, act: () => Promise<void>): Promise<void> {
  const dir = process.env.K_BENCH_TRACE;
  const browser = page.context().browser();
  if (!dir || !browser) return act();
  const path = `${dir}/${what}-${Date.now()}.json`;
  await browser.startTracing(page, {
    path,
    categories: [
      "toplevel",
      "devtools.timeline",
      "disabled-by-default-devtools.timeline",
      "disabled-by-default-devtools.timeline.frame",
      "v8",
      "v8.execute",
      "blink",
      "cc",
      "viz",
      "gpu",
      "benchmark",
      "disabled-by-default-v8.gc",
    ],
  });
  try {
    await act();
  } finally {
    await browser.stopTracing();
    console.log(`INFO trace ${what}: ${path}`);
  }
}

// The load of the machine at the start and at the end of each test (task 021-22e-l).
test.beforeEach(({}, info) => {
  console.log(`INFO machine at the start of « ${info.title} »: ${loadLine()}`);
});
test.afterEach(({}, info) => {
  console.log(`INFO machine at the end of « ${info.title} »: ${loadLine()}`);
});

/** The long animation frames of the page watched from now on (task 021-22e-i). */
const watchFrames = (page: Page) =>
  page.evaluate(() => {
    const w = window as unknown as { __kLoaf?: PerformanceEntry[] };
    if (w.__kLoaf) return;
    const seen: PerformanceEntry[] = [];
    w.__kLoaf = seen;
    new PerformanceObserver((list) => seen.push(...list.getEntries())).observe({
      type: "long-animation-frame",
    });
  });

/**
 * The animation frames of 100 ms or more that started between `from` and `to` (clock of the
 * page), and those that hold a long task (blocking time above zero, task 021-22e-l), printed
 * with where their time went (task 021-22e-i): tasks before the frame, its
 * `requestAnimationFrame` callbacks, style and layout, and the scripts the browser names.
 * Before them, every long animation frame of the window (50 ms or more) summed.
 */
async function longFramesBetween(page: Page, what: string, from: number, to: number) {
  const { lines, summary } = await page.evaluate(
    ([f, t]) => {
      interface Script {
        invoker: string;
        invokerType: string;
        sourceFunctionName: string;
        sourceURL: string;
        sourceCharPosition: number;
        duration: number;
        forcedStyleAndLayoutDuration: number;
      }
      type Loaf = PerformanceEntry & {
        renderStart: number;
        styleAndLayoutStart: number;
        blockingDuration: number;
        scripts: Script[];
      };
      const all = ((window as unknown as { __kLoaf?: Loaf[] }).__kLoaf ?? []) as Loaf[];
      const ms = (v: number) => Math.max(0, v).toFixed(0);
      const inWindow = all.filter((e) => e.startTime >= f && e.startTime <= t);
      const sum = (pick: (e: Loaf) => number) => ms(inWindow.reduce((s, e) => s + pick(e), 0));
      const summary =
        `${inWindow.length} of 50 ms or more, ${sum((e) => e.duration)} ms in all` +
        ` (blocking ${sum((e) => e.blockingDuration)} ms; style and layout` +
        ` ${sum((e) => (e.styleAndLayoutStart ? e.startTime + e.duration - e.styleAndLayoutStart : 0))} ms)`;
      const lines = inWindow
        .filter((e) => e.duration >= 100 || e.blockingDuration > 0)
        .map((e) => {
          const end = e.startTime + e.duration;
          const render = e.renderStart || end;
          const style = e.styleAndLayoutStart || end;
          const scripts = e.scripts
            .map(
              (s) =>
                `${s.invokerType} ${s.invoker} ${s.sourceFunctionName || "?"}` +
                `@${s.sourceURL.split("/").at(-1)}:${s.sourceCharPosition}` +
                ` ${ms(s.duration)} ms (forced style and layout ${ms(s.forcedStyleAndLayoutDuration)})`,
            )
            .join("; ");
          return (
            `at +${ms(e.startTime - f)} ms, ${ms(e.duration)} ms (blocking ${ms(e.blockingDuration)}):` +
            ` tasks ${ms(render - e.startTime)}, frame callbacks ${ms(style - render)},` +
            ` style and layout ${ms(end - style)}; ${scripts || "no script named"}`
          );
        });
      return { lines, summary };
    },
    [from, to] as const,
  );
  console.log(
    `INFO long frames ${what}: ${summary}; of 100 ms or blocking:` +
      ` ${lines.length === 0 ? "none" : `${lines.length}; ${lines.join(" | ")}`}`,
  );
}

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
async function scrollWhileWritten(page: Page, cdp: CDPSession): Promise<void> {
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
    const opened = await page.evaluate(() => performance.now());
    frames = await watched(page, cdp, `while scrolling, ${OTHER_DOMAIN} written`, async () => {
      span = await scrollRows(page);
    });
    duringWindow = writer.writes.written - before;
    await longFramesBetween(page, `while scrolling, ${OTHER_DOMAIN} written`, opened, span[1]);
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
  // A batch of properties reads each of its objects (task 021-11p10-c).
  const pathsOf = (request: Request) => readsOf(request.url());
  const add = (request: Request, n: number) => {
    for (const path of pathsOf(request)) pending.set(path, (pending.get(path) ?? 0) + n);
  };
  page.on("request", (request) => add(request, 1));
  page.on("requestfailed", (request) => add(request, -1));
  page.on("requestfinished", (request) => {
    // Its status known before it leaves the reads under way.
    void request.response().then(async (response) => {
      if (response && response.status() < 400) {
        // A batch answers the objects of its items alone: one left out is not fresh.
        const items = isBatch(request.url())
          ? ((await response.json()) as { items: { id: string }[] }).items
          : undefined;
        for (const path of readsOf(request.url(), items)) answered.set(path, Date.now());
      }
      add(request, -1);
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

/**
 * Runs in the page: for each page drawn, what parts the time from the end of its response
 * to its first row drawn (021-22f-e): to its body read, to the mutation that brings the row,
 * to the frame; the other reads in flight at its end, and those that landed before its
 * row; the long animation frames over it, their scripts and their style and layout.
 */
function pageSpans(samples: { url: string; ids: string[] }[]): string[] {
  const p = window.__kProbe;
  const kind = (name: string) =>
    new URL(name).pathname.replace(/^\/v1\//, "").replace(/[0-9a-f-]{36}/g, ":id");
  const tally = (names: string[]) => {
    const counts = new Map<string, number>();
    for (const n of names) counts.set(n, (counts.get(n) ?? 0) + 1);
    return [...counts].map(([k, n]) => `${k} ${n}`).join(", ") || "none";
  };
  const ms = (v: number) => v.toFixed(1);
  const resources = performance.getEntriesByType("resource") as PerformanceResourceTiming[];
  return samples.map(({ url, ids }, i) => {
    const timing = performance.getEntriesByName(url).at(-1) as PerformanceResourceTiming;
    const end = timing.responseEnd;
    const first = (at: Record<string, number>) =>
      Math.min(...ids.map((k) => at[k]).filter((v): v is number => v !== undefined));
    const drawn = first(p.rows);
    const inserted = first(p.inserted);
    const parsed = p.parsed[url] ?? NaN;
    const others = resources.filter((e) => e.name !== url && !e.name.includes("/events"));
    const flying = others.filter((e) => e.startTime < end && e.responseEnd > end);
    const landed = others.filter((e) => e.responseEnd > end && e.responseEnd <= drawn);
    const frames = p.frames.filter((f) => f.start < drawn && f.start + f.duration > end);
    const shownFrames = frames.map((f) => {
      const scripts = f.scripts.reduce((sum, s) => sum + s.duration, 0);
      const layout = f.layout > 0 ? f.start + f.duration - f.layout : 0;
      const top = [...f.scripts]
        .sort((a, b) => b.duration - a.duration)
        .slice(0, 2)
        .map((s) => `${s.invoker} ${ms(s.duration)}`)
        .join("; ");
      return `${ms(f.duration)} at ${ms(f.start - end)} (scripts ${ms(scripts)} [${top}], style and layout ${ms(layout)})`;
    });
    const stream = p.stream.filter((s) => s.at > end - 100 && s.at <= drawn).map((s) => s.kind);
    const dom = (at: number) => p.mounted.filter((m) => m.at <= at).at(-1);
    const shownDom = (m: { rows: number; virtual: boolean } | undefined) =>
      m ? `${m.rows}${m.virtual ? " virtualised" : ""}` : "?";
    return (
      `INFO page ${i + 1}: ${ms(drawn - end)} ms = body ${ms(parsed - end)} + render` +
      ` ${ms(inserted - parsed)} + frame ${ms(drawn - inserted)}; network ${ms(end - timing.startTime)};` +
      ` rows in the document ${shownDom(dom(end))} then ${shownDom(dom(drawn))};` +
      ` in flight at its end: ${tally(flying.map((e) => kind(e.name)))}; landed before its row:` +
      ` ${tally(landed.map((e) => kind(e.name)))}; long frames: ${shownFrames.join(" | ") || "none"}` +
      `; stream: ${stream.join(", ") || "none"}`
    );
  });
}

test("a page of 100 rows drawn within 100 ms of its response", async ({ page }) => {
  const cdp = await benchOf(page);
  await page.addInitScript(watchSpans);
  await loginIn(page, "alice");
  await slowDown(cdp);
  // The warning of the shell once it sets `slow` aside (shell/src/plugins/loader.ts).
  let setAside = false;
  page.on("console", (m) => {
    if (/^kartenn: plugin slow .* set aside/.test(m.text())) setAside = true;
  });
  await page.goto("/addresses");
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.locator("[role='gridcell']").first()).toBeVisible();
  // The plugins settled before the first scroll (021-22f-g): their fetches over (the
  // shell's mark), the columns of `machines` and `example` in the header, `slow` set aside
  // at 5 s (its warning), then two frames. Their start belongs to the budget of the first
  // screen, not to that of a page: scrolled at the first cell, the first page waited 139 to
  // 158 ms behind their evaluation (22f-e), and a page drawn as `slow` is set aside waited
  // 124 ms behind a timer task of 155 ms (22f-g, a rest of 3 s without this wait).
  await atRest(page);
  for (const column of ["x.machines.machine", "x.example.usage"])
    await expect(grid.locator(`[role='columnheader'][data-column='${column}']`)).toBeVisible();
  await expect.poll(() => setAside, { timeout: 15_000 }).toBe(true);
  await settle(page);
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
      window.__kProbe.mounted.push({
        at: now,
        rows: el.querySelectorAll(".k-data-grid__body > [role='row']").length,
        virtual:
          el.closest("[data-virtual]") !== null || el.querySelector("[data-virtual]") !== null,
      });
    };
    scan();
    let pending = false;
    // The mutation that brings a row, apart from its frame: what the render took, and what
    // the frame waited for after it.
    const inserted = window.__kProbe.inserted;
    const mark = (node: Element, at: number) => {
      const key = (node as HTMLElement).dataset.row;
      if (key && !(key in inserted)) inserted[key] = at;
    };
    new MutationObserver((records) => {
      const at = performance.now();
      for (const record of records) {
        if (record.type === "attributes" && record.attributeName === "data-row")
          mark(record.target as Element, at);
        for (const node of record.addedNodes)
          if (node instanceof Element) {
            mark(node, at);
            for (const inner of node.querySelectorAll("[data-row]")) mark(inner, at);
          }
      }
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
  const samples: { url: string; ids: string[] }[] = [];
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
      else {
        delays.push(delay);
        samples.push({ url, ids });
      }
    }
    measured = new Set(now);
    await settle(page);
    await page.waitForTimeout(300);
  }
  console.log(
    `INFO reads of /v1/addresses without rows: ${withoutRows}; pages of which no row was` +
      ` drawn within 5 s of its response: ${undrawn}`,
  );
  for (const line of await page.evaluate(pageSpans, samples)) console.log(line);
  expect(delays.length).toBeGreaterThanOrEqual(PAGES - 2);
  check(`page of 100 rows, p95 of ${delays.length}`, quantile(delays, 0.95), 100, "ms");
  spread("page of 100 rows", delays, "ms");
});

test("a container of 10⁵ scrolled over 10⁴ rows, Arrow down in the tree, the heap after 50 navigations", async ({
  page,
}) => {
  const cdp = await benchOf(page);
  await loginIn(page, "alice");
  // The witness before the tree (task 021-22e-l): the home of the shell, at rest.
  await slowDown(cdp);
  await witness(page, cdp, "at home, no tree");
  await fullSpeed(cdp);
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
  await watchFrames(page);
  // 10⁴ rows in 20 s (`scrollRows`, 500 rows/s), on a quiet instance: the window measures
  // the drawing alone, and the reads it would set off are counted.
  let span: [number, number] = [0, 0];
  const opened = await page.evaluate(() => performance.now());
  const frames = await watched(page, cdp, "while scrolling", () =>
    traced(page, "tree-scroll", async () => {
      span = await scrollRows(page);
    }),
  );
  // The levels of the tree go stale 30 s after their last read (`FRESHNESS.children`), and
  // the topography rebuilt every node then (task 021-22e-i): when, against the window.
  const lastRead = await page.evaluate(
    (path) =>
      Math.max(
        ...performance
          .getEntriesByType("resource")
          .filter((e) => new URL(e.name).pathname === path)
          .map((e) => (e as PerformanceResourceTiming).responseEnd),
      ),
    children,
  );
  console.log(
    `INFO the last read of the container's children ended ${((opened - lastRead) / 1000).toFixed(1)} s` +
      ` before the window opened`,
  );
  await longFramesBetween(page, "while scrolling", opened, span[1]);
  const reads = await readsWhile(page, "while scrolling", container, span);
  await streamBetween(page, "while scrolling", ...span);
  check("reads of children while scrolling", reads.children, 0, "");
  check(`dropped frames over ${ROWS} rows in ${SCROLL_MS / 1000} s`, frames.dropped, 0.02, "%");
  check("long tasks while scrolling", frames.longTasks, 0, "");
  console.log(`INFO longest task while scrolling: ${frames.longest.toFixed(1)} ms`);
  // A diagnosis, off by default (`K_BENCH_TREE_AB=1`, tasks 021-22e-c and 22e-d): the same
  // scroll again under sheets that stand for the options of the arbitration B, its dropped
  // frames printed. Never a verdict: the sheets change what the rows show. B1: the « + »
  // shown only on the active row (focus, selection, hover); B2: each icon one box painted
  // through a CSS mask, no SVG; B3: no hover while scrolling (in the tree since the task
  // 22e-e: its sheet only holds the hover off for the whole pass). Each at the speed of the
  // budget, 500 rows/s since the task 22e-f (the variants `@500` gone with it). B4 (fixed
  // slots, rows placed by an inline style) needs the code of the tree changed: see the
  // task 22e-d, its figures in tasks.md.
  if (process.env.K_BENCH_TREE_AB) {
    const B = ".k-tree-grid__body";
    const mask = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m9 18 6-6-6-6'/%3E%3C/svg%3E")`;
    const B1 = `${B} .k-tree-grid__row:not(:focus-within,:hover,[aria-selected="true"]) .k-tree-grid__add{display:none!important}`;
    const B2 =
      `${B} svg{display:none!important}\n` +
      `${B} .k-tree-grid__twisty::before,${B} .k-tree-grid__node::after,` +
      `${B} .k-tree-grid__row:is(:focus-within,:hover,[aria-selected="true"]) .k-tree-grid__add::before` +
      `{content:"";display:block;inline-size:16px;block-size:16px;flex:none;` +
      `background-color:currentColor;mask:${mask} center/contain no-repeat}`;
    const B3 = `${B}{pointer-events:none}`;
    const variants: [string, string][] = [
      ["base", ""],
      ["B1", B1],
      ["B2", B2],
      ["B3", B3],
      ["B1+B3", [B1, B3].join("\n")],
      ["B1+B2+B3", [B1, B2, B3].join("\n")],
      // What the rows cost at most: no icon and no hover at all.
      ["no svg, no pointer", `${B} svg{display:none!important}\n${B3}`],
      // The spread of two identical passes.
      ["base again", ""],
    ];
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
  await scrollWhileWritten(page, cdp);
  // The witness after the scrolls (task 021-22e-l): the tree mounted, at rest.
  await witness(page, cdp, "the tree mounted");
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
