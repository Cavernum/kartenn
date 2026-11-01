// @vitest-environment happy-dom
import { definePlugin, label, type ObjectRef } from "@kartenn/sdk";
import { announcePlugin } from "@kartenn/sdk/testing";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { CORE_SPACES, locate, sidebarGroups } from "../src/frame/spaces.ts";
import { createShellI18n, mergePluginCatalog, shellKey } from "../src/i18n/index.ts";
import ExtensionElement from "../src/plugins/ExtensionElement.vue";
import {
  beginPlugins,
  CAPABILITY_ATTEMPTS,
  configurePlugins,
  loadPlugins,
  PLUGINS_FETCHED,
  readPlugins,
  SHARED,
  startPlugins,
} from "../src/plugins/loader.ts";
import { preloadModules } from "../src/plugins/preload.ts";
import {
  pluginLetters,
  pluginSpaces,
  registerLetters,
  viewEntry,
} from "../src/plugins/navigation.ts";
import {
  announce,
  extensionsOf,
  reasonCode,
  resetPlugins,
  settle,
  useCapabilitiesRead,
  usePlugins,
  usePluginsUnavailable,
  useServerVersion,
  widgetsOf,
  type Loaded,
  type Status,
  type UiPlugin,
} from "../src/plugins/registry.ts";
import { actionItems, gridColumns, placeId, placed, tabItems } from "../src/plugins/slots.ts";
import { Rights } from "../src/rights/index.ts";
import { createShortcutRegistry } from "../src/shortcuts/registry.ts";
import { navigationTarget, shellShortcuts } from "../src/shortcuts/shell.ts";

// The host of the plugins in the new shell (spec 021, REQ-021-13; task 16): the extensions
// by point, type and permission (the type `changeset` and the types of modules among
// them), the tile of the home page, the views under their space, the `g` letters and their
// conflicts, the loader (capabilities, ranges), the catalogs of the plugins apart from the
// shell's, and the element in its place with its error confined.

const t = (key: string) => key;

function plugin(name: string, extensions: UiPlugin["extensions"], more: Partial<UiPlugin> = {}) {
  return {
    name,
    version: "1.0.0",
    sdk: "^1.0",
    base: `/plugins/${name}/1.0.0/`,
    entry: `/plugins/${name}/1.0.0/index.js`,
    integrity: {},
    catalogs: {},
    extensions,
    ...more,
  } satisfies UiPlugin;
}

const loaded = (p: UiPlugin, status: Status = { state: "loaded" }): Loaded => ({
  plugin: p,
  status,
});

const FLOW = plugin(
  "flow",
  [
    { point: "view", id: "flows", element: "flow-flows", nav: "nav.flows", go: "f" },
    {
      point: "view",
      id: "review",
      element: "flow-review",
      nav: "nav.review",
      permission: "flow.review",
    },
    {
      point: "tab",
      id: "flows",
      element: "flow-tab",
      object_types: ["address", "machines.machine"],
      label: "tab.flows",
      permission: "flow.read",
    },
    {
      point: "action",
      id: "review-in-flow",
      element: "flow-review-in-flow",
      object_types: ["changeset"],
      label: "action.review",
      permission: "flow.review",
    },
    { point: "column", id: "flag", element: "flow-flag", object_types: ["address"] },
    { point: "widget", id: "home", element: "flow-home" },
  ],
  { space: "flows" },
);
const POLICY = plugin(
  "policy",
  [
    { point: "view", id: "matrix", element: "policy-matrix", nav: "nav.matrix", go: "t" },
    {
      point: "widget",
      id: "home",
      element: "policy-home",
      permission: "policy.catalogue.read",
    },
  ],
  { space: "policy" },
);

