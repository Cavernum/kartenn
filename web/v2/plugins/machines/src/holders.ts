// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The interfaces of the rows of a list of addresses read by window (task 021-11p10-e,
// REQ-006-41): the machine column read the interfaces of each row apart, a thousand reads
// for ten pages scrolled, above the quota of a reader (REQ-006-29). The reads asked within a
// few milliseconds at the same dates go as one request `GET /v1/interfaces?address=…`, 64
// addresses at most, followed page by page to its end (a shared address may be held by more
// interfaces than a page); each interface says the requested addresses it holds
// (`addresses`, task 006-32e), and each row gets its own under the key of its read alone
// (`reads.list("/v1/interfaces", { address: [id] })`), the key the tab of an address and the
// search share. An address that no interface holds, or that the caller may not read, gets
// an empty page, as its read alone would. A read alone in its window goes by its read alone,
// with its `ETag`. The batcher of the properties of the SDK (`sdk/src/data/batch.ts`, task
// 11p10-c) is the model, kept here: an export of the SDK would be an API of the plugins
// (the choice of task 11p10-a).

import { api, reads, type Entry, type Page, type TimeKey } from "@kartenn/sdk";
import type { Interface } from "./reads.ts";

/** The milliseconds a read waits for others at its dates. */
export const HOLDERS_WINDOW = 10;
/** The addresses of one request at most (REQ-006-41: 400 beyond). */
export const HOLDERS_MAX = 64;
/** The interfaces of a page of a batch (REQ-006-7: 1 000 at most). */
export const HOLDERS_PAGE = 1000;
/**
 * The batches in flight at most: a fast scroll closes a window per frame, and a slow server
 * would have them pile up past its shedding limit (REQ-006-30). A window closed while none is
 * free waits for one, still open to the reads that come (up to `HOLDERS_MAX`), its reads
 * given up meanwhile gone before it leaves.
 */
export const HOLDERS_FLIGHTS = 4;

/** The page of a row, as its read alone keeps it. */
export type Held = Entry<Page<Interface>>;

/** A read waiting for its answer. */
interface Member {
  resolve(entry: Held): void;
  reject(error: unknown): void;
  /** The read of its address alone, sent when no other read joined its window. */
  alone(signal: AbortSignal): Promise<Held>;
  batch: Batch;
}

/** The reads of a window, then of its request, by address. */
interface Batch {
  readonly group: string;
  readonly time: TimeKey;
  readonly members: Map<string, Set<Member>>;
  /** The request under way, `null` while the window waits. */
  controller: AbortController | null;
  timer: ReturnType<typeof setTimeout> | undefined;
}

/** The URL of a batch: the addresses, then the dates of its reads. */
function urlOf(addresses: readonly string[], time: TimeKey): string {
  const params = new URLSearchParams();
  for (const address of addresses) params.append("address", address);
  params.set("page_size", String(HOLDERS_PAGE));
  if (time.valid_at) params.set("valid_at", time.valid_at);
  if (time.as_of) params.set("as_of", time.as_of);
  return `/v1/interfaces?${params.toString()}`;
}

/** The batcher over the client of the SDK (`client`, for the tests). */
export function createHolders(
  client: Pick<typeof api, "request"> = api,
  wait = HOLDERS_WINDOW,
  flights = HOLDERS_FLIGHTS,
) {
  /** The window that takes reads still, by dates. */
  const open = new Map<string, Batch>();
  /** The windows closed, waiting for a flight, oldest first. */
  const due: Batch[] = [];
  let flying = 0;

  /** A read leaves its batch: the request under way given up once no read waits for it. */
  function leave(member: Member, address: string): void {
    const { batch } = member;
    const waiting = batch.members.get(address);
    if (!waiting?.delete(member)) return;
    if (waiting.size === 0) batch.members.delete(address);
    if (batch.members.size === 0) batch.controller?.abort();
  }

  /** The window closed (its delay out, or full): sent once a flight is free. */
  function close(batch: Batch): void {
    clearTimeout(batch.timer);
    batch.timer = undefined;
    if (!due.includes(batch)) due.push(batch);
    if (batch.members.size >= HOLDERS_MAX && open.get(batch.group) === batch) {
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
    const signal = batch.controller!.signal;
    const addresses = [...batch.members.keys()];
    const settle = (address: string, done: (member: Member) => void) => {
      for (const member of batch.members.get(address) ?? []) done(member);
      batch.members.delete(address);
    };
    try {
      if (addresses.length === 1) {
        const [first] = batch.members.get(addresses[0]!)!;
        const entry = await first!.alone(signal);
        settle(addresses[0]!, (member) => member.resolve(entry));
        return;
      }
      // Each requested address, the interfaces that hold it, in the order of the list.
      const held = new Map<string, Interface[]>(addresses.map((address) => [address, []]));
      let next: string | null | undefined = urlOf(addresses, batch.time);
      while (next) {
        const answer = await client.request("GET", next, { signal });
        const page = answer.data as Page<Interface>;
        for (const iface of page.items) {
          for (const address of iface.addresses ?? []) {
            // As its read alone answers it: its own address only.
            held.get(address.id)?.push({ ...iface, addresses: [address] });
          }
        }
        next = page.links.next;
      }
      for (const [address, items] of held) {
        settle(address, (member) =>
          member.resolve({ data: { items, links: { next: null } }, etag: null }),
        );
      }
    } catch (error) {
      for (const address of addresses) settle(address, (member) => member.reject(error));
    }
  }

  return {
    /**
     * The interfaces that hold `address` at `time`, read with the others asked within the
     * window; `alone`, its read by itself, for a window of one. Given up (`signal`), the
     * read leaves its batch and the others go on.
     */
    load(
      address: string,
      time: TimeKey,
      signal: AbortSignal,
      alone: (signal: AbortSignal) => Promise<Held>,
    ): Promise<Held> {
      signal.throwIfAborted();
      const group = `${time.valid_at ?? ""}\n${time.as_of ?? ""}`;
      let window = open.get(group);
      if (!window) {
        const batch: Batch = {
          group,
          time,
          members: new Map(),
          controller: null,
          timer: undefined,
        };
        batch.timer = setTimeout(() => close(batch), wait);
        open.set(group, (window = batch));
      }
      const into = window;
      return new Promise<Held>((resolve, reject) => {
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
          leave(member, address);
          reject(signal.reason);
        };
        signal.addEventListener("abort", dropped, { once: true });
        const members = into.members.get(address) ?? new Set();
        members.add(member);
        into.members.set(address, members);
        // A full window goes at once (or first, once a flight is free).
        if (into.members.size >= HOLDERS_MAX) close(into);
      });
    },
  };
}

export type Holders = ReturnType<typeof createHolders>;

let shared: Holders | null = null;

/**
 * The interfaces that hold `address`, at the dates of now: the list of its read alone, under
 * its key, whose first page is read by window; a next page (a read alone that had one) as
 * the list reads it.
 */
export function heldBy(address: string) {
  const one = reads.list("/v1/interfaces", { address: [address] });
  const time = one.queryKey[4];
  return {
    ...one,
    queryFn: (context: Parameters<typeof one.queryFn>[0]) =>
      context.pageParam !== null
        ? one.queryFn(context)
        : (shared ??= createHolders()).load(address, time, context.signal, (signal) =>
            one.queryFn({ ...context, signal }),
          ),
  };
}
