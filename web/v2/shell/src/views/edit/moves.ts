import { moveInActiveDraft } from "@kartenn/sdk/host";
import {
  ApiProblem,
  canMove as check,
  changesets,
  FORBIDDEN_RULE,
  queryClient,
  reads,
  type Draft,
  type Entry,
  type MoveSubject,
  type MoveVerdict,
  type Operation,
  type Submission,
  type WriteOutcome,
} from "@kartenn/sdk";
import type { MoveCheck, MoveIntent, MoveState, TreeNode } from "@kartenn/ui";
import { shallowReactive } from "vue";
import { useI18n } from "vue-i18n";
import { problemCode, useProblems } from "../../problems/index.ts";
import { useSession } from "../../rights/index.ts";
import { useTime, useWrites, type Writes } from "../../time/index.ts";

// Moves in the trees whose parent is declared (REQ-021-32, 33; design 021 « Déplacements
// dans les arbres »): the `canMove` of the shell — the SDK's check (cycle, parent admitted
// by the contract, rights on both ends, present, codes of the server rules) with the rights
// of `/v1/me`, the date of the page and the ancestors the host has read, its reason in
// words — and the move itself, which becomes an operation of the draft or, applied now with
// its motive, a changeset of its own. Never optimistic (X33): no key of the tree is written;
// the node stays at its place, « en attente », until the answer, and changes place only
// when its tree read again shows it; a refusal (the simulation of the draft invalid, 422,
// 403, 412) leaves it at its place with its code. The server stays the only judge.

/** A node of the tree as a move needs it: its type, and its version when the host read it. */
export interface MoveNode extends MoveSubject {
  /** The version read (`expected` of the operation); read by identifier when unknown. */
  readonly version?: string | null;
  /** Its parent now: a move onto it changes nothing. */
  readonly parent?: string | null;
}

/** The operation that puts the node under `target` (`null`: a root), at its version read. */
export type MoveOperation = (
  node: MoveNode,
  expected: string | null,
  target: MoveNode | null,
) => Operation;

const ref = (id: string) => ({ id });
const expectedOf = (expected: string | null) => (expected ? { expected } : {});

/** The operations of the core, by type of the node moved (`via` of `PARENTS`). */
const OPERATIONS = new Map<string, MoveOperation>([
  [
    "core.entity",
    (node, expected, target) => ({
      op: "move_entity",
      id: ref(node.id),
      parent: target ? ref(target.id) : null,
      ...expectedOf(expected),
    }),
  ],
  [
    "core.site",
    (node, expected, target) => ({
      op: "move_site",
      id: ref(node.id),
      parent: target ? ref(target.id) : null,
      ...expectedOf(expected),
    }),
  ],
  [
    "core.vlan_group",
    (node, _expected, target) => ({
      op: "update_vlan_group",
      id: ref(node.id),
      // `PARENTS` admits no root for a group: canMove refuses `null` before this.
      anchor: { [target!.type.replace(/^core\./, "")]: ref(target!.id) } as never,
    }),
  ],
]);

/** What the host of a tree tells the moves. */
export interface MoveModel {
  /** The node of the tree as a move sees it; `undefined`: it does not move. */
  subject(node: TreeNode): MoveNode | undefined;
  /** The ancestors of a node the host has read, from its parent up. */
  ancestors(id: string): readonly string[];
}

/** A move to the draft, or applied now (`now`) with its motive. */
export interface MoveOptions {
  now?: boolean;
  motive?: string;
  /** The idempotency key of the very submission applied now; a new key each by default. */
  idempotencyKey?: (submission: Submission) => string;
}

/**
 * How a move ended: in the draft — its simulation holding it (`valid`), refusing it
 * (`refused`: the operation kept in the draft, marked, until a new move of the node
 * replaces it) or not done (`unknown`, the problem given) —, applied, held, or refused at
 * its place.
 */
export type MoveOutcome =
  | {
      kind: "drafted";
      draft: Draft;
      verdict: "valid" | "refused" | "unknown";
      /** Why the simulation could not be done (`unknown`): the network, a problem. */
      failure?: Extract<WriteOutcome<unknown>, { kind: "failed" }>;
    }
  | WriteOutcome<unknown>
  | { kind: "unchanged" }
  | { kind: "refused"; check: Refused };

