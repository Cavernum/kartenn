import type { BrowserContext, Page } from "@playwright/test";
import { apiCall, expect, loginIn, ownedDomain, test } from "../fixtures/journeys.ts";

// The event stream of the new shell against the real kartenn-server (spec 021, task 5;
// REQ-021-23; 022 « Contrat consommé par les clients »): a change applied in one browser
// shows in another in under 2 s, without a reload; three tabs of one browser share one
// `GET /v1/events` (`navigator.locks` and `BroadcastChannel`), and when the tab that holds
// it closes, another opens it and the remaining tabs still follow. Each test writes in a
// domain of its own (RFC 5737, TEST-NET-2).

/** After `production` in the order of the slugs, as the other journeys that write. */
const unique = (browser: string) => `zz-events-${browser}-${Date.now().toString(36)}`;

/** How long a change takes to show elsewhere at most (task 5). */
const VISIBLE_MS = 2_000;

declare global {
  interface Window {
    /** The `EventSource` this page opened (`watchStreams`). */
    __kartennStreams?: Set<EventSource>;
  }
}

/**
 * Keeps, in each page of `context`, the `EventSource` it opens: the shell constructs its
 * own at the time it connects (sdk/src/instance.ts), past this subclass.
 */
async function watchStreams(context: BrowserContext): Promise<void> {
  await context.addInitScript(() => {
    const streams = new Set<EventSource>();
    window.__kartennStreams = streams;
    const Native = window.EventSource;
    window.EventSource = class extends Native {
      constructor(url: string | URL, init?: EventSourceInit) {
        super(url, init);
        streams.add(this);
      }
    };
  });
}

/** The streams of `page` connected to `/v1/events` (`OPEN`), 0 for a page closed. */
async function openStreams(page: Page): Promise<number> {
  if (page.isClosed()) return 0;
  return page.evaluate(
    () =>
      [...(window.__kartennStreams ?? [])].filter(
        (source) =>
          source.readyState === EventSource.OPEN && new URL(source.url).pathname === "/v1/events",
      ).length,
  );
}

const prefixes = (page: Page) => page.getByRole("grid", { name: "Préfixes" });
/** The value cell of the only network of the domain. */
const valueOf = (page: Page) =>
  prefixes(page).locator('[role="gridcell"][data-column="value"]').first();

/** Renumbers the prefix `id` to `value`, by an API call of `page` past the shell's cache. */
async function renumber(page: Page, id: string, value: string): Promise<void> {
  const applied = await apiCall(page, "POST", "/v1/changesets?approval=self", {
    motive: "end-to-end event stream",
    operations: [{ op: "update_prefix", id: { id }, prefix: value }],
  });
  expect(applied.status, JSON.stringify(applied.body)).toBe(201);
}

test("a network renumbered in one browser shows in another in under 2 s, without a reload", async ({
  page,
  login,
  browser,
  browserName,
}) => {
  await login("alice");
  const { slug, prefixes: ids } = await ownedDomain(page, unique(browserName));

  // Bob, in a browser of his own, reads the list of the domain; its stream open.
  const other = await browser.newContext({ ignoreHTTPSErrors: true, locale: "fr-FR" });
  try {
    await watchStreams(other);
    const reader = await other.newPage();
    await loginIn(reader, "bob");
    await reader.goto(`/prefixes?domain=${slug}`);
    await expect(valueOf(reader)).toContainText("198.51.100.0/24");
    await expect.poll(() => openStreams(reader)).toBe(1);
    // A mark of this document: a reload would lose it.
    await reader.evaluate(() => ((window as { __kept?: boolean }).__kept = true));

    // Alice renumbers it, past any cache of Bob's: only the stream can tell him.
    await renumber(page, ids[0]!, "198.51.100.0/25");
    await expect(valueOf(reader)).toContainText("198.51.100.0/25", { timeout: VISIBLE_MS });
    expect(await reader.evaluate(() => (window as { __kept?: boolean }).__kept)).toBe(true);
  } finally {
    await other.close();
  }
});

test("three tabs share one stream; the holder closed, another opens it and all follow", async ({
  page,
  login,
  browserName,
}) => {
  const context = page.context();
  await watchStreams(context);
  await login("alice");
  const { slug, prefixes: ids } = await ownedDomain(page, unique(browserName));
  const list = `/prefixes?domain=${slug}`;

  await page.goto(list);
  const tabs = [page, await context.newPage(), await context.newPage()];
  for (const tab of tabs.slice(1)) await tab.goto(list);
  for (const tab of tabs) await expect(valueOf(tab)).toContainText("198.51.100.0/24");

  /** The open streams of every tab, in the order of `tabs`. */
  const streams = () => Promise.all(tabs.map(openStreams));
  const total = async () => (await streams()).reduce((a, b) => a + b, 0);
  // One connection for the three: one tab holds it, the others are relayed.
  await expect.poll(total).toBe(1);

  await renumber(page, ids[0]!, "198.51.100.0/25");
  for (const tab of tabs) {
    await expect(valueOf(tab)).toContainText("198.51.100.0/25", { timeout: VISIBLE_MS });
  }
  // Still one, the change relayed to the two others: none of them opened its own.
  expect(await total()).toBe(1);

  // The holder closed: another tab takes the lock and opens the stream again.
  const holder = tabs[(await streams()).indexOf(1)]!;
  await holder.close();
  const left = tabs.filter((tab) => tab !== holder);
  const remaining = async () =>
    (await Promise.all(left.map(openStreams))).reduce((a, b) => a + b, 0);
  await expect.poll(remaining).toBe(1);

  const writer = left[0]!;
  await renumber(writer, ids[0]!, "198.51.100.0/26");
  for (const tab of left) {
    await expect(valueOf(tab)).toContainText("198.51.100.0/26", { timeout: VISIBLE_MS });
  }
});
