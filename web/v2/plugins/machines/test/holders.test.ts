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
// read alone (that of the tab of an address and of the search).

type Interface = components["schemas"]["Interface"];

const MACHINE = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const ENTITY = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";

/** The identifier of the `n`th address, of the `n`th interface. */
const addressOf = (n: number) => `8f3c2da1-2e5b-4d4f-9c60-${n.toString(16).padStart(12, "0")}`;
const ifaceOf = (n: number) => `6a1d3e70-58c4-4f0b-9e2a-${n.toString(16).padStart(12, "0")}`;

/** The interface `n`, holding the address `n` (and those of `also`). */
const iface = (n: number, also: number[] = []): Interface => ({
  id: ifaceOf(n),
  href: `/v1/interfaces/${ifaceOf(n)}`,
  machine: { id: MACHINE },
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
  const first = mount("machines-column", {
    properties: { row: { id: addressOf(0) } },
    context: { locale: "en" },
    fetch: fakeFetch({ "GET /v1/machines/{id}": () => machine(), ...handlers }),
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

const machine = () =>
  ({
    id: MACHINE,
    href: `/v1/machines/${MACHINE}`,
    name: "srv-a",
    kind: "physical",
    owner: { id: ENTITY },
  }) as never;

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

/** The page kept for the row `n`, under the key of its read alone. */
const keptOf = (n: number) =>
  queryClient.getQueryData<ListData<Interface>>(
    reads.list("/v1/interfaces", { address: [addressOf(n)] }).queryKey,
  );

describe("the machine column, its interfaces read by window", () => {
  it("100 rows: two requests, 64 then 36 addresses; each row its interfaces, none for the others", async () => {
    // The even rows are held by an interface; the others by none.
    const { requests, handlers } = interfaces((asked) => ({
      items: asked
        .map((a) => Number.parseInt(a.slice(-12), 16))
        .filter((n) => n % 2 === 0)
        .map((n) => iface(n)),
      links: { next: null },
    }));
    const mounted = column(100, handlers);
    await vi.waitFor(() =>
      expect(mounted.filter((cell) => (cell.textContent ?? "").includes("srv-a"))).toHaveLength(50),
    );
    await vi.waitFor(() =>
      expect(mounted.filter((cell) => (cell.textContent ?? "").includes("—"))).toHaveLength(50),
    );
    expect(requests.map((r) => r.length)).toEqual([HOLDERS_MAX, 100 - HOLDERS_MAX]);
    // Each row under the key of its read alone, with its own address only.
    expect(keptOf(2)?.pages[0]?.data.items).toEqual([
      { ...iface(2), addresses: iface(2).addresses },
    ]);
    expect(keptOf(3)?.pages[0]?.data.items).toEqual([]);
    // No row read again: an empty page is an answer, not a failure.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(requests).toHaveLength(2);
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
