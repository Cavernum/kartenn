// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { queryClient, type components, type Extension, type User } from "@kartenn/sdk";
import { resetContext } from "@kartenn/sdk/host";
import { announcePlugin, fakeFetch, mount, type Mounted } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";

// The view of the interfaces and the « Interfaces » tab in Vue (spec 021, task 18cc; the
// behaviour of the 18a kept): the search by MAC, address and VID, its criteria in the query
// of the page, its results in a `DataGrid`; the record of an interface and the change of its
// 802.1Q mode in the draft; the tab of an address, of a network and of a VLAN. Mounted in the
// sandbox of the SDK, in the light DOM of its element, with a simulated `fetch`.

const MACHINE = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const ENTITY = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";
const IFACE = "6a1d3e70-58c4-4f0b-9e2a-1c7d5b3f9e42";
const VLAN = "7e2b1c90-1d4a-4c3e-8b5f-0a9d8c7b6e53";
const VLAN_B = "7e2b1c90-1d4a-4c3e-8b5f-0a9d8c7b6e54";
const ADDRESS = "8f3c2da1-2e5b-4d4f-9c60-1b0e9d8c7f64";
const OUTSIDE = "8f3c2da1-2e5b-4d4f-9c60-1b0e9d8c7f65";
const PREFIX = "9a4d3eb2-3f6c-4e50-8d71-2c1f0e9d8a75";
const DRAFT = "1b5e4fc3-4071-4f61-9e82-3d2a1f0e9b86";
const MAC = "00:00:5e:00:53:01";

type Machine = components["schemas"]["Machine"];
type Interface = components["schemas"]["Interface"];

const machine = (): Machine => ({
  id: MACHINE,
  href: `/v1/machines/${MACHINE}`,
  name: "srv-a",
  kind: "physical",
  owner: { id: ENTITY },
});
const card = () => ({
  ...machine(),
  interfaces: [
    {
      id: IFACE,
      href: `/v1/interfaces/${IFACE}`,
      name: "eth0",
      mac: MAC,
      enabled: true,
      addresses: [{ id: ADDRESS, value: "192.0.2.10", primary: true }],
    },
  ],
});
const iface = (vlan: Interface["vlan"] = { mode: "access", access: { id: VLAN }, tagged: [] }) =>
  ({
    id: IFACE,
    href: `/v1/interfaces/${IFACE}`,
    machine: { id: MACHINE },
    name: "eth0",
    mac: MAC,
    enabled: true,
    vlan,
  }) as Interface;
const vlan = { id: VLAN, vid: 310, name: "hotes", group: { id: ENTITY }, status: "active" };
const page = <T>(items: T[], next?: string) => ({ items, links: next ? { next } : {} }) as never;

/** A user who holds every permission, or none of `missing`. */
const user = (missing: string[] = []): User => ({
  identity: "alice",
  name: "Alice",
  kind: "user",
  has: (permission) => !missing.includes(permission),
});

const problem = (status: number, slug: string) =>
  new Response(JSON.stringify({ type: `urn:kartenn:problem:${slug}`, title: "Refused", status }), {
    status,
    headers: { "Content-Type": "application/problem+json", "Request-Id": "req-42" },
  });

/** A draft of the server holding `operations`. */
const draftOf = (operations: unknown[]) =>
  new Response(
    JSON.stringify({
      id: DRAFT,
      href: `/v1/drafts/${DRAFT}`,
      author: DRAFT,
      title: "t",
      operation_count: operations.length,
      version: 1,
      created_at: "2026-10-24T00:00:00Z",
      updated_at: "2026-10-24T00:00:00Z",
      reason: {},
      operations,
    }),
    { status: 201, headers: { "Content-Type": "application/json", ETag: '"1"' } },
  );

let mounted: Mounted<HTMLElement> | null = null;
/** The links the view followed (the shell's router would have). */
let followed: string[] = [];
const follow = (event: MouseEvent) => {
  const anchor = event.target instanceof Element ? event.target.closest("a") : null;
  if (!anchor) return;
  event.preventDefault();
  followed.push(anchor.getAttribute("href") ?? "");
};

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
    integrity: { "styles.css": "sha384-test" },
  });
  await import("../src/index.ts");
  await defined;
  document.addEventListener("click", follow);
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  followed = [];
  queryClient.clear();
  resetContext();
  sessionStorage.clear();
  history.replaceState(null, "", "/");
});

