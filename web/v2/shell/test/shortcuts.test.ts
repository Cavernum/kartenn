// @vitest-environment happy-dom
import { shortcuts as sdkShortcuts } from "@kartenn/sdk";
import { reserveKeys } from "@kartenn/sdk/host";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { effectScope } from "vue";
import { Rights } from "../src/rights/index.ts";
import { helpRows } from "../src/shortcuts/help.ts";
import { paletteGroups, pluginEntries, type PaletteEntry } from "../src/shortcuts/palette.ts";
import {
  forgetRecent,
  RECENT_KEPT,
  recentObjects,
  rememberObject,
  type FoundObject,
} from "../src/shortcuts/recent.ts";
import {
  createKeyListener,
  createShortcutRegistry,
  isSingleKey,
  keysOf,
  SEQUENCE_WAIT,
  takesCharacters,
  type ShellShortcut,
} from "../src/shortcuts/registry.ts";
import { looksLikeValue, pageOf, readableType, searchObjects } from "../src/shortcuts/search.ts";
import {
  navigationTarget,
  pluginShortcut,
  SHELL_KEYS,
  shellShortcuts,
  type ShellActions,
} from "../src/shortcuts/shell.ts";
import { offered, play, runnable, setViewAction, useViewAction } from "../src/shortcuts/view.ts";
import { timeOf, writesOf } from "../src/time/index.ts";

/** The gate of writes at the period of `query`, in a wide frame (what `play` and `runnable` take). */
const writesAt = (query: Record<string, string>) => writesOf(timeOf(query), "wide");

// The shortcuts of the shell (REQ-021-3, 7, 8; design « Raccourcis »): the registry and its
// conflicts, the one listener (sequences, fields, consumers of characters, layers,
// modifiers, the single-key setting), the shell's set by rights, the actions of the view,
// the palette's groups and commands, the search by value and by name, the recent objects.

const READER = new Rights(["core.prefix.read", "core.address.read", "core.changeset.read"]);
const ADMIN = new Rights(["*"]);

const shortcut = (id: string, keys: ShellShortcut["keys"], run = vi.fn()): ShellShortcut => ({
  id,
  keys,
  label: () => id,
  where: "outsideField",
  scope: "global",
  run,
});

function press(
  key: string,
  init: KeyboardEventInit & { capsLock?: boolean; altGraph?: boolean } = {},
  target: EventTarget = document.body,
): KeyboardEvent {
  const { capsLock = false, altGraph = false, ...rest } = init;
  const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...rest });
  // happy-dom knows neither lock: the states a browser gives.
  const state = event.getModifierState.bind(event);
  event.getModifierState = (name: string) =>
    name === "CapsLock" ? capsLock : name === "AltGraph" ? altGraph : state(name);
  target.dispatchEvent(event);
  return event;
}

describe("registry", () => {
  it("writes the keys of a combination and of a sequence once", () => {
    expect(keysOf(["Mod", "k"])).toBe("Mod+k");
    expect(keysOf([["g"], ["c"]])).toBe("g c");
    expect(isSingleKey([["g"], ["c"]])).toBe(true);
    expect(isSingleKey(["?"])).toBe(true);
    expect(isSingleKey(["Mod", "k"])).toBe(false);
  });

  it("refuses the same keys twice, and a key alone that starts a sequence", () => {
    const registry = createShortcutRegistry();
    registry.register(shortcut("shell.go.changesets", [["g"], ["c"]]));
    expect(() => registry.register(shortcut("plugin.go", [["g"], ["c"]]))).toThrow(/taken by/);
    expect(() => registry.register(shortcut("plugin.g", ["g"]))).toThrow(/taken by/);
    expect(() => registry.register(shortcut("shell.go.changesets", ["x"]))).toThrow(/already/);
    // A letter left free for a plugin's manifest is taken once.
    const remove = registry.register(shortcut("machines.go", [["g"], ["m"]]));
    expect(registry.list().map((s) => s.id)).toEqual(["shell.go.changesets", "machines.go"]);
    remove();
    expect(registry.list().map((s) => s.id)).toEqual(["shell.go.changesets"]);
  });

  it("registers the whole set of the shell without a conflict", () => {
    const registry = createShortcutRegistry();
    for (const s of shellShortcuts(actions())) registry.register(s);
    const keys = registry.list().map((s) => keysOf(s.keys));
    expect(keys).toEqual(
      expect.arrayContaining(["Mod+k", "/", "?", "g c", "g x", "c", "e", "[", "]"]),
    );
    expect(keys).toContain("Escape Escape");
  });

  it("refuses the keys of the shell to the plugins' shortcuts", () => {
    reserveKeys(SHELL_KEYS);
    for (const key of SHELL_KEYS) {
      expect(() =>
        sdkShortcuts.register({ id: `p.${key}`, key, label: () => key, run() {} }),
      ).toThrow(/key of the shell/);
    }
    const remove = sdkShortcuts.register({ id: "p.n", key: "n", label: () => "n", run() {} });
    remove();
  });
});

