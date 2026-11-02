// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import {
  context,
  queryClient,
  reads,
  type components,
  type Extension,
  type User,
} from "@kartenn/sdk";
import { resetContext, setContext } from "@kartenn/sdk/host";
import { announcePlugin, fakeFetch, mount, type Mounted } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";
import { within } from "../src/reads.ts";

// The machines plugin in the sandbox of the SDK (spec 021, task 18a): announced as the shell
// would, its module loaded, each element mounted with a context and a simulated `fetch`. Its
// reads go through the shared cache; its forms add operations of the module to the draft.

const MACHINE = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const ENTITY = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";
const IFACE = "6a1d3e70-58c4-4f0b-9e2a-1c7d5b3f9e42";
const VLAN = "7e2b1c90-1d4a-4c3e-8b5f-0a9d8c7b6e53";
const ADDRESS = "8f3c2da1-2e5b-4d4f-9c60-1b0e9d8c7f64";
const PREFIX = "9a4d3eb2-3f6c-4e50-8d71-2c1f0e9d8a75";
const DRAFT = "1b5e4fc3-4071-4f61-9e82-3d2a1f0e9b86";

type Machine = components["schemas"]["Machine"];
type Interface = components["schemas"]["Interface"];

const machine = (name = "srv-a"): Machine => ({
  id: MACHINE,
  href: `/v1/machines/${MACHINE}`,
  name,
  kind: "physical",
  owner: { id: ENTITY },
});

const iface = (): Interface => ({
  id: IFACE,
  href: `/v1/interfaces/${IFACE}`,
  machine: { id: MACHINE },
  name: "eth0",
  mac: "00:00:5e:00:53:01",
  enabled: true,
  vlan: { mode: "access", access: { id: VLAN }, tagged: [] },
});

const card = () => ({
  ...machine(),
  interfaces: [
    {
      id: IFACE,
      href: `/v1/interfaces/${IFACE}`,
      name: "eth0",
      mac: "00:00:5e:00:53:01",
      enabled: true,
      addresses: [{ id: ADDRESS, value: "192.0.2.10", primary: true }],
    },
  ],
});

const entity = { id: ENTITY, slug: "exemple", href: `/v1/entities/${ENTITY}` };
const vlan = { id: VLAN, vid: 310, name: "hotes", group: { id: ENTITY }, status: "active" };
const page = <T>(items: T[]) => ({ items, links: {} }) as never;

/** A user who holds every permission, or none of `missing`. */
const user = (missing: string[] = []): User => ({
  identity: "alice",
  name: "Alice",
  kind: "user",
  has: (permission) => !missing.includes(permission),
});

/** A problem of the API, with its `Request-Id`. */
const problem = (status: number, slug: string) =>
  new Response(JSON.stringify({ type: `urn:kartenn:problem:${slug}`, title: "Refused", status }), {
    status,
    headers: { "Content-Type": "application/problem+json", "Request-Id": "req-42" },
  });

/** A draft of the server, as `POST /v1/drafts` answers it. */
const created = () =>
  new Response(
    JSON.stringify({
      id: DRAFT,
      href: `/v1/drafts/${DRAFT}`,
      author: DRAFT,
      title: "t",
      operation_count: 1,
      version: 1,
      created_at: "2026-10-24T00:00:00Z",
      updated_at: "2026-10-24T00:00:00Z",
      reason: {},
      operations: [],
    }),
    { status: 201, headers: { "Content-Type": "application/json", ETag: '"1"' } },
  );

let mounted: Mounted<HTMLElement> | null = null;

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
    integrity: { "styles.css": "sha384-test" },
  });
  const sheet = document.createElement("link");
  sheet.href = "/assets/ui-test.css";
  sheet.integrity = "sha384-ui";
  sheet.dataset["kartennSheet"] = "@kartenn/ui/style.css";
  document.head.append(sheet);
  await import("../src/index.ts");
  await defined;
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  queryClient.clear();
  resetContext();
  sessionStorage.clear();
  history.replaceState(null, "", "/");
});

/** The text of the shadow root of `element`, once it holds `expected`. */
async function shadowText(element: HTMLElement, expected: string | RegExp): Promise<string> {
  let text = "";
  await vi.waitFor(() => {
    text = element.shadowRoot?.textContent ?? "";
    expect(text).toMatch(expected);
  });
  return text;
}

/** The rows of the first table of `element`, each its cells' texts. */
const rows = (element: HTMLElement) =>
  [...element.shadowRoot!.querySelectorAll("tbody tr")].map((tr) =>
    [...tr.children].map((cell) => cell.textContent?.trim()),
  );

