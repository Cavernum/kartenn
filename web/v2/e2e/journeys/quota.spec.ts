import { QUOTA_URL } from "../fixtures/env.ts";
import { expect, expectAccessible, QUOTA_DATA, QUOTA_USERS, test } from "../fixtures/journeys.ts";

// The shell at the read quota by default (task 021-11p10-b2; REQ-006-29, 50 a second and 200
// at once by identity; REQ-006-30, 20 requests in flight): the other journeys run on an
// instance whose read quota is raised out of their way (kartenn.toml, task 021-11p9), this
// one on its own instance (quota.toml, started by quota.setup.ts; the projects
// `quota-chromium` and `quota-firefox`, each browser its user, each user its quota). Ten
// pages of addresses scrolled screen by screen to their end, a column of a property shown:
// every row drawn with its value, no cell unreadable, no alert, no 503. The end-to-end proof
// of the one gate of the reads by row (11p10-a), of the refusals read again after
// `Retry-After` (11p10-b), of the batches of properties (11p10-c) and of the interfaces of
// the machine column read by window (11p10-e: before, one read per row, about 1 000 for ten
// pages and 75 to 102 429 a pass): no 429 nor 503 seen by the page, and at most
// `PER_PAGE` requests under /v1/ while each page is read. The requests by page read and by
// path, and the refusals by path, printed first.

/**
 * The requests under /v1/ while a page of the list is read, at most: the page, a batch of
 * properties and a window of interfaces per screen scrolled (three to four screens a page),
 * the names and the counts of the shell; before 11p10-e, 55 to 178.
 */
const PER_PAGE = 30;

declare global {
  interface Window {
    /** The rows whose cell of the property showed its value. */
    __drawn?: Set<string>;
    /** The rows whose cell said the value unreadable. */
    __unreadable?: Set<string>;
    /** The texts of the alerts the page showed, at any moment. */
    __alerts?: Set<string>;
  }
}

