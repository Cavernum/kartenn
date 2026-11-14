// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The batcher of the reads of resolved properties (task 021-11p10-c, REQ-006-40, H23): a
// list shows its rows by windows of forty to sixty, and each row read its properties apart,
// fifty reads a second when it scrolls (the quota of a reader, REQ-006-29). The reads asked
// within a few milliseconds, of one collection at the same dates, go as one request
// `GET /v1/<collection>/properties?id=…`, two hundred identifiers at most; each read keeps
// its own key in the cache (`reads.sub(…, "properties", …)`), so that the record and the
// list share it still, and an edit or an invalidation reads one row again. A read alone in
// its window goes by the read of one object, with its `ETag`.

import type { Client } from "../api/client.ts";
import { search } from "../api/client.ts";
import { ApiProblem } from "../api/problem.ts";
import type { components } from "../api/schema";
import type { TimeKey } from "./keys.ts";

type Resolved = components["schemas"]["Resolved"];
type ResolvedBatch = components["schemas"]["ResolvedBatch"];

/** What the cache keeps of an answer (`cache.ts`, `Entry`): a `Resolved` here. */
interface Kept {
  readonly data: unknown;
  readonly etag: string | null;
}

/** The milliseconds a read waits for others of its collection and dates. */
export const BATCH_WINDOW = 10;
/** The identifiers of one request at most (REQ-006-40). */
export const BATCH_MAX = 200;
/**
 * The batches in flight at most, for the whole client: a fast scroll closes a window per
 * frame, and a slow server would have them pile up past its shedding limit (REQ-006-30,
 * twenty in flight). A window closed while none is free waits for one, still open to the
 * reads that come (up to `BATCH_MAX`), its reads given up meanwhile gone before it leaves.
 */
export const BATCH_FLIGHTS = 4;

/** A read waiting for its answer. */
interface Member {
  resolve(entry: Kept): void;
  reject(error: unknown): void;
  /** The read of its object alone, sent when no other read joined its window. */
  alone(signal: AbortSignal): Promise<Kept>;
  /** The batch it waits in: the window open, then its request. */
  batch: Batch;
}

/** The reads of a window, then of its request, by identifier. */
interface Batch {
  readonly group: string;
  readonly collection: string;
  readonly time: TimeKey;
  readonly members: Map<string, Set<Member>>;
  /** The request under way, `null` while the window waits. */
  controller: AbortController | null;
  timer: ReturnType<typeof setTimeout> | undefined;
}

/**
 * What the server answers for an object it leaves out of a batch (unknown at the dates,
 * of another collection, or that the caller may not read, alike): the refusal the read of
 * that object alone would give, never tried again.
 */
function leftOut(): ApiProblem {
  return new ApiProblem({
    status: 404,
    type: "urn:kartenn:problem:not-found",
    title: "Not found",
  });
}

/** The batcher over a client: `load` reads the properties of an object, batched. */
export function createBatcher(client: Client, wait = BATCH_WINDOW, flights = BATCH_FLIGHTS) {
  /** The windows that take reads still, by collection and dates. */
  const open = new Map<string, Batch>();
  /** The windows closed, waiting for a flight, oldest first. */
  const due: Batch[] = [];
  let flying = 0;

  /** A read leaves its batch: the request under way given up once no read waits for it. */
  function leave(member: Member, id: string): void {
    const { batch } = member;
    const waiting = batch.members.get(id);
    if (!waiting?.delete(member)) return;
    if (waiting.size === 0) batch.members.delete(id);
    if (batch.members.size === 0) batch.controller?.abort();
  }

  /** The window closed (its delay out, or full): sent once a flight is free. */
  function close(batch: Batch): void {
    clearTimeout(batch.timer);
    batch.timer = undefined;
    if (!due.includes(batch)) due.push(batch);
    if (batch.members.size >= BATCH_MAX && open.get(batch.group) === batch) {
      open.delete(batch.group);
    }
    pump();
  }

  /** The windows due sent while flights are free; one emptied meanwhile, dropped. */
  function pump(): void {
    while (flying < flights && due.length > 0) {
      const batch = due.shift()!;
      if (open.get(batch.group) === batch) open.delete(batch.group);
      if (batch.members.size === 0) continue;
      flying++;
      batch.controller = new AbortController();
      void send(batch).finally(() => {
        flying--;
        pump();
      });
    }
  }

  async function send(batch: Batch): Promise<void> {
    const { collection, time } = batch;
    const signal = batch.controller!.signal;
    const ids = [...batch.members.keys()];
    const settle = (id: string, done: (member: Member) => void) => {
      for (const member of batch.members.get(id) ?? []) done(member);
      batch.members.delete(id);
    };
    try {
      if (ids.length === 1) {
        const [first] = batch.members.get(ids[0]!)!;
        const entry = await first!.alone(signal);
        settle(ids[0]!, (member) => member.resolve(entry));
        return;
      }
      const query = search({ id: ids, valid_at: time.valid_at, as_of: time.as_of });
      const answer = await client.request("GET", `${collection}/properties${query}`, { signal });
      const items = new Map((answer.data as ResolvedBatch).items.map((item) => [item.id, item]));
      for (const id of ids) {
        const item = items.get(id);
        if (!item) {
          settle(id, (member) => member.reject(leftOut()));
          continue;
        }
        const href = `${collection}/${encodeURIComponent(id)}`;
        // The item as the read of its object answers it: its links made again (H23).
        const entry: Kept & { data: Resolved } = {
          data: {
            properties: item.properties,
            sources: item.sources,
            masked: item.masked,
            links: { self: `${href}/properties`, object: href },
          },
          etag: null,
        };
        settle(id, (member) => member.resolve(entry));
      }
    } catch (error) {
      for (const id of ids) settle(id, (member) => member.reject(error));
    }
  }

  return {
    /**
     * The resolved properties of `id` of `collection` at `time`, read with the others asked
     * within the window; `alone`, its read by itself, for a window of one. Given up
     * (`signal`), the read leaves its batch and the others go on.
     */
    load(
      collection: string,
      id: string,
      time: TimeKey,
      signal: AbortSignal,
      alone: (signal: AbortSignal) => Promise<Kept>,
    ): Promise<Kept> {
      signal.throwIfAborted();
      const group = `${collection}\n${time.valid_at ?? ""}\n${time.as_of ?? ""}`;
      let window = open.get(group);
      if (!window) {
        const batch: Batch = {
          group,
          collection,
          time,
          members: new Map(),
          controller: null,
          timer: undefined,
        };
        batch.timer = setTimeout(() => close(batch), wait);
        open.set(group, (window = batch));
      }
      const into = window;
      return new Promise<Kept>((resolve, reject) => {
        const member: Member = {
          resolve: (entry) => {
            signal.removeEventListener("abort", dropped);
            resolve(entry);
          },
          reject: (error) => {
            signal.removeEventListener("abort", dropped);
            reject(error);
          },
          alone,
          batch: into,
        };
        const dropped = () => {
          leave(member, id);
          reject(signal.reason);
        };
        signal.addEventListener("abort", dropped, { once: true });
        const members = into.members.get(id) ?? new Set();
        members.add(member);
        into.members.set(id, members);
        // A full window goes at once (or first, once a flight is free).
        if (into.members.size >= BATCH_MAX) close(into);
      });
    },
  };
}

export type Batcher = ReturnType<typeof createBatcher>;