describe("the machines plugin", () => {
  it("defines every element its manifest declares, under the space hosts", () => {
    for (const extension of manifest.extensions) {
      expect(customElements.get(extension.element), extension.element).toBeDefined();
    }
    expect(manifest.space).toBe("hosts");
    // The server rejects a plugin whose extensions share an id, whatever their points.
    const ids = manifest.extensions.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(manifest.extensions.find((e) => e.point === "tab")?.object_types).toEqual([
      "address",
      "prefix",
      "vlan",
    ]);
  });

  it("has the same keys in both catalogs (REQ-007-29)", () => {
    const keys = (catalog: object, prefix = ""): string[] =>
      Object.entries(catalog).flatMap(([key, value]) =>
        typeof value === "object" ? keys(value as object, `${prefix}${key}.`) : [prefix + key],
      );
    expect(keys(fr).sort()).toEqual(keys(en).sort());
  });

  it("lists the machines through the shared cache, with their owner, again once invalidated", async () => {
    let name = "srv-a";
    let answer: () => void = () => {};
    const answered = new Promise<void>((resolve) => (answer = resolve));
    history.replaceState(null, "", "/x/machines/machines");
    mounted = mount("machines-machines", {
      context: { locale: "fr", user: user() },
      fetch: fakeFetch({
        "GET /v1/machines": async () => {
          await answered;
          return page([machine(name)]);
        },
        "GET /v1/entities": () => page([entity]),
        "GET /v1/entities/{id}": () => entity as never,
      }),
    });
    const { element } = mounted;
    await shadowText(element, "Chargement…");
    expect(element.shadowRoot!.querySelector(".k-skeleton")).not.toBeNull();
    answer();
    await vi.waitFor(() =>
      // No site, no host: a dash, said « aucun ».
      expect(rows(element)).toEqual([["srv-a", "physique", "exemple", "—aucun", "—aucun"]]),
    );
    const link = element.shadowRoot!.querySelector<HTMLAnchorElement>("tbody a")!;
    expect(new URL(link.href).pathname).toBe(`/x/machines/machines/${MACHINE}`);
    expect(element.shadowRoot!.querySelector("h1")?.textContent).toBe("Machines");

    // An event of the stream invalidates the list: the cache reads it again, the element
    // draws it in the same page.
    const heading = element.shadowRoot!.querySelector("h1");
    name = "srv-b";
    await queryClient.invalidateQueries({ queryKey: ["v1", "/v1/machines"], exact: false });
    await vi.waitFor(() => expect(rows(element)[0]?.[0]).toBe("srv-b"));
    expect(element.shadowRoot!.querySelector("h1")).toBe(heading);
  });

  it("says a failed list with its code and Request-Id, an empty one with what to do", async () => {
    mounted = mount("machines-clusters", {
      context: { locale: "en" },
      fetch: fakeFetch({ "GET /v1/clusters": () => problem(403, "forbidden") }),
    });
    await shadowText(mounted.element, "The clusters could not be read");
    const codes = [...mounted.element.shadowRoot!.querySelectorAll("[role=alert] code")];
    expect(codes.map((code) => code.textContent)).toEqual(["forbidden", "req-42"]);
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-clusters", {
      fetch: fakeFetch({ "GET /v1/clusters": () => page([]) }),
    });
    await shadowText(mounted.element, "No cluster");
    expect(mounted.element.shadowRoot!.querySelector(".k-empty-state")).not.toBeNull();
  });

  it("creates a machine in the draft, refusing an empty name before sending anything", async () => {
    const posted: unknown[] = [];
    history.replaceState(null, "", "/x/machines/machines");
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        "GET /v1/machines": () => page([]),
        "GET /v1/entities": () => page([entity]),
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => {
          posted.push(body);
          return created();
        },
      }),
    });
    const root = mounted.element.shadowRoot!;
    await shadowText(mounted.element, "No machine");
    await vi.waitFor(() =>
      expect(root.querySelector<HTMLSelectElement>("select[name=owner]")?.value).toBe(ENTITY),
    );
    const form = root.querySelector("form")!;
    const name = root.querySelector<HTMLInputElement>("input[name=name]")!;
    form.requestSubmit();
    expect(name.getAttribute("aria-invalid")).toBe("true");
    expect(root.getElementById(name.getAttribute("aria-describedby")!)?.textContent).toBe(
      "Required field.",
    );
    expect(posted).toEqual([]);
    name.value = "srv-c";
    root.querySelector<HTMLSelectElement>("select[name=kind]")!.value = "virtual";
    form.requestSubmit();
    await shadowText(mounted.element, "Operation added; the draft holds 1.");
    expect(posted).toMatchObject([
      {
        operations: [
          { op: "machines.create_machine", name: "srv-c", kind: "virtual", owner: { id: ENTITY } },
        ],
      },
    ]);
    expect(name.hasAttribute("aria-invalid")).toBe(false);
  });

  it("keeps the form visible but refused without the permission, hidden at another date", async () => {
    history.replaceState(null, "", "/x/machines/machines");
    mounted = mount("machines-machines", {
      context: { locale: "en", user: user(["machines.edit"]) },
      fetch: fakeFetch({
        "GET /v1/machines": () => page([]),
        "GET /v1/entities": () => page([entity]),
      }),
    });
    const root = mounted.element.shadowRoot!;
    await shadowText(mounted.element, "Needs the permission machines.edit");
    const submit = root.querySelector<HTMLButtonElement>("button[type=submit]")!;
    expect(submit.getAttribute("aria-disabled")).toBe("true");
    expect(root.getElementById(submit.getAttribute("aria-describedby")!)?.textContent).toBe(
      "Needs the permission machines.edit",
    );
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-machines", {
      context: { user: user(), time: { validAt: "2026-01-01T00:00:00Z", asOf: null } },
      fetch: fakeFetch({
        "GET /v1/machines": () => page([]),
        "GET /v1/entities": () => page([entity]),
      }),
    });
    await shadowText(mounted.element, "No machine");
    expect(mounted.element.shadowRoot!.querySelector("section")?.hidden).toBe(true);
  });

  it("draws the record of a machine: its interfaces, their VLAN and addresses", async () => {
    history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
    const posted: unknown[] = [];
    mounted = mount("machines-machines", {
      context: { locale: "fr", user: user() },
      fetch: fakeFetch({
        "GET /v1/machines/{id}": () => card() as never,
        "GET /v1/machines/{id}/hosted": () => page([]),
        "GET /v1/interfaces": ({ query }) => {
          expect(query.get("machine")).toBe(MACHINE);
          return page([iface()]);
        },
        "GET /v1/entities/{id}": () => entity as never,
        "GET /v1/vlans/{id}": () => vlan as never,
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => {
          posted.push(body);
          return created();
        },
      }),
    });
    const { element } = mounted;
    await vi.waitFor(() =>
      expect(element.shadowRoot!.querySelector("h1")?.textContent).toBe("srv-a"),
    );
    await vi.waitFor(() =>
      expect(rows(element)[0]).toEqual([
        "eth0",
        "00:00:5e:00:53:01",
        "accès 310",
        "192.0.2.10",
        "active",
      ]),
    );
    expect(element.shadowRoot!.textContent).toContain("Aucune machine hébergée");
    const root = element.shadowRoot!;
    const form = [...root.querySelectorAll("form")].at(-1)!;
    form.querySelector<HTMLInputElement>("input[name=name]")!.value = "eth1";
    form.querySelector<HTMLInputElement>("input[name=mac]")!.value = "00:00:5e:00:53:02";
    form.requestSubmit();
    await shadowText(element, "Opération ajoutée ; le brouillon en compte 1.");
    expect(posted).toMatchObject([
      {
        operations: [
          {
            op: "machines.create_interface",
            machine: { id: MACHINE },
            name: "eth1",
            mac: "00:00:5e:00:53:02",
          },
        ],
      },
    ]);
  });

  it("searches the interfaces by address value, the criteria in the address of the page", async () => {
    history.replaceState(null, "", "/x/machines/interfaces");
    const asked: string[] = [];
    mounted = mount("machines-interfaces", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        "GET /v1/interfaces": ({ query }) => {
          asked.push(query.toString());
          return page(query.get("address") === ADDRESS || !query.has("address") ? [iface()] : []);
        },
        "GET /v1/addresses": ({ query }) =>
          page(query.get("value") === "192.0.2.10" ? [{ id: ADDRESS }] : []),
        "GET /v1/machines/{id}": () => machine() as never,
        "GET /v1/vlans/{id}": () => vlan as never,
      }),
    });
    const { element } = mounted;
    // What the shell does of a link of the plugin: followed by its router, a new entry.
    const followed: string[] = [];
    const router = (event: Event) => {
      const anchor = event.composedPath().find((n) => n instanceof HTMLAnchorElement);
      if (!(anchor instanceof HTMLAnchorElement)) return;
      event.preventDefault();
      const url = new URL(anchor.href);
      followed.push(url.pathname + url.search);
      history.pushState({ shell: true }, "", url.pathname + url.search);
    };
    document.addEventListener("click", router);
    try {
      await vi.waitFor(() =>
        expect(rows(element)[0]?.slice(0, 3)).toEqual(["eth0", "srv-a", "00:00:5e:00:53:01"]),
      );
      const root = element.shadowRoot!;
      const field = root.querySelector<HTMLInputElement>("input[name=address]")!;
      field.value = "192.0.2.10";
      root.querySelector("form")!.requestSubmit();
      await vi.waitFor(() => expect(asked).toContain(`address=${ADDRESS}`));
      expect(followed).toEqual(["/x/machines/interfaces?address=192.0.2.10"]);
      await shadowText(element, "Interfaces found: 1.");
      // Said by the live region there from the start, the visible count hidden from it.
      expect(root.querySelector("[role=status].k-visually-hidden")?.textContent).toBe(
        "Interfaces found: 1.",
      );
      field.value = "192.0.2.99";
      root.querySelector("form")!.requestSubmit();
      await shadowText(element, "No address 192.0.2.99");
      // Back: the criteria of the former entry, in the field and searched again.
      history.back();
      await vi.waitFor(() => expect(field.value).toBe("192.0.2.10"));
      await shadowText(element, "Interfaces found: 1.");
    } finally {
      document.removeEventListener("click", router);
    }
  });

  it("tab of a network: the interfaces of its machines that hold one of its addresses", async () => {
    mounted = mount("machines-tab", {
      object: { type: "prefix", id: PREFIX, href: `/v1/prefixes/${PREFIX}` },
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}": () => ({ id: PREFIX, value: "192.0.2.0/24" }) as never,
        "GET /v1/machines": ({ query }) => {
          expect(query.get("network")).toBe(PREFIX);
          return page([machine()]);
        },
        "GET /v1/machines/{id}/addresses": () =>
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
                address: { id: VLAN },
                value: "198.51.100.1",
                interface: { id: IFACE },
                interface_name: "eth0",
                primary: false,
                href: "",
              },
            ],
            links: {},
          }) as never,
      }),
    });
    await vi.waitFor(() =>
      expect(rows(mounted!.element)).toEqual([["eth0", "srv-a", "192.0.2.10", "yes"]]),
    );
  });

  it("tab of an address and of a VLAN: the interfaces that hold or carry it", async () => {
    const asked: string[] = [];
    mounted = mount("machines-tab", {
      object: { type: "vlan", id: VLAN, href: `/v1/vlans/${VLAN}` },
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/interfaces": ({ query }) => {
          asked.push(query.toString());
          return page([iface()]);
        },
        "GET /v1/machines/{id}": () => machine() as never,
        "GET /v1/vlans/{id}": () => vlan as never,
      }),
    });
    await vi.waitFor(() =>
      expect(rows(mounted!.element)[0]).toEqual([
        "eth0",
        "srv-a",
        "00:00:5e:00:53:01",
        "access 310",
        "enabled",
      ]),
    );
    expect(asked).toEqual([`vlan=${VLAN}`]);
  });

  describe("the column and the action in Vue, in their light DOM (task 18ca)", () => {
    const address = { type: "address", id: ADDRESS, href: `/v1/addresses/${ADDRESS}` };

    /** The text of the element itself (no shadow root), once it holds `expected`. */
    async function lightText(element: HTMLElement, expected: string | RegExp): Promise<string> {
      let text = "";
      await vi.waitFor(() => {
        text = element.textContent ?? "";
        expect(text).toMatch(expected);
      });
      return text;
    }

    /** Types `text` in the machine field, then takes the suggestion `name`. */
    async function chooseMachine(element: HTMLElement, text: string, name: string): Promise<void> {
      const input = await vi.waitFor(() => {
        const found = element.querySelector<HTMLInputElement>("input[role=combobox]");
        expect(found).not.toBeNull();
        return found!;
      });
      input.value = text;
      input.dispatchEvent(new Event("input", { bubbles: true }));
      const option = await vi.waitFor(() => {
        const found = [...document.querySelectorAll<HTMLElement>("[role=option]")].find((o) =>
          o.textContent?.includes(name),
        );
        expect(found).toBeDefined();
        return found!;
      });
      option.click();
    }

    const buttonOf = (element: HTMLElement, name: string) =>
      [...element.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
        b.textContent?.includes(name),
      )!;

    it("mounts an application in the light DOM of the element, unmounted once removed", async () => {
      mounted = mount("machines-column", {
        properties: { row: { id: ADDRESS } },
        context: { locale: "en" },
        fetch: fakeFetch({
          "GET /v1/interfaces": () => page([]),
        }),
      });
      const element = mounted.element;
      await lightText(element, "none");
      expect(element.shadowRoot).toBeNull();
      // No sheet of its own: the document's @kartenn/ui applies (A23).
      expect(element.querySelector("link, style")).toBeNull();
      // Moved in the tree (a row reordered): the same application, its nodes kept.
      const drawn = element.firstChild;
      const elsewhere = document.body.appendChild(document.createElement("div"));
      elsewhere.append(element);
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(element.firstChild).toBe(drawn);
      mounted.unmount();
      elsewhere.remove();
      mounted = null;
      await vi.waitFor(() => expect(element.childNodes).toHaveLength(0));
    });

    it("column: the machines that hold the row's address, linked at the dates of the shell", async () => {
      const asked: string[] = [];
      mounted = mount("machines-column", {
        properties: { row: { id: ADDRESS, value: "192.0.2.10" } },
        context: { locale: "en", time: { validAt: "2026-10-01T00:00:00Z", asOf: null } },
        fetch: fakeFetch({
          "GET /v1/interfaces": ({ query }) => {
            asked.push(query.get("address") ?? "");
            return page([iface(), { ...iface(), id: PREFIX, name: "eth1" }]);
          },
          "GET /v1/machines/{id}": () => machine() as never,
        }),
      });
      await lightText(mounted.element, /^srv-a$/);
      const links = [...mounted.element.querySelectorAll("a")];
      expect(links).toHaveLength(1);
      expect(links[0]!.getAttribute("href")).toBe(
        `/x/machines/machines/${MACHINE}?valid_at=2026-10-01T00%3A00%3A00Z`,
      );
      expect(asked).toEqual([ADDRESS]);
    });

    it("follows the context and its reads while connected, moved or not; lets go once removed", async () => {
      // The subscriptions to the context held at a time.
      let held = 0;
      const subscribe = context.subscribe.bind(context);
      const spy = vi.spyOn(context, "subscribe").mockImplementation((listener) => {
        held++;
        const stop = subscribe(listener);
        let stopped = false;
        return () => {
          if (!stopped) held--;
          stopped = true;
          stop();
        };
      });
      try {
        mounted = mount("machines-column", {
          properties: { row: { id: ADDRESS } },
          context: { locale: "en" },
          fetch: fakeFetch({
            "GET /v1/interfaces": () => page([iface()]),
            "GET /v1/machines/{id}": () => machine() as never,
          }),
        });
        const element = mounted.element;
        await lightText(element, /^srv-a$/);
        const query = () =>
          queryClient
            .getQueryCache()
            .find({ queryKey: reads.list("/v1/interfaces", { address: ADDRESS }).queryKey });
        expect(held).toBe(1);
        expect(query()?.getObserversCount()).toBe(1);
        // Moved: let go at its disconnection, followed again at its connection.
        const elsewhere = document.body.appendChild(document.createElement("div"));
        elsewhere.append(element);
        await new Promise((resolve) => setTimeout(resolve, 0));
        expect(held).toBe(1);
        expect(query()?.getObserversCount()).toBe(1);
        mounted.unmount();
        mounted = null;
        elsewhere.remove();
        // At once for the context, at the unmounting for the reads.
        expect(held).toBe(0);
        await vi.waitFor(() => expect(query()?.getObserversCount() ?? 0).toBe(0));
      } finally {
        spy.mockRestore();
      }
    });

    it("column: read again once invalidated, and at each other date", async () => {
      const asked: string[] = [];
      mounted = mount("machines-column", {
        properties: { row: { id: ADDRESS } },
        context: { locale: "en" },
        fetch: fakeFetch({
          "GET /v1/interfaces": ({ query }) => {
            asked.push(query.get("valid_at") ?? "now");
            return page([iface()]);
          },
          "GET /v1/machines/{id}": () => machine() as never,
        }),
      });
      await lightText(mounted.element, /^srv-a$/);
      expect(asked).toEqual(["now"]);
      await queryClient.invalidateQueries();
      await vi.waitFor(() => expect(asked).toEqual(["now", "now"]));
      for (const validAt of ["2026-09-01T00:00:00Z", "2026-10-01T00:00:00Z"]) {
        setContext({ time: { validAt, asOf: null } });
        await vi.waitFor(() => expect(asked.at(-1)).toBe(validAt));
        await lightText(mounted.element, /^srv-a$/);
        expect(mounted.element.querySelector("a")!.getAttribute("href")).toContain(
          encodeURIComponent(validAt),
        );
      }
    });

    it("column: unreadable, said in a word with the code of the problem", async () => {
      mounted = mount("machines-column", {
        properties: { row: { id: ADDRESS } },
        context: { locale: "en" },
        fetch: fakeFetch({ "GET /v1/interfaces": () => problem(403, "forbidden") as never }),
      });
      await lightText(mounted.element, "unreadable");
      expect(mounted.element.querySelector(".k-code-badge")?.textContent).toContain("forbidden");
    });

    it("action: attaches the address to an interface of a machine searched by name, in the draft", async () => {
      const posted: unknown[] = [];
      const searched: string[] = [];
      mounted = mount("machines-attach", {
        object: address,
        context: { locale: "en", user: user() },
        fetch: fakeFetch({
          "GET /v1/machines": ({ query }) => {
            searched.push(query.get("q") ?? "");
            return page([machine()]);
          },
          "GET /v1/interfaces": () => page([iface()]),
          "GET /v1/drafts": () => page([]),
          "POST /v1/drafts": ({ body }) => {
            posted.push(body);
            return created();
          },
        }),
      });
      const element = mounted.element;
      await chooseMachine(element, "srv", "srv-a");
      // Its interfaces listed, the first one taken.
      await lightText(element, "eth0");
      element.querySelector<HTMLInputElement>("input[type=checkbox]")!.click();
      element.querySelector("form")!.requestSubmit();
      await lightText(element, "Operation added; the draft holds 1.");
      expect(searched).toContain("srv");
      expect(posted).toMatchObject([
        {
          operations: [
            {
              op: "machines.attach_address",
              interface: { id: IFACE },
              address: { id: ADDRESS },
              primary: true,
            },
          ],
        },
      ]);
      const closed = vi.fn();
      element.addEventListener("kartenn-close", closed);
      buttonOf(element, "Close").click();
      expect(closed).toHaveBeenCalledOnce();
    });

    it("action: refuses empty fields on themselves, nothing sent", async () => {
      const posted: unknown[] = [];
      mounted = mount("machines-attach", {
        object: address,
        context: { locale: "en", user: user() },
        fetch: fakeFetch({
          "GET /v1/machines": () => page([machine()]),
          "POST /v1/drafts": ({ body }) => {
            posted.push(body);
            return created();
          },
        }),
      });
      const element = mounted.element;
      await lightText(element, "Choose the machine first");
      element.querySelector("form")!.requestSubmit();
      await lightText(element, "Required field.");
      const input = element.querySelector<HTMLInputElement>("input[role=combobox]")!;
      expect(input.getAttribute("aria-invalid")).toBe("true");
      // A text typed, no suggestion taken: said so.
      input.value = "srv";
      input.dispatchEvent(new Event("input", { bubbles: true }));
      element.querySelector("form")!.requestSubmit();
      await lightText(element, "Choose a machine among the suggestions.");
      expect(posted).toEqual([]);
    });

    it("action: a refusal of the draft is an alert with its rules and codes", async () => {
      mounted = mount("machines-attach", {
        object: address,
        context: { locale: "en", user: user() },
        fetch: fakeFetch({
          "GET /v1/machines": () => page([machine()]),
          "GET /v1/interfaces": () => page([iface()]),
          "GET /v1/drafts": () => page([]),
          "POST /v1/drafts": () =>
            new Response(
              JSON.stringify({
                type: "urn:kartenn:problem:validation",
                title: "Invalid",
                status: 422,
                violations: [
                  { pointer: "/interface", rule: "machines.address.taken", detail: "Held." },
                ],
              }),
              {
                status: 422,
                headers: { "Content-Type": "application/problem+json", "Request-Id": "req-42" },
              },
            ),
        }),
      });
      const element = mounted.element;
      await chooseMachine(element, "srv", "srv-a");
      await lightText(element, "eth0");
      element.querySelector("form")!.requestSubmit();
      const text = await lightText(element, "Operation refused");
      expect(text).toContain("Held. (machines.address.taken)");
      const codes = [...element.querySelectorAll(".k-code-badge code")].map((c) => c.textContent);
      expect(codes).toEqual(expect.arrayContaining(["machines.address.taken", "validation"]));
      expect(codes).toContain("req-42");
    });

    it("action: refused with its reason without the permission, hidden at another date", async () => {
      mounted = mount("machines-attach", {
        object: address,
        context: { locale: "en", user: user(["machines.interface.edit"]) },
        fetch: fakeFetch({ "GET /v1/machines": () => page([machine()]) }),
      });
      const element = mounted.element;
      await lightText(element, "Add to the draft");
      const submit = buttonOf(element, "Add to the draft");
      expect(submit.getAttribute("aria-disabled")).toBe("true");
      setContext({ time: { validAt: "2026-10-01T00:00:00Z", asOf: null } });
      await lightText(element, "The draft takes operations at the present only.");
      expect(element.querySelector("form")).toBeNull();
    });
  });

  describe("the host of a machine, moved by `canMove` and `draft.move` (task 18b)", () => {
    const GUEST = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b21";
    const OTHER = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b22";
    const CLUSTER = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b23";
    const listed = () =>
      page([
        machine(),
        {
          ...machine("vm-a"),
          id: GUEST,
          kind: "virtual",
          host: { machine: { id: MACHINE } },
        },
        { ...machine("srv-b"), id: OTHER },
      ]);
    const cluster = {
      id: CLUSTER,
      href: `/v1/clusters/${CLUSTER}`,
      slug: "grappe",
      owner: { id: ENTITY },
    };
    const draftOf = (operations: unknown[], version = 1) =>
      new Response(
        JSON.stringify({
          id: DRAFT,
          href: `/v1/drafts/${DRAFT}`,
          title: "t",
          operation_count: operations.length,
          version,
          operations,
        }),
        { status: 200, headers: { "Content-Type": "application/json", ETag: `"${version}"` } },
      );
    const simulation = (valid: boolean, violations: unknown[]) => ({
      valid,
      violations,
      diff: [],
      sample: [],
      cascaded: [],
      sections: [],
    });
    const reads = {
      "GET /v1/machines/{id}": () => card() as never,
      "GET /v1/machines/{id}/hosted": () => page([]),
      "GET /v1/interfaces": () => page([]),
      "GET /v1/entities/{id}": () => entity as never,
      "GET /v1/machines": () => listed(),
      "GET /v1/clusters": () => page([cluster]),
    };
    const hostSelect = (element: HTMLElement) =>
      element.shadowRoot!.querySelector<HTMLSelectElement>("select[name=host]")!;
    const optionOf = (element: HTMLElement, value: string) =>
      [...hostSelect(element).options].find((o) => o.value === value)!;
    const choose = (element: HTMLElement, value: string) => {
      const select = hostSelect(element);
      select.value = value;
      select.dispatchEvent(new Event("change"));
      select.closest("form")!.requestSubmit();
    };

    it("offers each host checked by canMove: a cycle and a right refused, with their code", async () => {
      history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
      const posted: unknown[] = [];
      mounted = mount("machines-machines", {
        context: { locale: "en", user: user(["machines.cluster.edit"]) },
        fetch: fakeFetch({
          ...reads,
          "POST /v1/drafts": ({ body }) => (posted.push(body), created()),
        }),
      });
      const { element } = mounted;
      await vi.waitFor(() => expect(optionOf(element, `machine:${OTHER}`)).toBeDefined());
      // Each refused host reachable and read: its reason and the code in its name.
      const said = (value: string) => optionOf(element, value).textContent;
      // Itself and the machine it hosts: a cycle, the rule of the module.
      expect(said(`machine:${MACHINE}`)).toBe(
        "srv-a — refused: it hosts it, or is it (machines.host.invalid)",
      );
      expect(said(`machine:${GUEST}`)).toBe(
        "vm-a — refused: it hosts it, or is it (machines.host.invalid)",
      );
      // A cluster without its right; none, its host now; another machine, offered.
      expect(said(`cluster:${CLUSTER}`)).toBe(
        "grappe — refused: needs the permission machines.cluster.edit",
      );
      expect(said("none")).toBe("None (not hosted) — current host");
      expect(said(`machine:${OTHER}`)).toBe("srv-b");
      // Grouped by type of host; the placeholder not offered.
      expect([...hostSelect(element).querySelectorAll("optgroup")].map((g) => g.label)).toEqual([
        "Clusters",
        "Machines",
      ]);
      expect(optionOf(element, "")).toMatchObject({ disabled: true, hidden: true });
      // Sent anyway: refused on the field with its code, nothing in the draft.
      choose(element, `machine:${GUEST}`);
      const select = hostSelect(element);
      expect(select.getAttribute("aria-invalid")).toBe("true");
      const described = select.getAttribute("aria-describedby")!.split(" ");
      expect(element.shadowRoot!.getElementById(described[0]!)?.textContent).toBe(
        "refused: it hosts it, or is it (machines.host.invalid)",
      );
      // The hint kept beside the error.
      expect(element.shadowRoot!.getElementById(described[1]!)?.textContent).toContain(
        "A host refused is said « refused » in the list",
      );
      choose(element, "none");
      expect(
        element.shadowRoot!.getElementById(select.getAttribute("aria-describedby")!.split(" ")[0]!)
          ?.textContent,
      ).toBe("It is its host already: nothing is added.");
      await new Promise((done) => setTimeout(done, 20));
      expect(posted).toEqual([]);
      // happy-dom keeps the focus of a removed shadow root and stalls the next mount's reads.
      select.blur();
    });

    it("refuses the button without machines.edit, says why beside it", async () => {
      history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
      const posted: unknown[] = [];
      mounted = mount("machines-machines", {
        context: { locale: "en", user: user(["machines.edit"]) },
        fetch: fakeFetch({
          ...reads,
          "POST /v1/drafts": ({ body }) => (posted.push(body), created()),
        }),
      });
      const { element } = mounted;
      await vi.waitFor(() => expect(optionOf(element, `cluster:${CLUSTER}`)).toBeDefined());
      const submit = hostSelect(element).closest("form")!.querySelector("button[type=submit]")!;
      expect(submit.getAttribute("aria-disabled")).toBe("true");
      expect(
        element.shadowRoot!.getElementById(submit.getAttribute("aria-describedby")!)?.textContent,
      ).toBe("Needs the permission machines.edit");
      choose(element, `machine:${OTHER}`);
      await new Promise((done) => setTimeout(done, 20));
      expect(posted).toEqual([]);
    });

    it("in the draft, « en attente » at its place until simulated; the warnings of the server said", async () => {
      history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
      const posted: unknown[] = [];
      let release!: () => void;
      const held = new Promise<void>((done) => (release = done));
      const warning = {
        rule: "machines.host.site_mismatch",
        severity: "warning",
        detail: "the machine is hosted on another site",
        subjects: [MACHINE],
        operation: null,
        pointer: null,
      };
      mounted = mount("machines-machines", {
        context: { locale: "en", user: user() },
        fetch: fakeFetch({
          ...reads,
          "GET /v1/drafts": () => page([]),
          "POST /v1/drafts": ({ body }) => {
            posted.push(body);
            return draftOf((body as { operations: unknown[] }).operations);
          },
          "POST /v1/drafts/{id}/simulation": async () => {
            await held;
            return simulation(true, [warning]) as never;
          },
        }),
      });
      const { element } = mounted;
      await vi.waitFor(() => expect(optionOf(element, `cluster:${CLUSTER}`)?.disabled).toBe(false));
      choose(element, `cluster:${CLUSTER}`);
      await shadowText(element, "Move to grappe waiting for the server");
      const submit = hostSelect(element).closest("form")!.querySelector("button[type=submit]")!;
      expect(submit.getAttribute("aria-busy")).toBe("true");
      // A second intent in flight: ignored, the list back on the host sent, its wait still said.
      choose(element, `machine:${OTHER}`);
      expect(hostSelect(element).value).toBe(`cluster:${CLUSTER}`);
      expect(element.shadowRoot!.querySelector("p[role=status]")?.textContent).toBe(
        "Move to grappe waiting for the server; the host shown stays until it answers.",
      );
      // Never optimistic: the host read stays (none).
      const host = [...element.shadowRoot!.querySelectorAll("dt")].find(
        (dt) => dt.textContent === "Hosted by",
      )!;
      expect(host.nextElementSibling?.textContent).toBe("—none");
      release();
      await shadowText(element, "Move to grappe in the draft, held by its simulation");
      expect(element.shadowRoot!.querySelector("[data-tone=warn]")?.textContent).toContain(
        "machines.host.site_mismatch",
      );
      expect(posted).toEqual([
        {
          title: expect.any(String),
          operations: [
            {
              op: "machines.update_machine",
              id: { id: MACHINE },
              host: { cluster: { id: CLUSTER } },
            },
          ],
        },
      ]);
      expect(host.nextElementSibling?.textContent).toBe("—none");
      expect(submit.hasAttribute("aria-busy")).toBe(false);
    });

    it("refused by the simulation: its rule said, the operation replaced by the next move", async () => {
      history.replaceState(null, "", `/x/machines/machines/${MACHINE}`);
      sessionStorage.setItem("kartenn.draft.active", DRAFT);
      const earlier = {
        op: "machines.update_machine",
        id: { id: MACHINE },
        host: { cluster: { id: CLUSTER } },
      };
      let operations: unknown[] = [earlier];
      const patched: unknown[] = [];
      const invalid = {
        rule: "machines.host.invalid",
        severity: "error",
        detail: "une machine physical ne peut pas être hébergée par une machine physical",
        subjects: [MACHINE],
        operation: 0,
        pointer: "/operations/0",
      };
      mounted = mount("machines-machines", {
        context: { locale: "en", user: user() },
        fetch: fakeFetch({
          ...reads,
          "GET /v1/drafts/{id}": () => draftOf(operations),
          "PATCH /v1/drafts/{id}": ({ body }) => {
            patched.push(body);
            operations = (body as { operations: unknown[] }).operations;
            return draftOf(operations, 2);
          },
          "POST /v1/drafts/{id}/simulation": () => simulation(false, [invalid]) as never,
        }),
      });
      const { element } = mounted;
      await vi.waitFor(() => expect(optionOf(element, `machine:${OTHER}`)?.disabled).toBe(false));
      choose(element, `machine:${OTHER}`);
      await shadowText(element, "Move to srv-b refused by the server");
      const alert = element.shadowRoot!.querySelector("[role=alert]")!;
      expect(alert.textContent).toContain("machines.host.invalid");
      expect(element.shadowRoot!.textContent).toContain(
        "The operation stays in the draft; another host chosen replaces it.",
      );
      // The earlier move of the machine replaced in place, never a second one.
      expect(patched).toEqual([
        {
          operations: [
            {
              op: "machines.update_machine",
              id: { id: MACHINE },
              host: { machine: { id: OTHER } },
            },
          ],
        },
      ]);
    });
  });

  it("tells an address within a network, in IPv4 and IPv6", () => {
    expect(within("192.0.2.10", "192.0.2.0/24")).toBe(true);
    expect(within("198.51.100.1", "192.0.2.0/24")).toBe(false);
    expect(within("2001:db8::1", "2001:db8::/32")).toBe(true);
    expect(within("2001:db9::1", "2001:db8::/32")).toBe(false);
    expect(within("::ffff:192.0.2.1", "::ffff:192.0.2.0/120")).toBe(true);
    expect(within("192.0.2.10", "2001:db8::/32")).toBe(false);
  });
});
