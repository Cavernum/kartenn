// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The writes of the SDK (design « Écritures », ADR 0037 decision 6, REQ-021-16, 24): the
// quick edit of an object by its route, optimistic at the present only, and the submission
// of a draft as one changeset, with its simulation as a query of the cache. Each answer is
// told as an outcome: `applied`, `proposed` (202, held for a decision: never shown
// applied), `conflict` (412: read again, the input given back), `invalid` (422, with the
// codes of the violations), `forbidden`, `failed`.

import type { QueryClient } from "@tanstack/query-core";
import type { Client } from "../api/client.ts";
import { ApiProblem, type Violation } from "../api/problem.ts";
import type { components } from "../api/schema";
import { FRESHNESS, type Entry } from "./cache.ts";
import { refsOf } from "./dependents.ts";
import type { Invalidation } from "./invalidate.ts";
import { isImmutable, isPresent, type ObjectKey } from "./keys.ts";
import { collectionOf, qualify } from "./types.ts";

type Applied = components["schemas"]["ChangesetApplied"];
type Pending = components["schemas"]["ChangesetPending"];
export type Simulated = components["schemas"]["ChangesetSimulated"];
/** Why a changeset its author asked to apply stays pending (024, REQ-024-13). */
export type PendingReason = components["schemas"]["PendingReason"];
export type Submission = components["schemas"]["Submission"];

/** A quick edit: merge patch members of the object of `key`, and why. */
export interface QuickWrite {
  key: ObjectKey;
  patch: Record<string, unknown>;
  motive: string;
  /** The `ETag` of the read; that of the cache entry by default. */
  ifMatch?: string;
}

export type WriteOutcome<I = Record<string, unknown>> =
  | { kind: "applied"; changeset: string | null; warnings: readonly Violation[] }
  /**
   * Held for a decision; `reason` says why a write that asked to apply (`?approval=self`) did
   * not: `changeset.approval.self.missing` or `changeset.approval.self` (024, REQ-024-13).
   */
  | {
      kind: "proposed";
      changeset: string | null;
      review: string;
      reason: PendingReason | null;
    }
  /** The object changed since the read: `current` is read again, `input` given back. */
  | { kind: "conflict"; current: Entry | undefined; input: I; problem: ApiProblem }
  /** Violations by member (`/slug` → `slug`), the others in `general`. */
  | {
      kind: "invalid";
      byField: Readonly<Record<string, readonly Violation[]>>;
      general: readonly Violation[];
      input: I;
      problem: ApiProblem;
    }
  | { kind: "forbidden"; input: I; problem: ApiProblem }
  /** The network failed (`problem` null), or another problem. */
  | { kind: "failed"; input: I; problem: ApiProblem | null };

/** JSON Merge Patch (RFC 7396 § 2): what the object becomes once the patch is applied. */
export function mergePatch(target: unknown, patch: unknown): unknown {
  if (typeof patch !== "object" || patch === null || Array.isArray(patch)) return patch;
  const base =
    typeof target === "object" && target !== null && !Array.isArray(target)
      ? { ...(target as Record<string, unknown>) }
      : {};
  for (const [name, value] of Object.entries(patch)) {
    if (value === null) delete base[name];
    else base[name] = mergePatch(base[name], value);
  }
  return base;
}

/** Rules that refuse for lack of rights rather than for the content (REQ-005-22). */
const FORBIDDEN_RULE = /\.forbidden(_effect)?$/;

/** The member a JSON Pointer of the request names (`/slug` → `slug`), if one. */
export function fieldOf(pointer: string | null | undefined): string | null {
  const match = /^\/([^/]+)/.exec(pointer ?? "");
  return match ? match[1]!.replace(/~1/g, "/").replace(/~0/g, "~") : null;
}

/** The outcome of a write held for a decision. */
export type Proposed = Extract<WriteOutcome, { kind: "proposed" }>;

/** The outcomes of a write that did not go through. */
export type Refusal<I> = Exclude<WriteOutcome<I>, { kind: "applied" | "proposed" }>;

/** The outcome of a 202: held for a decision, with the reason when one is given. */
export function proposedOf(pending: Pending | undefined): Proposed {
  return {
    kind: "proposed",
    changeset: pending?.changeset.id ?? null,
    review: pending?.kind ?? "approval",
    reason: pending?.reason ?? null,
  };
}

/** The outcome of a refused write. */
export function refused<I>(problem: ApiProblem, input: I): Refusal<I> {
  const denied = problem.violations.some((violation) => FORBIDDEN_RULE.test(violation.rule));
  if (problem.status === 403 || denied) return { kind: "forbidden", input, problem };
  if (problem.status === 422 || problem.status === 400) {
    const byField: Record<string, Violation[]> = {};
    const general: Violation[] = [];
    for (const violation of problem.violations) {
      const field = fieldOf(violation.pointer);
      if (field) (byField[field] ??= []).push(violation);
      else general.push(violation);
    }
    return { kind: "invalid", byField, general, input, problem };
  }
  return { kind: "failed", input, problem };
}

/** A stable digest of a submission (FNV-1a, 64 bits), the key of its simulation. */
export function fingerprint(value: unknown): string {
  const text = JSON.stringify(value);
  let hash = 0xcbf29ce484222325n;
  for (let i = 0; i < text.length; i += 1) {
    hash ^= BigInt(text.charCodeAt(i));
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, "0");
}