describe("one listener", () => {
  let now = 0;
  let single = true;
  let list: ShellShortcut[] = [];
  let listener: (event: KeyboardEvent) => void;
  beforeEach(() => {
    now = 1_000;
    single = true;
    list = [];
    listener = createKeyListener({
      shortcuts: () => list,
      singleKeys: () => single,
      now: () => now,
    });
    document.addEventListener("keydown", listener);
  });
  afterEach(() => {
    document.removeEventListener("keydown", listener);
    document.body.replaceChildren();
  });

  it("plays `g` then `c` within 1.5 s, not later", () => {
    const go = vi.fn();
    list = [shortcut("go", [["g"], ["c"]], go)];
    expect(press("g").defaultPrevented).toBe(true);
    now += SEQUENCE_WAIT;
    expect(press("c").defaultPrevented).toBe(true);
    expect(go).toHaveBeenCalledTimes(1);
    press("g");
    now += SEQUENCE_WAIT + 1;
    press("c");
    expect(go).toHaveBeenCalledTimes(1);
  });

  it("starts afresh on a key that continues no sequence", () => {
    const go = vi.fn();
    const help = vi.fn();
    list = [shortcut("go", [["g"], ["c"]], go), shortcut("help", ["?"], help)];
    press("g");
    press("?", { shiftKey: true });
    expect(help).toHaveBeenCalledTimes(1);
    press("c");
    expect(go).not.toHaveBeenCalled();
  });

  it("does nothing in a field, nor in a widget that consumes the characters, nor in a dialog", () => {
    const go = vi.fn();
    list = [shortcut("go", [["g"], ["c"]], go)];
    const field = document.createElement("input");
    const tree = document.createElement("div");
    tree.setAttribute("data-k-typeahead", "");
    const item = document.createElement("span");
    tree.append(item);
    const menu = document.createElement("div");
    menu.setAttribute("role", "menu");
    const entry = document.createElement("div");
    entry.setAttribute("role", "menuitem");
    menu.append(entry);
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    const button = document.createElement("button");
    dialog.append(button);
    document.body.append(field, tree, menu, dialog);
    for (const target of [field, item, entry, button]) {
      press("g", {}, target);
      press("c", {}, target);
    }
    expect(go).not.toHaveBeenCalled();
    // « gc » typed in a field: the second key ends nothing in the page either.
    press("g", {}, field);
    press("c");
    expect(go).not.toHaveBeenCalled();
  });

  it("knows the fields and the consumers of characters", () => {
    const check = document.createElement("input");
    check.type = "checkbox";
    const area = document.createElement("textarea");
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "true");
    const box = document.createElement("div");
    box.setAttribute("role", "combobox");
    expect(takesCharacters(check)).toBe(false);
    expect(takesCharacters(area)).toBe(true);
    expect(takesCharacters(box)).toBe(true);
    document.body.append(editable);
    expect(takesCharacters(editable)).toBe(true);
    expect(takesCharacters(document.body)).toBe(false);
  });

  it("ignores Ctrl, Alt and Méta with a single key, an event handled, a composition, a key held", () => {
    const create = vi.fn();
    list = [shortcut("create", ["c"], create)];
    for (const init of [
      { ctrlKey: true },
      { altKey: true },
      { metaKey: true },
      { isComposing: true },
      { repeat: true },
    ]) {
      press("c", init);
      expect(create, JSON.stringify(init)).not.toHaveBeenCalled();
    }
    // Handled on its way up (a component's own key).
    const field = document.createElement("div");
    field.addEventListener("keydown", (event) => event.preventDefault());
    document.body.append(field);
    press("c", {}, field);
    expect(create).not.toHaveBeenCalled();
    press("c");
    expect(create).toHaveBeenCalledTimes(1);
  });

  it("plays Ctrl+K and ⌘K everywhere, in a field too", () => {
    const palette = vi.fn();
    list = [shortcut("palette", ["Mod", "k"], palette)];
    const field = document.createElement("input");
    document.body.append(field);
    expect(press("k", { ctrlKey: true }, field).defaultPrevented).toBe(true);
    press("k", { metaKey: true });
    // Ctrl+Maj+K is another combination (a browser's console).
    press("K", { ctrlKey: true, shiftKey: true });
    press("k", { ctrlKey: true, altKey: true });
    press("k");
    expect(palette).toHaveBeenCalledTimes(2);
  });

  it("plays a letter typed with Caps Lock as without it", () => {
    const go = vi.fn();
    const help = vi.fn();
    list = [shortcut("go", [["g"], ["c"]], go), shortcut("help", ["?"], help)];
    press("G", { capsLock: true });
    press("C", { capsLock: true });
    expect(go).toHaveBeenCalledTimes(1);
    // Caps Lock and Shift: the capital, which no shortcut of the shell takes.
    press("g", { capsLock: true, shiftKey: true });
    press("c", { capsLock: true, shiftKey: true });
    expect(go).toHaveBeenCalledTimes(1);
    press("?", { capsLock: true, shiftKey: true });
    expect(help).toHaveBeenCalledTimes(1);
  });

  it("plays no single key with Alt, nor AltGr (no modifier but Shift)", () => {
    const sidebar = vi.fn();
    list = [shortcut("sidebar", ["["], sidebar)];
    press("[", { altKey: true });
    press("[", { ctrlKey: true, altKey: true, altGraph: true });
    expect(sidebar).not.toHaveBeenCalled();
    press("[");
    expect(sidebar).toHaveBeenCalledTimes(1);
  });

  it("ends a sequence on any other key, not on Shift", () => {
    const go = vi.fn();
    list = [shortcut("go", [["g"], ["C"]], go)];
    press("g");
    press("Tab");
    press("C", { shiftKey: true });
    expect(go).not.toHaveBeenCalled();
    press("g");
    press("Shift", { shiftKey: true });
    press("C", { shiftKey: true });
    expect(go).toHaveBeenCalledTimes(1);
  });

  it("with the single keys off, keeps Ctrl+K only", () => {
    const go = vi.fn();
    const palette = vi.fn();
    list = [shortcut("go", [["g"], ["c"]], go), shortcut("palette", ["Mod", "k"], palette)];
    single = false;
    press("g");
    press("c");
    expect(go).not.toHaveBeenCalled();
    press("k", { ctrlKey: true });
    expect(palette).toHaveBeenCalledTimes(1);
  });

  it("leaves the key to the page when the shortcut cannot act, or is hidden", () => {
    list = [
      shortcut(
        "create",
        ["c"],
        vi.fn(() => false),
      ),
      { ...shortcut("edit", ["e"]), shown: () => false },
    ];
    expect(press("c").defaultPrevented).toBe(false);
    expect(press("e").defaultPrevented).toBe(false);
    expect(list[1]!.run).not.toHaveBeenCalled();
  });

  it("plays a plugin's shortcut of its view", () => {
    const run = vi.fn();
    list = [pluginShortcut({ id: "racks.new", key: "n", label: () => "Nouveau", run })];
    press("n");
    expect(run).toHaveBeenCalledTimes(1);
    expect(list[0]).toMatchObject({ where: "plugin", scope: "view" });
  });
});

