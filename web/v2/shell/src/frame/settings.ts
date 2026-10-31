import { applyMotion, type MotionSetting } from "@kartenn/ui";
import { ref, watch, type Ref } from "vue";

// The settings of the user menu and the state of the frame (REQ-021-1, 2, 7; design 021
// « Cadre », « Raccourcis »), each kept in `localStorage` under `kartenn.*` and applied to
// the root of the document: theme (`data-theme`, the system's when unset), density
// (`data-density`), motion (`data-motion`, SCR40), the single-key shortcuts (read by the
// registry of task 10, WCAG 2.1.4), the sidebar collapsed (`kartenn.sidebar`), the right
// panel (`kartenn.panel`: open, tab, width). Applied before the first render, so that the
// application mounts in the geometry it keeps.

export const THEMES = ["system", "light", "dark"] as const;
export type Theme = (typeof THEMES)[number];
export const DENSITIES = ["compact", "comfortable"] as const;
export type Density = (typeof DENSITIES)[number];

export const THEME_KEY = "kartenn.theme";
export const DENSITY_KEY = "kartenn.density";
export const MOTION_KEY = "kartenn.motion";
export const SINGLE_KEYS_KEY = "kartenn.shortcuts.single";
export const SIDEBAR_KEY = "kartenn.sidebar";
export const PANEL_KEY = "kartenn.panel";

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

function readOne<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  const value = storage()?.getItem(key);
  return (allowed as readonly string[]).includes(value ?? "") ? (value as T) : fallback;
}

/** A setting kept under `key`: read once, written back at each change, applied by `apply`. */
function kept<T extends string>(
  key: string,
  allowed: readonly T[],
  fallback: T,
  apply: (value: T) => void,
): Ref<T> {
  const value = ref(readOne(key, allowed, fallback)) as Ref<T>;
  apply(value.value);
  watch(value, (now) => {
    storage()?.setItem(key, now);
    apply(now);
  });
  return value;
}

const root = () => document.documentElement;

export function applyTheme(theme: Theme): void {
  if (theme === "system") delete root().dataset.theme;
  else root().dataset.theme = theme;
}

export function applyDensity(density: Density): void {
  if (density === "comfortable") root().dataset.density = "comfortable";
  else delete root().dataset.density;
}

export interface Settings {
  theme: Ref<Theme>;
  density: Ref<Density>;
  motion: Ref<MotionSetting>;
  /** The single-key shortcuts on (WCAG 2.1.4: they can be turned off). */
  singleKeys: Ref<boolean>;
  sidebarCollapsed: Ref<boolean>;
}

let settings: Settings | null = null;

/** The settings of this page, read and applied the first time. */
export function useSettings(): Settings {
  settings ??= {
    theme: kept(THEME_KEY, THEMES, "system", applyTheme),
    density: kept(DENSITY_KEY, DENSITIES, "compact", applyDensity),
    motion: kept(MOTION_KEY, ["system", "reduce"] as const, "system", (m) => applyMotion(m)),
    singleKeys: boolKept(SINGLE_KEYS_KEY, true),
    sidebarCollapsed: boolKept(SIDEBAR_KEY, false, "collapsed", "expanded"),
  };
  return settings;
}

/** A boolean kept as one of two words. */
function boolKept(key: string, fallback: boolean, on = "on", off = "off"): Ref<boolean> {
  const word = readOne(key, [on, off], fallback ? on : off);
  const value = ref(word === on);
  watch(value, (now) => storage()?.setItem(key, now ? on : off));
  return value;
}

/** The right panel as kept: open, its tab, its width. */
export interface PanelState {
  open: boolean;
  tab: string;
  width: number | null;
}

export function readPanel(): PanelState {
  try {
    const kept = JSON.parse(storage()?.getItem(PANEL_KEY) ?? "null") as Partial<PanelState> | null;
    return {
      open: kept?.open === true,
      tab: typeof kept?.tab === "string" ? kept.tab : "detail",
      width: typeof kept?.width === "number" && Number.isFinite(kept.width) ? kept.width : null,
    };
  } catch {
    return { open: false, tab: "detail", width: null };
  }
}

export function keepPanel(state: PanelState): void {
  storage()?.setItem(PANEL_KEY, JSON.stringify(state));
}

/** For the tests: the next `useSettings` reads the storage again. */
export function forgetSettings(): void {
  settings = null;
}
