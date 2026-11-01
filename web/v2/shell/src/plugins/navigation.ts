import { CORE_SPACES, SPACE_ORDER, type Space, type SpaceId } from "../frame/spaces.ts";
import type { Rights } from "../rights/index.ts";
import type { ShortcutRegistry } from "../shortcuts/registry.ts";
import { pluginKey, viewPath, type Loaded, type RefusedLetter } from "./registry.ts";

// Where the views of the plugins go in the frame (REQ-021-2, 13; design « API des
// plugins »): an entry of the sidebar under the space their manifest declares (`space`),
// in the order of `ui_plugins`; the `g` letter a view proposes (`go`). Only loaded plugins
// place anything: one set aside leaves no entry that would lead nowhere.

/** The space of a plugin that declares none: the first, the referential. */
export const DEFAULT_SPACE: SpaceId = "referential";

/** The id of the entry of a view of a plugin, apart from the core's (`x.<plugin>.<view>`). */
export function viewEntry(plugin: string, view: string): string {
  return `x.${plugin}.${view}`;
}

/**
 * The spaces of the core with the views of the loaded plugins that have an entry of
 * navigation (`nav`), each after the core's entries of its space; a space only plugins
 * fill (`hosts`, `policy`, `flows`) is added in its place in the order.
 */
export function pluginSpaces(
  loaded: readonly Loaded[],
  core: readonly Space[] = CORE_SPACES,
): Space[] {
  const spaces = new Map<SpaceId, Space>(core.map((space) => [space.id, { ...space }]));
  for (const { plugin, status } of loaded) {
    if (status.state !== "loaded") continue;
    const id = plugin.space ?? DEFAULT_SPACE;
    const space = spaces.get(id) ?? { id, label: `space.${id}`, entries: [] };
    const entries = plugin.extensions
      .filter((e) => e.point === "view" && e.nav)
      .map((e) => ({
        id: viewEntry(plugin.name, e.id),
        label: pluginKey(plugin.name, e.nav!),
        icon: "plug" as const,
        to: viewPath(plugin.name, e.id),
        ...(e.permission ? { anyOf: [e.permission] } : {}),
      }));
    if (entries.length > 0) spaces.set(id, { ...space, entries: [...space.entries, ...entries] });
  }
  return [...spaces.values()].sort((a, b) => SPACE_ORDER.indexOf(a.id) - SPACE_ORDER.indexOf(b.id));
}

/** A `g` letter a view of a plugin proposes. */
export interface PluginLetter {
  plugin: string;
  view: string;
  letter: string;
  /** The key of its label: the `nav` of the view, else its `label`, else its id. */
  label: string;
  to: string;
  permission?: string | undefined;
}

/** What the letters of the plugins act with: the frame gives it. */
export interface LetterActions {
  t: (key: string, params?: Record<string, unknown>) => string;
  rights: () => Rights;
  navigate: (to: string) => void;
}

/** The letters registered: how to remove them, those granted by entry, those refused. */
export interface Letters {
  drop(): void;
  /** The letter of each entry of a view (`viewEntry`), for the palette. */
  granted: ReadonlyMap<string, string>;
  /** The letters refused to each plugin: taken by the shell or a plugin before it. */
  refused: ReadonlyMap<string, readonly RefusedLetter[]>;
}

/**
 * Registers `g` + letter for each view of `letters` in the registry of the frame
 * (`createShortcutRegistry`, which refuses a conflict), in their order: the view's path,
 * shown by its permission. A letter taken is refused, said in the console, and the plugin
 * stays loaded (design « Erreurs et cas limites »).
 */
export function registerLetters(
  registry: ShortcutRegistry,
  letters: readonly PluginLetter[],
  actions: LetterActions,
): Letters {
  const drops: (() => void)[] = [];
  const granted = new Map<string, string>();
  const refused = new Map<string, RefusedLetter[]>();
  for (const asked of letters) {
    const held = () => !asked.permission || actions.rights().has(asked.permission);
    try {
      drops.push(
        registry.register({
          id: `${asked.plugin}.go.${asked.view}`,
          keys: [["g"], [asked.letter]],
          label: () => actions.t("shortcuts.goTo", { name: actions.t(asked.label) }),
          where: "outsideField",
          scope: "global",
          shown: held,
          run: () => {
            if (!held()) return false;
            actions.navigate(asked.to);
          },
        }),
      );
      granted.set(viewEntry(asked.plugin, asked.view), asked.letter);
    } catch (error) {
      console.warn(`kartenn: plugin ${asked.plugin}: ${String(error)}`);
      const own = refused.get(asked.plugin) ?? [];
      refused.set(asked.plugin, [...own, { view: asked.view, letter: asked.letter }]);
    }
  }
  return {
    drop: () => {
      for (const drop of drops) drop();
    },
    granted,
    refused,
  };
}

/** The letters the views of the loaded plugins propose, in the order of `ui_plugins`. */
export function pluginLetters(loaded: readonly Loaded[]): PluginLetter[] {
  return loaded.flatMap(({ plugin, status }) =>
    status.state !== "loaded"
      ? []
      : plugin.extensions
          .filter((e) => e.point === "view" && e.go)
          .map((e) => ({
            plugin: plugin.name,
            view: e.id,
            letter: e.go!,
            label: pluginKey(plugin.name, e.nav ?? e.label ?? e.id),
            to: viewPath(plugin.name, e.id),
            permission: e.permission ?? undefined,
          })),
  );
}
