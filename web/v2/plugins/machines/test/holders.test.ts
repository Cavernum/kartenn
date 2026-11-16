// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { queryClient, reads, type components, type Extension, type ListData } from "@kartenn/sdk";
import { resetContext } from "@kartenn/sdk/host";
import { announcePlugin, fakeFetch, mount, type Handlers } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";
import { HOLDERS_MAX } from "../src/holders.ts";

// The machine column of a list of addresses reads the interfaces of its rows by window
// (task 021-11p10-e): one request `GET /v1/interfaces?address=…` per window of 64 addresses
// at most, followed to its last page, each row given its interfaces under the key of its
// read alone (that of the tab of an address and of the search). Each machine by the summary
// of its interface (task 11p10-d4): nothing read, nor mounted to read it.

type Interface = components["schemas"]["Interface"];

const MACHINE = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
/** The waits of the 100 rows: happy-dom mounts them slowly on a loaded machine. */
const SLOW = { timeout: 4000 };

/** The identifier of the `n`th address, of the `n`th interface. */
const addressOf = (n: number) => `8f3c2da1-2e5b-4d4f-9c60-${n.toString(16).padStart(12, "0")}`;
const ifaceOf = (n: number) => `6a1d3e70-58c4-4f0b-9e2a-${n.toString(16).padStart(12, "0")}`;

/** The interface `n`, holding the address `n` (and those of `also`). */
const iface = (n: number, also: number[] = []): Interface => ({
  id: ifaceOf(n),
  href: `/v1/interfaces/${ifaceOf(n)}`,
  machine: { id: MACHINE, label: "srv-a", href: `/v1/machines/${MACHINE}` },
  name: `eth${n}`,
  mac: null,
  enabled: true,
  vlan: { mode: "none", access: null, tagged: [] },
  addresses: [n, ...also].map((a) => ({
    id: addressOf(a),
    value: `2001:db8::${a}`,
    primary: false,
  })),
});

let cells: HTMLElement[] = [];
/** The paths of the requests sent, in order. */
const paths: string[] = [];

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
  });
  await import("../src/index.ts");
  await defined;
});

afterEach(() => {
  for (const cell of cells) cell.remove();
  cells = [];
  queryClient.clear();
  resetContext();
});

/** `n` cells of the column, rows 0 to n − 1, mounted at once over `handlers`. */
function column(n: number, handlers: Handlers): HTMLElement[] {
  const fake = fakeFetch(handlers);
  paths.length = 0;
  const first = mount("machines-column", {
    properties: { row: { id: addressOf(0) } },
    context: { locale: "en" },
    fetch: (input, init) => {
      paths.push(
        new URL(String(input instanceof Request ? input.url : input), "http://x").pathname,
      );
      return fake(input, init);
    },
  });
  cells = [first.element];
  for (let row = 1; row < n; row++) {
    const cell = document.createElement("machines-column");
    Object.assign(cell, { row: { id: addressOf(row) } });
    document.body.append(cell);
    cells.push(cell);
  }
  return cells;
}

/** The rows read so far by the requests on `/v1/interfaces`, request by request. */
function interfaces(answer: (asked: string[], cursor: string | null) => unknown) {
  const requests: string[][] = [];
  const handlers: Handlers = {
    "GET /v1/interfaces": ({ query }) => {
      const asked = query.getAll("address");
      requests.push(asked);
      return answer(asked, query.get("cursor")) as never;
    },
  };
  return { requests, handlers };
}

/**
 * What was read or mounted to read besides the lists of interfaces: the paths of the other
 * requests, and those of the other queries of the cache (an observer, even of a disabled
 * query, holds one). None for a machine named by its summary.
 */
const others = () => ({
  requests: paths.filter((path) => path !== "/v1/interfaces"),
  queries: queryClient
    .getQueryCache()
    .getAll()
    .map((query) => String(query.queryKey[1]))
    .filter((path) => path !== "/v1/interfaces"),
});
const NOTHING = { requests: [], queries: [] };

/** The page kept for the row `n`, under the key of its read alone. */
const keptOf = (n: number) =>
  queryClient.getQueryData<ListData<Interface>>(
    reads.list("/v1/interfaces", { address: [addressOf(n)] }).queryKey,
  );

