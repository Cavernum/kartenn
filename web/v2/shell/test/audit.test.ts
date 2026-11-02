import { ApiProblem, keys, NetworkError } from "@kartenn/sdk";
import { QueryClient } from "@tanstack/query-core";
import { describe, expect, it, vi } from "vitest";
import {
  cachedEvent,
  EXPORTS,
  ExportCut,
  ExportStorage,
  exportAudit,
  exportName,
  NDJSON,
  outcomeBadge,
  type ExportDeps,
} from "../src/admin/audit.ts";
import {
  importBadge,
  importBusy,
  importOpen,
  importReport,
  sortViolations,
  type ImportItem,
} from "../src/admin/imports.ts";
import { frameRoutes } from "../src/frame/routes.ts";

// The audit journal and the imports in the administration (task 021-15c; REQ-021-12): the
// export written to a file as its body comes, never held whole by the page (a body cut
// short leaves no file), the event chosen found again in the pages the cache holds, the
// states of an import, why its sort failed, and the routes whose objects open in the right
// panel.

const LINES = ['{"seq":2,"name":"authn.login.succeeded"}\n', '{"seq":1,"name":"x.y.z"}\n'];
const encoder = new TextEncoder();

/** A body of chunks, the last one failing when `cut`. */
function body(chunks: string[], cut = false): ReadableStream<Uint8Array> {
  let i = 0;
  return new ReadableStream({
    pull(controller) {
      if (i < chunks.length) controller.enqueue(encoder.encode(chunks[i++]));
      else if (cut) controller.error(new TypeError("network cut"));
      else controller.close();
    },
  });
}

/** A private storage in memory: its files, their chunks as written, those removed. */
function storage(refuse: "open" | "write" | null = null) {
  const files = new Map<string, Uint8Array[]>();
  const removed: string[] = [];
  const pulled: number[] = [];
  const full = () => new DOMException("no room", "QuotaExceededError");
  const dir = {
    getFileHandle: async (name: string) => {
      files.set(name, []);
      return {
        createWritable: async () => {
          if (refuse === "open") throw full();
          return new WritableStream<Uint8Array>({
            write(chunk) {
              if (refuse === "write") throw full();
              // How many chunks the file holds when each comes: one at a time.
              files.get(name)!.push(chunk);
              pulled.push(files.get(name)!.length);
            },
          });
        },
        getFile: async () => new File(files.get(name)! as BlobPart[], name),
      };
    },
    removeEntry: async (name: string) => {
      removed.push(name);
      files.delete(name);
    },
  };
  const root = {
    removeEntry: async (name: string) => void removed.push(name),
    getDirectoryHandle: async () => dir,
  } as unknown as FileSystemDirectoryHandle;
  return { root, files, removed, pulled };
}

function deps(response: Response | Error, root: FileSystemDirectoryHandle | null) {
  const saved: { file: Blob; name: string; release: () => void }[] = [];
  const fetch = vi.fn(async () => {
    if (response instanceof Error) throw response;
    return response;
  });
  const used: ExportDeps = {
    fetch: fetch as unknown as typeof globalThis.fetch,
    storage: async () => root,
    save: (file, name, release) => saved.push({ file, name, release }),
    now: () => new Date("2026-11-01T10:20:30.400Z"),
  };
  return { used, saved, fetch };
}

const ndjson = (stream: ReadableStream<Uint8Array>) =>
  new Response(stream, { headers: { "content-type": NDJSON } });