describe("extensions by point, type and permission (REQ-021-13)", () => {
  const reviewer = new Rights(["flow.read", "flow.review"]);

  it("offers an action on the type changeset, a tab on a type of a module", () => {
    const list = [loaded(FLOW)];
    expect(extensionsOf(list, "action", reviewer, "changeset").map((e) => e.id)).toEqual([
      "review-in-flow",
    ]);
    expect(extensionsOf(list, "tab", reviewer, "machines.machine")).toMatchObject([
      { id: "flows", plugin: "flow", element: "flow-tab" },
    ]);
    expect(extensionsOf(list, "tab", reviewer, "prefix")).toEqual([]);
  });

  it("hides what needs a permission not held, and every extension of a plugin not loaded", () => {
    const reader = new Rights(["flow.read"]);
    expect(extensionsOf([loaded(FLOW)], "action", reader, "changeset")).toEqual([]);
    for (const status of [
      { state: "loading" },
      { state: "rejected", reason: "timeout" },
    ] as const) {
      expect(extensionsOf([loaded(FLOW, status)], "tab", reviewer, "address")).toEqual([]);
    }
  });

  it("gives one tile per plugin, still loading or set aside, by its permission", () => {
    const officer = new Rights(["policy.catalogue.read"]);
    const list = [loaded(FLOW), loaded(POLICY, { state: "loading" })];
    expect(widgetsOf(list, officer).map((w) => [w.extension.element, w.status.state])).toEqual([
      ["flow-home", "loaded"],
      ["policy-home", "loading"],
    ]);
    expect(widgetsOf(list, new Rights([])).map((w) => w.extension.element)).toEqual(["flow-home"]);
  });

  it("places tabs, columns and actions apart from the screen's own ids", () => {
    // The catalogs of the plugin merged: its words; not yet, its name and the id.
    const words = (key: string) => `«${key}»`;
    const tabs = extensionsOf([loaded(FLOW)], "tab", reviewer, "address");
    expect(tabItems(tabs, words)).toEqual([
      { value: "x.flow.flows", label: "«plugin.flow.tab.flows»" },
    ]);
    expect(tabItems(tabs, t)).toEqual([{ value: "x.flow.flows", label: "flow · flows" }]);
    const columns = gridColumns(extensionsOf([loaded(FLOW)], "column", reviewer, "address"), t);
    expect(columns).toEqual([
      { id: "x.flow.flag", label: "flow · flag", element: "flow-flag", priority: 3 },
    ]);
    const actions = extensionsOf([loaded(FLOW)], "action", reviewer, "changeset");
    expect(actionItems(actions, words)).toEqual([
      { kind: "separator" },
      { id: "x.flow.review-in-flow", label: "«plugin.flow.action.review»", icon: "plug" },
    ]);
    expect(actionItems([], t)).toEqual([]);
    expect(placed(actions, placeId(actions[0]!))?.element).toBe("flow-review-in-flow");
  });
});

describe("views under their space (REQ-021-2, 13)", () => {
  it("adds the spaces only plugins fill, in their order, and hides a view by its permission", () => {
    const spaces = pluginSpaces([loaded(POLICY), loaded(FLOW)]);
    const groups = sidebarGroups(new Rights(["core.prefix.read"]), t, spaces);
    expect(groups.map((g) => g.id)).toEqual(["referential", "policy", "flows"]);
    expect(groups.find((g) => g.id === "flows")!.entries).toEqual([
      { id: "x.flow.flows", label: "plugin.flow.nav.flows", icon: "plug", to: "/x/flow/flows" },
    ]);
    const reviewer = sidebarGroups(new Rights(["flow.review"]), t, spaces);
    expect(reviewer.find((g) => g.id === "flows")!.entries.map((e) => e.id)).toEqual([
      "x.flow.flows",
      "x.flow.review",
    ]);
    expect(locate("/x/flow/review/123", spaces)?.entry.id).toBe("x.flow.review");
  });

  it("puts the views of a plugin without a space in the referential, after the core's", () => {
    const notes = plugin("notes", [
      { point: "view", id: "overview", element: "notes-overview", nav: "nav.overview" },
    ]);
    const referential = pluginSpaces([loaded(notes)]).find((s) => s.id === "referential")!;
    expect(referential.entries.at(-1)?.to).toBe("/x/notes/overview");
    expect(CORE_SPACES.find((s) => s.id === "referential")!.entries.at(-1)?.id).toBe("properties");
  });

  it("places nothing for a plugin set aside or still loading", () => {
    const spaces = pluginSpaces([
      loaded(FLOW, { state: "rejected", reason: "exception" }),
      loaded(POLICY, { state: "loading" }),
    ]);
    expect(spaces.map((s) => s.id)).toEqual(CORE_SPACES.map((s) => s.id));
  });
});

describe("the route of a view of a plugin (design « Routes »)", () => {
  afterEach(() => resetPlugins());

  it("opens /x/<plugin>/<id>/*, under its space, its modules preloaded on intent", async () => {
    announce([FLOW]);
    settle("flow", { state: "loaded" });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: { render: () => null }, children: frameRoutes }],
    });
    const to = router.resolve("/x/flow/flows/42");
    expect(to.name).toBe("plugin-view");
    const record = to.matched.at(-1)!;
    expect(record.meta.modules?.(to.params)).toBe(FLOW);
    expect(record.meta.breadcrumb?.(to.params, t)).toEqual([
      { label: "space.flows", to: "/x/flow/flows" },
      { label: "plugin.flow.nav.flows" },
    ]);
    expect(
      router.resolve("/x/absent/view").matched.at(-1)?.meta.modules?.({ plugin: "absent" }),
    ).toBeNull();
  });
});