describe("the machine column, its interfaces read by window", () => {
  it("100 rows: two requests, 64 then 36 addresses; each row its interfaces, none for the others; no read of a machine", async () => {
    // The even rows are held by an interface; the others by none.
    const { requests, handlers } = interfaces((asked) => ({
      items: asked
        .map((a) => Number.parseInt(a.slice(-12), 16))
        .filter((n) => n % 2 === 0)
        .map((n) => iface(n)),
      links: { next: null },
    }));
    const mounted = column(100, handlers);
    await vi.waitFor(
      () =>
        expect(mounted.filter((cell) => (cell.textContent ?? "").includes("srv-a"))).toHaveLength(
          50,
        ),
      SLOW,
    );
    await vi.waitFor(
      () =>
        expect(mounted.filter((cell) => (cell.textContent ?? "").includes("—"))).toHaveLength(50),
      SLOW,
    );
    expect(requests.map((r) => r.length)).toEqual([HOLDERS_MAX, 100 - HOLDERS_MAX]);
    // Each machine by the summary of its interface, linked to its record: nothing else read,
    // nothing mounted to read it.
    expect(mounted[0]!.querySelector("a")?.textContent).toBe("srv-a");
    expect(others()).toEqual(NOTHING);
    // Each row under the key of its read alone, with its own address only.
    expect(keptOf(2)?.pages[0]?.data.items).toEqual([
      { ...iface(2), addresses: iface(2).addresses },
    ]);
    expect(keptOf(3)?.pages[0]?.data.items).toEqual([]);
    // No row read again: an empty page is an answer, not a failure.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(requests).toHaveLength(2);
  });

  it("a machine the caller may not read: its identifier in fixed pitch, unlinked, nothing read (11p10-d4)", async () => {
    const { handlers } = interfaces(() => ({
      items: [{ ...iface(0), machine: { id: MACHINE } }],
      links: { next: null },
    }));
    const [cell] = column(1, handlers);
    // Said unreadable to the assistive technologies, the fixed pitch out of their reach.
    await vi.waitFor(() => expect(cell!.textContent).toBe(`${MACHINE} (unreadable)`));
    expect(cell!.querySelector("a")).toBeNull();
    expect(cell!.querySelector(".k-mono")?.textContent).toBe(MACHINE);
    expect(cell!.querySelector(".k-visually-hidden")?.textContent).toBe(" (unreadable)");
    expect(others()).toEqual(NOTHING);
  });

  it("a machine cited by two interfaces, once: by its summary read, whatever their order", async () => {
    const { handlers } = interfaces(() => ({
      items: [
        { ...iface(0), machine: { id: MACHINE } },
        { ...iface(0), id: ifaceOf(9) },
      ],
      links: { next: null },
    }));
    const [cell] = column(1, handlers);
    await vi.waitFor(() => expect(cell!.textContent).toBe("srv-a"));
    expect(cell!.querySelector("a")?.textContent).toBe("srv-a");
  });

  it("an interface holding two requested addresses goes to each row, with its address", async () => {
    const { handlers } = interfaces(() => ({ items: [iface(0, [1])], links: { next: null } }));
    const mounted = column(2, handlers);
    await vi.waitFor(() =>
      expect(mounted.map((cell) => cell.textContent)).toEqual(["srv-a", "srv-a"]),
    );
    expect(keptOf(0)?.pages[0]?.data.items[0]?.addresses?.map((a) => a.id)).toEqual([addressOf(0)]);
    expect(keptOf(1)?.pages[0]?.data.items[0]?.addresses?.map((a) => a.id)).toEqual([addressOf(1)]);
  });

  it("follows the pages of a batch to the last one, the rows answered once all are read", async () => {
    const { requests, handlers } = interfaces((_, cursor) =>
      cursor === null
        ? { items: [iface(0)], links: { next: "/v1/interfaces?cursor=c2" } }
        : { items: [iface(1), iface(2)], links: { next: null } },
    );
    const mounted = column(3, handlers);
    await vi.waitFor(() =>
      expect(mounted.map((cell) => cell.textContent)).toEqual(["srv-a", "srv-a", "srv-a"]),
    );
    expect(requests).toHaveLength(2);
    expect(keptOf(1)?.pages).toHaveLength(1);
    expect(keptOf(1)?.pages[0]?.data.links.next).toBeNull();
  });

  it("a batch refused by the quota goes again as one batch after its Retry-After", async () => {
    let refusals = 1;
    const { requests, handlers } = interfaces((asked) =>
      refusals-- > 0
        ? new Response(
            JSON.stringify({
              type: "urn:kartenn:problem:rate-limited",
              title: "Too many",
              status: 429,
            }),
            {
              status: 429,
              headers: { "Content-Type": "application/problem+json", "Retry-After": "1" },
            },
          )
        : {
            items: asked.map((a) => iface(Number.parseInt(a.slice(-12), 16))),
            links: { next: null },
          },
    );
    const mounted = column(40, handlers);
    await vi.waitFor(
      () => expect(mounted.filter((cell) => cell.textContent === "srv-a")).toHaveLength(40),
      { timeout: 4000 },
    );
    expect(requests.map((r) => r.length)).toEqual([40, 40]);
  });

  it("a row removed before its window closes is not asked; the others are", async () => {
    const { requests, handlers } = interfaces(() => ({ items: [], links: { next: null } }));
    const mounted = column(5, handlers);
    mounted[4]!.remove();
    await vi.waitFor(() => expect(requests).toHaveLength(1));
    expect(requests[0]).toEqual([0, 1, 2, 3].map(addressOf));
  });
});
