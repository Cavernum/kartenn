import type { components } from "@kartenn/sdk";
import type { State } from "@kartenn/ui";

// The imports of the user (spec 006, REQ-006-17 to 19, H18; REQ-007-19, REQ-021-12): their
// states, those its task works on (read again until over), those still open (cancelled,
// or resumed when paused), and why the sort of its elements failed (006/H18).

/** An import, as followed by its author (`GET /v1/imports/{id}`). */
export type ImportItem = components["schemas"]["ImportDto"];

/** A violation of the sort of an import (`invalid-import`), by its rank in the source. */
export interface SortViolation {
  rule: string;
  detail: string;
  element?: number;
}

/** The states of an import, in their order of the 006 (`failed` kept for the old ones). */
export const IMPORT_STATES = [
  "uploading",
  "sorting",
  "ready",
  "running",
  "paused",
  "done",
  "failed",
  "cancelled",
] as const;

/** The states in which an import may still be cancelled, or resumed (`paused`). */
export const IMPORT_OPEN: readonly string[] = [
  "uploading",
  "sorting",
  "ready",
  "running",
  "paused",
];

/** The states its background task works on: the import is read again until it leaves them. */
export const IMPORT_BUSY: readonly string[] = ["sorting", "ready", "running"];

/** How often an import is read again while its task works on it. */
export const IMPORT_POLL_MS = 1_000;

/** Whether the task of an import works on it (the page reads it again). */
export const importBusy = (item: Pick<ImportItem, "state"> | undefined): boolean =>
  item !== undefined && IMPORT_BUSY.includes(item.state);

/** Whether an import may still be cancelled. */
export const importOpen = (item: Pick<ImportItem, "state">): boolean =>
  IMPORT_OPEN.includes(item.state);

/** The state of an import, as a `StatusBadge` shows it. */
export function importBadge(state: string): State {
  if (state === "done") return "ok";
  if (state === "failed") return "ko";
  if (state === "paused") return "warn";
  if (IMPORT_BUSY.includes(state)) return "info";
  return "neutral";
}

/** The violations of the failed sort of an import, as served (none when it did not fail). */
export function sortViolations(item: Pick<ImportItem, "error">): SortViolation[] {
  const list = item.error?.["violations"];
  if (!Array.isArray(list)) return [];
  return list.filter(
    (v): v is SortViolation =>
      typeof v === "object" && v !== null && typeof (v as SortViolation).rule === "string",
  );
}

/** The report of the batches run, as text, `null` while empty. */
export function importReport(item: Pick<ImportItem, "report">): string | null {
  return Object.keys(item.report ?? {}).length > 0 ? JSON.stringify(item.report, null, 2) : null;
}