describe("g letters of the plugins (design « API des plugins »)", () => {
  function frame() {
    const registry = createShortcutRegistry();
    for (const shortcut of shellShortcuts({
      t,
      rights: () => new Rights(["*"]),
      navigate: () => {},
      palette: () => {},
      help: () => {},
      filter: () => false,
      create: () => false,
      edit: () => false,
      sidebar: () => {},
      panel: () => {},
    })) {
      registry.register(shortcut);
    }
    return registry;
  }

  it("registers the letters the manifests propose, the plugin's view their target", () => {
    const registry = frame();
    const went: string[] = [];
    const letters = registerLetters(registry, pluginLetters([loaded(FLOW), loaded(POLICY)]), {
      t,
      rights: () => new Rights([]),
      navigate: (to) => went.push(to),
    });
    const flow = registry.list().find((s) => s.id === "flow.go.flows")!;
    expect(flow.keys).toEqual([["g"], ["f"]]);
    flow.run?.();
    expect(went).toEqual(["/x/flow/flows"]);
    expect(letters.granted).toEqual(
      new Map([
        [viewEntry("flow", "flows"), "f"],
        [viewEntry("policy", "matrix"), "t"],
      ]),
    );
    expect(letters.refused.size).toBe(0);
    letters.drop();
    expect(registry.list().some((s) => s.id === "flow.go.flows")).toBe(false);
  });

  it("refuses a letter of the shell, or taken by a plugin before, and keeps the plugin", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    const thief = plugin("thief", [
      { point: "view", id: "prefixes", element: "thief-prefixes", nav: "nav.p", go: "p" },
      { point: "view", id: "flows", element: "thief-flows", nav: "nav.f", go: "f" },
      { point: "view", id: "own", element: "thief-own", nav: "nav.o", go: "q" },
    ]);
    const registry = frame();
    const letters = registerLetters(registry, pluginLetters([loaded(FLOW), loaded(thief)]), {
      t,
      rights: () => new Rights([]),
      navigate: () => {},
    });
    expect(letters.refused.get("thief")).toEqual([
      { view: "prefixes", letter: "p" },
      { view: "flows", letter: "f" },
    ]);
    expect(letters.granted.get(viewEntry("thief", "own"))).toBe("q");
    // The shell's `g p` still leads to the prefixes.
    expect(navigationTarget("prefixes", new Rights(["core.prefix.read"]))).toBe("/prefixes");
  });

  it("hides and refuses a letter whose view needs a permission not held", () => {
    const gated = plugin("gated", [
      { point: "view", id: "v", element: "gated-v", nav: "nav.v", go: "m", permission: "x.read" },
    ]);
    const registry = frame();
    let rights = new Rights([]);
    registerLetters(registry, pluginLetters([loaded(gated)]), {
      t,
      rights: () => rights,
      navigate: () => {},
    });
    const shortcut = registry.list().find((s) => s.id === "gated.go.v")!;
    expect(shortcut.shown?.()).toBe(false);
    expect(shortcut.run?.()).toBe(false);
    rights = new Rights(["x.read"]);
    expect(shortcut.shown?.()).toBe(true);
  });
});

/** A `fetch` that answers each call with the next response of `answers`. */
function answering(...answers: (Response | Error)[]) {
  const calls: string[] = [];
  const fetcher = vi.fn((input: RequestInfo | URL) => {
    calls.push(String(input));
    const next = answers.shift() ?? new Error("no more answers");
    return next instanceof Error ? Promise.reject(next) : Promise.resolve(next);
  });
  return { fetcher: fetcher as unknown as typeof fetch, calls };
}

const json = (status: number, body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });

const noWait = () => Promise.resolve();