/** The text of the element (light DOM), once it holds `expected`. */
async function text(element: HTMLElement, expected: string | RegExp): Promise<string> {
  let found = "";
  await vi.waitFor(() => {
    found = element.textContent ?? "";
    expect(found).toMatch(expected);
  });
  return found;
}

/** The rows of the grid named `name`, each its cells' texts. */
const rows = (element: HTMLElement, name: string) => {
  const grid = [...element.querySelectorAll("[role=grid]")].find(
    (g) => g.getAttribute("aria-label") === name,
  );
  return [...(grid?.querySelectorAll("[role=row]") ?? [])]
    .slice(1)
    .map((row) => [...row.querySelectorAll("[role=gridcell]")].map((c) => c.textContent?.trim()));
};

const buttonOf = (element: HTMLElement, name: string) =>
  [...element.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
    b.textContent?.includes(name),
  )!;

/** The input of the field named `label`. */
function field(element: HTMLElement, label: string): HTMLInputElement {
  const found = [...element.querySelectorAll("label")].find((l) =>
    l.textContent?.startsWith(label),
  );
  return element.querySelector<HTMLInputElement>(`#${CSS.escape(found!.htmlFor)}`)!;
}
/** The value of the field named `label`, typed as the user would. */
function type(element: HTMLElement, label: string, value: string): void {
  const input = field(element, label);
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

const ROW = ["eth0", "srv-a", MAC, "access 310", "enabled"];

describe("the search of the interfaces (task 18cc)", () => {
  /** The routes of a search; `asked`: `mac|address|vlan` of each list read. */
  const routes = (asked: string[]) => ({
    "GET /v1/interfaces": ({ query }: { query: URLSearchParams }) => {
      const sent = ["mac", "address", "vlan"].map((key) => query.get(key) ?? "");
      asked.push(sent.join("|"));
      const [mac, address] = sent;
      return page((!mac || mac === MAC) && (!address || address === ADDRESS) ? [iface()] : []);
    },
    "GET /v1/addresses": ({ query }: { query: URLSearchParams }) =>
      page(query.get("value") === "192.0.2.10" ? [{ id: ADDRESS }] : []),
    // A VID in two groups: two lists, their interfaces merged.
    "GET /v1/vlans": ({ query }: { query: URLSearchParams }) =>
      page(query.get("vid") === "310" ? [{ id: VLAN }, { id: VLAN_B }] : []),
    "GET /v1/machines/{id}": () => machine() as never,
    "GET /v1/vlans/{id}": () => vlan as never,
  });

  it("searches by MAC, address and VID, the criteria in the query of the page, found again at a step back", async () => {
    const asked: string[] = [];
    history.replaceState(null, "", `/x/machines/interfaces?mac=${encodeURIComponent(MAC)}`);
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch(routes(asked) as never),
    });
    const { element } = mounted;
    // The criteria of the page in their fields, searched at once.
    await vi.waitFor(() => expect(field(element, "MAC").value).toBe(MAC));
    await vi.waitFor(() => expect(rows(element, "Interfaces found")).toEqual([ROW]));
    expect(asked).toEqual([`${MAC}||`]);
    // Said by the live region there from the start, the visible count hidden from it.
    const status = element.querySelector("[role=status]")!;
    expect(status.textContent).toBe("Interfaces found: 1.");
    // No sheet of its own, no shadow root (A23).
    expect(element.shadowRoot).toBeNull();
    expect(element.querySelector("link, style")).toBeNull();

    type(element, "Address", "192.0.2.10");
    type(element, "VID", " 310 ");
    element.querySelector<HTMLFormElement>("form[role=search]")!.requestSubmit();
    await vi.waitFor(() =>
      expect(asked).toEqual(
        expect.arrayContaining([`${MAC}|${ADDRESS}|${VLAN}`, `${MAC}|${ADDRESS}|${VLAN_B}`]),
      ),
    );
    // By a link the shell follows: a new entry of the history, the criteria trimmed.
    expect(followed).toEqual([
      `/x/machines/interfaces?mac=${encodeURIComponent(MAC)}&address=192.0.2.10&vid=310`,
    ]);
    // The same interface in both lists: once.
    await vi.waitFor(() => expect(rows(element, "Interfaces found")).toEqual([ROW]));
    expect(status.textContent).toBe("Interfaces found: 1.");

    type(element, "Address", "192.0.2.99");
    element.querySelector<HTMLFormElement>("form[role=search]")!.requestSubmit();
    await text(element, "No address 192.0.2.99");
    expect(status.textContent).toBe("No address 192.0.2.99");

    // Back in the history: the criteria of the former entry, in the fields and searched.
    history.replaceState(null, "", "/x/machines/interfaces?address=192.0.2.10");
    window.dispatchEvent(new PopStateEvent("popstate"));
    await vi.waitFor(() => expect(asked).toContain(`|${ADDRESS}|`));
    expect(field(element, "MAC").value).toBe("");
    expect(field(element, "Address").value).toBe("192.0.2.10");
    await vi.waitFor(() => expect(rows(element, "Interfaces found")).toEqual([ROW]));
  });

  it("says a refused search with its codes, read again by « Retry »; nothing found, with what to do", async () => {
    let refused = true;
    history.replaceState(null, "", "/x/machines/interfaces?mac=zz");
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        "GET /v1/interfaces": () => (refused ? problem(400, "machines.mac.invalid") : page([])),
      }),
    });
    const { element } = mounted;
    await text(element, "The interfaces could not be read");
    await text(element, "req-42");
    expect(element.textContent).toContain("urn:kartenn:problem:machines.mac.invalid");
    refused = false;
    buttonOf(element, "Retry").click();
    await text(element, "No interface found");
    expect(element.textContent).toContain("Check the criteria");
  });

  it("stops reading the lists at a next page refused, read again by « Retry » only", async () => {
    const second: string[] = [];
    let refused = true;
    history.replaceState(null, "", "/x/machines/interfaces?vid=310");
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...routes([]),
        "GET /v1/interfaces": ({ query }: { query: URLSearchParams }) => {
          const cursor = query.get("cursor");
          if (!cursor) {
            const vlanOf = query.get("vlan")!;
            return page([{ ...iface(), id: vlanOf }], `/v1/interfaces?cursor=${vlanOf}`);
          }
          second.push(cursor);
          return refused && cursor === VLAN_B ? problem(403, "forbidden") : page([]);
        },
      } as never),
    });
    const { element } = mounted;
    await text(element, "The interfaces could not be read");
    await new Promise((done) => setTimeout(done, 50));
    expect(second.sort()).toEqual([VLAN, VLAN_B]);
    // The rows of both lists kept above the alert.
    expect(rows(element, "Interfaces found")).toHaveLength(2);
    refused = false;
    buttonOf(element, "Retry").click();
    await vi.waitFor(() =>
      expect(element.textContent).not.toContain("The interfaces could not be read"),
    );
    expect(second.sort()).toEqual([VLAN, VLAN_B, VLAN_B]);
  });

  it("says what it leaves out: objects beyond the pages resolved, pairs beyond the bound", async () => {
    const lists: string[] = [];
    history.replaceState(null, "", "/x/machines/interfaces?address=192.0.2.10");
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        // Five addresses a page, a next page always: three pages read, the rest said.
        "GET /v1/addresses": ({ query }: { query: URLSearchParams }) => {
          const at = Number(query.get("cursor") ?? 0);
          const ids = [0, 1, 2, 3, 4].map((i) => ({ id: `a${at * 5 + i}` }));
          return page(ids, `/v1/addresses?cursor=${at + 1}`);
        },
        "GET /v1/interfaces": ({ query }: { query: URLSearchParams }) => {
          lists.push(query.get("address")!);
          return page(query.get("address") === "a0" ? [iface()] : []);
        },
        "GET /v1/machines/{id}": () => machine() as never,
        "GET /v1/vlans/{id}": () => vlan as never,
      } as never),
    });
    const { element } = mounted;
    await text(element, "Incomplete result");
    expect(element.textContent).toContain(
      "More than 15 addresses have the value 192.0.2.10: only the first 15 are searched.",
    );
    expect(element.textContent).toContain(
      "15 pairs of address and VLAN: only the first 12 are searched",
    );
    await vi.waitFor(() =>
      expect(element.querySelector("[role=status]")?.textContent).toBe(
        "Interfaces found: 1, incomplete result.",
      ),
    );
    expect(new Set(lists).size).toBe(12);
  });

  it("opens the record of a row by Enter on a cell without link", async () => {
    history.replaceState(null, "", "/x/machines/interfaces");
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch(routes([]) as never),
    });
    const { element } = mounted;
    await vi.waitFor(() => expect(rows(element, "Interfaces found")).toEqual([ROW]));
    const cell = [...element.querySelectorAll<HTMLElement>("[role=gridcell]")].at(-1)!;
    cell.focus();
    cell.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await vi.waitFor(() => expect(followed).toEqual([`/x/machines/interfaces/${IFACE}`]));
  });
});

