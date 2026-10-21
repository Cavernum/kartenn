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

// The VLAN plugin in the sandbox of the SDK (spec 008, REQ-008-24): announced as the shell
// would, its module loaded, each element mounted with a context and a simulated `fetch`.

const PREFIX = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const GROUP = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";
const VLAN = "7a1e3c55-2b64-4d1f-9e0a-5c3b8d2f6a90";
const DECLARATION = "1f2e3d4c-5b6a-4978-8695-a4b3c2d1e0f9";
const object = { type: "prefix", id: PREFIX, href: `/v1/prefixes/${PREFIX}` };

const group = {
  id: GROUP,
  slug: "core",
  anchor: { entity: { id: GROUP } },
  vid_min: 100,
  vid_max: 199,
  href: `/v1/vlan-groups/${GROUP}`,
};
const vlan = {
  id: VLAN,
  group: { id: GROUP },
  vid: 120,
  name: "servers",
  status: "active",
  href: `/v1/vlans/${VLAN}`,
};
/** An answer of the simulated API, outside the types of the contract. */
const json = (value: unknown, status = 200) =>
  new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });
const page = (items: unknown[]) => json({ items, links: { next: null } });
const properties = (values: Record<string, unknown>) =>
  json({
    links: { object: object.href, self: `${object.href}/properties` },
    masked: [],
    properties: values,
  });

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

describe("the VLAN plugin", () => {
  it("defines every element its manifest declares, for the module vlan only", () => {
    expect(manifest.module).toBe("vlan");
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

  it("reads its column a few rows at a time, and drops a row gone by its turn", async () => {
    const { limited, COLUMN_READS } = await import("../src/index.ts");
    let running = 0;
    let highest = 0;
    const read = async () => {
      running++;
      highest = Math.max(highest, running);
      await new Promise((resolve) => setTimeout(resolve, 5));
      running--;
      return "read";
    };
    const rows = Array.from({ length: 40 }, (_, i) => limited(() => i % 2 === 0, read));
    const results = await Promise.all(rows);
    expect(highest).toBe(COLUMN_READS);
    expect(results.filter((r) => r === "read")).toHaveLength(20);
    expect(results.filter((r) => r === null)).toHaveLength(20);
  });

  it("shows the VLAN of a network in its column", async () => {
    mounted = mount("vlan-column", {
      context: { locale: "fr" },
      object,
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}/properties": () => properties({ "vlan.vlan": VLAN }),
        "GET /v1/vlans/{id}": () => json(vlan),
      }),
    });
    expect(await shadowText(mounted.element, (s) => s.includes("120"))).toBe("VLAN 120 · servers");
  });

  it("proposes the next free VID of the chosen group, then creates and sets the VLAN", async () => {
    const posted: { path: string; body: unknown }[] = [];
    mounted = mount("vlan-create", {
      context: { locale: "en" },
      object,
      fetch: fakeFetch({
        "GET /v1/vlan-groups": () => page([group]),
        "GET /v1/vlan-groups/{id}/next-free": () => json({ group: { id: GROUP }, vid: 121 }),
        "GET /v1/vlans": ({ query }) =>
          page(
            query.get("group") === GROUP && query.get("vid") === "121"
              ? [{ ...vlan, vid: 121, name: "backup" }]
              : [],
          ),
        "GET /v1/property-declarations": () => page([{ id: DECLARATION, key: "vlan.vlan" }]),
        "POST /v1/vlans": ({ body }) => {
          posted.push({ path: "/v1/vlans", body });
          return json({ state: "applied" }, 201);
        },
        "POST /v1/changesets": ({ body }) => {
          posted.push({ path: "/v1/changesets", body });
          return json({ state: "applied" }, 201);
        },
      }),
    });
    await shadowText(mounted.element, () => !!mounted?.element.shadowRoot?.querySelector("form"));
    const root = mounted.element.shadowRoot!;
    expect(root.querySelector<HTMLInputElement>("input[name=vid]")?.value).toBe("121");
    root.querySelector<HTMLInputElement>("input[name=name]")!.value = "backup";
    root.querySelector<HTMLInputElement>("input[name=motive]")!.value = "new segment";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    expect(await shadowText(mounted.element, (s) => s.includes("created"))).toContain(
      "VLAN 121 · backup created and set on the network.",
    );
    expect(posted).toEqual([
      {
        path: "/v1/vlans",
        body: { motive: "new segment", group: { id: GROUP }, vid: 121, name: "backup" },
      },
      {
        path: "/v1/changesets",
        body: {
          motive: "new segment",
          operations: [
            {
              op: "assign_property",
              kind: "prefix",
              object: { id: PREFIX },
              property: { id: DECLARATION },
              values: [VLAN],
            },
          ],
        },
      },
    ]);
  });

  it("offers no write away from the present (W9)", async () => {
    mounted = mount("vlan-network", {
      context: { locale: "en", time: { validAt: null, asOf: "2026-01-01T00:00:00Z" } },
      object,
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}/properties": () => properties({}),
      }),
    });
    expect(await shadowText(mounted.element, (s) => s.includes("carries"))).toContain(
      "This network carries no VLAN.",
    );
    expect(mounted.element.shadowRoot?.querySelector("form")).toBeNull();
  });
});