test("ten pages scrolled to their end at the read quota by default: every row drawn with its property, no alert, no 429 nor 503, a ceiling of requests by page", async ({
  page,
  login,
  browserName,
}) => {
  test.setTimeout(180_000);
  // The pages of the list read (the first identifier of each, once), the requests under
  // /v1/ sent while `n` pages were read (`byPage[n]`), and the refusals by load.
  const pages: string[] = [];
  const byPage: number[] = [0];
  const byPath = new Map<string, number>();
  const refused: string[] = [];
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (url.origin !== QUOTA_URL || !url.pathname.startsWith("/v1/")) return;
    byPage[pages.length] = (byPage[pages.length] ?? 0) + 1;
    const path = url.pathname.replace(/[0-9a-f]{8}-[0-9a-f-]{27}/g, "{id}");
    byPath.set(path, (byPath.get(path) ?? 0) + 1);
  });
  page.on("response", (response) => {
    const url = new URL(response.url());
    if (url.origin !== QUOTA_URL || !url.pathname.startsWith("/v1/")) return;
    if (response.status() === 429 || response.status() === 503)
      refused.push(`${response.status()} ${url.pathname}${url.search}`);
    if (url.pathname !== "/v1/addresses" || !response.ok()) return;
    void response
      .json()
      .then((body: { items?: { id: string }[] }) => {
        const first = body.items?.[0]?.id;
        if (first && !pages.includes(first)) pages.push(first);
      })
      .catch(() => undefined);
  });

  // Any alert a page of the shell shows, kept: one shown then gone fails the journey too.
  await page.addInitScript(() => {
    const alerts = new Set<string>();
    window.__alerts = alerts;
    const scan = () => {
      for (const alert of document.querySelectorAll('[role="alert"], .k-inline-alert')) {
        const text = alert.textContent?.trim();
        if (text) alerts.add(text);
      }
    };
    new MutationObserver(scan).observe(document, {
      subtree: true,
      childList: true,
      characterData: true,
    });
  });
  await login(QUOTA_USERS[browserName as keyof typeof QUOTA_USERS]);
  await page.goto(`/addresses?within=${encodeURIComponent(QUOTA_DATA.prefix)}`);
  const grid = page.getByRole("grid", { name: "Adresses" });
  await expect(grid.getByRole("link", { name: "2001:db8:4::1", exact: true })).toBeVisible();
  // The column of the property, by « Colonnes » (none by default).
  const column = `prop.${QUOTA_DATA.property}`;
  await page.getByRole("button", { name: "Colonnes" }).click();
  await page.getByRole("menuitemcheckbox", { name: QUOTA_DATA.property }).click();
  await page.keyboard.press("Escape");
  await expect(grid.locator(`[role="gridcell"][data-column="${column}"]`).first()).toContainText(
    QUOTA_DATA.value,
  );
  await expectAccessible(page);

  // Every cell of the property in the document once, by its row: its value, or unreadable.
  await grid.evaluate(
    (el, { column, value }) => {
      const drawn = new Set<string>();
      const unreadable = new Set<string>();
      window.__drawn = drawn;
      window.__unreadable = unreadable;
      const scan = () => {
        for (const cell of el.querySelectorAll<HTMLElement>(
          `[role="gridcell"][data-column="${column}"]`,
        )) {
          const text = cell.textContent ?? "";
          if (text.includes(value)) drawn.add(cell.dataset.row!);
          else if (text.includes("Illisible")) unreadable.add(cell.dataset.row!);
        }
      };
      scan();
      new MutationObserver(scan).observe(el, {
        subtree: true,
        childList: true,
        characterData: true,
        attributes: true,
      });
    },
    { column, value: QUOTA_DATA.value },
  );
  const drawn = () => page.evaluate(() => window.__drawn?.size ?? 0);
  /** The rows in view (in the grid and in the window) whose cell has no value yet. */
  const pendingInView = () =>
    grid.evaluate((el, column) => {
      const box = el.getBoundingClientRect();
      const top = Math.max(box.top, 0);
      const bottom = Math.min(box.bottom, window.innerHeight);
      let pending = 0;
      for (const cell of el.querySelectorAll<HTMLElement>(
        `[role="gridcell"][data-column="${column}"]`,
      )) {
        const r = cell.getBoundingClientRect();
        if (r.bottom <= top || r.top >= bottom) continue;
        if (!window.__drawn?.has(cell.dataset.row ?? "")) pending++;
      }
      return pending;
    }, column);
  /**
   * Down by most of a screen (the rows in view drawn first): the grid scrolls in itself once
   * virtualised, the surface before. Two frames later, the rows that came into view are
   * rendered (the virtualiser draws them in a frame): measured before that, the rows of the
   * former screen, all drawn, would let the next step skip a screen of rows never seen.
   */
  const down = () =>
    grid.evaluate(async (el) => {
      el.scrollBy(0, el.clientHeight * 0.8);
      const surface = document.getElementById("k-surface");
      surface?.scrollBy(0, surface.clientHeight * 0.8);
      for (let frame = 0; frame < 2; frame++)
        await new Promise((resolve) => requestAnimationFrame(resolve));
    });

  // As fast as the shell draws: each screen down once its rows show their value.
  const started = Date.now();
  let steps = 0;
  for (; steps < 500 && (await drawn()) < QUOTA_DATA.count; steps++) {
    await expect
      .poll(pendingInView, { message: `rows in view drawn (step ${steps})`, timeout: 15_000 })
      .toBe(0);
    await down();
  }
  const seconds = (Date.now() - started) / 1000;

  // The measure first, whatever follows: the requests by page read and by path, the
  // refusals by status and path.
  const counted = (entries: Iterable<[string, number]>) =>
    [...entries]
      .sort((a, b) => b[1] - a[1])
      .map(([key, n]) => `${key} ${n}`)
      .join(", ");
  const refusals = new Map<string, number>();
  for (const refusal of refused) {
    const key = refusal.split("?")[0]!.replace(/[0-9a-f]{8}-[0-9a-f-]{27}/g, "{id}");
    refusals.set(key, (refusals.get(key) ?? 0) + 1);
  }
  console.log(
    `INFO quota ${browserName}: ${byPage.reduce((sum, n) => sum + n, 0)} requests under /v1/, ` +
      `${seconds.toFixed(1)} s of scrolling in ${steps} steps, ${await drawn()} rows drawn; ` +
      `by page read (0: before the first) ` +
      `${byPage.map((n, i) => `${i}:${n}`).join(" ")}; by path ${counted(byPath)}; ` +
      `refused ${refused.length} (${counted(refusals)})`,
  );

  // The count in the words of the language (« 1 000 », a narrow space in French).
  await expect(page.getByText(/^1\s000 éléments$/)).toBeVisible();
  expect(await drawn(), "rows drawn with their property").toBe(QUOTA_DATA.count);
  expect(pages, "pages of the list read").toHaveLength(QUOTA_DATA.count / 100);
  expect(await page.evaluate(() => [...(window.__unreadable ?? [])]), "unreadable").toEqual([]);
  expect(await page.evaluate(() => [...(window.__alerts ?? [])]), "alerts").toEqual([]);
  // The live regions of the shell are alerts too, empty until they speak.
  await expect(page.getByRole("alert").filter({ hasText: /\S/ })).toHaveCount(0);
  // Under the quota and the shedding limit all along, by window and by batch.
  expect(refused, "429 and 503 seen by the page").toEqual([]);
  expect(
    byPage.filter((n) => n > PER_PAGE),
    `pages read with more than ${PER_PAGE} requests under /v1/`,
  ).toEqual([]);
});
