import { expect, type CDPSession, type Page } from "@playwright/test";

// The protocol of the component benches (spec 020, design « Budgets », from the prototype
// D-1): a latency runs from the `timeStamp` of its event to the first animation frame after
// the expected state; dropped frames are 1 − frames observed ÷ (duration ÷ 16.67 ms),
// over a run or over the windows where a component works; a long task is one the browser
// reports (> 50 ms); the heap is read through CDP after a garbage collection. Each
// measure is the median of five passes, or their worst for a budget of « none ».

/** The state a measure waits for after its event. */
export type Until =
  /** An element matching the selector is in the document, with a box. */
  | { shown: string }
  /** No element matches the selector any more. */
  | { gone: string }
  /** The element matching the selector has the attribute with that value. */
  | { attr: [selector: string, name: string, value: string] }
  /** The element matching the selector holds the text, and no field. */
  | { text: [selector: string, text: string] }
  /** The focus is on another row or cell (`data-node`, `data-row`, `data-column`). */
  | { focusMoved: true };

/** A measure: the first event of one of `on` after it is armed, until a state. */
export interface Timing {
  on: string[];
  until: Until;
}

/** A latency, its bounds (clock of the page) and the long tasks that started during it. */
export interface Timed {
  latency: number;
  start: number;
  end: number;
  longTasks: number;
}

/** A run of animation frames: the part dropped, the long tasks and the longest of them. */
export interface Frames {
  dropped: number;
  longTasks: number;
  longest: number;
  /** The part dropped within the windows given to `framesStop` only. */
  windowDropped?: number;
  /** For a scroll: the rows it went through. */
  rows?: number;
}

interface Runtime {
  arm(timing: Timing, slot?: string): void;
  result(slot?: string): Promise<Timed>;
  framesStart(): void;
  framesStop(windows?: [start: number, end: number][]): Promise<Frames>;
  scroll(selector: string, rows: number, duration: number): Promise<Frames>;
}

declare global {
  interface Window {
    __kBench: Runtime;
  }
}

