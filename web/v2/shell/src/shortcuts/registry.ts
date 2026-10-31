import { shortcutSteps, type Shortcut } from "@kartenn/ui";

// The shortcuts of the shell (spec 021, REQ-021-7, 8; design « Raccourcis »): one registry,
// read by the help `?`, the palette and the menus (one source), and one `keydown` listener
// on `document`, in the bubbling phase. The registry refuses a conflict: two shortcuts on
// the same keys, or a key alone that starts a sequence (`g` and `g c`). The single-key
// shortcuts turn off with the setting of the user menu (WCAG 2.1.4): Ctrl+K, Ctrl+Entrée
// and Échap stay.

/** Where a shortcut acts, as the help says it (its column « Portée »). */
export type Where =
  "everywhere" | "outsideField" | "present" | "editor" | "filter" | "gridHeader" | "plugin";

export interface ShellShortcut {
  /** Unique: `shell.<name>`, `<plugin>.<name>`. */
  id: string;
  /** A combination (`["Mod", "k"]`, `["?"]`) or a sequence (`[["g"], ["c"]]`). */
  keys: Shortcut;
  /** What it does, in the language of the moment. */
  label: () => string;
  where: Where;
  /** `global`: the shell's, at every view; `view`: the current view's, gone when it leaves. */
  scope: "global" | "view";
  /**
   * What the listener does; `false` when it could not act now (nothing to create, a date
   * out of the present), so that the key keeps its default. Without it, the shortcut is
   * a component's own (Échap, Ctrl+Entrée, Alt+Bas), listed for the help only.
   */
  run?: () => boolean | void;
  /** Whether the user is shown it (rights): the help, the palette and the listener skip it. */
  shown?: () => boolean;
}

/** The keys of a shortcut, written once (`Mod+k`, `g c`): what a conflict compares. */
export function keysOf(keys: Shortcut): string {
  return shortcutSteps(keys)
    .map((step) => step.join("+"))
    .join(" ");
}

const MODIFIERS = new Set(["Mod", "Control", "Alt", "Meta"]);

/** Whether a shortcut is one of single keys: without a modifier but Shift (WCAG 2.1.4). */
export function isSingleKey(keys: Shortcut): boolean {
  return shortcutSteps(keys).every((step) => !step.some((key) => MODIFIERS.has(key)));
}

export interface ShortcutRegistry {
  /** Adds a shortcut, refused on a conflict; the returned function removes it. */
  register(shortcut: ShellShortcut): () => void;
  /** The shortcuts now, in their order of registration. */
  list(): readonly ShellShortcut[];
  /** Called after each change; returns the unsubscription. */
  subscribe(listener: () => void): () => void;
}

