// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { Extension } from "@kartenn/sdk";
import { resetContext } from "@kartenn/sdk/host";
import { announcePlugin, fakeFetch, mount, type Mounted } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";

// The plugin of the machines in the sandbox of the SDK (spec 009, REQ-009-20): announced as
// the shell would, its module loaded, each element mounted with a context and a simulated
// `fetch`.

const ADDRESS = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const MACHINE = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";
const IFACE = "7a1e3c55-2b64-4d1f-9e0a-5c3b8d2f6a90";
const object = { type: "address", id: ADDRESS, href: `/v1/addresses/${ADDRESS}` };

const machine = {
  id: MACHINE,
  name: "web1",
  kind: "physical",
  owner: null,
  site: null,
  host: null,
  member_of: null,
  href: `/v1/machines/${MACHINE}`,
};
const iface = {
  id: IFACE,
  machine: { id: MACHINE },
  name: "eth0",
  mac: null,
  enabled: true,
  vlan: { mode: "none", access: null, native: null, tagged: [] },
  href: `/v1/interfaces/${IFACE}`,
};
/** An answer of the simulated API, outside the types of the contract. */
const json = (value: unknown, status = 200, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json", ...headers },
  });
const page = (items: unknown[]) => json({ items, links: { next: null } });

let mounted: Mounted<HTMLElement> | null = null;

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
    integrity: { "styles.css": "sha384-test" },
  });
  await import("../src/index.ts");
  await defined;
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  resetContext();
});

/** The text of the shadow root, once `predicate` holds. */
async function shadowText(element: HTMLElement, predicate: (text: string) => boolean) {
  for (let i = 0; i < 100; i++) {
    const text = element.shadowRoot?.textContent ?? "";
    if (predicate(text)) return text;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  return element.shadowRoot?.textContent ?? "";
}

describe("the plugin of the machines", () => {
  it("defines every element its manifest declares, for the module machines only", () => {
    expect(manifest.module).toBe("machines");
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

  it("shows the machine that holds an address in its column", async () => {
    mounted = mount("machines-column", {
      context: { locale: "fr" },
      object,
      fetch: fakeFetch({
        "GET /v1/interfaces": ({ query }) => page(query.get("address") === ADDRESS ? [iface] : []),
        "GET /v1/machines/{id}": () => json(machine),
      }),
    });
    expect(await shadowText(mounted.element, (s) => s.includes("web1"))).toBe("web1");
  });

  it("attaches an address to the chosen interface under its If-Match", async () => {
    const posted: { ifMatch: string | null; body: unknown }[] = [];
    mounted = mount("machines-attach", {
      context: { locale: "en" },
      object,
      fetch: fakeFetch({
        "GET /v1/machines": () => page([machine]),
        "GET /v1/interfaces": ({ query }) => page(query.get("machine") === MACHINE ? [iface] : []),
        "GET /v1/interfaces/{id}": () => json(iface, 200, { ETag: '"v1"' }),
        "POST /v1/interfaces/{id}/attachments": ({ body, headers }) => {
          posted.push({ ifMatch: headers.get("If-Match"), body });
          return json({ state: "applied" });
        },
      }),
    });
    await shadowText(mounted.element, () => !!mounted?.element.shadowRoot?.querySelector("form"));
    const root = mounted.element.shadowRoot!;
    await shadowText(
      mounted.element,
      () => !!root.querySelector<HTMLSelectElement>("select[name=interface]")?.value,
    );
    root.querySelector<HTMLInputElement>("input[name=primary]")!.checked = true;
    root.querySelector<HTMLInputElement>("input[name=motive]")!.value = "new server";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    expect(await shadowText(mounted.element, (s) => s.includes("Attached"))).toContain(
      "Attached to eth0 of web1.",
    );
    expect(posted).toEqual([
      {
        ifMatch: '"v1"',
        body: { motive: "new server", address: { id: ADDRESS }, primary: true },
      },
    ]);
  });

  it("offers no write away from the present (W9)", async () => {
    mounted = mount("machines-machines", {
      context: { locale: "en", time: { validAt: null, asOf: "2026-01-01T00:00:00Z" } },
      fetch: fakeFetch({
        "GET /v1/machines": () => page([machine]),
      }),
    });
    expect(await shadowText(mounted.element, (s) => s.includes("web1"))).toContain("web1");
    expect(mounted.element.shadowRoot?.querySelector("form")).toBeNull();
  });
});
