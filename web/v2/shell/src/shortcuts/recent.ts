import type { IconName } from "@kartenn/ui";

// The objects opened lately from the palette (REQ-021-3, « objets récents »), kept in
// `localStorage` as the recent searches of the 007 were (REQ-007-28): a sensitive
// preference, which the logout clears (`forgetRecent`, task 16).

export const RECENT_KEY = "kartenn.recent.objects";
/** How many the palette offers. */
export const RECENT_KEPT = 8;

/**
 * An object found by the palette: what it shows of it (its type in words, where it lies,
 * its value in fixed pitch), and the page it leads to.
 */
export interface FoundObject {
  /** Its type (`prefix`, `address`…) and identifier, unique together. */
  type: string;
  id: string;
  /** Its value, slug or VID, in fixed pitch. */
  value: string;
  /** Where it lies: its domain, its parent; a VLAN's name. */
  context?: string;
  icon: IconName;
  /** The path of its record (`/prefixes/<id>`). */
  to: string;
}

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

function isFound(value: unknown): value is FoundObject {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return ["type", "id", "value", "icon", "to"].every((k) => typeof v[k] === "string");
}

/** The recent objects, the latest first. */
export function recentObjects(): FoundObject[] {
  try {
    const value = JSON.parse(storage()?.getItem(RECENT_KEY) ?? "[]") as unknown;
    return Array.isArray(value) ? value.filter(isFound).slice(0, RECENT_KEPT) : [];
  } catch {
    return [];
  }
}

/** Keeps `found` first among the recent objects, without repeating it. */
export function rememberObject(found: FoundObject): void {
  const others = recentObjects().filter((o) => o.type !== found.type || o.id !== found.id);
  storage()?.setItem(RECENT_KEY, JSON.stringify([found, ...others].slice(0, RECENT_KEPT)));
}

/** Clears the recent objects (the logout). */
export function forgetRecent(): void {
  storage()?.removeItem(RECENT_KEY);
}