function actions(over: Partial<ShellActions> = {}): ShellActions {
  return {
    t: (key) => key,
    rights: () => READER,
    navigate: vi.fn(),
    palette: vi.fn(),
    help: vi.fn(),
    filter: vi.fn(() => false),
    create: vi.fn(() => false),
    edit: vi.fn(() => false),
    sidebar: vi.fn(),
    panel: vi.fn(),
    ...over,
  };
}

describe("the shortcuts of the shell", () => {
  it("leads `g` + letter to the entries the rights show", () => {
    expect(navigationTarget("changesets", READER)).toBe("/changesets");
    expect(navigationTarget("prefixes", READER)).toBe("/prefixes");
    expect(navigationTarget("domains", READER)).toBeNull();
    expect(navigationTarget("admin", READER)).toBeNull();
    expect(navigationTarget("admin", ADMIN)).toBe("/admin");
  });

  it("hides from a reader the navigation out of their rights, and navigates", () => {
    const navigate = vi.fn();
    const set = shellShortcuts(actions({ navigate }));
    const byId = new Map(set.map((s) => [s.id, s]));
    expect(byId.get("shell.go.admin")!.shown!()).toBe(false);
    expect(byId.get("shell.go.changesets")!.shown!()).toBe(true);
    byId.get("shell.go.changesets")!.run!();
    expect(navigate).toHaveBeenCalledWith("/changesets");
    expect(byId.get("shell.go.admin")!.run!()).toBe(false);
  });

  it("lists the keys the components handle, for the help only", () => {
    const set = shellShortcuts(actions());
    const listed = set.filter((s) => !s.run).map((s) => s.id);
    expect(listed).toEqual([
      "shell.apply",
      "shell.escape",
      "shell.filterEscape",
      "shell.columnFilter",
    ]);
  });
});

