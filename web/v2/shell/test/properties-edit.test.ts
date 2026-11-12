// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { collectionById } from "../src/views/collections.ts";
import {
  checkProperty,
  draftedProperties,
  guarded,
  originOf,
  propertyOperation,
  propertyPatch,
  propertyChoices,
  propertyValueOf,
  typedInPlace,
  type Declared,
} from "../src/views/edit/properties.ts";
import { listedOperations, withoutOperation } from "../src/draft/operations.ts";

// The tab « Propriétés » of a record edited in place (task 021-12h; REQ-021-15, 16, 37;
// Q-021-19): which values a cell edits and why not, the value checked by its declared
// type, the operation of the draft and the `properties` of the patch; then the tab against
// a fake API — a value of the object's own to the draft, an inherited one said overridden
// before it is sent, Ctrl+Enter applied under `If-Match`, held in review and why.

const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const S1 = "0b5e3c1a-0000-4000-8000-0000000000b1";
const V1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";
const CODE = "0b5e3c1a-0000-4000-8000-0000000000e1";
const ZONE = "0b5e3c1a-0000-4000-8000-0000000000e2";
const OWNER = "0b5e3c1a-0000-4000-8000-0000000000e3";

const declared = (
  key: string,
  type: string,
  params: Record<string, unknown> = {},
  cardinality = "single",
): Declared => ({ id: key, key, type, cardinality, params });

