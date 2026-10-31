import type { Shortcut as PluginShortcut } from "@kartenn/sdk";
import type { Rights } from "../rights/index.ts";
import { CORE_SPACES, visible, type SpaceEntry } from "../frame/spaces.ts";
import type { ShellShortcut } from "./registry.ts";

// The shortcuts of the shell (design 021 « Raccourcis »), in the order of the help: the
// palette, the filter of the view, the help, the navigation `g` + letter, create and
// modify, then the keys the components handle themselves (Ctrl+Entrée, Échap, the two
// Échap of a filter, Alt+Bas on a header), and the sidebar and the panel. Letters chosen on
// the French and English words when they agree, else on the English one (Q-021-6).

/**
 * `g` + letter → an entry of the sidebar (its path), or `/admin` for `x`. `v` is the VLAN's,
 * screens of the core since the 026; `m`, `t`, `q` and `f` stay for the manifests of the
 * plugins (machines, matrix, compliance, flows), which declare them (task 16).
 */
export const NAVIGATION: readonly { letter: string; entry: string }[] = [
  { letter: "h", entry: "home" },
  { letter: "o", entry: "topology" },
  { letter: "d", entry: "domains" },
  { letter: "p", entry: "prefixes" },
  { letter: "r", entry: "ranges" },
  { letter: "a", entry: "addresses" },
  { letter: "e", entry: "entities" },
  { letter: "s", entry: "sites" },
  { letter: "c", entry: "changesets" },
  { letter: "i", entry: "proposals" },
  { letter: "b", entry: "drafts" },
  { letter: "v", entry: "vlans" },
  { letter: "x", entry: "admin" },
];

/** The keys the shell holds alone: no plugin shortcut takes them (`reserveKeys`). */
export const SHELL_KEYS = ["g", "/", "?", "c", "e", "[", "]"] as const;

const ENTRIES = new Map(
  CORE_SPACES.flatMap((space) => space.entries.map((entry) => [entry.id, entry] as const)),
);
const ADMIN = CORE_SPACES.find((space) => space.id === "admin")!;

/** The target of `g` + letter for `rights`: its path, or `null` when hidden from them. */
export function navigationTarget(entry: string, rights: Rights): string | null {
  if (entry === "admin") {
    return ADMIN.entries.some((e) => visible(e, rights)) ? "/admin" : null;
  }
  const found: SpaceEntry | undefined = ENTRIES.get(entry);
  return found && visible(found, rights) ? found.to : null;
}

/** What the shortcuts of the shell act on: the frame gives it. */
export interface ShellActions {
  t: (key: string) => string;
  rights: () => Rights;
  navigate: (to: string) => void;
  palette: () => boolean | void;
  help: () => void;
  /** `/`, `c`, `e`: the view's, `false` when it has none (or cannot now). */
  filter: () => boolean;
  create: () => boolean;
  edit: () => boolean;
  /** `false` where it does nothing (the narrow class keeps a rail). */
  sidebar: () => boolean | void;
  panel: () => void;
}

/** The shortcuts of the shell, in the order of the help. */
export function shellShortcuts(actions: ShellActions): ShellShortcut[] {
  const { t } = actions;
  const navigation = NAVIGATION.map(({ letter, entry }): ShellShortcut => ({
    id: `shell.go.${entry}`,
    keys: [["g"], [letter]],
    label: () => t(`shortcuts.go.${entry}`),
    where: "outsideField",
    scope: "global",
    shown: () => navigationTarget(entry, actions.rights()) !== null,
    run: () => {
      const to = navigationTarget(entry, actions.rights());
      if (to === null) return false;
      actions.navigate(to);
    },
  }));
  return [
    {
      id: "shell.palette",
      keys: ["Mod", "k"],
      label: () => t("shortcuts.palette"),
      where: "everywhere",
      scope: "global",
      run: actions.palette,
    },
    {
      id: "shell.filter",
      keys: ["/"],
      label: () => t("shortcuts.filter"),
      where: "outsideField",
      scope: "global",
      run: actions.filter,
    },
    {
      id: "shell.help",
      keys: ["?"],
      label: () => t("shortcuts.help"),
      where: "outsideField",
      scope: "global",
      run: actions.help,
    },
    ...navigation,
    {
      id: "shell.create",
      keys: ["c"],
      label: () => t("shortcuts.create"),
      where: "present",
      scope: "global",
      run: actions.create,
    },
    {
      id: "shell.edit",
      keys: ["e"],
      label: () => t("shortcuts.edit"),
      where: "present",
      scope: "global",
      run: actions.edit,
    },
    {
      id: "shell.apply",
      keys: ["Control", "Enter"],
      label: () => t("shortcuts.apply"),
      where: "editor",
      scope: "global",
    },
    {
      id: "shell.escape",
      keys: ["Escape"],
      label: () => t("shortcuts.escape"),
      where: "everywhere",
      scope: "global",
    },
    {
      // The two Escape of a filter (020, A8): the list first, then the panel.
      id: "shell.filterEscape",
      keys: [["Escape"], ["Escape"]],
      label: () => t("shortcuts.filterEscape"),
      where: "filter",
      scope: "global",
    },
    {
      id: "shell.columnFilter",
      keys: ["Alt", "ArrowDown"],
      label: () => t("shortcuts.columnFilter"),
      where: "gridHeader",
      scope: "global",
    },
    {
      id: "shell.sidebar",
      keys: ["["],
      label: () => t("shortcuts.sidebar"),
      where: "outsideField",
      scope: "global",
      run: actions.sidebar,
    },
    {
      id: "shell.panel",
      keys: ["]"],
      label: () => t("shortcuts.panel"),
      where: "outsideField",
      scope: "global",
      run: actions.panel,
    },
  ];
}

/** A shortcut of a plugin's view (SDK `shortcuts.register`), as the shell lists it. */
export function pluginShortcut(shortcut: PluginShortcut): ShellShortcut {
  return {
    id: shortcut.id,
    keys: [shortcut.key],
    label: shortcut.label,
    where: "plugin",
    scope: "view",
    run: shortcut.run,
  };
}