describe("help", () => {
  const t = (key: string, named?: Record<string, unknown>) =>
    named ? `${key}(${String(named.action)})` : key;

  it("marks off in words the single keys the setting turns off, not Échap nor Ctrl+K", () => {
    const set = shellShortcuts(actions({ rights: () => ADMIN }));
    const on = new Map(helpRows(set, true, t).map((r) => [r.id, r.action]));
    const off = new Map(helpRows(set, false, t).map((r) => [r.id, r.action]));
    expect(on.get("shell.go.changesets")).toBe("shortcuts.go.changesets");
    expect(off.get("shell.go.changesets")).toBe("shortcuts.off(shortcuts.go.changesets)");
    expect(off.get("shell.help")).toBe("shortcuts.off(shortcuts.help)");
    expect(off.get("shell.palette")).toBe("shortcuts.palette");
    expect(off.get("shell.escape")).toBe("shortcuts.escape");
    expect(off.get("shell.filterEscape")).toBe("shortcuts.filterEscape");
  });

  it("lists only what the rights show", () => {
    const ids = helpRows(shellShortcuts(actions()), true, t).map((r) => r.id);
    expect(ids).toContain("shell.go.changesets");
    expect(ids).not.toContain("shell.go.admin");
  });

  it("lists `[` and `]` only in a frame with its sidebar and panel (task 16i)", () => {
    const rows = (framed: boolean) =>
      helpRows(shellShortcuts({ ...actions(), framed: () => framed }), true, t).map((r) => r.id);
    expect(rows(true)).toEqual(expect.arrayContaining(["shell.sidebar", "shell.panel"]));
    expect(rows(false)).not.toContain("shell.sidebar");
    expect(rows(false)).not.toContain("shell.panel");
    expect(rows(false)).toContain("shell.help");
  });
});