describe("a property edited in place", () => {
  it("is typed in a cell when it is one value of a type a cell takes", () => {
    expect(typedInPlace(declared("custom.code", "text"))).toBe(true);
    expect(typedInPlace(declared("custom.level", "enum"))).toBe(true);
    expect(typedInPlace(declared("core.owner", "ref", { target: "entity" }))).toBe(false);
    expect(typedInPlace(declared("custom.tags", "text", {}, "set"))).toBe(false);
  });

  it("checks the value by its declared type and parameters (REQ-003-8)", () => {
    const text = declared("custom.code", "text", { min_len: 2, max_len: 4, pattern: "[a-z]+" });
    expect(checkProperty(text, "abc", true)).toBeUndefined();
    expect(checkProperty(text, "a", true)?.key).toBe("edit.check.length");
    expect(checkProperty(text, "abcde", true)?.values).toEqual({ min: 2, max: 4 });
    // One bound alone: said by it alone.
    expect(checkProperty(declared("x.y", "text", { min_len: 3 }), "ab", true)).toEqual({
      key: "edit.check.lengthMin",
      values: { min: 3 },
    });
    expect(checkProperty(declared("x.y", "int", { max: 9 }), "10", true)?.key).toBe(
      "edit.check.betweenMax",
    );
    // The pattern is the server's alone (the `regex` crate is not JavaScript's engine).
    expect(checkProperty(text, "AB", true)).toBeUndefined();
    expect(checkProperty(text, "a\u0007b", true)?.key).toBe("edit.check.control");
    // Counted in NFC, as the server: « café » decomposed is 4 characters, not 5.
    const four = declared("custom.word", "text", { min_len: 0, max_len: 4 });
    expect(checkProperty(four, "café", true)).toBeUndefined();
    const int = declared("custom.rank", "int", { min: -5, max: 10 });
    expect(checkProperty(int, "-3", true)).toBeUndefined();
    expect(checkProperty(int, "11", true)?.key).toBe("edit.check.between");
    expect(checkProperty(int, "2.5", true)?.key).toBe("edit.check.integer");
    expect(checkProperty(declared("custom.on", "bool"), "oui", true)?.key).toBe("edit.check.bool");
    const date = declared("custom.end", "date");
    expect(checkProperty(date, "2026-02-28", true)).toBeUndefined();
    expect(checkProperty(date, "2026-02-30", true)?.key).toBe("edit.check.date");
    // The years 0 to 99 as they are (`Date.UTC` would read 1950).
    expect(checkProperty(date, "0050-01-01", true)).toBeUndefined();
    expect(checkProperty(date, "0050-02-29", true)?.key).toBe("edit.check.date");
    const fqdn = declared("dns.name", "fqdn");
    expect(checkProperty(fqdn, "srv-01.example.org.", true)).toBeUndefined();
    expect(checkProperty(fqdn, "srv_01.example.org", true)?.key).toBe("edit.check.fqdn");
    expect(checkProperty(fqdn, "-a.example.org", true)?.key).toBe("edit.check.fqdn");
    // Never stricter than UTS #46: combining marks, the full stops it maps to a dot.
    expect(checkProperty(fqdn, "हिन्दी.example", true)).toBeUndefined();
    expect(checkProperty(fqdn, "café。example．org", true)).toBeUndefined();
    const level = declared("custom.level", "enum");
    expect(checkProperty(level, "high", true, ["low", "high"])).toBeUndefined();
    expect(checkProperty(level, "top", true, ["low", "high"])?.key).toBe("edit.check.choice");
    // A part of the list only: the server judges.
    expect(checkProperty(level, "top", true, null)).toBeUndefined();
  });

  it("removes a value of the object's own when emptied, never an inherited one", () => {
    const text = declared("custom.code", "text");
    expect(checkProperty(text, " ", true)).toBeUndefined();
    expect(checkProperty(text, "", false)?.key).toBe("edit.property.nothingToRemove");
    expect(propertyValueOf(text, " ")).toBeNull();
    expect(propertyValueOf(declared("custom.rank", "int"), "7")).toBe(7);
    expect(propertyValueOf(declared("custom.on", "bool"), "false")).toBe(false);
  });

  it("chooses a bool or an enum read whole in a list, a value of its own removable (task 12g)", () => {
    const words = (value: string) => (value === "nord" ? "Zone nord" : null);
    const none = "Aucune valeur (retirer)";
    const bool = declared("custom.on", "bool");
    expect(propertyChoices(bool, undefined, { words, own: false, none })).toEqual([
      { value: "true", label: "true", mono: true },
      { value: "false", label: "false", mono: true },
    ]);
    const zone = declared("custom.zone", "enum");
    expect(propertyChoices(zone, ["nord", "sud"], { words, own: true, none })).toEqual([
      { value: "nord", label: "Zone nord", mono: false },
      { value: "sud", label: "sud", mono: true },
      { value: "", label: none },
    ]);
    // Not read yet, or only a part of it: typed, the server checking it.
    expect(propertyChoices(zone, undefined, { words, own: true, none })).toBeUndefined();
    expect(propertyChoices(zone, null, { words, own: true, none })).toBeUndefined();
    // Declared without values: not a list of « no value » alone.
    expect(propertyChoices(zone, [], { words, own: true, none })).toBeUndefined();
    expect(
      propertyChoices(declared("custom.code", "text"), null, { words, own: true, none }),
    ).toBeUndefined();
  });

  it("gives the operation of the draft and the `properties` of the patch", () => {
    expect(propertyOperation("prefix", P1, CODE, "abc")).toEqual({
      op: "assign_property",
      kind: "prefix",
      object: { id: P1 },
      property: { id: CODE },
      values: ["abc"],
    });
    // Removed: the object inherits again (REQ-003-12, Q-021-19).
    expect(propertyOperation("prefix", P1, CODE, null)).toEqual({
      op: "unassign_property",
      kind: "prefix",
      object: { id: P1 },
      property: { id: CODE },
    });
    expect(propertyPatch("custom.code", "abc")).toEqual({ properties: { "custom.code": "abc" } });
    expect(propertyPatch("custom.code", null)).toEqual({ properties: { "custom.code": null } });
  });

  it("guards the draft on the version read, once by object, the guard going with its operation (Q-021-21)", () => {
    const code = propertyOperation("prefix", P1, CODE, "a");
    const zone = propertyOperation("prefix", P1, ZONE, "b");
    const guard = { op: "expect", kind: "prefix", id: { id: P1 }, expected: V1 };
    const first = guarded([], "prefix", P1, V1, code);
    expect(first).toEqual([guard, code]);
    // A second property of the same object: no second guard.
    const draft = [...first, ...guarded(first, "prefix", P1, V1, zone)];
    expect(draft).toEqual([guard, code, zone]);
    // Listed without its guard (the counter of the server leaves it out too), carried by
    // the first operation of its object.
    expect(listedOperations(draft).map((l) => [l.index, l.guards])).toEqual([
      [1, [0]],
      [2, []],
    ]);
    // Removed with the last operation of its object, kept while one remains.
    expect(withoutOperation(draft, 1)).toEqual([guard, zone]);
    expect(withoutOperation(withoutOperation(draft, 1), 1)).toEqual([]);
    // No version read: no guard.
    expect(guarded([], "prefix", P1, null, code)).toEqual([code]);
  });

  it("reads what a draft changes in the properties of the object, the last winning", () => {
    const keyOf = (id: string) => ({ [CODE]: "custom.code", [ZONE]: "custom.zone" })[id];
    const drafted = draftedProperties(
      [
        propertyOperation("prefix", P1, CODE, "a"),
        propertyOperation("prefix", P1, CODE, "b"),
        propertyOperation("prefix", P1, ZONE, null),
        propertyOperation("prefix", S1, CODE, "other"),
      ],
      P1,
      keyOf,
    );
    expect([...drafted]).toEqual([
      ["custom.code", "b"],
      ["custom.zone", null],
    ]);
  });

  it("tells a value of the object's own from an inherited or a computed one", () => {
    const explained = (origin: unknown, provenance: string | null = null, value: unknown = "x") =>
      ({
        property: "custom.code",
        value,
        masked: false,
        chain: [],
        links: {},
        origin,
        provenance,
      }) as never;
    expect(originOf(explained({ id: P1 }), P1)).toEqual({ kind: "own" });
    expect(originOf(explained({ id: S1, kind: "site" }), P1)).toEqual({
      kind: "inherited",
      source: { id: S1, kind: "site" },
    });
    expect(originOf(explained(null, "source:inference@3"), P1)).toEqual({ kind: "computed" });
    expect(originOf(explained(null), P1)).toEqual({ kind: "inherited", source: null });
    expect(originOf(explained(null, null, null), P1)).toEqual({ kind: "unset" });
  });
});

