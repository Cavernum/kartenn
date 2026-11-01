// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { queryClient, type components, type Extension, type PluginHandle } from "@kartenn/sdk";
import { resetContext, setContext } from "@kartenn/sdk/host";
import { announcePlugin, fakeFetch, mount, type Mounted } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";

// The example plugin in the sandbox of the SDK (REQ-007-16; spec 021, REQ-021-25, 27):
// announced as the shell would, its module loaded, each element mounted with a context and
// a simulated `fetch`. Its reads go through the shared cache, which an invalidation makes
// read again: the element draws what it then holds.

const PREFIX = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const DOMAIN = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";
const DRAFT = "6a1d3e70-58c4-4f0b-9e2a-1c7d5b3f9e42";
const object = { type: "prefix", id: PREFIX, href: `/v1/prefixes/${PREFIX}` };

const prefix = (value = "192.0.2.0/24"): components["schemas"]["Prefix"] => ({
  id: PREFIX,
  value,
  domain: { id: DOMAIN, href: `/v1/domains/${DOMAIN}`, slug: "lan" },
  nature: "network",
  anycast: false,
  point_to_point: false,
  public: false,
  links: {
    self: `/v1/prefixes/${PREFIX}`,
    history: `/v1/prefixes/${PREFIX}/history`,
    properties: `/v1/prefixes/${PREFIX}/properties`,
  },
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

let handle: PluginHandle;
let mounted: Mounted<HTMLElement> | null = null;
/** An element in the page before the module defines it: upgraded by `definePlugin`. */
let early: HTMLElement;

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
    integrity: { "styles.css": "sha384-test" },
  });
  // The shell's link of the sheet of @kartenn/ui (its build writes it), never fetched here.
  const sheet = document.createElement("link");
  sheet.href = "/assets/ui-test.css";
  sheet.integrity = "sha384-ui";
  sheet.dataset["kartennSheet"] = "@kartenn/ui/style.css";
  document.head.append(sheet);
  early = document.createElement("example-summary");
  document.body.append(early);
  await import("../src/index.ts");
  handle = await defined;
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  queryClient.clear();
  resetContext();
  sessionStorage.clear();
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

/** The observers the cache holds, all queries together. */
const observers = () =>
  queryClient
    .getQueryCache()
    .findAll()
    .reduce((count, query) => count + query.getObserversCount(), 0);

/** The codes of the alert of `element`: that of the problem, then its `Request-Id`. */
const alertCodes = (element: HTMLElement) =>
  [...element.shadowRoot!.querySelectorAll("[role=alert] code")].map((code) => code.textContent);