describe("actions of the view", () => {
  it("plays `c` and `e` at the present only, when allowed; `/` at any date", () => {
    const present = writesAt({});
    const past = writesAt({ as_of: "2026-01-01T00:00:00Z" });
    const scope = effectScope();
    let allowed = true;
    scope.run(() => {
      useViewAction("create", { label: () => "Créer", run: vi.fn(), allowed: () => allowed });
      useViewAction("filter", { label: () => "Filtrer", run: vi.fn() });
    });
    expect(runnable("create", present)).not.toBeNull();
    expect(runnable("create", past)).toBeNull();
    expect(runnable("filter", past)).not.toBeNull();
    expect(runnable("edit", present)).toBeNull();
    allowed = false;
    expect(runnable("create", present)).toBeNull();
    scope.stop();
    expect(runnable("filter", present)).toBeNull();
  });

  it("plays a `dated` action under a validity date alone, never under `as_of`", () => {
    const scope = effectScope();
    scope.run(() => {
      useViewAction("edit", { label: () => "Modifier", run: vi.fn(), dated: true });
      useViewAction("create", { label: () => "Créer", run: vi.fn() });
    });
    const valid_at = "2026-01-01T00:00:00Z";
    const as_of = "2026-06-30T12:00:00Z";
    expect(runnable("edit", writesAt({ valid_at }))).not.toBeNull();
    expect(runnable("create", writesAt({ valid_at }))).toBeNull();
    expect(runnable("edit", writesAt({ valid_at, as_of }))).toBeNull();
    expect(runnable("edit", writesAt({ as_of }))).toBeNull();
    scope.stop();
  });

  it("says why a refused `c` or `e` does nothing, by the status of the view (task 12k)", () => {
    const said: string[] = [];
    const say = (text: string) => said.push(text);
    const t = (key: string) => `<${key}>`;
    const run = vi.fn();
    let allowed = false;
    let why: string | undefined = "Lecture seule : il faut core.prefix.edit";
    const scope = effectScope();
    scope.run(() => {
      useViewAction("edit", {
        label: () => "Modifier",
        run,
        allowed: () => allowed,
        why: () => why,
        say,
        dated: true,
      });
      useViewAction("create", { label: () => "Créer", run, say });
      useViewAction("filter", { label: () => "Filtrer", run, allowed: () => false });
    });
    const present = writesAt({});
    const as_of = "2026-06-30T12:00:00Z";
    const valid_at = "2026-01-01T00:00:00Z";
    // Without the right: its permission said, the key taken, nothing run.
    expect(play("edit", present, t)).toBe(true);
    expect(said).toEqual(["Lecture seule : il faut core.prefix.edit"]);
    // Under `as_of`: off the present before the right (as the cells say it).
    expect(play("edit", writesAt({ as_of }), t)).toBe(true);
    expect(said.at(-1)).toBe("<edit.why.past>");
    // Nothing to act on (no row, no node): nothing said, the key let go, at any date.
    why = undefined;
    expect(play("edit", present, t)).toBe(false);
    expect(play("edit", writesAt({ as_of }), t)).toBe(false);
    expect(said).toHaveLength(2);
    // Allowed: under `valid_at` alone a `dated` action runs, another says the present.
    allowed = true;
    expect(play("edit", writesAt({ valid_at }), t)).toBe(true);
    expect(run).toHaveBeenCalledTimes(1);
    expect(play("create", writesAt({ valid_at }), t)).toBe(true);
    expect(said.at(-1)).toBe("<edit.why.past>");
    expect(play("create", present, t)).toBe(true);
    expect(run).toHaveBeenCalledTimes(2);
    // A view without a status, or without the action: the key let go.
    expect(play("filter", present, t)).toBe(false);
    scope.stop();
    expect(play("edit", present, t)).toBe(false);
    expect(run).toHaveBeenCalledTimes(2);
  });

  it("says the lasting reason of a `lasting` action before the gate (task 021-21e)", () => {
    const said: string[] = [];
    const t = (key: string) => `<${key}>`;
    let allowed = false;
    const scope = effectScope();
    scope.run(() =>
      useViewAction("edit", {
        label: () => "Modifier",
        run: vi.fn(),
        allowed: () => allowed,
        why: () => "Il faut core.rbac.admin",
        say: (text) => said.push(text),
        lasting: true,
      }),
    );
    const narrow = writesOf(timeOf({}), "narrow");
    // The right lacking: said at any width and date, as its button says it.
    expect(play("edit", narrow, t)).toBe(true);
    expect(play("edit", writesAt({ as_of: "2026-06-30T12:00:00Z" }), t)).toBe(true);
    expect(said).toEqual(["Il faut core.rbac.admin", "Il faut core.rbac.admin"]);
    // The right held: the gate says why.
    allowed = true;
    expect(play("edit", narrow, t)).toBe(true);
    expect(said.at(-1)).toBe("<edit.why.narrow>");
    scope.stop();
  });

  it("keeps the action of the view that came after", () => {
    const first = { label: () => "a", run: vi.fn() };
    const second = { label: () => "b", run: vi.fn() };
    const removeFirst = setViewAction("edit", first);
    const removeSecond = setViewAction("edit", second);
    const present = writesAt({});
    removeFirst();
    expect(runnable("edit", present)).toBe(second);
    removeSecond();
    expect(runnable("edit", present)).toBeNull();
  });

  it("refuses `c` and `e` in a frame under 1024 px, the width said, the palette keeping the command (task 021-21c)", () => {
    const said: string[] = [];
    const t = (key: string) => `<${key}>`;
    const run = vi.fn();
    let allowed = true;
    const scope = effectScope();
    scope.run(() => {
      useViewAction("edit", {
        label: () => "Modifier",
        run,
        allowed: () => allowed,
        why: () => "Lecture seule : il faut core.prefix.edit",
        say: (text) => said.push(text),
        dated: true,
      });
      useViewAction("filter", { label: () => "Filtrer", run });
    });
    const narrow = writesOf(timeOf({}), "narrow");
    const compact = writesOf(timeOf({ valid_at: "2026-01-01T00:00:00Z" }), "compact");
    const asOf = writesOf(timeOf({ as_of: "2026-06-30T12:00:00Z" }), "narrow");
    // The key: taken, the width said, nothing run; `/` still filters.
    expect(play("edit", narrow, t)).toBe(true);
    expect(play("edit", compact, t)).toBe(true);
    expect(said).toEqual(["<edit.why.narrow>", "<edit.why.narrow>"]);
    expect(run).not.toHaveBeenCalled();
    expect(runnable("edit", narrow)).toBeNull();
    expect(play("filter", narrow, t)).toBe(true);
    expect(run).toHaveBeenCalledTimes(1);
    // The past before the width: widening would not open a date gone.
    expect(play("edit", asOf, t)).toBe(true);
    expect(said.at(-1)).toBe("<edit.why.past>");
    // Without the right too, the width first, as the cells say it.
    allowed = false;
    expect(play("edit", narrow, t)).toBe(true);
    expect(said.at(-1)).toBe("<edit.why.narrow>");
    // The palette: the command kept under the width alone, its run says why; none without
    // the right, nor under `as_of` (as before).
    const now = timeOf({});
    expect(offered("edit", now, "narrow", t)).toBeNull();
    allowed = true;
    const command = offered("edit", now, "narrow", t);
    expect(command?.label()).toBe("Modifier");
    command!.run();
    expect(said.at(-1)).toBe("<edit.why.narrow>");
    expect(run).toHaveBeenCalledTimes(1);
    expect(offered("edit", timeOf({ as_of: "2026-06-30T12:00:00Z" }), "narrow", t)).toBeNull();
    // At 1024 px, the action again.
    expect(offered("edit", now, "standard", t)?.run).toBe(run);
    scope.stop();
  });

  it("keeps a command in a narrow frame only when the period lets it run (task 021-21c)", () => {
    const t = (key: string) => `<${key}>`;
    const scope = effectScope();
    scope.run(() => {
      useViewAction("edit", { label: () => "Modifier", run: vi.fn(), say: vi.fn(), dated: true });
      useViewAction("create", { label: () => "Créer", run: vi.fn(), say: vi.fn() });
    });
    const dated = timeOf({ valid_at: "2026-01-01T00:00:00Z" });
    // Under `valid_at` alone: a `dated` action is held by the width only, kept; another is
    // held by the period too, absent, as it is at any width.
    expect(offered("edit", dated, "narrow", t)?.label()).toBe("Modifier");
    expect(offered("create", dated, "narrow", t)).toBeNull();
    expect(offered("create", dated, "wide", t)).toBeNull();
    scope.stop();
  });
});

