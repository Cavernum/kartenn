// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The drafts of the server (spec 024 § A, 021 design « Écritures »): reads as queries of
// the cache (family `drafts`), writes under `If-Match` with the `ETag` of the draft read,
// the simulation as a query waited 400 ms, the submission as one changeset. The active
// draft of a tab is its identifier in `sessionStorage`; nothing is kept in IndexedDB.

import type { QueryClient } from "@tanstack/query-core";
import type { Client } from "../api/client.ts";
import { ApiProblem } from "../api/problem.ts";
import type { components } from "../api/schema";
import type { Entry, Queries, ReadOptions } from "./cache.ts";
import { FRESHNESS } from "./cache.ts";
import type { Invalidation } from "./invalidate.ts";
import type { PathKey } from "./keys.ts";
import {
  fingerprint,
  proposedOf,
  refused,
  simulation,
  type Proposed,
  type Refusal,
  type Simulated,
  type Writes,
} from "./write.ts";

export type Draft = components["schemas"]["Draft"];
export type DraftList = components["schemas"]["DraftList"];
export type DraftCreation = components["schemas"]["DraftCreation"];
export type DraftChange = components["schemas"]["DraftChange"];
export type DraftPeriod = components["schemas"]["DraftSubmission"];
type Operation = components["schemas"]["Operation"];
type Applied = components["schemas"]["ChangesetApplied"];
type Pending = components["schemas"]["ChangesetPending"];

/** The wait of a simulation after the last change of its draft (design « Écritures »). */
export const SIMULATION_WAIT = 400;

/** The key of `sessionStorage` that holds the active draft of the tab. */
export const ACTIVE_KEY = "kartenn.draft.active";

/** What the SDK needs of `sessionStorage`. */
export type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/** A write of a draft: saved (the draft as the server holds it now), or refused. */
export type DraftOutcome<I> = { kind: "saved"; draft: Draft; etag: string | null } | Refusal<I>;

/** A submission: the changeset applied, held (with its reason), or refused. */
export type DraftSubmitOutcome =
  | { kind: "applied"; changeset: string | null; warnings: Applied["warnings"] }
  | Proposed
  | Refusal<DraftPeriod>;

export interface SubmitOptions {
  /** `?apply=own`: applied at once when its author may (024, REQ-024-13). */
  applyOwn: boolean;
  /** The period of the changeset; now and unbounded by default. */
  period?: DraftPeriod;
  /** Kept by the caller until the answer, so that a retry replays it (REQ-006-15). */
  idempotencyKey?: string;
}

/** A draft that left the tab: deleted, submitted or taken over elsewhere. */
export interface Gone {
  id: string;
  /** Whether it was the active draft of this tab (the shell leaves it with a notice). */
  active: boolean;
}

export interface DraftDeps {
  client: Client;
  queryClient: QueryClient;
  reads: Queries;
  invalidation: Invalidation;
  writes: Pick<Writes, "submitted">;
  /** `sessionStorage`, looked up at each use; `null` when the browser refuses it. */
  storage: () => StorageLike | null;
  /** The title of a draft created by `addToActive`. */
  title?: () => string;
  pause?: (ms: number, signal?: AbortSignal) => Promise<void>;
}

/** A wait that an abort ends early, with the abort's reason. */
function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((done, fail) => {
    if (signal?.aborted) return fail(signal.reason);
    const timer = setTimeout(done, ms);
    signal?.addEventListener(
      "abort",
      () => {
        clearTimeout(timer);
        fail(signal.reason);
      },
      { once: true },
    );
  });
}

/** A title in no language: the minute of creation, `2026-10-24 14:03`. */
const stamp = () => new Date().toISOString().slice(0, 16).replace("T", " ");

