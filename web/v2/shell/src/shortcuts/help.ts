import type { Shortcut } from "@kartenn/ui";
import { isSingleKey, type ShellShortcut } from "./registry.ts";

// The rows of the help of the shortcuts (REQ-021-7; maquette E8): one per shortcut the user
// is shown, in the order of the registry.

export interface HelpRow extends Record<string, unknown> {
  id: string;
  keys: Shortcut;
  action: string;
  scope: string;
}

/**
 * The rows of the help. With the single keys off (WCAG 2.1.4), each one the listener plays
 * is marked off in words with what it does (a column always shown), not by a colour; Échap
 * and the keys of the components stay as they are.
 */
export function helpRows(
  shortcuts: readonly ShellShortcut[],
  singleKeys: boolean,
  t: (key: string, named?: Record<string, unknown>) => string,
): HelpRow[] {
  const off = (s: ShellShortcut) => !singleKeys && !!s.run && isSingleKey(s.keys);
  return shortcuts
    .filter((s) => s.shown?.() !== false)
    .map((s) => ({
      id: s.id,
      keys: s.keys,
      action: off(s) ? t("shortcuts.off", { action: s.label() }) : s.label(),
      scope: t(`shortcuts.where.${s.where}`),
    }));
}
