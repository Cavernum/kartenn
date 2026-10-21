import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { STATE } from "../fixtures/env.ts";
import { loginIn } from "../fixtures/index.ts";

// The budgets of the browser (spec 007, REQ-007-32, design « Budgets »), on the dataset of
// bench-core and a container of 10⁵ networks, the CPU slowed down four times. Each test
// prints its measure as `PASS|FAIL <budget>: <measured> (limit <limit>)`.

/** The ids `serve.sh budgets` got from `kartenn-bench seed`. */
function seeded(): { domain: string; container: string } {
  const line = readFileSync(`${STATE}budgets.env`, "utf8");
  const domain = /domain=([0-9a-f-]{36})/.exec(line)?.[1];
  const container = /container=([0-9a-f-]{36})/.exec(line)?.[1];
  if (!domain || !container) throw new Error(`no seeded ids in ${STATE}budgets.env`);
  return { domain, container };
}

async function slowDown(page: Page): Promise<void> {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
}

function report(name: string, measured: number, limit: number, unit = "ms"): void {
  const verdict = measured <= limit ? "PASS" : "FAIL";
  console.log(`${verdict} ${name}: ${measured.toFixed(1)} ${unit} (limit ${limit} ${unit})`);
}

const p95 = (samples: number[]) =>
  [...samples].sort((a, b) => a - b)[Math.max(0, Math.ceil(samples.length * 0.95) - 1)]!;

test("first useful screen within 2 s of the callback, a slow plugin set aside", async ({
  page,
}) => {
  await slowDown(page);
  await loginIn(page, "alice");
  // From the start of the fetch of the shell, after the redirects of the callback, to the
  // roots of the first domain drawn.
  const elapsed = await page.evaluate(async () => {
    const ready = await new Promise<PerformanceEntry>((resolve) => {
      const done = performance.getEntriesByName("k:ready")[0];
      if (done) return resolve(done);
      new PerformanceObserver((list, observer) => {
        const entry = list.getEntriesByName("k:ready")[0];
        if (entry) {
          observer.disconnect();
          resolve(entry);
        }
      }).observe({ type: "mark" });
    });
    const navigation = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming;
    return ready.startTime - navigation.fetchStart;
  });
  report("first useful screen", elapsed, 2_000);
  expect(elapsed).toBeLessThanOrEqual(2_000);

  // The plugin that defines itself after 6 s is set aside at 5 s.
  await page.goto("/about");
  await expect(
    page.getByRole("main").getByRole("listitem").filter({ hasText: "slow 1.0.0" }),
  ).toContainText("aucune extension définie dans le délai", { timeout: 15_000 });
});

test("a page of 100 rows drawn within 100 ms of its response", async ({ page }) => {
  await loginIn(page, "alice");
  await slowDown(page);
  await page.goto("/addresses");
  const scroller = page.locator(".k-table-scroll");
  await expect(scroller.locator("tbody tr").first()).toBeVisible();
  // Twenty pages, read as the end of the table comes into the window.
  await expect(async () => {
    await scroller.evaluate((element) => element.scrollTo(0, element.scrollHeight));
    const marks = await page.evaluate(() => performance.getEntriesByName("k:rows").length);
    expect(marks).toBeGreaterThanOrEqual(21);
  }).toPass({ timeout: 120_000 });
  const delays = await page.evaluate(() => {
    const pages = performance
      .getEntriesByType("resource")
      .filter((entry) => new URL(entry.name).pathname === "/v1/addresses")
      .map((entry) => (entry as PerformanceResourceTiming).responseEnd);
    const marks = performance.getEntriesByName("k:rows").map((mark) => mark.startTime);
    // Each page, to the first mark after its response; the first page is left out, the
    // warm-up of the view.
    return pages.slice(1).flatMap((end) => {
      const drawn = marks.find((mark) => mark >= end);
      return drawn === undefined ? [] : [drawn - end];
    });
  });
  expect(delays.length).toBeGreaterThanOrEqual(20);
  const measured = p95(delays);
  report("page of 100 rows, p95 of 20", measured, 100);
  expect(measured).toBeLessThanOrEqual(100);
});

test("10⁴ rows of a container of 10⁵ scrolled in 10 s without a long task", async ({ page }) => {
  const { domain } = seeded();
  await loginIn(page, "alice");
  await page.goto(`/?domain=${domain}`);
  const tree = page.getByRole("tree");
  const root = tree.getByRole("treeitem").first();
  await expect(root).toHaveAttribute("aria-expanded", "false");
  await tree.focus();
  await page.keyboard.press("ArrowRight");
  await expect(root).toHaveAttribute("aria-expanded", "true");
  await expect(tree.getByRole("treeitem").nth(1)).toBeVisible();
  await slowDown(page);
  const { reached, longTasks } = await tree.evaluate(async (element) => {
    const long: number[] = [];
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) long.push(entry.duration);
    });
    observer.observe({ type: "longtask" });
    const row = element.querySelector<HTMLElement>("[role=treeitem]")!.offsetHeight;
    const started = performance.now();
    // 100 steps of 100 rows, one every 100 ms; a step waits for the rows it needs.
    for (let step = 1; step <= 100; step++) {
      element.scrollTop = step * 100 * row;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    const ranks = [...element.querySelectorAll("[role=treeitem]")].map((item) =>
      Number(item.getAttribute("aria-posinset") ?? 0),
    );
    observer.disconnect();
    return {
      reached: Math.max(...ranks),
      longTasks: long,
      elapsed: performance.now() - started,
    };
  });
  console.log(
    `${reached >= 10_000 ? "PASS" : "FAIL"} rows of the container reached in 10 s: ${reached} (at least 10000)`,
  );
  report("longest task while scrolling", Math.max(0, ...longTasks), 50);
  expect(reached).toBeGreaterThanOrEqual(10_000);
  expect(longTasks.filter((duration) => duration > 50)).toEqual([]);
});