export function createShortcutRegistry(): ShortcutRegistry {
  let items: readonly ShellShortcut[] = [];
  const listeners = new Set<() => void>();
  return {
    register(shortcut) {
      if (items.some((other) => other.id === shortcut.id)) {
        throw new Error(`${shortcut.id} is already registered`);
      }
      // The same keys twice; or, both played by the listener, a key alone that starts a
      // sequence (Échap and the two Échap of a filter are the components', not a conflict).
      const keys = keysOf(shortcut.keys);
      const taken = items.find((other) => {
        const held = keysOf(other.keys);
        if (held === keys) return true;
        const prefix = keys.startsWith(`${held} `) || held.startsWith(`${keys} `);
        return prefix && !!shortcut.run && !!other.run;
      });
      if (taken) throw new Error(`${shortcut.id}: ${keys} is taken by ${taken.id}`);
      items = [...items, shortcut];
      for (const listener of listeners) listener();
      return () => {
        if (!items.includes(shortcut)) return;
        items = items.filter((other) => other !== shortcut);
        for (const listener of listeners) listener();
      };
    },
    list: () => items,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** The input types that take no text: a key on them is not typed. */
const NOT_TYPED = new Set(["checkbox", "radio", "button", "submit", "reset", "image", "file"]);
const TEXT_ROLES = new Set(["textbox", "combobox", "searchbox", "spinbutton"]);
/**
 * What consumes the characters typed: a component that says so, and the widgets that search
 * as one types (APG: menu, listbox and tree typeahead).
 */
const TYPEAHEAD = [
  "[data-k-typeahead]",
  ...["menu", "menubar", "listbox", "tree"].map((role) => `[role='${role}']`),
].join(", ");

/** Whether `target` takes the characters typed: a field, or a widget that consumes them. */
export function takesCharacters(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (target instanceof HTMLInputElement) return !NOT_TYPED.has(target.type);
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  if (target instanceof HTMLElement && target.isContentEditable) return true;
  if (target.closest("[contenteditable]:not([contenteditable='false'])")) return true;
  if (TEXT_ROLES.has(target.getAttribute("role") ?? "")) return true;
  return !!target.closest(TYPEAHEAD);
}

/** Whether `target` is in a dialog: the frame's single keys act behind no layer. */
function inLayer(target: EventTarget | null): boolean {
  return target instanceof Element && !!target.closest("[role='dialog'], [role='alertdialog']");
}

/** Whether the keys of a combination are those of the event (`Mod`: Ctrl, or ⌘). */
function matches(step: readonly string[], event: KeyboardEvent): boolean {
  const key = step.find((k) => !MODIFIERS.has(k) && k !== "Shift");
  if (key === undefined || key.toLowerCase() !== event.key.toLowerCase()) return false;
  const mod = step.includes("Mod");
  const ctrl = step.includes("Control") || (mod && !event.metaKey);
  const meta = step.includes("Meta") || (mod && event.metaKey);
  if (event.ctrlKey !== ctrl || event.metaKey !== meta) return false;
  if (event.altKey !== step.includes("Alt")) return false;
  // A combination says its Shift: Ctrl+Maj+K is not Ctrl+K (the console of a browser).
  return event.shiftKey === step.includes("Shift");
}

/** Whether the event holds Ctrl, Alt or Méta, AltGr apart (it types `[` on many layouts). */
function modified(event: KeyboardEvent): boolean {
  // AltGr: alone on some systems, Ctrl+Alt on others; never Alt alone, nor with Méta.
  const altGr =
    event.getModifierState?.("AltGraph") && event.ctrlKey === event.altKey && !event.metaKey;
  if (altGr) return false;
  return event.ctrlKey || event.altKey || event.metaKey;
}

export interface ListenerOptions {
  /** The shortcuts the listener plays, those of the plugins after the shell's. */
  shortcuts: () => readonly ShellShortcut[];
  /** The setting « Raccourcis à une touche ». */
  singleKeys: () => boolean;
  /** The wait for the second key of a sequence (design: 1.5 s). */
  timeout?: number;
  now?: () => number;
}

/** The keys pressed on the way to a character, which leave a sequence going. */
const LOCKS = new Set(["Shift", "CapsLock", "AltGraph"]);

/** The time a sequence waits for its next key (design « Raccourcis »). */
export const SEQUENCE_WAIT = 1500;

/**
 * The one listener of the shell's shortcuts. It ignores an event already handled
 * (`defaultPrevented`), a composition, a key held down. Single keys act outside any field,
 * any widget that consumes the characters (`data-k-typeahead`, a menu, a list, a tree)
 * and any dialog, without Ctrl, Alt or Méta, and only when the setting allows them; a
 * combination (Ctrl+K) acts everywhere, in a field too.
 */
export function createKeyListener(options: ListenerOptions): (event: KeyboardEvent) => void {
  const timeout = options.timeout ?? SEQUENCE_WAIT;
  const now = options.now ?? (() => Date.now());
  /** The first key of a sequence typed, and when. */
  let pending: { key: string; at: number } | null = null;

  const live = () => options.shortcuts().filter((s) => s.run && s.shown?.() !== false);
  const act = (shortcut: ShellShortcut, event: KeyboardEvent): boolean => {
    if (shortcut.run?.() === false) return false;
    event.preventDefault();
    return true;
  };

  return (event) => {
    if (event.defaultPrevented || event.isComposing) return;
    const shortcuts = live();
    if (modified(event)) {
      pending = null;
      const combination = shortcuts.find((s) => {
        const steps = shortcutSteps(s.keys);
        return steps.length === 1 && !isSingleKey(s.keys) && matches(steps[0]!, event);
      });
      if (combination) act(combination, event);
      return;
    }
    // Any other key (Tab, Échap, an arrow) or a key held ends a sequence; Shift alone not.
    if (event.key.length !== 1 || event.repeat) {
      if (!LOCKS.has(event.key)) pending = null;
      return;
    }
    if (!options.singleKeys() || takesCharacters(event.target) || inLayer(event.target)) {
      pending = null;
      return;
    }
    const single = shortcuts.filter((s) => isSingleKey(s.keys));
    const first = pending;
    pending = null;
    if (first && now() - first.at <= timeout) {
      const second = single.find((s) => {
        const steps = shortcutSteps(s.keys);
        return steps.length === 2 && steps[0]![0] === first.key && steps[1]![0] === event.key;
      });
      if (second) {
        act(second, event);
        return;
      }
    }
    const starts = single.some((s) => {
      const steps = shortcutSteps(s.keys);
      return steps.length > 1 && steps[0]![0] === event.key;
    });
    if (starts) {
      pending = { key: event.key, at: now() };
      event.preventDefault();
      return;
    }
    const one = single.find((s) => {
      const steps = shortcutSteps(s.keys);
      return steps.length === 1 && steps[0]!.length === 1 && steps[0]![0] === event.key;
    });
    if (one) act(one, event);
  };
}