describe("the loader (REQ-007-9, 10; REQ-021-26)", () => {
  afterEach(() => {
    resetPlugins();
    vi.restoreAllMocks();
  });

  it("reads the capabilities again after a refusal for load, honouring Retry-After", async () => {
    const waits: number[] = [];
    const { fetcher, calls } = answering(
      json(503, {}, { "retry-after": "2" }),
      new TypeError("network"),
      json(200, { ui_plugins: [], modules: [{ name: "core", version: "0.4.0" }] }),
    );
    const list = await readPlugins(fetcher, (ms) => {
      waits.push(ms);
      return Promise.resolve();
    });
    expect(list).toEqual([]);
    expect(calls).toHaveLength(3);
    expect(waits[0]).toBe(2_000);
    expect(useServerVersion().value).toBe("0.4.0");
  });

  it("gives up after its attempts, and takes a body without plugins for a failure", async () => {
    const answers = Array.from({ length: CAPABILITY_ATTEMPTS }, () => json(503, {}));
    expect(await readPlugins(answering(...answers).fetcher, noWait)).toBeNull();
    expect(await readPlugins(answering(json(200, { modules: [] })).fetcher, noWait)).toBeNull();
  });

  it("says the plugins unknown when the capabilities cannot be read, then read", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    expect(useCapabilitiesRead().value).toBe(false);
    await startPlugins(["fr"], () => {}, answering(json(500, {})).fetcher);
    expect(usePluginsUnavailable().value).toBe(true);
    expect(useCapabilitiesRead().value).toBe(true);
    await startPlugins(["fr"], () => {}, answering(json(200, { ui_plugins: [] })).fetcher);
    expect(usePluginsUnavailable().value).toBe(false);
  });

  it("sets aside, before fetching anything, a plugin whose ranges the shell misses", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    const merged: string[] = [];
    const fetching = vi.spyOn(globalThis, "fetch");
    await loadPlugins(
      [
        plugin("old", [], { sdk: "^9.0", catalogs: { fr: "i18n/fr.json" } }),
        plugin("ui3", [], { ui: "^3.0.0" }),
        plugin("vue2", [], { framework: "^2.7.0" }),
      ],
      ["fr"],
      (_, name) => merged.push(name),
    );
    expect(usePlugins().value.map((p) => p.status)).toEqual([
      { state: "rejected", reason: "incompatible", detail: "sdk" },
      { state: "rejected", reason: "incompatible", detail: "ui" },
      { state: "rejected", reason: "incompatible", detail: "framework" },
    ]);
    expect(merged).toEqual([]);
    expect(fetching).not.toHaveBeenCalled();
    expect(SHARED.sdk).toBe("1.0.0");
  });

  it("marks the plugins fetched once each is past its fetches, or the capabilities unread", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    performance.clearMarks(PLUGINS_FETCHED);
    const marks = () => performance.getEntriesByName(PLUGINS_FETCHED, "mark").length;
    let answer!: (response: Response) => void;
    vi.spyOn(globalThis, "fetch").mockReturnValue(new Promise((done) => (answer = done)));
    const loading = loadPlugins(
      [
        plugin("ui3", [], { ui: "^3.0.0" }),
        plugin("words", [], { catalogs: { fr: "i18n/fr.json" } }),
      ],
      ["fr"],
      () => {},
    );
    // `ui3` set aside at once; `words` still fetches its catalog.
    await nextTick();
    expect(marks()).toBe(0);
    answer(json(404, {}));
    await loading;
    expect(usePlugins().value.map((p) => p.status)).toEqual([
      { state: "rejected", reason: "incompatible", detail: "ui" },
      { state: "rejected", reason: "load-failed", detail: "i18n/fr.json" },
    ]);
    expect(marks()).toBe(1);
    await loadPlugins([], ["fr"], () => {});
    expect(marks()).toBe(2);
    await startPlugins(["fr"], () => {}, answering(json(500, {})).fetcher);
    expect(marks()).toBe(3);
  });

  it("sets a plugin aside once every module it preloads has settled", async () => {
    const preloading = preloadModules({
      base: "/plugins/pair/1.0.0/",
      integrity: { "a.js": "sha384-a", "b.js": "sha384-b" },
    });
    let rejected = false;
    preloading.catch(() => (rejected = true));
    const [a, b] = document.head.querySelectorAll<HTMLLinkElement>(
      'link[rel="modulepreload"][href^="/plugins/pair/"]',
    );
    a!.dispatchEvent(new Event("error"));
    await nextTick();
    // `b` still loads: the plugin is not set aside before it is done.
    expect(rejected).toBe(false);
    b!.dispatchEvent(new Event("load"));
    await expect(preloading).rejects.toThrow("/plugins/pair/1.0.0/a.js");
  });

  it("loads once a session is there, in an idle moment, never twice", () => {
    const idle: (() => void)[] = [];
    beginPlugins((start) => idle.push(start));
    expect(idle).toHaveLength(0);
    configurePlugins(["fr"], () => {});
    beginPlugins((start) => idle.push(start));
    beginPlugins((start) => idle.push(start));
    expect(idle).toHaveLength(1);
  });
});