/** The runtime of the bench in the page, installed before its first script. */
function runtime(): void {
  const FRAME = 1000 / 60;
  const longTasks: { start: number; duration: number }[] = [];
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries())
      longTasks.push({ start: entry.startTime, duration: entry.duration });
  }).observe({ type: "longtask", buffered: true });
  const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
  const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
  /** The long tasks that started between `from` and `to`, once the browser said them. */
  async function tasksBetween(from: number, to: number) {
    await sleep(100);
    const during = longTasks.filter((task) => task.start >= from && task.start <= to);
    return {
      longTasks: during.length,
      longest: Math.max(0, ...during.map((task) => task.duration)),
    };
  }
  const focusKey = () => {
    const el = document.activeElement as HTMLElement | null;
    return el ? `${el.dataset.node}|${el.dataset.row}|${el.dataset.column}` : "";
  };
  const boxed = (el: Element | null) => !!el && el.getBoundingClientRect().height > 0;
  function holds(until: Until, from: string): boolean {
    if ("shown" in until) return boxed(document.querySelector(until.shown));
    if ("gone" in until) return document.querySelector(until.gone) === null;
    if ("attr" in until) {
      const [selector, name, value] = until.attr;
      return document.querySelector(selector)?.getAttribute(name) === value;
    }
    if ("text" in until) {
      const el = document.querySelector(until.text[0]);
      return !!el && !el.querySelector("input") && !!el.textContent?.includes(until.text[1]);
    }
    return focusKey() !== from;
  }
  /** The measures armed, by slot: two can wait at once (a move asked, then answered). */
  const pending = new Map<string, Promise<Timed>>();
  /** The element that scrolls `el`: itself or its first ancestor that scrolls. */
  function scroller(el: Element | null): HTMLElement {
    for (let at = el as HTMLElement | null; at; at = at.parentElement)
      if (at.scrollHeight > at.clientHeight && getComputedStyle(at).overflowY !== "visible")
        return at;
    throw new Error("nothing scrolls");
  }
  let started = 0;
  const times: number[] = [];
  let running = false;
  window.__kBench = {
    arm(timing, slot = "") {
      const from = focusKey();
      const measure = new Promise<Timed>((resolve, reject) => {
        let t0: number | undefined;
        let done = false;
        const observer = new MutationObserver(() => check());
        const stop = () => {
          done = true;
          observer.disconnect();
          for (const type of timing.on) window.removeEventListener(type, onEvent, true);
          document.removeEventListener("focusin", check, true);
          clearTimeout(timer);
        };
        const timer = setTimeout(() => {
          stop();
          reject(new Error(`no ${JSON.stringify(timing.until)} within 10 s`));
        }, 10_000);
        function check(): void {
          if (done || t0 === undefined || !holds(timing.until, from)) return;
          const start = t0;
          stop();
          requestAnimationFrame(() => {
            const end = performance.now();
            void tasksBetween(start, end).then(({ longTasks }) =>
              resolve({ latency: end - start, start, end, longTasks }),
            );
          });
        }
        const poll = () => {
          if (done) return;
          check();
          requestAnimationFrame(poll);
        };
        function onEvent(event: Event): void {
          if (t0 !== undefined) return;
          t0 = event.timeStamp;
          queueMicrotask(check);
          requestAnimationFrame(poll);
        }
        for (const type of timing.on) window.addEventListener(type, onEvent, true);
        document.addEventListener("focusin", check, true);
        observer.observe(document.body, {
          subtree: true,
          childList: true,
          attributes: true,
          characterData: true,
        });
      });
      // Read later: never an unhandled rejection meanwhile.
      measure.catch(() => undefined);
      pending.set(slot, measure);
    },
    result(slot = "") {
      const measure = pending.get(slot);
      if (!measure) throw new Error(`nothing armed in ${slot || "the default slot"}`);
      pending.delete(slot);
      return measure;
    },
    framesStart() {
      times.length = 0;
      running = true;
      started = performance.now();
      const tick = () => {
        if (!running) return;
        times.push(performance.now());
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    },
    async framesStop(windows = []) {
      running = false;
      const end = performance.now();
      const dropped = Math.max(0, 1 - times.length / ((end - started) / FRAME));
      // Within the windows: the frames drawn there (a window ends on a frame, its own
      // callback a moment before or after this one's), against their summed duration.
      const inside = windows.reduce(
        (sum, [from, to]) => sum + times.filter((t) => t > from && t <= to + 1).length,
        0,
      );
      const span = windows.reduce((sum, [from, to]) => sum + (to - from), 0);
      return {
        dropped,
        ...(span > 0 ? { windowDropped: Math.max(0, 1 - inside / (span / FRAME)) } : {}),
        ...(await tasksBetween(started, end)),
      };
    },
    async scroll(selector, rows, duration) {
      const list = document.querySelector(selector);
      const el = scroller(list);
      const row = list!.querySelector("[data-node]")!.getBoundingClientRect().height;
      el.scrollTop = 0;
      await frame();
      await frame();
      const target = rows * row;
      if (el.scrollHeight - el.clientHeight < target) throw new Error("fewer rows than asked");
      // The position follows the clock: a dropped frame does not lengthen the run.
      const start = performance.now();
      let frames = 0;
      await new Promise<void>((resolve) => {
        const tick = () => {
          frames++;
          const done = Math.min(1, (performance.now() - start) / duration);
          el.scrollTop = done * target;
          if (done < 1) requestAnimationFrame(tick);
          else resolve();
        };
        requestAnimationFrame(tick);
      });
      const end = performance.now();
      return {
        dropped: Math.max(0, 1 - frames / ((end - start) / FRAME)),
        rows: Math.round(el.scrollTop / row),
        ...(await tasksBetween(start, end)),
      };
    },
  };
}

/** A page with the runtime of the bench, and its CDP session for the heap. */
export async function benchPage(page: Page): Promise<CDPSession> {
  await page.addInitScript(runtime);
  return page.context().newCDPSession(page);
}

/** Opens `path`, waits for `ready` and the fonts, then two quiet frames. */
export async function load(page: Page, path: string, ready: string): Promise<void> {
  await page.goto(path);
  await page.locator(ready).first().waitFor();
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
  await settle(page);
}