const PREFIX: FoundObject = {
  type: "prefix",
  id: "p1",
  value: "192.0.2.0/24",
  context: "production",
  icon: "prefix",
  to: "/prefixes/p1",
};
const entry = (id: string, label: string, keywords?: string[]): PaletteEntry => ({
  id,
  label,
  icon: "search",
  ...(keywords ? { keywords } : {}),
  run: vi.fn(),
});
const LABELS = {
  objects: "Objets",
  recent: "Récents",
  commands: "Commandes",
  navigation: "Navigation",
};
const typeLabel = (type: string) => ({ prefix: "Préfixe", vlan: "VLAN" })[type] ?? type;

describe("palette", () => {
  it("offers the recent objects, the commands and the pages without a text", () => {
    const groups = paletteGroups({
      query: "",
      objects: [],
      recent: [PREFIX],
      commands: [entry("cmd:help", "Raccourcis clavier")],
      pages: [entry("page:prefixes", "Préfixes", ["Référentiel"])],
      labels: LABELS,
      typeLabel,
    });
    expect(groups.map((g) => g.label)).toEqual(["Récents", "Commandes", "Navigation"]);
    // The type said in words in the name of the option, the value in fixed pitch after it.
    expect(groups[0]!.items[0]).toMatchObject({
      id: "object:prefix:p1",
      label: "Préfixe · production",
      detail: "192.0.2.0/24",
    });
  });

  it("puts the objects found first, then what holds every word typed", () => {
    const groups = paletteGroups({
      query: "  réf pré ",
      objects: [PREFIX],
      recent: [PREFIX],
      commands: [entry("cmd:help", "Raccourcis clavier")],
      pages: [
        entry("page:prefixes", "Préfixes", ["Référentiel"]),
        entry("page:vlans", "VLAN", ["Réseau"]),
      ],
      labels: LABELS,
      typeLabel,
    });
    expect(groups.map((g) => g.label)).toEqual(["Objets", "Navigation"]);
    expect(groups[1]!.items.map((i) => i.id)).toEqual(["page:prefixes"]);
  });

  it("leaves out a plugin's command without its permission", () => {
    const run = vi.fn();
    const command = {
      id: "vlans.createGroup",
      label: () => "Créer un groupe de VLAN",
      permission: "core.vlan_group.edit",
      run,
    };
    expect(pluginEntries([command], (p) => READER.has(p))).toEqual([]);
    const entries = pluginEntries([command], (p) => ADMIN.has(p));
    expect(entries.map((e) => e.label)).toEqual(["Créer un groupe de VLAN"]);
    entries[0]!.run();
    expect(run).toHaveBeenCalledTimes(1);
  });
});