describe("the export of the audit journal", () => {
  it("asks the search as NDJSON, written to a file as it comes", async () => {
    const store = storage();
    const { used, saved, fetch } = deps(ndjson(body(LINES)), store.root);
    const name = await exportAudit({ name: "authn.login.succeeded" }, used);

    expect(name).toBe("kartenn-audit-2026-11-01T10-20-30-400Z.ndjson");
    expect(name).toBe(exportName(new Date("2026-11-01T10:20:30.400Z")));
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/v1/audit-events?name=authn.login.succeeded");
    expect((init.headers as Record<string, string>).Accept).toBe(NDJSON);
    // Chunk by chunk into the file, never gathered first.
    expect(store.pulled).toEqual([1, 2]);
    expect(saved).toHaveLength(1);
    expect(await saved[0]!.file.text()).toBe(LINES.join(""));
    // The leftovers of an export of a closed page removed first; this one at its release.
    expect(store.removed).toEqual([EXPORTS]);
    saved[0]!.release();
    await Promise.resolve();
    expect(store.removed).toEqual([EXPORTS, name]);
  });

  it("keeps no file of a body cut short", async () => {
    const store = storage();
    const { used, saved } = deps(ndjson(body(LINES.slice(0, 1), true)), store.root);
    await expect(exportAudit({}, used)).rejects.toBeInstanceOf(ExportCut);
    expect(saved).toHaveLength(0);
    expect(store.files.size).toBe(0);
  });

  it("falls back to the blob when the storage refuses the file at its opening", async () => {
    const store = storage("open");
    const { used, saved } = deps(ndjson(body(LINES)), store.root);
    await exportAudit({}, used);
    expect(await saved[0]!.file.text()).toBe(LINES.join(""));
    expect(store.files.size).toBe(0);
  });

  it("tells the storage full while writing from a body cut short", async () => {
    const store = storage("write");
    const { used, saved } = deps(ndjson(body(LINES)), store.root);
    const error = await exportAudit({}, used).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ExportStorage);
    expect(error).not.toBeInstanceOf(ExportCut);
    expect(saved).toHaveLength(0);
    expect(store.files.size).toBe(0);
  });

  it("falls back to the browser's blob without a private storage", async () => {
    const { used, saved, fetch } = deps(ndjson(body(LINES)), null);
    await exportAudit({}, used);
    expect((fetch.mock.calls[0] as unknown as [string])[0]).toBe("/v1/audit-events");
    expect(await saved[0]!.file.text()).toBe(LINES.join(""));
  });

  it("throws the problem of a refusal, and a network error without answer", async () => {
    const refused = new Response(
      JSON.stringify({ type: "urn:kartenn:problem:forbidden", title: "Forbidden", status: 403 }),
      { status: 403, headers: { "content-type": "application/problem+json" } },
    );
    const store = storage();
    const first = deps(refused, store.root);
    const problem = await exportAudit({}, first.used).catch((e: unknown) => e);
    expect(problem).toBeInstanceOf(ApiProblem);
    expect((problem as ApiProblem).status).toBe(403);
    expect(first.saved).toHaveLength(0);
    expect(store.removed).toEqual([]);

    const second = deps(new TypeError("offline"), store.root);
    await expect(exportAudit({}, second.used)).rejects.toBeInstanceOf(NetworkError);
  });
});

describe("the events of the journal", () => {
  it("finds the event chosen in the pages the cache holds, whatever their filters", () => {
    const queryClient = new QueryClient();
    const page = (items: object[]) => ({ data: { items, links: {} }, etag: null });
    queryClient.setQueryData([...keys.allOn("/v1/audit-events"), { name: "a" }], {
      pages: [page([{ id: "e1" }]), page([{ id: "e2", name: "authn.login.succeeded" }])],
      pageParams: [null, "c"],
    });
    expect(cachedEvent(queryClient, "e2")?.name).toBe("authn.login.succeeded");
    expect(cachedEvent(queryClient, "e3")).toBeUndefined();
  });

  it("shows an outcome by its state", () => {
    expect(outcomeBadge("success")).toBe("ok");
    expect(outcomeBadge("failure")).toBe("ko");
    expect(outcomeBadge("denied")).toBe("warn");
    expect(outcomeBadge("other")).toBe("neutral");
  });
});

describe("the imports", () => {
  const item = (state: string, extra: Partial<ImportItem> = {}) =>
    ({ state, report: {}, ...extra }) as ImportItem;

  it("is read again while its task works on it, cancelled while open", () => {
    expect(["sorting", "ready", "running"].every((s) => importBusy(item(s)))).toBe(true);
    expect(["uploading", "paused", "done", "cancelled"].some((s) => importBusy(item(s)))).toBe(
      false,
    );
    expect(importBusy(undefined)).toBe(false);
    expect(importOpen(item("paused"))).toBe(true);
    expect(importOpen(item("done"))).toBe(false);
    expect(importBadge("paused")).toBe("warn");
    expect(importBadge("running")).toBe("info");
    expect(importBadge("done")).toBe("ok");
  });

  it("says why its sort failed, by the rank of each element", () => {
    const error = {
      type: "urn:kartenn:problem:invalid-import",
      title: "Invalid import",
      status: 422,
      violations: [{ rule: "import.unknown-reference", detail: "…", element: 0 }, "noise"],
    };
    expect(sortViolations(item("uploading", { error }))).toEqual([
      { rule: "import.unknown-reference", detail: "…", element: 0 },
    ]);
    expect(sortViolations(item("uploading"))).toEqual([]);
    expect(importReport(item("done"))).toBeNull();
    expect(importReport(item("done", { report: { applied: 1 } as never }))).toContain('"applied"');
  });
});

describe("the routes", () => {
  it("opens an event of the journal and an import in the right panel", () => {
    for (const [name, path] of [
      ["audit-record", "admin/audit/:id"],
      ["imports-record", "imports/:id"],
    ]) {
      const route = frameRoutes.find((r) => r.name === name);
      expect(route?.path, name).toBe(path);
      expect(route?.meta?.detail, name).toBeDefined();
    }
  });
});
