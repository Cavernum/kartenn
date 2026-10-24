// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The commands of the palette and the shortcuts a plugin declares (spec 021, REQ-021-3, 7;
// design « API des plugins », « Raccourcis »). The SDK keeps them; the shell reads them
// (palette, help `?`, its one `keydown` listener) through `@kartenn/sdk/host`.

/** A command of the palette (Ctrl+K). */
export interface Command {
  /** Unique among the commands: `<plugin>.<name>`. */
  id: string;
  /** Its label, read each time the palette shows it (in the language of the moment). */
  label: () => string;
  /** More words the palette matches. */
  keywords?: string[];
  /** The permission without which the palette leaves it out (REQ-021-3). */
  permission?: string;
  run: () => void | Promise<void>;
}

/** A shortcut of the view of a plugin: one key, outside any field (REQ-021-8). */
export interface Shortcut {
  /** Unique among the shortcuts: `<plugin>.<name>`. */
  id: string;
  /**
   * One character, as `KeyboardEvent.key` gives it (`n`, `N`, `#`): no modifier but
   * Shift, never a key of the shell.
   */
  key: string;
  /** Its label, for the help `?` and the menus. */
  label: () => string;
  /** A plugin's shortcut lives with its view: the view unregisters it when it leaves. */
  scope?: "view";
  run: () => void;
}

/** A list of items the shell reads and follows. */
export interface Registry<T> {
  /** Adds an item; the returned function removes it (and only it). */
  register(item: T): () => void;
  /** The items now, in their order of registration. */
  list(): readonly T[];
  /** Called after each change; returns the unsubscription. */
  subscribe(listener: () => void): () => void;
}

function registry<T extends { id: string }>(
  check: (item: T, items: readonly T[]) => void,
): Registry<T> {
  let items: readonly T[] = [];
  const listeners = new Set<() => void>();
  const notify = () => {
    for (const listener of listeners) listener();
  };
  return {
    register(item) {
      if (items.some((other) => other.id === item.id)) {
        throw new Error(`${item.id} is already registered`);
      }
      check(item, items);
      items = [...items, item];
      notify();
      return () => {
        if (!items.includes(item)) return;
        items = items.filter((other) => other !== item);
        notify();
      };
    },
    list: () => items,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** The keys of the shell's own shortcuts, which no plugin may take (design « Raccourcis »). */
let reserved = new Set<string>();

/** The shell's side: the keys its own shortcuts use. */
export function reserveKeys(keys: Iterable<string>): void {
  reserved = new Set(keys);
}

export const commandRegistry = registry<Command>(() => {});

export const shortcutRegistry = registry<Shortcut>((shortcut, shortcuts) => {
  const { key } = shortcut;
  if ([...key].length !== 1 || key.trim() === "") {
    throw new Error(`${shortcut.id}: ${JSON.stringify(key)} is not one character`);
  }
  if (reserved.has(key)) throw new Error(`${shortcut.id}: ${key} is a key of the shell`);
  const taken = shortcuts.find((other) => other.key === key);
  if (taken) throw new Error(`${shortcut.id}: ${key} is taken by ${taken.id}`);
});
