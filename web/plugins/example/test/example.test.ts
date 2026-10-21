// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { draft, type components, type Extension, type PluginHandle } from "@kartenn/sdk";
import { draftHost, resetContext } from "@kartenn/sdk/host";
import { announcePlugin, fakeFetch, mount, type Mounted } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";

// The example plugin in the sandbox of the SDK (REQ-007-16): announced as the shell would,
// its module loaded, each element mounted with a context and a simulated `fetch`.

const PREFIX = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const DOMAIN = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";
const object = { type: "prefix", id: PREFIX, href: `/v1/prefixes/${PREFIX}` };

const prefix = (): components["schemas"]["Prefix"] => ({
  id: PREFIX,
  value: "10.1.0.0/24",
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

let handle: PluginHandle;
let mounted: Mounted<HTMLElement> | null = null;

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
    integrity: { "styles.css": "sha384-test" },
  });
  await import("../src/index.ts");
  handle = await defined;
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  draftHost.set([]);
  resetContext();
});

/** The text of the shadow root, once `predicate` holds. */
async function shadowText(element: HTMLElement, predicate: (text: string) => boolean) {
  for (let i = 0; i < 50; i++) {
    const text = element.shadowRoot?.textContent ?? "";
    if (predicate(text)) return text;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  return element.shadowRoot?.textContent ?? "";
}

describe("the example plugin", () => {
  it("defines every element its manifest declares", () => {
    for (const extension of manifest.extensions) {
      expect(customElements.get(extension.element), extension.element).toBeDefined();
    }
  });

  it("has the same keys in both catalogs (W11)", () => {
    const keys = (catalog: object, prefix = ""): string[] =>
      Object.entries(catalog).flatMap(([key, value]) =>
        typeof value === "object" ? keys(value as object, `${prefix}${key}.`) : [prefix + key],
      );
    expect(keys(fr).sort()).toEqual(keys(en).sort());
  });

  it("translates under its namespace, in the language of the context", () => {
    mounted = mount("example-overview", { context: { locale: "fr" } });
    expect(mounted.element.shadowRoot?.querySelector("h1")?.textContent).toBe("Plugin d'exemple");
    mounted.unmount();
    mounted = mount("example-overview", { context: { locale: "en" } });
    expect(mounted.element.shadowRoot?.querySelector("h1")?.textContent).toBe("Example plugin");
    expect(handle.t("nav.overview")).toBe("Example");
  });

  it("asks the shell for its style sheet, with its digest", () => {
    mounted = mount("example-summary", { context: { locale: "en" }, object });
    const link = mounted.element.shadowRoot?.querySelector<HTMLLinkElement>("link[rel=stylesheet]");
    expect(link?.getAttribute("href")).toBe("/plugins/example/0.0.0/styles.css");
    expect(link?.integrity).toBe("sha384-test");
  });

  it("tells the present from a dated view, and follows the context", () => {
    mounted = mount("example-overview", {
      context: { locale: "en", time: { validAt: "2026-03-01T00:00:00Z", asOf: null } },
    });
    expect(mounted.element.shadowRoot?.textContent).toContain("another date than the present");
  });

  it("reads the prefix of its tab through the client of the SDK", async () => {
    let asked = "";
    mounted = mount("example-details", {
      context: { locale: "en" },
      object,
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}": ({ params }) => {
          asked = params["id"] ?? "";
          return prefix();
        },
      }),
    });
    const text = await shadowText(mounted.element, (s) => s.includes("10.1.0.0/24"));
    expect(asked).toBe(PREFIX);
    expect(text).toContain("Prefix 10.1.0.0/24, domain lan");
  });

  it("says why a read fails, from the Problem Details", async () => {
    mounted = mount("example-details", {
      context: { locale: "en" },
      object,
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}": () =>
          new Response(
            JSON.stringify({ type: "about:blank", title: "Forbidden", status: 403, detail: "no" }),
            { status: 403, headers: { "Content-Type": "application/problem+json" } },
          ),
      }),
    });
    expect(await shadowText(mounted.element, (s) => s.includes("Cannot"))).toContain(
      "Cannot read: no",
    );
  });

  it("shows the usage of the row in its column", () => {
    mounted = mount("example-usage", {
      context: { locale: "fr" },
      properties: { row: { usage: "reserved" } },
    });
    expect(mounted.element.shadowRoot?.textContent).toContain("réservée");
  });

  it("adds an operation to the draft, without sending anything", async () => {
    const sent: string[] = [];
    mounted = mount("example-draft-address", {
      context: { locale: "en" },
      object,
      fetch: async (input, init) => {
        sent.push(init?.method ?? "GET");
        return fakeFetch({ "GET /v1/prefixes/{id}": () => prefix() })(input, init);
      },
    });
    const root = mounted.element.shadowRoot!;
    root.querySelector("input")!.value = "10.1.0.42";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    expect(await shadowText(mounted.element, (s) => s.includes("draft holds"))).toContain(
      "the draft holds 1",
    );
    expect(draft.operations()).toEqual([
      { op: "create_address", address: "10.1.0.42", domain: { id: DOMAIN }, usage: "reserved" },
    ]);
    expect(sent).toEqual(["GET"]);
  });

  it("closes through the event the shell listens to", () => {
    mounted = mount("example-draft-address", { context: { locale: "en" }, object });
    let closed = false;
    mounted.element.addEventListener("kartenn-close", () => (closed = true));
    [...mounted.element.shadowRoot!.querySelectorAll("button")]
      .find((b) => b.textContent === "Close")!
      .click();
    expect(closed).toBe(true);
  });
});
