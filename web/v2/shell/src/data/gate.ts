import { queryClient } from "@kartenn/sdk";
import type { QueryClient, QueryKey } from "@tanstack/query-core";

// The one gate of the reads the shell makes row by row or cell by cell (task 021-11p10-a):
// the name of an object an answer only references (`views/record/ObjectName.vue`). The
// properties of the rows of a list and the VLAN of a network no longer pass it: the SDK
// sends them as one request per window (task 021-11p10-c). A list shows dozens of rows,
// each with its reads: all at
// once they overflow the shedding limit of the server (REQ-006-30, twenty in flight) and
// the reader's quota (REQ-006-29), refusing the other reads of the page with them. One cap
// for the whole shell, well under the server's, leaves room for the reads of the screen
// itself (its list, its record) and for those of the plugins.
//
// The gate holds a read while it is sent, not while it waits: a read refused by load
// (429, 503) waits out its `Retry-After` in the cache of the SDK (task 11p10-b) outside the
// gate, and takes its turn again with the other reads when it is sent anew.

/** At most this many reads of the gate in flight at once, for the whole shell. */
export const COLUMN_READS = 4;
let reading = 0;
/** The reads waiting for a turn, oldest first; each given the turn of the read that ends. */
const waiting: (() => void)[] = [];

/** A turn of the gate: at once while one is free, else once a read ahead ends. */
function turn(signal: AbortSignal | undefined): Promise<void> {
  if (reading < COLUMN_READS) {
    reading++;
    return Promise.resolve();
  }
  return new Promise<void>((resolve, reject) => {
    const given = () => {
      signal?.removeEventListener("abort", dropped);
      resolve();
    };
    // Given up while waiting (its row or cell left the view): out of the queue at once.
    const dropped = () => {
      waiting.splice(waiting.indexOf(given), 1);
      reject(signal!.reason);
    };
    waiting.push(given);
    signal?.addEventListener("abort", dropped, { once: true });
  });
}

/** The turn of a read that ends, given to the oldest one waiting, else freed. */
function release(): void {
  const next = waiting.shift();
  if (next) next();
  else reading--;
}

/**
 * Runs `read` within the gate; a read given up (`signal` aborted) before its turn is never
 * sent and leaves the queue at once.
 */
export async function limited<T>(
  signal: AbortSignal | undefined,
  read: () => Promise<T>,
): Promise<T> {
  signal?.throwIfAborted();
  await turn(signal);
  try {
    signal?.throwIfAborted();
    return await read();
  } finally {
    release();
  }
}

/**
 * Gives up the read of `queryKey` under way once nothing in view wants it any more: no
 * observer of it enabled (a cell out of view disables its own). Another cell in view that
 * reads the same object (the same VLAN on two rows) keeps it going.
 */
export function giveUpUnseen(queryKey: QueryKey, client: QueryClient = queryClient): void {
  void client.cancelQueries({
    queryKey,
    exact: true,
    fetchStatus: "fetching",
    predicate: (query) => !query.isActive(),
  });
}
