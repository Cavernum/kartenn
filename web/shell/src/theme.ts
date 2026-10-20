// Light and dark modes of the design tokens (REQ-007-30): the system's by default
// (`prefers-color-scheme`), or the choice kept locally, set as `data-theme` on the root.

export const THEMES = ["system", "light", "dark"] as const;
export type Theme = (typeof THEMES)[number];
export const THEME_KEY = "kartenn.theme";

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

export function storedTheme(): Theme {
  const stored = storage()?.getItem(THEME_KEY);
  return (THEMES as readonly string[]).includes(stored ?? "") ? (stored as Theme) : "system";
}

/** Applies a theme to the document and keeps it. */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") delete root.dataset["theme"];
  else root.dataset["theme"] = theme;
  storage()?.setItem(THEME_KEY, theme);
}
