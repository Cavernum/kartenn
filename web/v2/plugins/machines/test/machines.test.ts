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

// The machines plugin in the sandbox of the SDK (spec 021, task 18a): announced as the shell
// would, its module loaded, each element mounted with a context and a simulated `fetch`. Its
// reads go through the shared cache; its forms add operations of the module to the draft.
// The view of the machines, in Vue since the 18cb: `machines-view.test.ts`; the view of the
// interfaces and the tab, in Vue since the 18cc: `interfaces-view.test.ts`; the view of the
// clusters, in Vue since the 18cd: `clusters-view.test.ts`.

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
  });
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
      // No machine: the absent value of the library (`NoValue`), in its language, the one
      // the shell sets for the page (French by default here), not the context's.
      await lightText(element, "—non renseigné");
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

    it("action: stops reading the interfaces at a page refused, read again by « Retry » only", async () => {
      const second: string[] = [];
      let refused = true;
      mounted = mount("machines-attach", {
        object: address,
        context: { locale: "en", user: user() },
        fetch: fakeFetch({
          "GET /v1/machines": () => page([machine()]),
          "GET /v1/interfaces": ({ query }) => {
            if (!query.get("cursor")) {
              return { items: [iface()], links: { next: "/v1/interfaces?cursor=p2" } } as never;
            }
            second.push(query.get("cursor")!);
            return refused ? problem(403, "forbidden") : page([]);
          },
        }),
      });
      const element = mounted.element;
      await chooseMachine(element, "srv", "srv-a");
      await lightText(element, "The interfaces could not be read");
      await new Promise((done) => setTimeout(done, 50));
      expect(second).toEqual(["p2"]);
      refused = false;
      buttonOf(element, "Retry").click();
      await vi.waitFor(() => expect(element.textContent).not.toContain("could not be read"));
      expect(second).toEqual(["p2", "p2"]);
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
});
