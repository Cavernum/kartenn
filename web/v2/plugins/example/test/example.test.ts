// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { queryClient, type components, type Extension, type PluginHandle } from "@kartenn/sdk";
import { resetContext } from "@kartenn/sdk/host";
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

let handle: PluginHandle;
let mounted: Mounted<HTMLElement> | null = null;

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
  await import("../src/index.ts");
  handle = await defined;
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  queryClient.clear();
  resetContext();
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

  it("view: reads the prefixes through the shared cache, again once they are invalidated", async () => {
    let value = "192.0.2.0/24";
    let reads = 0;
    let answer: () => void = () => {};
    const answered = new Promise<void>((resolve) => (answer = resolve));
    mounted = mount("example-overview", {
      context: { locale: "fr" },
      fetch: fakeFetch({
        "GET /v1/prefixes": async () => {
          reads++;
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
    expect(element.shadowRoot!.querySelector("h1")?.textContent).toBe("Plugin d'exemple");
    expect(element.shadowRoot!.textContent).toContain("au présent");

    // What an event of the stream does (`changes` on a prefix): the list is invalidated, the
    // cache reads it again, the element draws the new page.
    value = "198.51.100.0/24";
    await queryClient.invalidateQueries({ queryKey: ["v1", "/v1/prefixes"], exact: false });
    await shadowText(element, "198.51.100.0/24");
    expect(reads).toBe(2);

    // Read again to the same page: nothing redrawn (a reader's place is kept), only the time
    // of the read.
    const heading = element.shadowRoot!.querySelector("h1");
    await queryClient.invalidateQueries({ queryKey: ["v1", "/v1/prefixes"], exact: false });
    await vi.waitFor(() => expect(reads).toBe(3));
    expect(element.shadowRoot!.querySelector("h1")).toBe(heading);

    // Another date of the shell: another query, said in the view.
    mounted.unmount();
    mounted = mount("example-overview", {
      context: { locale: "fr", time: { validAt: "2026-03-01T00:00:00Z", asOf: null } },
      fetch: fakeFetch({ "GET /v1/prefixes": () => ({ items: [], links: {} }) as never }),
    });
    await shadowText(mounted.element, "Aucun préfixe");
    expect(mounted.element.shadowRoot!.textContent).toContain("à une autre date");
  });

  it("view: a failed read shows its code and its Request-Id", async () => {
    mounted = mount("example-overview", {
      fetch: fakeFetch({ "GET /v1/prefixes": () => problem(403, "forbidden") }),
    });
    const text = await shadowText(mounted.element, "forbidden");
    expect(mounted.element.shadowRoot!.querySelector("[role=alert]")).not.toBeNull();
    const codes = [...mounted.element.shadowRoot!.querySelectorAll("[role=alert] code")];
    expect(codes.map((code) => code.textContent)).toEqual(["forbidden", "req-42"]);
    expect(text).toContain("Request");
  });

  it("tab: reads its prefix through the shared cache", async () => {
    mounted = mount("example-details", {
      object,
      fetch: fakeFetch({ "GET /v1/prefixes/{id}": () => prefix() }),
    });
    const text = await shadowText(mounted.element, "192.0.2.0/24");
    expect(text).toContain("lan");
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
          return new Response(
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
        },
      }),
    });
    sessionStorage.clear();
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

    const closed = vi.fn();
    mounted.element.addEventListener("kartenn-close", closed);
    root.querySelector<HTMLButtonElement>("button[type=button]")!.click();
    expect(closed).toHaveBeenCalledOnce();
    sessionStorage.clear();
  });

  it("widget: the counters of the home page, read through the shared cache", async () => {
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
  });
});