describe("the record of an interface (task 18cc)", () => {
  const reads = (vlanOf: Interface["vlan"] | undefined = undefined) => ({
    "GET /v1/interfaces/{id}": () => iface(vlanOf) as never,
    "GET /v1/machines/{id}": () => card() as never,
    "GET /v1/vlans": () => page([vlan]),
    "GET /v1/vlans/{id}": () => vlan as never,
  });

  it("draws its attributes and its addresses; changes its 802.1Q mode in the draft", async () => {
    const posted: unknown[] = [];
    history.replaceState(null, "", `/x/machines/interfaces/${IFACE}`);
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...reads(),
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => {
          posted.push(body);
          return draftOf((body as { operations: unknown[] }).operations);
        },
      }),
    });
    const { element } = mounted;
    await vi.waitFor(() => expect(element.querySelector("h1")?.textContent?.trim()).toBe("eth0"));
    await vi.waitFor(() =>
      expect([...element.querySelectorAll("dd")].map((d) => d.textContent?.trim())).toEqual([
        "srv-a",
        MAC,
        "access 310",
        "enabled",
      ]),
    );
    expect(element.querySelector("dd a")?.getAttribute("href")).toBe(
      `/x/machines/machines/${MACHINE}`,
    );
    await vi.waitFor(() =>
      expect(rows(element, "Attached addresses")).toEqual([["192.0.2.10", "yes"]]),
    );
    expect(element.querySelector("[role=gridcell] a")?.getAttribute("href")).toBe(
      `/addresses/${ADDRESS}`,
    );
    // Its mode and its VLAN, to change from; sent as they are.
    await text(element, "310 · hotes");
    element.querySelector<HTMLFormElement>("section form")!.requestSubmit();
    await text(element, "Operation added; the draft holds 1.");
    expect(posted).toMatchObject([
      {
        operations: [
          {
            op: "machines.update_interface",
            id: { id: IFACE },
            vlan: { mode: "access", access: { id: VLAN } },
          },
        ],
      },
    ]);
  });

  it("refuses a trunk sent without a mode chosen, on its field; and without the permission", async () => {
    const posted: unknown[] = [];
    history.replaceState(null, "", `/x/machines/interfaces/${IFACE}`);
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...reads({ mode: "trunk", tagged: [{ id: VLAN }] }),
        "POST /v1/drafts": ({ body }) => (posted.push(body), draftOf([])),
      }),
    });
    let { element } = mounted;
    await text(element, "trunk 310");
    const form = element.querySelector<HTMLFormElement>("section form")!;
    form.requestSubmit();
    await text(form, "Required field.");
    await vi.waitFor(() =>
      expect(document.activeElement?.getAttribute("aria-invalid")).toBe("true"),
    );
    expect(posted).toEqual([]);
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user(["machines.interface.edit"]) },
      fetch: fakeFetch({
        ...reads(),
        "POST /v1/drafts": ({ body }) => (posted.push(body), draftOf([])),
      }),
    });
    element = mounted.element;
    await text(element, "310 · hotes");
    const submit = element.querySelector<HTMLButtonElement>("section button[type=submit]")!;
    expect(submit.getAttribute("aria-disabled")).toBe("true");
    submit.closest("form")!.requestSubmit();
    await text(element, "Needs the permission machines.interface.edit");
    expect(posted).toEqual([]);
  });

  it("says an unreadable interface with its codes, the form gone at another date", async () => {
    history.replaceState(null, "", `/x/machines/interfaces/${IFACE}`);
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({ "GET /v1/interfaces/{id}": () => problem(404, "not-found") }),
    });
    await text(mounted.element, "The interface could not be read");
    await text(mounted.element, "req-42");
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-interfaces", {
      context: {
        locale: "en",
        user: user(),
        time: { validAt: "2026-01-01T00:00:00Z", asOf: null },
      },
      fetch: fakeFetch(reads()),
    });
    await vi.waitFor(() => expect(rows(mounted!.element, "Attached addresses")).toHaveLength(1));
    expect(mounted.element.querySelector("form")).toBeNull();
  });
});