describe("search by value and by name", () => {
  it("knows the start of a value", () => {
    for (const text of ["10.1.", "192.0.2.0/24", "2001:db8::", "2001"]) {
      expect(looksLikeValue(text)).toBe(true);
    }
    for (const text of ["cafe", "salle-a", "production"]) expect(looksLikeValue(text)).toBe(false);
    expect(pageOf("/v1/prefixes/p1")).toBe("/prefixes/p1");
    expect(pageOf("https://elsewhere/p1")).toBeNull();
  });

  it("searches the prefixes and the addresses by value, in the collections read", async () => {
    const read = vi.fn(async (path: string) =>
      path === "/v1/prefixes"
        ? [
            {
              id: "p1",
              value: "192.0.2.0/24",
              domain: { slug: "production" },
              links: { self: "/v1/prefixes/p1" },
            },
          ]
        : [],
    );
    const signal = new AbortController().signal;
    const found = await searchObjects(" 192.0.2. ", (p) => READER.has(p), signal, read);
    expect(read.mock.calls.map((c) => c[0])).toEqual(["/v1/prefixes", "/v1/addresses"]);
    expect(read).toHaveBeenCalledWith("/v1/prefixes", "192.0.2.", signal);
    expect(found).toEqual([PREFIX]);
  });

  it("knows the collections a user reads", () => {
    expect(readableType("prefix", (p) => READER.has(p))).toBe(true);
    expect(readableType("vlan", (p) => READER.has(p))).toBe(false);
    expect(readableType("machine", (p) => ADMIN.has(p))).toBe(false);
  });

  it("searches the other collections by name, leaves a failed one out", async () => {
    const read = vi.fn(async (path: string) => {
      if (path === "/v1/sites") throw new Error("refused");
      return path === "/v1/vlans"
        ? [{ id: "v1", name: "supervision", vid: 10, href: "/v1/vlans/v1" }]
        : [];
    });
    const signal = new AbortController().signal;
    const found = await searchObjects("sup", (p) => ADMIN.has(p), signal, read);
    expect(read.mock.calls.map((c) => c[0])).not.toContain("/v1/prefixes");
    expect(found).toEqual([
      {
        type: "vlan",
        id: "v1",
        value: "VID 10",
        context: "supervision",
        icon: "vlan",
        to: "/vlans/v1",
      },
    ]);
    expect(await searchObjects("   ", () => true, signal, read)).toEqual([]);
  });
});