export function createDrafts(deps: DraftDeps) {
  const { client, queryClient, reads, invalidation, writes } = deps;
  const wait = deps.pause ?? sleep;
  const activeListeners = new Set<(id: string | null) => void>();
  const goneListeners = new Set<(gone: Gone) => void>();
  /** Drafts this tab is deleting or submitting: their `draft` event is no news. */
  const ending = new Set<string>();
  let title = deps.title ?? stamp;

  const path = (id: string) => `/v1/drafts/${encodeURIComponent(id)}`;

  const list = (): ReadOptions<DraftList, PathKey> =>
    reads.read("/v1/drafts", { family: "drafts" }) as ReadOptions<DraftList, PathKey>;
  const get = (id: string): ReadOptions<Draft, PathKey> =>
    reads.read("/v1/drafts/{id}", { params: { id }, family: "drafts" }) as ReadOptions<
      Draft,
      PathKey
    >;

  // The active draft of the tab.
  function active(): string | null {
    try {
      return deps.storage()?.getItem(ACTIVE_KEY) ?? null;
    } catch {
      return null;
    }
  }
  function setActive(id: string | null): void {
    if (active() === id) return;
    try {
      const storage = deps.storage();
      if (id === null) storage?.removeItem(ACTIVE_KEY);
      else storage?.setItem(ACTIVE_KEY, id);
    } catch {
      // Without `sessionStorage`, the tab has no active draft kept across a reload.
    }
    for (const listener of [...activeListeners]) listener(id);
  }

  /** The `ETag` of the draft read, read first if the cache has none. */
  async function etagOf(id: string): Promise<string> {
    const cached = queryClient.getQueryData<Entry<Draft>>(get(id).queryKey)?.etag;
    if (cached) return cached;
    const entry = await queryClient.fetchQuery({ ...get(id), staleTime: 0 });
    if (!entry.etag) throw new TypeError(`no ETag for the draft ${id}`);
    return entry.etag;
  }

  /** Reads the draft again (after a 412), whatever its freshness. */
  async function reread(id: string): Promise<Entry<Draft> | undefined> {
    await queryClient.fetchQuery({ ...get(id), staleTime: 0 }).catch(() => undefined);
    return queryClient.getQueryData<Entry<Draft>>(get(id).queryKey);
  }

  /** The draft as a write answered it: into the cache, the list read again. */
  async function saved(data: Draft, etag: string | null): Promise<DraftOutcome<never>> {
    queryClient.setQueryData<Entry<Draft>>(get(data.id).queryKey, { data, etag });
    await invalidation.path("/v1/drafts");
    return { kind: "saved", draft: data, etag };
  }

  /** A draft that no longer exists for this tab: out of the cache, no longer active. */
  async function forget(id: string): Promise<boolean> {
    const wasActive = active() === id;
    if (wasActive) setActive(null);
    queryClient.removeQueries({ queryKey: get(id).queryKey, exact: true });
    queryClient.removeQueries({ queryKey: ["simulate", "draft", id] });
    await invalidation.path("/v1/drafts");
    return wasActive;
  }

  async function gone(id: string): Promise<void> {
    const wasActive = await forget(id);
    for (const listener of [...goneListeners]) listener({ id, active: wasActive });
  }

  /** The outcome of a write that threw. */
  async function failure<I>(id: string | null, error: unknown, input: I): Promise<Refusal<I>> {
    if (!(error instanceof ApiProblem)) return { kind: "failed", input, problem: null };
    if (error.status === 412 && id !== null) {
      return { kind: "conflict", current: await reread(id), input, problem: error };
    }
    // Absent or someone else's (024 « Erreurs »): it left this tab.
    if (error.status === 404 && id !== null) await gone(id);
    return refused(error, input);
  }

  const create = async (body: DraftCreation): Promise<DraftOutcome<DraftCreation>> => {
    try {
      const answer = await client.request("POST", "/v1/drafts", { body });
      return await saved(answer.data as Draft, answer.etag);
    } catch (error) {
      return failure(null, error, body);
    }
  };

  const update = async (id: string, change: DraftChange): Promise<DraftOutcome<DraftChange>> => {
    try {
      const ifMatch = await etagOf(id);
      const answer = await client.request("PATCH", path(id), { body: change, ifMatch });
      return await saved(answer.data as Draft, answer.etag);
    } catch (error) {
      return failure(id, error, change);
    }
  };

  /**
   * Adds operations after those of the draft. Modified meanwhile (another tab, 412): read
   * again, then tried once more, an addition commuting with what changed (024 « Erreurs »).
   */
  const addOperations = async (
    id: string,
    operations: readonly Operation[],
  ): Promise<DraftOutcome<readonly Operation[]>> => {
    const send = async () => {
      const answer = await client.request("POST", `${path(id)}/operations`, {
        body: { operations },
        ifMatch: await etagOf(id),
      });
      return saved(answer.data as Draft, answer.etag);
    };
    try {
      return await send();
    } catch (error) {
      if (!(error instanceof ApiProblem) || error.status !== 412) {
        return failure(id, error, operations);
      }
    }
    await reread(id);
    try {
      return await send();
    } catch (error) {
      return failure(id, error, operations);
    }
  };

  const remove = async (id: string): Promise<{ kind: "removed" } | Refusal<string>> => {
    ending.add(id);
    try {
      await client.request("DELETE", path(id), { ifMatch: await etagOf(id) });
      await forget(id);
      return { kind: "removed" };
    } catch (error) {
      return failure(id, error, id);
    } finally {
      ending.delete(id);
    }
  };

  /**
   * The simulation of a draft at its version, as a query: its function waits 400 ms
   * before asking, and the next version of the draft (another key) abandons the wait of
   * the previous one, so that only the last change of a burst is simulated.
   */
  const simulate = (draft: Pick<Draft, "id" | "version">, period: DraftPeriod = {}) => ({
    queryKey: ["simulate", "draft", draft.id, draft.version, fingerprint(period)] as const,
    queryFn: async ({ signal }: { signal: AbortSignal }): Promise<Simulated> => {
      await wait(SIMULATION_WAIT, signal);
      return simulation(client, `${path(draft.id)}/simulation`, period, signal);
    },
    ...FRESHNESS.simulation,
  });

  /**
   * Submits the draft as one changeset; once accepted (applied or held) the server deletes
   * it. A key still in progress (409) is tried again five times; refused (422), the draft
   * stays.
   */
  const submit = async (id: string, options: SubmitOptions): Promise<DraftSubmitOutcome> => {
    const period = options.period ?? {};
    const key = options.idempotencyKey ?? crypto.randomUUID();
    const url = `${path(id)}/submission${options.applyOwn ? "?apply=own" : ""}`;
    const operations = queryClient.getQueryData<Entry<Draft>>(get(id).queryKey)?.data.operations;
    ending.add(id);
    try {
      for (let attempt = 0; ; attempt += 1) {
        try {
          const ifMatch = queryClient.getQueryData<Entry<Draft>>(get(id).queryKey)?.etag;
          const answer = await client.request("POST", url, {
            body: period,
            idempotencyKey: key,
            ...(ifMatch ? { ifMatch } : {}),
          });
          await forget(id);
          if (answer.status === 202) return proposedOf(answer.data as Pending | undefined);
          const applied = answer.data as Applied | undefined;
          await writes.submitted(operations ?? [], applied);
          return {
            kind: "applied",
            changeset: applied?.changeset.id ?? null,
            warnings: applied?.warnings ?? [],
          };
        } catch (error) {
          if (error instanceof ApiProblem && error.status === 409 && attempt < 5) {
            await wait(1000);
            continue;
          }
          return await failure(id, error, period);
        }
      }
    } finally {
      ending.delete(id);
    }
  };

  /**
   * Adds operations to the active draft of the tab, creating one if there is none, or if
   * the active one is gone (`draft.add` of the plugins).
   */
  const addToActive = async (
    operations: readonly Operation[],
  ): Promise<DraftOutcome<readonly Operation[]>> => {
    const id = active();
    if (id !== null) {
      const outcome = await addOperations(id, operations);
      const absent = outcome.kind === "failed" && outcome.problem?.status === 404;
      if (!absent) return outcome;
    }
    const created = await create({ title: title(), operations: [...operations] });
    if (created.kind !== "saved") {
      return { ...created, input: operations } as Refusal<readonly Operation[]>;
    }
    setActive(created.draft.id);
    return created;
  };

  /**
   * The event `draft` (024 § A): the draft at another version is read again, the list
   * too; a draft deleted, submitted or taken over leaves the tab with a signal.
   */
  async function changed(data: Record<string, unknown>): Promise<void> {
    const id = data["id"];
    if (typeof id !== "string") return;
    if (data["deleted"] === true) {
      if (ending.has(id)) await forget(id);
      else await gone(id);
      return;
    }
    const version = data["version"];
    const key = get(id).queryKey;
    await Promise.all([
      invalidation.path("/v1/drafts"),
      queryClient.invalidateQueries({
        queryKey: key,
        exact: true,
        predicate: (query) =>
          (query.state.data as Entry<Draft> | undefined)?.data.version !== version,
      }),
    ]);
  }

  return {
    list,
    get,
    create,
    update,
    addOperations,
    remove,
    simulate,
    submit,
    addToActive,
    active,
    setActive,
    /** Each change of the active draft of the tab; returns the function that stops. */
    onActive(listener: (id: string | null) => void): () => void {
      activeListeners.add(listener);
      return () => activeListeners.delete(listener);
    },
    /** Each draft that left the tab (deleted, submitted, taken over elsewhere). */
    onGone(listener: (gone: Gone) => void): () => void {
      goneListeners.add(listener);
      return () => goneListeners.delete(listener);
    },
    /** The title of the drafts `addToActive` creates (the shell's, in its language). */
    setTitle(next: (() => string) | null): void {
      title = next ?? stamp;
    },
    changed,
  };
}

export type Drafts = ReturnType<typeof createDrafts>;