describe("the « Interfaces » tab (task 18cc)", () => {
  const addresses = () =>
    ({
      items: [
        {
          address: { id: ADDRESS },
          value: "192.0.2.10",
          interface: { id: IFACE },
          interface_name: "eth0",
          primary: true,
          href: "",
        },
        {
          address: { id: OUTSIDE },
          value: "198.51.100.1",
          interface: { id: IFACE },
          interface_name: "eth0",
          primary: false,
          href: "",
        },
      ],
      links: {},
    }) as never;

  it("of a network: the interfaces of its machines that hold one of its addresses", async () => {
    mounted = mount("machines-tab", {
      object: { type: "prefix", id: PREFIX, href: `/v1/prefixes/${PREFIX}` },
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}": () => ({ id: PREFIX, value: "192.0.2.0/24" }) as never,
        "GET /v1/machines": ({ query }) => {
          expect(query.get("network")).toBe(PREFIX);
          return page([machine()]);
        },
        "GET /v1/machines/{id}/addresses": addresses,
      }),
    });
    const { element } = mounted;
    await vi.waitFor(() =>
      expect(rows(element, "Interfaces")).toEqual([["eth0", "srv-a", "192.0.2.10", "yes"]]),
    );
    const links = [...element.querySelectorAll("[role=gridcell] a")].map((a) =>
      a.getAttribute("href"),
    );
    expect(links).toEqual([
      `/x/machines/interfaces/${IFACE}`,
      `/x/machines/machines/${MACHINE}`,
      `/addresses/${ADDRESS}`,
    ]);
    // No heading of its own: the tab of the shell names its panel.
    expect(element.querySelector("h1, h2")).toBeNull();
  });

  it("of a network: the addresses read a few at a time, the next page after them, each once", async () => {
    const asked = new Map<string, number>();
    let reading = 0;
    let most = 0;
    let answered = 0;
    /** How many addresses were answered when the second page of machines was asked. */
    let answeredAtPage2: number | null = null;
    const many = (from: number, n: number) =>
      Array.from({ length: n }, (_, i) => ({
        ...machine(),
        id: `m${from + i}`,
        name: `srv-${from + i}`,
      }));
    mounted = mount("machines-tab", {
      object: { type: "prefix", id: PREFIX, href: `/v1/prefixes/${PREFIX}` },
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}": () => ({ id: PREFIX, value: "192.0.2.0/24" }) as never,
        "GET /v1/machines": ({ query }: { query: URLSearchParams }) => {
          if (!query.get("cursor")) return page(many(0, 6), "/v1/machines?cursor=p2");
          answeredAtPage2 = answered;
          return page(many(6, 1));
        },
        "GET /v1/machines/{id}/addresses": async ({ params }: { params: { id: string } }) => {
          asked.set(params.id, (asked.get(params.id) ?? 0) + 1);
          most = Math.max(most, ++reading);
          await new Promise((done) => setTimeout(done, 5));
          reading--;
          answered++;
          const i = Number(params.id.slice(1));
          return {
            items: [
              {
                address: { id: `addr-${i}` },
                value: `192.0.2.${i + 1}`,
                interface: { id: IFACE },
                interface_name: "eth0",
                primary: true,
                href: "",
              },
            ],
            links: {},
          } as never;
        },
      } as never),
    });
    const { element } = mounted;
    await vi.waitFor(() => expect(rows(element, "Interfaces")).toHaveLength(7));
    // Never more than four at once; the page 2 once the six of the first answered.
    expect(most).toBeLessThanOrEqual(4);
    expect(answeredAtPage2).toBe(6);
    // The observers of the first page kept with the second: each machine read once.
    expect([...asked.values()]).toEqual([1, 1, 1, 1, 1, 1, 1]);
    // Let go once the tab is gone: an invalidation reads nothing again.
    mounted.unmount();
    mounted = null;
    // The application is unmounted a microtask after the disconnection (`host.ts`).
    await Promise.resolve();
    const observed = queryClient
      .getQueryCache()
      .getAll()
      .filter((q) => q.getObserversCount() > 0);
    expect(observed.map((q) => q.queryKey)).toEqual([]);
    await queryClient.invalidateQueries();
    await new Promise((done) => setTimeout(done, 20));
    expect([...asked.values()]).toEqual([1, 1, 1, 1, 1, 1, 1]);
  });

  it("of an address and of a VLAN: the interfaces that hold or carry it", async () => {
    const asked: string[] = [];
    const routes = {
      "GET /v1/interfaces": ({ query }: { query: URLSearchParams }) => {
        asked.push(query.toString());
        return page([iface()]);
      },
      "GET /v1/machines/{id}": () => machine() as never,
      "GET /v1/vlans/{id}": () => vlan as never,
    };
    mounted = mount("machines-tab", {
      object: { type: "vlan", id: VLAN, href: `/v1/vlans/${VLAN}` },
      context: { locale: "en" },
      fetch: fakeFetch(routes as never),
    });
    await vi.waitFor(() => expect(rows(mounted!.element, "Interfaces")).toEqual([ROW]));
    mounted.unmount();
    mounted = mount("machines-tab", {
      object: { type: "address", id: ADDRESS, href: `/v1/addresses/${ADDRESS}` },
      context: { locale: "en" },
      fetch: fakeFetch(routes as never),
    });
    await vi.waitFor(() => expect(rows(mounted!.element, "Interfaces")).toEqual([ROW]));
    expect(asked).toEqual([`vlan=${VLAN}`, `address=${ADDRESS}`]);
  });

  it("stops reading at a next page refused, the rows kept; read again by « Retry » only", async () => {
    const second: string[] = [];
    let refused = true;
    mounted = mount("machines-tab", {
      object: { type: "vlan", id: VLAN, href: `/v1/vlans/${VLAN}` },
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/interfaces": ({ query }) => {
          if (!query.get("cursor")) return page([iface()], "/v1/interfaces?cursor=p2");
          second.push(query.get("cursor")!);
          return refused ? problem(403, "forbidden") : page([]);
        },
        "GET /v1/machines/{id}": () => machine() as never,
        "GET /v1/vlans/{id}": () => vlan as never,
      }),
    });
    const { element } = mounted;
    await text(element, "The interfaces could not be read");
    await new Promise((done) => setTimeout(done, 50));
    expect(second).toEqual(["p2"]);
    expect(rows(element, "Interfaces")).toEqual([ROW]);
    refused = false;
    buttonOf(element, "Retry").click();
    await vi.waitFor(() =>
      expect(element.textContent).not.toContain("The interfaces could not be read"),
    );
    expect(second).toEqual(["p2", "p2"]);
  });

  it("says a refused read with its codes, read again by « Retry »; none, with what to do", async () => {
    let refused = true;
    mounted = mount("machines-tab", {
      object: { type: "address", id: ADDRESS, href: `/v1/addresses/${ADDRESS}` },
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/interfaces": () => (refused ? problem(403, "forbidden") : page([])),
      }),
    });
    const { element } = mounted;
    await text(element, "The interfaces could not be read");
    await text(element, "req-42");
    refused = false;
    buttonOf(element, "Retry").click();
    await text(element, "No interface");
    expect(element.textContent).toContain(
      "An address is attached to an interface from its record.",
    );
  });
});
