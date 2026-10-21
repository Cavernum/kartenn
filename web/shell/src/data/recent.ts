// The recent searches, kept locally: a sensitive preference, cleared at logout
// (REQ-007-28, `clearSensitive`).

export const RECENT_KEY = "kartenn.recent.searches";
const KEPT = 10;

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/** The recent searches, the latest first. */
export function recent(): string[] {
  try {
    const value = JSON.parse(storage()?.getItem(RECENT_KEY) ?? "[]") as unknown;
    return Array.isArray(value) ? value.filter((v): v is string => typeof v === "string") : [];
  } catch {
    return [];
  }
}

/** Keeps `term` first among the recent searches, without repeating it. */
export function remember(term: string): void {
  const list = [term, ...recent().filter((t) => t !== term)].slice(0, KEPT);
  storage()?.setItem(RECENT_KEY, JSON.stringify(list));
}