describe("recent objects", () => {
  afterEach(forgetRecent);
  const ALICE = "0b5e0000-0000-4000-8000-00000000a11c";
  const BOB = "0b5e0000-0000-4000-8000-000000000b0b";
  const all = () => true;

  it("keeps the latest first, once, and so many", () => {
    rememberObject(ALICE, PREFIX);
    for (let i = 0; i < RECENT_KEPT; i++) {
      rememberObject(ALICE, { ...PREFIX, id: `a${i}`, type: "address" });
    }
    rememberObject(ALICE, PREFIX);
    const recent = recentObjects(ALICE, all);
    expect(recent).toHaveLength(RECENT_KEPT);
    expect(recent[0]).toEqual(PREFIX);
    expect(recent.filter((o) => o.id === "p1")).toHaveLength(1);
    localStorage.setItem(`kartenn.recent.objects.${ALICE}`, "{not json");
    expect(recentObjects(ALICE, all)).toEqual([]);
  });

  it("never shows them to another user of the browser, nor signed out", () => {
    rememberObject(ALICE, PREFIX);
    expect(recentObjects(BOB, all)).toEqual([]);
    expect(recentObjects(null, all)).toEqual([]);
    rememberObject(null, PREFIX);
    expect(Object.keys(localStorage).filter((k) => k.startsWith("kartenn.recent"))).toEqual([
      `kartenn.recent.objects.${ALICE}`,
    ]);
    forgetRecent();
    expect(recentObjects(ALICE, all)).toEqual([]);
  });

  it("leaves out the objects of a collection the user may no longer read", () => {
    rememberObject(ALICE, PREFIX);
    rememberObject(ALICE, { ...PREFIX, id: "v1", type: "vlan", icon: "vlan", to: "/vlans/v1" });
    const read = (rights: Rights) => (type: string) => readableType(type, (p) => rights.has(p));
    expect(recentObjects(ALICE, read(ADMIN)).map((o) => o.type)).toEqual(["vlan", "prefix"]);
    expect(recentObjects(ALICE, read(READER)).map((o) => o.type)).toEqual(["prefix"]);
    expect(recentObjects(ALICE, read(new Rights([])))).toEqual([]);
  });
});
