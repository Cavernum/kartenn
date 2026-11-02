// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The surface of the draft offered to plugins (W10, amended by the 024): operations in the
// format of the contract, added to the active draft of the tab, a draft of the server
// created when there is none (021 design « Écritures »).

import { PARENTS } from "./api/parents.ts";
import { ApiProblem, NetworkError, type Violation } from "./api/problem.ts";
import type { components } from "./api/schema";
import type { Entry } from "./data/cache.ts";
import type { Draft, DraftOutcome } from "./data/drafts.ts";
import type { MoveSubject } from "./data/moves.ts";
import type { Refusal, Simulated } from "./data/write.ts";
import { drafts, queryClient } from "./instance.ts";

/** An operation of a changeset, as the contract defines it. */
export type Operation = components["schemas"]["Operation"];

let onAdd: ((draft: Draft) => void) | null = null;

/**
 * How a change of parent put in the active draft ended (REQ-021-33; design 021
 * « Déplacements dans les arbres »): its simulation holding it (`valid`, with the warnings
 * of its own), refusing it by a rule of its own (`refused`: the operation stays in the
 * draft until a new move of the node replaces it), or not done (`unknown`: the network,
 * `problem` null, or a problem); else the write of the draft refused, nothing added.
 */
export type DraftMove =
  | { kind: "drafted"; draft: Draft; verdict: "valid"; warnings: readonly Violation[] }
  | { kind: "drafted"; draft: Draft; verdict: "refused"; violation: Violation }
  | { kind: "drafted"; draft: Draft; verdict: "unknown"; problem: ApiProblem | null }
  | Refusal<unknown>;

/**
 * The violations of a simulation of `severity` about the operation `index`: those of that
 * operation (its index, or its pointer `/operations/<index>`), else the rules of the final
 * state (`operation` and `pointer` null) about the node; never one of another operation.
 */
function ownViolations(
  simulated: Simulated,
  index: number,
  id: string,
  severity: Violation["severity"],
): Violation[] {
  const of = simulated.violations.filter((v) => v.severity === severity);
  const own = of.filter(
    (v) =>
      v.operation === index ||
      v.pointer === `/operations/${index}` ||
      (v.pointer ?? "").startsWith(`/operations/${index}/`),
  );
  return own.length
    ? own
    : of.filter((v) => v.operation == null && v.pointer == null && v.subjects.includes(id));
}

/**
 * Puts the change of parent `op` of `node` in the active draft — replacing an earlier change
 * of parent of the same node there (`drafts.update`), never a second one, else added by
 * `add` — then simulates the draft. Nothing of the cache is written (X33): the node stays
 * where its reads show it. `saved` is told the draft once the operation is in it.
 */
export async function moveInDraft(
  op: Operation,
  node: MoveSubject,
  add: (operations: readonly Operation[]) => Promise<DraftOutcome<readonly Operation[]>>,
  saved?: (draft: Draft) => void,
): Promise<DraftMove> {
  // The member that writes the parent (`move_entity.parent` → `parent`, `MachinePatch.host`).
  const member = PARENTS[node.type]?.via.split(".").pop();
  const replaces = (operation: Operation) => {
    const o = operation as Record<string, unknown>;
    const target = (o["id"] as { id?: unknown } | undefined)?.id;
    return (
      member !== undefined &&
      o["op"] === (op as { op: string }).op &&
      target === node.id &&
      member in o
    );
  };
  const active = drafts.active();
  const current = active
    ? ((await queryClient.fetchQuery(drafts.get(active)).catch(() => undefined)) as
        Entry<Draft> | undefined)
    : undefined;
  const operations = current?.data.operations ?? [];
  const earlier = operations.findIndex(replaces);
  const added =
    earlier >= 0
      ? await drafts.update(current!.data.id, {
          operations: operations.map((o, i) => (i === earlier ? op : o)),
        })
      : await add([op]);
  if (added.kind !== "saved") return added;
  saved?.(added.draft);
  const index = earlier >= 0 ? earlier : (added.draft.operations?.length ?? 1) - 1;
  const simulated = await queryClient
    .fetchQuery(drafts.simulate(added.draft))
    .then((answer) => ({ answer }))
    .catch((error: unknown) => ({ error }));
  if ("error" in simulated) {
    // No answer (`NetworkError`) or a problem; anything else is a bug of the client, never
    // passed off as the network's.
    const { error } = simulated;
    if (!(error instanceof ApiProblem) && !(error instanceof NetworkError)) throw error;
    return {
      kind: "drafted",
      draft: added.draft,
      verdict: "unknown",
      problem: error instanceof ApiProblem ? error : null,
    };
  }
  const refused = simulated.answer.valid
    ? undefined
    : ownViolations(simulated.answer, index, node.id, "error")[0];
  if (refused)
    return { kind: "drafted", draft: added.draft, verdict: "refused", violation: refused };
  return {
    kind: "drafted",
    draft: added.draft,
    verdict: "valid",
    warnings: ownViolations(simulated.answer, index, node.id, "warning"),
  };
}

export const draft = {
  /** The identifier of the active draft of the tab, `null` if none. */
  active(): string | null {
    return drafts.active();
  },
  /**
   * Adds an operation at the end of the active draft, creating one if there is none; the
   * shell then opens the draft.
   */
  async add(op: Operation): Promise<DraftOutcome<readonly Operation[]>> {
    const outcome = await drafts.addToActive([op]);
    if (outcome.kind === "saved") onAdd?.(outcome.draft);
    return outcome;
  },
  /**
   * Puts the change of parent `op` of `node` in the active draft (a node of a tree with a
   * declared parent, checked first by `canMove`), replacing an earlier one of the same node,
   * then simulates the draft: whether its rules hold the move. The shell opens the draft.
   * Never optimistic: the node changes place only when its reads, once the draft is
   * applied, show it there (REQ-021-33).
   */
  move(op: Operation, node: MoveSubject): Promise<DraftMove> {
    return moveInDraft(op, node, drafts.addToActive, (saved) => onAdd?.(saved));
  },
};

/** The shell's side: what an addition by a plugin does. */
export const draftHost = {
  onAdd(handler: ((draft: Draft) => void) | null): void {
    onAdd = handler;
  },
};
