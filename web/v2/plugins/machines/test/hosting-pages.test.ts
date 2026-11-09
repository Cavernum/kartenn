// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { queryClient, type Extension, type User } from "@kartenn/sdk";
import { resetContext } from "@kartenn/sdk/host";
import { announcePlugin, mount, type Mounted } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";

// A page the tree « Hébergement et clusters » asks for, against the reads of an
// invalidation (task 021-22k, on the model of the topography's, task 22d): the tree asks
// once per number of children read (`treePages.ts` of `@kartenn/ui`), so the ask must end
// with its page. A change read again (`invalidateQueries`) cancels a read under way: the
// page asked was lost, the list left as it was and never asked again. Mounted in the
// sandbox of the SDK; each answer held until `settle`, the next page asked by Enter on
// the row that reads it (the rows in view ask the same way, once).

const HOST = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";

const user: User = { identity: "alice", name: "Alice", kind: "user", has: () => true };

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
  });
  await import("../src/index.ts");
  await defined;
});

let mounted: Mounted<HTMLElement> | null = null;
afterEach(() => {
  mounted?.unmount();
  mounted = null;
  queryClient.clear();
  resetContext();
  history.replaceState(null, "", "/");
});

/**
 * The reads of the tree: no cluster; the machines without a host and the guests of `HOST`
 * 10 by page, a `next` always, each answer held until `settle`; a read aborted rejects.
 * Each machine names its list, its page and the read that served it: `r<page>-<i>@<read>`
 * at the first level, `g<page>-<i>@<read>` under `HOST`, which is physical (it hosts).
 */
function held() {
  const seen: string[] = [];
  const flying = new Map<string, number>();
  let most = 0;
  const waiting: (() => void)[] = [];
  const json = (body: unknown) =>
    new Response(JSON.stringify(body), { headers: { "Content-Type": "application/json" } });
  const fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input instanceof Request ? input.url : input), "http://x");
    const read = seen.push(`${url.pathname}${url.search}`);
    const list = url.searchParams.has("host") ? "g" : "r";
    // A page in flight: of the clusters, or of a list of machines.
    const key = `${url.pathname} ${list}${url.searchParams.get("cursor") ?? "0"}`;
    flying.set(key, (flying.get(key) ?? 0) + 1);
    most = Math.max(most, flying.get(key)!);
    try {
      await new Promise<void>((resolve, reject) => {
        waiting.push(resolve);
        const signal = init?.signal ?? (input instanceof Request ? input.signal : undefined);
        signal?.addEventListener("abort", () => reject(signal.reason));
      });
    } finally {
      flying.set(key, flying.get(key)! - 1);
    }
    if (url.pathname === "/v1/clusters") return json({ items: [], links: {} });
    const n = Number(url.searchParams.get("cursor") ?? 0);
    const items = Array.from({ length: 10 }, (_, i) => {
      const name = `${list}${n}-${i}@${read}`;
      const first = list === "r" && n === 0 && i === 0;
      return {
        id: first ? HOST : name,
        href: `/v1/machines/${name}`,
        name,
        kind: first ? "physical" : "virtual",
        owner: { id: HOST },
      };
    });
    const query = new URLSearchParams(url.search);
    query.set("cursor", `${n + 1}`);
    return json({ items, links: { next: `/v1/machines?${query}` } });
  }) as typeof globalThis.fetch;
  /** The reads answered until none waits, the cache's notices between them. */
  async function settle(): Promise<void> {
    for (let idle = 0; idle < 5;) {
      const now = waiting.splice(0);
      for (const go of now) go();
      idle = now.length ? 0 : idle + 1;
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }
  /** The reads of page `n` (0 the first) of a list: `r` the first level, `g` the guests. */
  const of = (list: "r" | "g", n: number) =>
    seen.filter((path) => {
      const url = new URL(path, "http://x");
      if (url.pathname !== "/v1/machines" || url.searchParams.has("host") !== (list === "g"))
        return false;
      return (url.searchParams.get("cursor") ?? "0") === `${n}`;
    });
  return { fetch, settle, of, most: () => most };
}

/** The labels of the rows of the tree. */
const labels = (element: HTMLElement) =>
  [...element.querySelectorAll<HTMLElement>("[role=treegrid] [role=row][data-node]")].map(
    (row) => row.querySelector(".k-tree-grid__label")?.textContent?.trim() ?? "",
  );
/** A key pressed on the node cell of the row `id`. */
function press(element: HTMLElement, id: string, key: string): void {
  const cell = element.querySelector<HTMLElement>(`[data-row="${id}"][data-column=""]`)!;
  cell.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
}

async function opened() {
  history.replaceState(null, "", "/x/machines/clusters");
  const net = held();
  mounted = mount("machines-clusters", {
    context: { locale: "en", user },
    fetch: net.fetch,
  });
  const { element } = mounted;
  await net.settle();
  const of = (list: "r" | "g") => labels(element).filter((label) => label.startsWith(list));
  /** The row that reads the next page of `parent` (`null`: the first level) activated. */
  const ask = (parent: string | null) => press(element, `more:${parent ?? ""}`, "Enter");
  return { net, element, of, ask };
}

describe("a page the hosting tree asks for, against the reads of an invalidation", () => {
  it("asks again, once that read is done, a page of the first level an invalidation's read cancelled", async () => {
    const { net, of, ask } = await opened();
    expect(of("r")).toHaveLength(10);
    ask(null);
    await Promise.resolve();
    // A change while the second page is under way: every list read again.
    void queryClient.invalidateQueries();
    await net.settle();
    expect(of("r")).toHaveLength(20);
    expect(net.most()).toBe(1);
  });

  it("reads a page asked twice while it is under way once", async () => {
    const { net, of, ask } = await opened();
    ask(null);
    ask(null);
    await net.settle();
    expect(net.of("r", 1)).toHaveLength(1);
    expect(of("r")).toHaveLength(20);
  });

  it("asks a page during an invalidation's read once it is done, without cancelling it", async () => {
    const { net, of, ask } = await opened();
    void queryClient.invalidateQueries();
    await Promise.resolve();
    ask(null);
    await net.settle();
    // The first page read again, then the second: the list is the one read again.
    expect(net.of("r", 0)).toHaveLength(2);
    expect(net.of("r", 1)).toHaveLength(1);
    expect(of("r")[0]).toMatch(/^r0-0@[3-9]/);
    expect(of("r")).toHaveLength(20);
  });

  it("asks again the page of a host's guests an invalidation's read cancelled", async () => {
    const { net, element, of, ask } = await opened();
    press(element, HOST, "ArrowRight");
    await net.settle();
    expect(of("g")).toHaveLength(10);
    ask(HOST);
    await Promise.resolve();
    void queryClient.invalidateQueries();
    await net.settle();
    expect(of("g")).toHaveLength(20);
    expect(net.most()).toBe(1);
  });
});