/** Two animation frames and a pause: the page has drawn what it had to. */
export async function settle(page: Page): Promise<void> {
  await page.evaluate(
    () =>
      new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 50))),
      ),
  );
}

/** Arms `timing` in `slot`, before what will set it off; `measured` reads it. */
export async function arm(page: Page, timing: Timing, slot = ""): Promise<void> {
  await page.evaluate(([t, s]) => window.__kBench.arm(t, s), [timing, slot] as const);
}

/** The measure armed in `slot`, once its state is reached. */
export function measured(page: Page, slot = ""): Promise<Timed> {
  return page.evaluate((s) => window.__kBench.result(s), slot);
}

/** The latency of what `act` does, by `timing`. */
export async function timed(
  page: Page,
  timing: Timing,
  act: () => Promise<unknown>,
): Promise<Timed> {
  await arm(page, timing);
  await act();
  return measured(page);
}

/**
 * Frames counted while `act` runs; `act` gives the windows (clock of the page) to count
 * apart as well.
 */
export async function framed(page: Page, act: () => Promise<[number, number][]>): Promise<Frames> {
  await page.evaluate(() => window.__kBench.framesStart());
  const windows = await act();
  return page.evaluate((w) => window.__kBench.framesStop(w), windows);
}

/** A scroll of `rows` rows of the list `selector` in `duration` ms. */
export function scroll(page: Page, selector: string, rows: number, duration: number) {
  return page.evaluate(([s, r, d]) => window.__kBench.scroll(s, r, d), [
    selector,
    rows,
    duration,
  ] as const);
}

/** The JavaScript heap in use after a garbage collection, in MB (10⁶ bytes). */
export async function heap(cdp: CDPSession): Promise<number> {
  await cdp.send("HeapProfiler.collectGarbage");
  const { usedSize } = await cdp.send("Runtime.getHeapUsage");
  return usedSize / 1e6;
}

/** The value at rank `q` (0 to 1) of `samples`, by the nearest rank. */
export function quantile(samples: number[], q: number): number {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(sorted.length * q) - 1)]!;
}

/**
 * One measure of a bench: its budget (`limit`, none for a figure given for reading),
 * checked on the median of the passes, or on the worst of them (`worst`: a budget of
 * « none », such as long tasks, holds on every pass).
 */
export interface Budget {
  name: string;
  unit: "ms" | "%" | "MB" | "";
  limit?: number;
  worst?: boolean;
}

/** The samples of the passes, measure by measure; the median of each, checked at the end. */
export class Passes {
  private readonly samples = new Map<string, number[]>();
  constructor(private readonly budgets: Record<string, Budget>) {}

  add(key: string, value: number): void {
    if (!(key in this.budgets)) throw new Error(`no budget ${key}`);
    this.samples.set(key, [...(this.samples.get(key) ?? []), value]);
  }

  /**
   * One line per measure, `PASS|FAIL <budget>: <median or worst> (limit <limit>)` or
   * `INFO <figure>: <median>`, and a soft expectation for each budget: the bench goes on
   * and fails at the end if one is exceeded.
   */
  check(): void {
    for (const [key, { name, unit, limit, worst }] of Object.entries(this.budgets)) {
      const values = this.samples.get(key) ?? [];
      expect(values.length, `${name}: no sample`).toBeGreaterThan(0);
      const median = worst ? Math.max(...values) : quantile(values, 0.5);
      const shown = (v: number) =>
        unit === "%"
          ? `${(v * 100).toFixed(2)} %`
          : unit === ""
            ? String(v)
            : `${v.toFixed(1)} ${unit}`;
      const spread = values.length > 1 ? ` [${values.map((v) => shown(v)).join(", ")}]` : "";
      if (limit === undefined) {
        console.log(`INFO ${name}: ${shown(median)}${spread}`);
        continue;
      }
      console.log(
        `${median <= limit ? "PASS" : "FAIL"} ${name}: ${shown(median)} (limit ${shown(limit)})${spread}`,
      );
      expect.soft(median, name).toBeLessThanOrEqual(limit);
    }
  }
}