describe("the example plugin", () => {
  it("defines every element its manifest declares, one of each point", () => {
    for (const extension of manifest.extensions) {
      expect(customElements.get(extension.element), extension.element).toBeDefined();
    }
    expect(new Set(manifest.extensions.map((e) => e.point))).toEqual(
      new Set(["view", "tab", "panel", "column", "action", "widget"]),
    );
  });

  it("has the same keys in both catalogs (REQ-007-29)", () => {
    const keys = (catalog: object, prefix = ""): string[] =>
      Object.entries(catalog).flatMap(([key, value]) =>
        typeof value === "object" ? keys(value as object, `${prefix}${key}.`) : [prefix + key],
      );
    expect(keys(fr).sort()).toEqual(keys(en).sort());
  });

  it("links the sheet of @kartenn/ui, then its own, in each shadow root (REQ-021-27)", async () => {
    mounted = mount("example-summary", { object });
    await shadowText(mounted.element, "Example summary");
    const links = [...mounted.element.shadowRoot!.querySelectorAll("link")];
    expect(links.map((link) => [link.href, link.integrity])).toEqual([
      [document.querySelector<HTMLLinkElement>("link[data-kartenn-sheet]")!.href, "sha384-ui"],
      [expect.stringMatching(/\/plugins\/example\/0\.0\.0\/styles\.css$/), "sha384-test"],
    ]);
  });

  it("gives an element already in the page its sheets and its texts once defined", async () => {
    await shadowText(early, "Example summary");
    expect(early.shadowRoot!.querySelectorAll("link")).toHaveLength(2);
    early.remove();
  });

  it("says why a sheet is refused, and draws the element anyway", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const styles = vi.spyOn(handle, "styles").mockRejectedValueOnce(new Error("no digest"));
    try {
      mounted = mount("example-usage", { properties: { row: { usage: "reserved" } } });
      await shadowText(mounted.element, /^reserved$/);
      expect(warn).toHaveBeenCalledWith(
        expect.stringContaining("example-usage: sheet refused"),
        expect.objectContaining({ message: "no digest" }),
      );
    } finally {
      styles.mockRestore();
      warn.mockRestore();
    }
  });

  it("view: reads the IPv4 prefixes through the shared cache, again once invalidated", async () => {
    let value = "192.0.2.0/24";
    const asked: URLSearchParams[] = [];
    let answer: () => void = () => {};
    const answered = new Promise<void>((resolve) => (answer = resolve));
    mounted = mount("example-overview", {
      context: { locale: "fr" },
      fetch: fakeFetch({
        "GET /v1/prefixes": async ({ query }) => {
          asked.push(query);
          await answered;
          return { items: [prefix(value)], links: {} } as never;
        },
      }),
    });
    const { element } = mounted;
    // A skeleton of fixed size, said loading, until the first page.
    await shadowText(element, "Chargement…");
    expect(element.shadowRoot!.querySelector(".k-skeleton")).not.toBeNull();
    answer();
    await shadowText(element, "192.0.2.0/24");
    expect(asked[0]?.get("within")).toBe("0.0.0.0/0");
    const heading = element.shadowRoot!.querySelector("h1");
    expect(heading?.textContent).toBe("Plugin d'exemple");
    expect(element.shadowRoot!.textContent).toContain("au présent");

    // What an event of the stream does (`changes` on a prefix): the list is invalidated, the
    // cache reads it again, the element draws the new page in the place of the list — the
    // page around it stays.
    value = "198.51.100.0/24";
    await queryClient.invalidateQueries({ queryKey: ["v1", "/v1/prefixes"], exact: false });
    await shadowText(element, "198.51.100.0/24");
    expect(asked).toHaveLength(2);
    expect(element.shadowRoot!.querySelector("h1")).toBe(heading);

    // Read again to the same page: the list itself is not drawn again.
    const list = element.shadowRoot!.querySelector("ul");
    await queryClient.invalidateQueries({ queryKey: ["v1", "/v1/prefixes"], exact: false });
    await vi.waitFor(() => expect(asked).toHaveLength(3));
    expect(element.shadowRoot!.querySelector("ul")).toBe(list);
  });

  it("view: another date of the shell, once connected, moves its observer to another query", async () => {
    const asked: (string | null)[] = [];
    mounted = mount("example-overview", {
      context: { locale: "fr" },
      fetch: fakeFetch({
        "GET /v1/prefixes": ({ query }) => {
          asked.push(query.get("valid_at"));
          return { items: query.get("valid_at") ? [] : [prefix()], links: {} } as never;
        },
      }),
    });
    await shadowText(mounted.element, "192.0.2.0/24");
    setContext({ time: { validAt: "2026-03-01T00:00:00Z", asOf: null } });
    await shadowText(mounted.element, "Aucun préfixe IPv4");
    expect(mounted.element.shadowRoot!.textContent).toContain("à une autre date");
    expect(asked).toEqual([null, "2026-03-01T00:00:00Z"]);
    // One observer, on the dated query.
    expect(observers()).toBe(1);
  });

  it("lets go of its queries once disconnected, and holds none if it leaves before its sheets", async () => {
    mounted = mount("example-overview", {
      fetch: fakeFetch({ "GET /v1/prefixes": () => ({ items: [prefix()], links: {} }) as never }),
    });
    await shadowText(mounted.element, "192.0.2.0/24");
    expect(observers()).toBe(1);
    mounted.unmount();
    expect(observers()).toBe(0);

    // Gone before its sheets settled: it never follows anything, never draws.
    mounted = mount("example-overview", {
      fetch: fakeFetch({ "GET /v1/prefixes": () => ({ items: [prefix()], links: {} }) as never }),
    });
    const { element } = mounted;
    mounted.unmount();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(observers()).toBe(0);
    expect(element.shadowRoot!.querySelector(".k-example-content")?.childNodes).toHaveLength(0);
    setContext({ locale: "fr" });
    expect(element.shadowRoot!.querySelector(".k-example-content")?.childNodes).toHaveLength(0);
  });

  it("view: a failed read shows its code and its Request-Id", async () => {
    mounted = mount("example-overview", {
      fetch: fakeFetch({ "GET /v1/prefixes": () => problem(403, "forbidden") }),
    });
    const text = await shadowText(mounted.element, "forbidden");
    expect(alertCodes(mounted.element)).toEqual(["forbidden", "req-42"]);
    // A space between the word and the value: the name read is « Code forbidden ».
    expect(text).toContain("Code forbidden");
    expect(text).toContain("Request req-42");
  });

  it("tab: reads its prefix through the shared cache, or says why it cannot", async () => {
    mounted = mount("example-details", {
      object,
      fetch: fakeFetch({ "GET /v1/prefixes/{id}": () => prefix() }),
    });
    const text = await shadowText(mounted.element, "192.0.2.0/24");
    expect(text).toContain("lan");
    mounted.unmount();
    queryClient.clear();

    mounted = mount("example-details", {
      object,
      fetch: fakeFetch({ "GET /v1/prefixes/{id}": () => problem(404, "not_found") }),
    });
    await shadowText(mounted.element, "Cannot read");
    expect(alertCodes(mounted.element)).toEqual(["not_found", "req-42"]);
  });

  it("panel and column: the object and the row they are given", async () => {
    mounted = mount("example-summary", { object });
    expect(await shadowText(mounted.element, PREFIX)).toContain("prefix");
    mounted.unmount();
    mounted = mount("example-usage", { object, properties: { row: { usage: "reserved" } } });
    await shadowText(mounted.element, /^reserved$/);
  });

  it("action: adds an address to the draft without sending a changeset", async () => {
    const posted: string[] = [];
    mounted = mount("example-draft-address", {
      object,
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}": () => prefix(),
        "GET /v1/drafts": () => ({ items: [] }) as never,
        "POST /v1/drafts": ({ body }) => {
          posted.push(JSON.stringify(body));
          return created();
        },
      }),
    });
    const root = mounted.element.shadowRoot!;
    await shadowText(mounted.element, "Add to the draft");
    const input = root.querySelector("input")!;
    expect(root.querySelector(`label[for="${input.id}"]`)?.textContent).toBe("Address *(required)");
    expect(root.getElementById(input.getAttribute("aria-describedby")!)?.textContent).toMatch(
      /^An IPv4 or IPv6 address/,
    );
    input.value = "192.0.2.20";
    root.querySelector("form")!.requestSubmit();
    await shadowText(mounted.element, "Operation added; the draft holds 1.");
    expect(posted).toHaveLength(1);
    expect(JSON.parse(posted[0]!)).toMatchObject({
      operations: [
        { op: "create_address", address: "192.0.2.20", domain: { id: DOMAIN }, usage: "reserved" },
      ],
    });
    expect(input.hasAttribute("aria-invalid")).toBe(false);

    const closed = vi.fn();
    mounted.element.addEventListener("kartenn-close", closed);
    root.querySelector<HTMLButtonElement>("button[type=button]")!.click();
    expect(closed).toHaveBeenCalledOnce();
  });

  it("action: follows the language without losing what is typed", async () => {
    mounted = mount("example-draft-address", { object });
    const root = mounted.element.shadowRoot!;
    await shadowText(mounted.element, "Add to the draft");
    const input = root.querySelector("input")!;
    input.value = "192.0.2.21";
    setContext({ locale: "fr" });
    expect(root.querySelector("label")?.textContent).toBe("Adresse *(requis)");
    expect(root.querySelector("button[type=submit]")?.textContent).toBe("Ajouter au brouillon");
    expect(root.querySelector("input")).toBe(input);
    expect(input.value).toBe("192.0.2.21");
  });

  it("action: a refused draft or a failed read is an alert tied to the field", async () => {
    for (const handlers of [
      // The draft refused by the server.
      {
        "GET /v1/prefixes/{id}": () => prefix(),
        "GET /v1/drafts": () => ({ items: [] }) as never,
        "POST /v1/drafts": () => problem(422, "validation"),
      },
      // The prefix cannot be read.
      { "GET /v1/prefixes/{id}": () => problem(403, "forbidden") },
    ] as const) {
      mounted = mount("example-draft-address", { object, fetch: fakeFetch(handlers) });
      const root = mounted.element.shadowRoot!;
      await shadowText(mounted.element, "Add to the draft");
      const input = root.querySelector("input")!;
      input.value = "192.0.2.22";
      root.querySelector("form")!.requestSubmit();
      await shadowText(mounted.element, "Cannot add");
      const alert = root.querySelector("[role=alert]")!;
      expect(input.getAttribute("aria-invalid")).toBe("true");
      expect(input.getAttribute("aria-describedby")?.split(" ")).toContain(alert.parentElement!.id);
      expect(alertCodes(mounted.element)).toHaveLength(2);
      expect(alertCodes(mounted.element)[1]).toBe("req-42");
      mounted.unmount();
      queryClient.clear();
      sessionStorage.clear();
    }
    mounted = null;
  });

  it("widget: the counters of the home page, or why they cannot be read", async () => {
    mounted = mount("example-counts", {
      fetch: fakeFetch({
        "GET /v1/stats": () =>
          ({
            objects: { "core.prefix": 1200, "core.address": 3 },
            modules: {},
            computed_at: "2026-10-24T00:00:00Z",
          }) as never,
      }),
    });
    const text = await shadowText(mounted.element, "Prefixes");
    expect(text).toMatch(/1[,  ]?200/);
    expect(handle.t("widget.counts")).toBe("Objects of the repository");
    mounted.unmount();
    queryClient.clear();

    mounted = mount("example-counts", {
      fetch: fakeFetch({ "GET /v1/stats": () => problem(403, "forbidden") }),
    });
    await shadowText(mounted.element, "Cannot read");
    expect(alertCodes(mounted.element)).toEqual(["forbidden", "req-42"]);
  });
});
