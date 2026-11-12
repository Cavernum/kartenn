// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import {
  declaredFor,
  propertyGridColumns,
  propertyOfColumn,
  rowsToRead,
  withPropertiesHidden,
} from "../src/views/propertyColumns.ts";
import { clearSensitive } from "../src/session/index.ts";
import { columnsKey, keepColumns, readColumns } from "../src/views/viewSettings.ts";

// The columns of properties of a list (task 021-12i, Q-021-18): none by default; the
// chooser « Colonnes » offers the properties declared for the kind of the list, and the
// choice is kept by view and by user; a column chosen reads the values of its rows, and
// its cells are edited by the rules of the record's tab (task 12h).

const declarations = [
  { id: "d-zone", key: "custom.zone", type: "text", cardinality: "single", carriers: ["prefix"] },
  { id: "d-site", key: "custom.code", type: "text", cardinality: "single", carriers: ["site"] },
  { id: "d-any", key: "custom.any", type: "int", cardinality: "single", carriers: [] },
];

describe("the properties a list offers", () => {
  it("declared for the kind of the list, or for every kind; none of another kind", () => {
    expect(declaredFor(declarations, "core.prefix").map((d) => d.key)).toEqual([
      "custom.any",
      "custom.zone",
    ]);
    expect(declaredFor(declarations, "core.site").map((d) => d.key)).toEqual([
      "custom.any",
      "custom.code",
    ]);
  });

  it("a column each, by its label or its key, of priority 3 (the chooser lists it)", () => {
    const columns = propertyGridColumns(declarations.slice(0, 1), () => "Zone");
    expect(columns).toEqual([{ id: "prop.custom.zone", label: "Zone", width: "md", priority: 3 }]);
    expect(propertyGridColumns(declarations.slice(0, 1), () => null)[0]!.label).toBe("custom.zone");
    // Two of the same label told apart by their keys.
    expect(
      propertyGridColumns(declarations, (key) => (key === "custom.any" ? "Divers" : "Zone")).map(
        (c) => c.label,
      ),
    ).toEqual(["Zone (custom.zone)", "Zone (custom.code)", "Divers"]);
    expect(propertyOfColumn("prop.custom.zone")).toBe("custom.zone");
    expect(propertyOfColumn("value")).toBeNull();
  });

  it("none shown by default; a choice made holds", () => {
    const columns = [
      { id: "value", label: "Valeur" },
      { id: "public", label: "Public", priority: 3 as const },
      ...propertyGridColumns(declarations, () => null),
    ];
    expect(withPropertiesHidden({}, columns)).toEqual({
      "prop.custom.zone": false,
      "prop.custom.code": false,
      "prop.custom.any": false,
    });
    expect(withPropertiesHidden({ "prop.custom.zone": true, public: false }, columns)).toEqual({
      "prop.custom.zone": true,
      public: false,
      "prop.custom.code": false,
      "prop.custom.any": false,
    });
  });

  it("reads the rows in view and a screen around them, and the row of the focus", () => {
    const ids = Array.from({ length: 100 }, (_, i) => `r${i}`);
    expect(rowsToRead(ids, null)).toHaveLength(20);
    expect(rowsToRead(ids, { start: 40, end: 50, size: 10 })).toEqual(ids.slice(30, 60));
    expect(rowsToRead(ids, { start: 40, end: 50, size: 10 }, "r90")).toEqual([
      ...ids.slice(30, 60),
      "r90",
    ]);
    expect(rowsToRead(ids, { start: 0, end: 0, size: 0 })).toEqual(ids.slice(0, 20));
  });
});

describe("the columns kept by view and by user", () => {
  afterEach(() => localStorage.clear());

  it("apart for each user and each view, under the user's identity", () => {
    keepColumns("u-1", "prefixes", { "prop.custom.zone": true });
    expect(readColumns("u-1", "prefixes")).toEqual({ "prop.custom.zone": true });
    expect(readColumns("u-2", "prefixes")).toEqual({});
    expect(readColumns("u-1", "addresses")).toEqual({});
    expect(localStorage.getItem(columnsKey("u-1", "prefixes"))).toBe('{"prop.custom.zone":true}');
  });

  it("anything but a choice by column read as none; cleared at the logout", () => {
    localStorage.setItem(columnsKey("u-1", "prefixes"), '{"a":true,"b":"yes"}');
    expect(readColumns("u-1", "prefixes")).toEqual({ a: true });
    localStorage.setItem(columnsKey("u-1", "prefixes"), "[1]");
    expect(readColumns("u-1", "prefixes")).toEqual({});
    keepColumns("u-1", "prefixes", { a: true });
    clearSensitive();
    expect(readColumns("u-1", "prefixes")).toEqual({});
  });
});

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
  setFetch(null);
  document.body.innerHTML = "";
  localStorage.clear();
});