/** A target refused, with its reason and code. */
export type Refused = Extract<MoveCheck, { ok: false }>;

/** A node that does not move, or a target of no admitted type. */
const INCOMPATIBLE = { ok: false, refusal: "incompatible" } as const;

/**
 * « Changer de parent… » of a record (its menu of actions): the refusal of the record's
 * writes (`refused`, a permission or `as_of`), or, a move being checked and written at the
 * present only, `dated` off it — under `valid_at` alone too (REQ-021-5); in a frame under
 * 1024 px, `narrow` (the gate's refusal, before a permission, as the cells say it).
 */
export function moveEntryRefusal<R extends object>(
  refused: R,
  writes: Pick<Writes, "quickEdit" | "refused">,
  words: { dated: string; narrow: string },
): R | { disabled: true; reason: string } {
  if (writes.refused === "narrow") return { disabled: true, reason: words.narrow };
  return "forbidden" in refused || writes.quickEdit
    ? refused
    : { disabled: true, reason: words.dated };
}

export function useMoves(model: MoveModel) {
  const { t } = useI18n();
  const problems = useProblems();
  const session = useSession();
  const time = useTime();
  const writes = useWrites();
  /** The moves in flight or refused, by node; the nodes moved in the draft. */
  const states = shallowReactive(new Map<string, MoveState>());
  const drafted = shallowReactive(new Set<string>());

  const env = () => ({
    ancestors: (id: string) => model.ancestors(id),
    can: (permission: string) => session.value.rights.has(permission),
    present: time.value.present,
  });

  /** The verdict in the words of the shell: the reason, the code of the rule. */
  function said(verdict: MoveVerdict, target: TreeNode | null): MoveCheck {
    return verdict.ok ? { ok: true } : refusedFor(verdict, target);
  }
  function refusedFor(
    verdict: Extract<MoveVerdict, { ok: false }>,
    target: TreeNode | null,
  ): Refused {
    const reason = t(`edit.move.${verdict.refusal}`, {
      target: target?.label ?? t("edit.move.root"),
      permission: verdict.permission ?? "",
    });
    return { ok: false, reason, ...(verdict.code ? { code: verdict.code } : {}) };
  }

  /**
   * `canMove` of the tree: for each target hovered or listed, synchronous. In a frame under
   * 1024 px, every target is refused by the width (REQ-021-29), whatever the node.
   */
  function canMove(node: TreeNode, target: TreeNode | null): MoveCheck {
    if (writes.value.refused === "narrow") return { ok: false, reason: t("edit.why.narrow") };
    const subject = model.subject(node);
    const to = target ? model.subject(target) : null;
    if (!subject || (target && !to)) return refusedFor(INCOMPATIBLE, target);
    return said(check(subject, to ?? null, env()), target);
  }

  /** The state of a node: its move in flight, or refused with its code. */
  function moveState(node: TreeNode): MoveState | undefined {
    return states.get(node.id);
  }

  /**
   * The version the operation expects: the host's, else (none, or `null`) the object read by
   * identifier; `null` when it cannot be read — the move is then not sent.
   */
  async function versionOf(node: MoveNode): Promise<string | null> {
    if (node.version) return node.version;
    const entry = await queryClient
      .fetchQuery(reads.object<{ version?: string }>(node.type, node.id))
      .catch(() => undefined);
    return (entry as Entry<{ version?: string }> | undefined)?.data.version ?? null;
  }

  function refuse(id: string, outcome: Refused): MoveOutcome {
    states.set(id, { refused: outcome });
    return { kind: "refused", check: outcome };
  }

  /** The refusal of a write, as the node shows it: its first rule, else its problem. */
  function refusalOf(
    outcome: Exclude<WriteOutcome<unknown>, { kind: "applied" | "proposed" }>,
  ): Refused {
    const problem = outcome.problem instanceof ApiProblem ? outcome.problem : null;
    const violations =
      outcome.kind === "invalid"
        ? [...Object.values(outcome.byField).flat(), ...outcome.general]
        : (problem?.violations ?? []);
    const first =
      outcome.kind === "forbidden"
        ? (violations.find((v) => FORBIDDEN_RULE.test(v.rule)) ?? violations[0])
        : violations[0];
    // A violation says itself with its rule; else the problem, as `problems/` says it: the
    // sentence of its `type`, its code as served (the network without one).
    if (first) return { ok: false, reason: first.detail, code: first.rule };
    const code = problemCode(problem);
    return { ok: false, reason: problems.sentence(problem), ...(code ? { code } : {}) };
  }

  /**
   * Moves `intent.node` under `intent.target`, checked again first: to the draft by default,
   * applied now (`now`, with its motive) otherwise. Nothing of the tree is written here.
   */
  async function move(intent: MoveIntent, options: MoveOptions = {}): Promise<MoveOutcome> {
    const { node } = intent;
    // A second intent while one is in flight: ignored, whatever the target.
    if (states.get(node.id) === "pending") return { kind: "unchanged" };
    try {
      return await moved(intent, options);
    } catch (error) {
      // Rejected outright (the network, an unreadable answer): the node is no longer in
      // flight, so that a move of it is tried again; the rejection is the caller's to say.
      if (states.get(node.id) === "pending") states.delete(node.id);
      throw error;
    }
  }
  async function moved(intent: MoveIntent, options: MoveOptions): Promise<MoveOutcome> {
    const { node, target } = intent;
    const subject = model.subject(node);
    const to = target ? model.subject(target) : null;
    const verdict = canMove(node, target);
    if (!verdict.ok) return refuse(node.id, verdict);
    if (!subject) return { kind: "unchanged" };
    if ((subject.parent ?? null) === (to?.id ?? null)) return { kind: "unchanged" };
    const operation = OPERATIONS.get(subject.type);
    if (!operation) return refuse(node.id, refusedFor(INCOMPATIBLE, target));
    states.set(node.id, "pending");
    drafted.delete(node.id);
    const expected = await versionOf(subject);
    // Never sent without the version it was read at (`expected`): said instead.
    if (expected === null) return refuse(node.id, { ok: false, reason: t("edit.move.noVersion") });
    const op = operation(subject, expected, to ?? null);
    if (options.now) {
      const submission: Submission = { motive: options.motive ?? "", operations: [op] };
      const outcome = await changesets.submit(
        submission,
        options.idempotencyKey?.(submission) ?? crypto.randomUUID(),
        { approvalSelf: true },
      );
      // Applied: the reads of the tree were invalidated and read again before the answer
      // (X24); the node is where they show it. Held for a review: it did not move.
      if (outcome.kind === "applied" || outcome.kind === "proposed") {
        states.delete(node.id);
        return outcome;
      }
      refuse(node.id, refusalOf(outcome));
      return outcome;
    }
    // In the draft (the SDK's, shared with the plugins): a node moved already there has its
    // operation replaced, never doubled; the shell's own addition is said here, the draft
    // not opened. Its simulation says whether the move holds; the node stays at its place,
    // marked as moved in the draft. Refused, the operation stays in the draft, the node
    // marked refused with the rule; a new move of the node replaces it.
    const added = await moveInActiveDraft(op, subject);
    if (added.kind !== "drafted") {
      refuse(node.id, refusalOf(added));
      return added;
    }
    if (added.verdict === "refused") {
      const { violation } = added;
      refuse(node.id, { ok: false, reason: violation.detail, code: violation.rule });
      return { kind: "drafted", draft: added.draft, verdict: "refused" };
    }
    states.delete(node.id);
    drafted.add(node.id);
    // Not simulated (no answer, a problem): said by `problems/`.
    if (added.verdict === "unknown") {
      return {
        kind: "drafted",
        draft: added.draft,
        verdict: "unknown",
        failure: { kind: "failed", input: op, problem: added.problem },
      };
    }
    return { kind: "drafted", draft: added.draft, verdict: "valid" };
  }

  /** Forgets the state of a node (its tree read again, the user moved on). */
  function settle(id: string): void {
    states.delete(id);
  }

  return {
    canMove,
    moveState,
    move,
    settle,
    /** Whether the node was moved in the active draft (shown at its place, « en brouillon »). */
    inDraft: (id: string) => drafted.has(id),
  };
}

export type Moves = ReturnType<typeof useMoves>;
