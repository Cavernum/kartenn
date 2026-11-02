import {
  ApiProblem,
  filterSearch,
  keys,
  NetworkError,
  type Filters,
  type QueryClient,
} from "@kartenn/sdk";
import type { State } from "@kartenn/ui";
import { rowsOf } from "../views/data.ts";

// The audit journal (spec 006, REQ-006-4; 012/REQ-012-17; REQ-007-19, REQ-021-12): its
// events as `GET /v1/audit-events` serves them, the one chosen found again in the pages
// the cache holds, and the export of a search, `Accept: application/x-ndjson`, written to a
// file as it comes: piped into a file of the origin's private storage (OPFS), never held
// whole by the page, then handed to the browser as a download. Without that storage (a
// private window), the browser's own blob, which it may keep on disk past a size.

/** An event of the journal, as journaled (`audit.event`, 012). */
export type AuditEvent = {
  id: string;
  seq: number;
  name: string;
  severity: string;
  criticality: string;
  occurred_at: string;
  actor_kind: string;
  actor_id: string | null;
  auth_method: string | null;
  issuer: string | null;
  claimed: string | null;
  source_ip: string | null;
  user_agent: string | null;
  target_kind: string | null;
  target_ids: string[] | null;
  outcome: string;
  reason: string | null;
  motive: string | null;
  changeset_id: string | null;
  trace_id: string | null;
  span_id: string | null;
  request_id: string | null;
  service_version: string | null;
  instance_id: string | null;
  details: Record<string, unknown> | null;
};

/** The media type of an export: one event per line. */
export const NDJSON = "application/x-ndjson";

/** The outcomes of an event, as a `StatusBadge` shows them. */
export function outcomeBadge(outcome: string): State {
  if (outcome === "success") return "ok";
  if (outcome === "failure") return "ko";
  if (outcome === "denied") return "warn";
  return "neutral";
}

/** The event `id` among the pages of the journal the cache holds, whatever their filters. */
export function cachedEvent(
  queryClient: Pick<QueryClient, "getQueriesData">,
  id: string,
): AuditEvent | undefined {
  const lists = queryClient.getQueriesData<{ pages: never[] }>({
    queryKey: keys.allOn("/v1/audit-events"),
  });
  for (const [, data] of lists) {
    const found = rowsOf<AuditEvent>(data).find((event) => event.id === id);
    if (found) return found;
  }
  return undefined;
}

/** The name of the file of an export made at `now`. */
export function exportName(now: Date): string {
  return `kartenn-audit-${now.toISOString().replace(/[:.]/g, "-")}.ndjson`;
}

/** The directory of the private storage that holds the exports being written. */
export const EXPORTS = "kartenn-exports";

/** How long the file of an export stays after the browser took it over. */
const RELEASE_MS = 60_000;

/** The body of an export stopped before its end: a partial file is never handed over. */
export class ExportCut extends Error {
  constructor(cause: unknown) {
    super("the export was cut short", { cause });
    this.name = "ExportCut";
  }
}

/** The file of an export refused by the storage while written (its quota): none kept. */
export class ExportStorage extends Error {
  constructor(cause: unknown) {
    super("the storage refused the export", { cause });
    this.name = "ExportStorage";
  }
}

/** What an export needs of the browser (replaced in the tests). */
export interface ExportDeps {
  fetch: typeof fetch;
  /** The root of the origin's private storage; `null` when there is none. */
  storage: () => Promise<FileSystemDirectoryHandle | null>;
  /** Hands `file` to the browser as a download named `name`, then calls `release`. */
  save: (file: Blob, name: string, release: () => void) => void;
  now: () => Date;
}

/** The private storage of the origin, if the browser gives one (not in a private window). */
async function privateStorage(): Promise<FileSystemDirectoryHandle | null> {
  try {
    return (await navigator.storage?.getDirectory?.()) ?? null;
  } catch {
    return null;
  }
}

/** A download of `file` by a link, its address released a minute later. */
function download(file: Blob, name: string, release: () => void): void {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(file);
  link.download = name;
  link.click();
  setTimeout(() => {
    URL.revokeObjectURL(link.href);
    release();
  }, RELEASE_MS);
}

export const browserExport: ExportDeps = {
  fetch: (input, init) => fetch(input, init),
  storage: privateStorage,
  save: download,
  now: () => new Date(),
};

/** Writes `body` into a new file `name` of `root`'s exports; `null` without a writer. */
async function written(
  root: FileSystemDirectoryHandle,
  name: string,
  body: ReadableStream<Uint8Array>,
): Promise<{ file: File; release: () => void } | null> {
  // The file opened for writing; the storage refusing it (no writer, no room), nothing of
  // the body is read yet: `null`, and the blob of the browser takes the export.
  let release: () => void = () => undefined;
  let handle: FileSystemFileHandle;
  let writable: FileSystemWritableFileStream;
  try {
    // The files of an export whose page closed before its release.
    await root.removeEntry(EXPORTS, { recursive: true }).catch(() => undefined);
    const dir = await root.getDirectoryHandle(EXPORTS, { create: true });
    handle = await dir.getFileHandle(name, { create: true });
    release = () => void dir.removeEntry(name).catch(() => undefined);
    if (typeof handle.createWritable !== "function") throw new TypeError("no writer");
    writable = await handle.createWritable();
  } catch {
    release();
    return null;
  }
  // Which side failed: the body (cut short) or the file (the storage full).
  const reader = body.getReader();
  let cut = false;
  const watched = new ReadableStream<Uint8Array>({
    async pull(controller) {
      try {
        const { done, value } = await reader.read();
        if (done) controller.close();
        else controller.enqueue(value);
      } catch (error) {
        cut = true;
        controller.error(error);
      }
    },
    cancel: (reason) => reader.cancel(reason),
  });
  try {
    // `pipeTo` closes the file at the end of the body, aborts it on a failure.
    await watched.pipeTo(writable);
  } catch (error) {
    release();
    throw cut ? new ExportCut(error) : new ExportStorage(error);
  }
  return { file: await handle.getFile(), release };
}

/**
 * Exports the events `filters` select, newest first, as a file the browser saves; its
 * name answered. A refusal of the server throws its `ApiProblem`, no answer a
 * `NetworkError`, a body cut short an {@link ExportCut}, the storage full while writing an
 * {@link ExportStorage}.
 */
export async function exportAudit(
  filters: Filters,
  deps: ExportDeps = browserExport,
): Promise<string> {
  const query = filterSearch(filters);
  let response: Response;
  try {
    response = await deps.fetch(`/v1/audit-events${query ? `?${query}` : ""}`, {
      headers: { Accept: NDJSON },
      credentials: "same-origin",
    });
  } catch (error) {
    throw new NetworkError(error);
  }
  if (!response.ok) throw await ApiProblem.from(response);
  const name = exportName(deps.now());
  const root = response.body ? await deps.storage() : null;
  const streamed = root && response.body ? await written(root, name, response.body) : null;
  if (streamed) {
    deps.save(streamed.file, name, streamed.release);
    return name;
  }
  let blob: Blob;
  try {
    blob = await response.blob();
  } catch (error) {
    throw new ExportCut(error);
  }
  deps.save(blob, name, () => undefined);
  return name;
}