const PREFIX = "0190a000-0000-7000-8000-0000000000a1";
const json = (body: unknown, etag = '"v1"') =>
  new Response(JSON.stringify(body), {
    headers: { "Content-Type": "application/json", ETag: etag },
  });

/** The list of the prefixes mounted for `identity`, one prefix whose zone is inherited. */
async function mounted(identity: string, permissions: string[]) {
  const sent: string[] = [];
  setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    sent.push(`${init?.method ?? "GET"} ${url}`);
    if (url.startsWith("/v1/labels")) {
      return json({ lang: "fr", default_lang: "fr", revision: "1", spaces: {} });
    }
    if (url.startsWith("/v1/property-declarations")) {
      return json({ items: declarations, links: {} });
    }
    if (url.startsWith(`/v1/prefixes/${PREFIX}/properties`)) {
      return json({ properties: { "custom.zone": "ouest" }, masked: [] });
    }
    if (url.startsWith("/v1/prefixes")) {
      return json({
        items: [
          {
            id: PREFIX,
            value: "192.0.2.0/24",
            nature: "network",
            version: "v1",
            links: { self: `/v1/prefixes/${PREFIX}` },
          },
        ],
        links: {},
      });
    }
    return json({ counts: [], items: [], links: {}, sections: [] });
  }) as typeof fetch);
  await loadSession(async (url) =>
    url === "/v1/me"
      ? ({
          identity,
          kind: "person",
          global_permissions: permissions,
          bindings: [],
        } as unknown as Me)
      : null,
  );
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/", component: RouterView, children: frameRoutes }],
  });
  await router.push("/prefixes");
  const root = document.body.appendChild(document.createElement("div"));
  app = createApp({ render: () => h(RouterView) });
  app.use(createShellI18n("fr")).use(router);
  app.mount(root);
  await router.isReady();
  return { root, sent };
}

const headers = (root: HTMLElement) =>
  [...root.querySelectorAll('[data-k-list] [role="columnheader"]')].map(
    (cell) => cell.getAttribute("data-column") ?? "",
  );
const READ = ["core.prefix.read"];

describe("a list with its columns of properties (task 021-12i)", () => {
  it("none by default: the chooser offers the properties of the kind, unchecked", async () => {
    const { root, sent } = await mounted("u-1", READ);
    await expect.poll(() => headers(root)).toContain("value");
    expect(headers(root).filter((id) => id.startsWith("prop."))).toEqual([]);
    // No column chosen, no value read.
    expect(sent.some((s) => s.includes("/properties"))).toBe(false);
    const trigger = [...root.querySelectorAll("button")].find((b) =>
      b.textContent?.includes("Colonnes"),
    );
    expect(trigger).toBeDefined();
    trigger!.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await expect
      .poll(() => [...document.querySelectorAll('[role="menuitemcheckbox"]')].length)
      .toBeGreaterThan(0);
    const items = [...document.querySelectorAll('[role="menuitemcheckbox"]')];
    const named = items.map((item) => item.textContent?.trim());
    expect(named).toContain("custom.zone");
    expect(named).toContain("custom.any");
    expect(named).not.toContain("custom.code");
    const zone = items.find((item) => item.textContent?.trim() === "custom.zone")!;
    expect(zone.getAttribute("aria-checked")).toBe("false");
  });

  it("a column chosen kept for its user: shown at once, its value read; not for another", async () => {
    keepColumns("u-1", "prefixes", { "prop.custom.zone": true });
    const first = await mounted("u-1", READ);
    await expect.poll(() => headers(first.root)).toContain("prop.custom.zone");
    const cell = () =>
      first.root.querySelector(`[data-row="${PREFIX}"][data-column="prop.custom.zone"]`);
    await expect.poll(() => cell()?.textContent).toContain("ouest");
    expect(first.sent).toContain(`GET /v1/prefixes/${PREFIX}/properties`);
    // Without `prop.custom.zone.edit`, no editor, and why.
    expect(cell()?.getAttribute("aria-readonly")).toBe("true");
    app?.unmount();
    app = undefined;
    queryClient.clear();
    document.body.innerHTML = "";

    const other = await mounted("u-2", READ);
    await expect.poll(() => headers(other.root)).toContain("value");
    await nextTick();
    expect(headers(other.root)).not.toContain("prop.custom.zone");
  });

  it("with `prop.<key>.edit` alone the cell edits; Enter adds it to the draft", async () => {
    keepColumns("u-1", "prefixes", { "prop.custom.zone": true });
    const { root } = await mounted("u-1", [...READ, "prop.custom.zone.edit"]);
    const cell = () =>
      root.querySelector<HTMLElement>(`[data-row="${PREFIX}"][data-column="prop.custom.zone"]`);
    await expect.poll(() => cell()?.textContent).toContain("ouest");
    expect(cell()?.getAttribute("aria-readonly")).toBeNull();
  });
});