describe("the tab « Propriétés » edited in place", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    document.body.innerHTML = "";
  });

  it("drafts a value of its own, says an inherited one overridden, applies under If-Match", async () => {
    const seen: { method: string; url: string; ifMatch: string | null; body: unknown }[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const method = init?.method ?? "GET";
      const url = String(input);
      const body = init?.body ? JSON.parse(String(init.body)) : null;
      seen.push({ method, url, ifMatch: new Headers(init?.headers).get("If-Match"), body });
      const json = (status: number, payload: unknown) =>
        new Response(JSON.stringify(payload), {
          status,
          headers: { "Content-Type": "application/json" },
        });
      if (url.startsWith("/v1/property-declarations")) {
        return json(200, {
          items: [
            {
              id: CODE,
              key: "custom.code",
              type: "text",
              params: { max_len: 8 },
              cardinality: "single",
            },
            {
              id: ZONE,
              key: "custom.zone",
              type: "text",
              params: { max_len: 8 },
              cardinality: "single",
            },
            {
              id: OWNER,
              key: "core.owner",
              type: "ref",
              params: { target: "entity" },
              cardinality: "single",
            },
          ].map((d) => ({ ...d, carriers: ["prefix"], sensitive: false, labels: {}, links: {} })),
          links: {},
        });
      }
      if (url.startsWith(`/v1/prefixes/${P1}/properties/custom.code/explanation`)) {
        return json(200, {
          property: "custom.code",
          value: "abc",
          masked: false,
          origin: { id: P1, kind: "prefix" },
          chain: [],
          links: {},
        });
      }
      if (url.startsWith(`/v1/prefixes/${P1}/properties/custom.zone/explanation`)) {
        return json(200, {
          property: "custom.zone",
          value: "ouest",
          masked: false,
          origin: { id: S1, kind: "site" },
          chain: [],
          links: {},
        });
      }
      if (url.startsWith(`/v1/prefixes/${P1}/properties`)) {
        return json(200, {
          properties: { "custom.code": "abc", "custom.zone": "ouest", "core.owner": null },
          masked: ["custom.secret"],
          links: {},
        });
      }
      if (url.startsWith(`/v1/sites/${S1}`))
        return json(200, { id: S1, slug: "site-a", links: {} });
      if (method === "POST" && url === "/v1/drafts") {
        return json(201, {
          id: D1,
          title: "Brouillon",
          version: 1,
          operation_count: 1,
          operations: body.operations,
        });
      }
      if (method === "PATCH") {
        return json(202, {
          changeset: { id: D1, state: "pending" },
          reason: { code: "changeset.approval.self", message: null },
        });
      }
      return json(200, { items: [], links: {} });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "alice",
            kind: "person",
            global_permissions: [
              "prop.custom.code.edit",
              "prop.custom.code.approval.self",
              "prop.custom.zone.edit",
              "prop.core.owner.edit",
            ],
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const { default: PropertiesTab } = await import("../src/views/record/PropertiesTab.vue");
    const Host = defineComponent({
      setup: () => () =>
        h(PropertiesTab, {
          collection: collectionById("prefixes")!,
          id: P1,
          name: "192.0.2.0/24",
          version: V1,
        }),
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Host }],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp(RouterView);
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    const cell = (key: string) =>
      root.querySelector<HTMLElement>(`[data-row="${key}"][data-column="value"]`);
    const description = (key: string) =>
      document.getElementById(cell(key)!.getAttribute("aria-describedby") ?? "")?.textContent;
    await expect.poll(() => cell("custom.code")?.textContent).toContain("abc");

    // No editor, and why: a reference, a masked value.
    await expect.poll(() => cell("core.owner")?.getAttribute("aria-readonly")).toBe("true");
    expect(description("core.owner")).toBe(
      "Une valeur de ce type (référence, ensemble) ne se saisit pas dans une cellule.",
    );
    expect(description("custom.secret")).toBe(
      "Valeur masquée : sans son droit de lecture, elle ne se modifie pas ici.",
    );
    expect(cell("custom.code")!.getAttribute("aria-readonly")).toBeNull();

    /** The cell of `key` focused, its editor opened, `text` typed, then Enter (Ctrl or not). */
    async function type(key: string, text: string, ctrlKey = false): Promise<void> {
      cell(key)!.click();
      cell(key)!.focus();
      await nextTick();
      cell(key)!.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }),
      );
      await expect.poll(() => root.querySelector("[data-k-properties] input")).not.toBeNull();
      const input = root.querySelector<HTMLInputElement>("[data-k-properties] input")!;
      input.value = text;
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey, bubbles: true }));
      await nextTick();
    }

    // Inherited: the foot of the grid says, before anything is sent, what the edit writes.
    cell("custom.zone")!.click();
    cell("custom.zone")!.focus();
    await expect
      .poll(() => root.querySelector(".k-keyboard-hint")?.textContent)
      .toContain("Une valeur saisie ici surcharge la valeur héritée de site-a.");

    // Too long for its declaration: refused on the cell, nothing sent.
    await type("custom.code", "beaucoup-trop-long");
    expect(seen.some((s) => s.method === "POST")).toBe(false);
    root
      .querySelector<HTMLInputElement>("[data-k-properties] input")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    await nextTick();

    // Enter: `assign_property` to the draft, guarded by the version read; « en brouillon ».
    await type("custom.code", "xyz");
    await expect.poll(() => cell("custom.code")!.getAttribute("data-state")).toBe("draft");
    const post = seen.find((s) => s.method === "POST" && s.url === "/v1/drafts")!;
    expect((post.body as { operations: unknown[] }).operations).toEqual([
      { op: "expect", kind: "prefix", id: { id: P1 }, expected: V1 },
      {
        op: "assign_property",
        kind: "prefix",
        object: { id: P1 },
        property: { id: CODE },
        values: ["xyz"],
      },
    ]);
    expect(cell("custom.code")!.textContent).toContain("xyz");

    // Ctrl+Enter: the motive asked under the grid, then `properties` under If-Match: held.
    await type("custom.code", "uvw", true);
    await expect
      .poll(() => root.querySelector("form[aria-label^='Appliquer maintenant']"))
      .not.toBeNull();
    root
      .querySelector<HTMLFormElement>("form[aria-label^='Appliquer maintenant']")!
      .requestSubmit();
    await expect.poll(() => seen.some((s) => s.method === "PATCH")).toBe(true);
    const patch = seen.find((s) => s.method === "PATCH")!;
    expect(patch.url).toBe(`/v1/prefixes/${P1}?approval=self`);
    expect(patch.ifMatch).toBe(`"${V1}"`);
    expect((patch.body as { properties: unknown }).properties).toEqual({ "custom.code": "uvw" });
    await expect.poll(() => cell("custom.code")!.getAttribute("data-state")).toBe("review");
    expect(description("custom.code")).toBe(
      "Une politique d'approbation retient la modification : elle attend une revue.",
    );
    // Its row says it too, on its first cell, beside « (en revue) » (task 021-12n).
    const first = root.querySelector<HTMLElement>('[data-row="custom.code"][data-column="key"]')!;
    expect(first.textContent).toContain("(en revue)");
    expect(document.getElementById(first.getAttribute("aria-describedby") ?? "")?.textContent).toBe(
      "Une politique d'approbation retient la modification : elle attend une revue.",
    );
  });
});