describe("catalogs of the plugins (W11)", () => {
  it("keeps a plugin's words apart from the shell's, and routes its keys there", async () => {
    const i18n = createShellI18n("fr");
    mergePluginCatalog(i18n, "fr", "app", { name: "Usurpé", nav: { home: "Maison" } });
    expect(i18n.global.t("app.name")).toBe("kartenn");
    // Routed for the plugin only, never for a key of the shell or of the SDK.
    expect(shellKey("app.nav.home", "app")).toBe("plugin.app.nav.home");
    expect(shellKey("app.nav.home")).toBe("app.nav.home");
    expect(shellKey("labels.state.ok", "app")).toBe("labels.state.ok");
    // The `t` of the plugin `app` reads its own catalog; a key it lacks stays as written.
    const announced = announcePlugin({ name: "app", extensions: [] });
    definePlugin({ name: "app", elements: {} });
    const handle = await announced;
    expect(handle.t("nav.home")).toBe("Maison");
    expect(handle.t("absent")).toBe("app.absent");
  });

  it("lets a plugin named labels give nothing to the labels of the shell", async () => {
    const i18n = createShellI18n("fr");
    mergePluginCatalog(i18n, "fr", "labels", { state: { ok: "Usurpé" } });
    // The SDK's fallback of a label (`labels.<space>.<key>`, no plugin) is not routed.
    expect(shellKey("labels.state.ok")).toBe("labels.state.ok");
    expect(i18n.global.t("labels.state.ok")).toBe("labels.state.ok");
    vi.spyOn(globalThis, "fetch").mockRejectedValue(new TypeError("offline"));
    expect(label("state", "ok")).toBe("ok");
    const announced = announcePlugin({ name: "labels", extensions: [] });
    definePlugin({ name: "labels", elements: {} });
    expect((await announced).t("state.ok")).toBe("Usurpé");
  });
});

describe("an extension in its place (REQ-021-13)", () => {
  let app: App | null = null;
  afterEach(() => {
    app?.unmount();
    app = null;
    document.body.replaceChildren();
  });

  function mount(
    props: { tag: string; object?: ObjectRef; status?: Status },
    onClose = () => {},
  ): HTMLElement {
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({
      render: () => h(ExtensionElement, { plugin: "notes", label: "Notes", onClose, ...props }),
    });
    app.use(createShellI18n("fr"));
    app.mount(root);
    return root;
  }

  it("creates the element of the plugin with its object, and passes its close on", async () => {
    class Tab extends HTMLElement {
      object?: { id: string };
      connectedCallback() {
        this.textContent = `onglet de ${this.object?.id}`;
      }
    }
    customElements.define("notes-tab-ok", Tab);
    const closed = vi.fn();
    const root = mount(
      { tag: "notes-tab-ok", object: { type: "prefix", id: "p1", href: "/v1/prefixes/p1" } },
      closed,
    );
    await nextTick();
    const element = root.querySelector("notes-tab-ok")!;
    expect(element.textContent).toBe("onglet de p1");
    element.dispatchEvent(new Event("kartenn-close", { bubbles: true }));
    expect(closed).toHaveBeenCalledOnce();
  });

  it("confines an element that throws to its place, with its code", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    // A browser reports the exception (an `error` of the window), happy-dom throws it: the
    // host takes both (the journey `plugins` sees the browser's way).
    customElements.define(
      "notes-tab-throws",
      class extends HTMLElement {
        constructor() {
          super();
          throw new Error("boom");
        }
      },
    );
    const root = mount({ tag: "notes-tab-throws" });
    await nextTick();
    await nextTick();
    expect(root.querySelector("notes-tab-throws")).toBeNull();
    expect(root.textContent).toContain("« Notes » n'a pas pu s'afficher");
    expect(root.textContent).toContain(reasonCode("element"));
  });

  it("says an element never defined, a plugin set aside, a plugin loading", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    let root = mount({ tag: "notes-never" });
    await nextTick();
    expect(root.textContent).toContain("plugin.element");
    app!.unmount();
    root = mount({ tag: "notes-x", status: { state: "rejected", reason: "timeout" } });
    await nextTick();
    expect(root.textContent).toContain("aucune extension définie dans le délai");
    expect(root.textContent).toContain("plugin.timeout");
    app!.unmount();
    root = mount({ tag: "notes-x", status: { state: "loading" } });
    await nextTick();
    expect(root.querySelector("[aria-busy='true']")?.textContent).toContain("Chargement de Notes");
    expect(root.querySelector("notes-x")).toBeNull();
  });
});