/** A simulation by `POST`: its report, a 422 being one too (the violations found). */
export async function simulation(
  client: Client,
  url: string,
  body: unknown,
  signal: AbortSignal,
): Promise<Simulated> {
  try {
    const answer = await client.request("POST", url, { body, signal });
    return answer.data as Simulated;
  } catch (error) {
    if (error instanceof ApiProblem && error.status === 422) {
      return {
        valid: false,
        violations: [...error.violations],
        diff: [],
        sample: [],
        cascaded: [],
        sections: [],
      };
    }
    throw error;
  }
}

export interface WriteDeps {
  client: Client;
  queryClient: QueryClient;
  invalidation: Invalidation;
  /** Waits before another try of a submission whose key is in progress (409). */
  pause?: (ms: number) => Promise<void>;
}

export function createWrites({ client, queryClient, invalidation, pause }: WriteDeps) {
  const wait = pause ?? ((ms: number) => new Promise<void>((done) => setTimeout(done, ms)));

  /** What a changeset applied invalidates: the objects named, their lists, dependents. */
  function afterApplied(named: { type: string; id: string }[], body: Applied | undefined) {
    const cascaded = (body?.cascaded ?? []).map((c) => ({ type: qualify(c.kind), id: c.id }));
    return invalidation.objects([...named, ...cascaded], body?.changeset.id ?? null);
  }

  /**
   * What a changeset of operations applied invalidates: the objects the operations name,
   * those it cascaded to, and every list (those it creates are new to every list).
   */
  function submitted(operations: unknown, body: Applied | undefined): Promise<unknown> {
    const ids = [...refsOf(operations)];
    return Promise.all([invalidation.ids(ids), afterApplied([], body), invalidation.lists()]);
  }

  const quick = async ({ key, patch, motive, ifMatch }: QuickWrite): Promise<WriteOutcome> => {
    if (isImmutable(key)) throw new TypeError("no write on the past as recorded (as_of)");
    const [, type, id] = key;
    const collection = collectionOf(type);
    if (collection === null) throw new TypeError(`unknown type ${type}`);
    await queryClient.cancelQueries({ queryKey: key, exact: true });
    const previous = queryClient.getQueryData<Entry>(key);
    const tag = ifMatch ?? previous?.etag ?? undefined;
    if (tag === undefined) throw new TypeError(`no ETag read for ${type} ${id}: If-Match`);
    // Optimism at the present only (D-7): under `valid_at` the value shown is not the one
    // written.
    const optimistic = isPresent(key) && previous !== undefined;
    if (optimistic) {
      queryClient.setQueryData<Entry>(key, {
        data: mergePatch(previous.data, patch),
        etag: previous.etag,
      });
    }
    const rollback = () => {
      if (optimistic) queryClient.setQueryData<Entry>(key, previous);
    };
    try {
      // Applied at once when its author holds `approval.self`, held otherwise (024, REQ-024-13).
      const url = `${collection}/${encodeURIComponent(id)}?approval=self`;
      const answer = await client.request("PATCH", url, {
        body: { ...patch, motive },
        ifMatch: tag,
      });
      if (answer.status === 202) {
        rollback();
        return proposedOf(answer.data as Pending | undefined);
      }
      const applied = answer.data as Applied | undefined;
      await afterApplied([{ type, id }], applied);
      return {
        kind: "applied",
        changeset: applied?.changeset.id ?? null,
        warnings: applied?.warnings ?? [],
      };
    } catch (error) {
      rollback();
      if (!(error instanceof ApiProblem)) return { kind: "failed", input: patch, problem: null };
      if (error.status === 412) {
        await queryClient.refetchQueries({ queryKey: key, exact: true }).catch(() => undefined);
        return {
          kind: "conflict",
          current: queryClient.getQueryData<Entry>(key),
          input: patch,
          problem: error,
        };
      }
      return refused(error, patch);
    }
  };

  /** The simulation of a submission: a query of the cache, by digest (ADR 0037, 6). */
  const simulate = (submission: Submission) => {
    const queryKey = ["simulate", fingerprint(submission)] as const;
    return {
      queryKey,
      queryFn: ({ signal }: { signal: AbortSignal }): Promise<Simulated> =>
        simulation(client, "/v1/changesets?simulate=true", submission, signal),
      ...FRESHNESS.simulation,
    };
  };

  /**
   * Submits a draft as one changeset, under an idempotency key the caller keeps until the
   * answer: a retry, a reload replay the same key (REQ-006-15). A key still in progress
   * (409) is tried again five times.
   */
  const submit = async (
    submission: Submission,
    idempotencyKey: string,
  ): Promise<WriteOutcome<Submission>> => {
    for (let attempt = 0; ; attempt += 1) {
      try {
        const answer = await client.request("POST", "/v1/changesets", {
          body: submission,
          idempotencyKey,
        });
        if (answer.status === 202) return proposedOf(answer.data as Pending | undefined);
        const applied = answer.data as Applied | undefined;
        await submitted(submission.operations, applied);
        return {
          kind: "applied",
          changeset: applied?.changeset.id ?? null,
          warnings: applied?.warnings ?? [],
        };
      } catch (error) {
        if (!(error instanceof ApiProblem)) {
          return { kind: "failed", input: submission, problem: null };
        }
        if (error.status === 409 && attempt < 5) {
          await wait(1000);
          continue;
        }
        return refused(error, submission);
      }
    }
  };

  return { quick, simulate, submit, submitted };
}

export type Writes = ReturnType<typeof createWrites>;
