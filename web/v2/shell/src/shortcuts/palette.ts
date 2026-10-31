import type { Command } from "@kartenn/sdk";
import type { IconName, PaletteGroup, PaletteItem, Shortcut } from "@kartenn/ui";
import type { FoundObject } from "./recent.ts";

// The entries of the palette Ctrl+K (REQ-021-3; maquette E8): with a text, the objects found
// (by value, by name), then the commands and the pages whose words hold it; without one,
// the recent objects, the commands of the view and every page. A command out of the
// rights is absent (the frame and the registry of the plugins leave it out).

/** An entry of the palette that runs something: a command, or a page to go to. */
export interface PaletteEntry {
  id: string;
  label: string;
  icon: IconName;
  shortcut?: Shortcut;
  /** More words matched (the space of a page, a plugin's keywords). */
  keywords?: readonly string[];
  run: () => void;
}

export interface PaletteContent {
  query: string;
  /** The objects found for the query. */
  objects: readonly FoundObject[];
  recent: readonly FoundObject[];
  commands: readonly PaletteEntry[];
  pages: readonly PaletteEntry[];
  /** The titles of the groups, in the language of the moment. */
  labels: { objects: string; recent: string; commands: string; navigation: string };
  /** The name of a type of object (`prefix` → « Préfixe »), in the language of the moment. */
  typeLabel: (type: string) => string;
}

/** The commands of the plugins (SDK `commands.register`) the user may run: none out of rights. */
export function pluginEntries(
  commands: readonly Command[],
  has: (permission: string) => boolean,
): PaletteEntry[] {
  return commands
    .filter((command) => !command.permission || has(command.permission))
    .map((command) => ({
      id: `plugin:${command.id}`,
      label: command.label(),
      icon: "command",
      ...(command.keywords ? { keywords: command.keywords } : {}),
      run: () => void command.run(),
    }));
}

/** The id of an object's entry: its type and identifier. */
export const objectEntry = (found: FoundObject) => `object:${found.type}:${found.id}`;

/**
 * An object as the palette shows it (maquette E8): its type in words and where it lies
 * (« Préfixe · production »), its value in fixed pitch after it — the type said in the name
 * of the option, not by the icon alone.
 */
const objectItem =
  (typeLabel: (type: string) => string) =>
  (found: FoundObject): PaletteItem => ({
    id: objectEntry(found),
    label: found.context ? `${typeLabel(found.type)} · ${found.context}` : typeLabel(found.type),
    icon: found.icon,
    detail: found.value,
  });

const entryItem = (entry: PaletteEntry): PaletteItem => ({
  id: entry.id,
  label: entry.label,
  icon: entry.icon,
  ...(entry.shortcut ? { shortcut: entry.shortcut } : {}),
});

/** Whether every word of `wanted` is in the words of `entry`. */
function holds(entry: PaletteEntry, wanted: string): boolean {
  const words = [entry.label, ...(entry.keywords ?? [])].join(" ").toLowerCase();
  return wanted.split(/\s+/).every((word) => words.includes(word));
}

/** The groups of the palette, non empty, in their order. */
export function paletteGroups(content: PaletteContent): PaletteGroup[] {
  const wanted = content.query.trim().toLowerCase();
  const item = objectItem(content.typeLabel);
  const groups: PaletteGroup[] = [
    wanted === ""
      ? { label: content.labels.recent, items: content.recent.map(item) }
      : { label: content.labels.objects, items: content.objects.map(item) },
  ];
  const commands = wanted ? content.commands.filter((c) => holds(c, wanted)) : content.commands;
  const pages = wanted ? content.pages.filter((p) => holds(p, wanted)) : content.pages;
  groups.push({ label: content.labels.commands, items: commands.map(entryItem) });
  groups.push({ label: content.labels.navigation, items: pages.map(entryItem) });
  return groups.filter((group) => group.items.length > 0);
}
