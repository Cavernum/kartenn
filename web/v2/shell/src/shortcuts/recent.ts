import type { IconName } from "@kartenn/ui";

// The objects opened lately from the palette (REQ-021-3, « objets récents »), kept in
// `localStorage` as the recent searches of the 007 were (REQ-007-28): a sensitive
// preference, which the logout clears (`forgetRecent`, task 16). Kept apart for each user,
// under their identity (a UUID, never a name): another user of the same browser never sees
// them. Shown only while the user may still read their collection.

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

/** The key of a user's recent objects: under their identity. */
const keyOf = (owner: string) => `${RECENT_KEY}.${owner}`;

function isFound(value: unknown): value is FoundObject {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return ["type", "id", "value", "icon", "to"].every((k) => typeof v[k] === "string");
}

function kept(owner: string): FoundObject[] {
  try {
    const value = JSON.parse(storage()?.getItem(keyOf(owner)) ?? "[]") as unknown;
    return Array.isArray(value) ? value.filter(isFound).slice(0, RECENT_KEPT) : [];
  } catch {
    return [];
  }
}

/**
 * The recent objects of `owner` (their identity; none signed out), the latest first, of
 * the collections they may read now (`readable`: the same rule as the search).
 */
export function recentObjects(
  owner: string | null,
  readable: (type: string) => boolean,
): FoundObject[] {
  return owner === null ? [] : kept(owner).filter((o) => readable(o.type));
}

/** Keeps `found` first among the recent objects of `owner`, without repeating it. */
export function rememberObject(owner: string | null, found: FoundObject): void {
  if (owner === null) return;
  const others = kept(owner).filter((o) => o.type !== found.type || o.id !== found.id);
  storage()?.setItem(keyOf(owner), JSON.stringify([found, ...others].slice(0, RECENT_KEPT)));
}

/** Clears the recent objects of every user of this browser (the logout). */
export function forgetRecent(): void {
  const store = storage();
  if (!store) return;
  const keys = Array.from({ length: store.length }, (_, i) => store.key(i));
  for (const key of keys) if (key?.startsWith(`${RECENT_KEY}.`)) store.removeItem(key);
}
