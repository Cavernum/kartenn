import {
  ApiProblem,
  canMove as check,
  changesets,
  draft,
  drafts,
  FORBIDDEN_RULE,
  queryClient,
  reads,
  type Draft,
  type Entry,
  type MoveSubject,
  type MoveVerdict,
  type Operation,
  type Simulated,
  type Violation,
  type WriteOutcome,
} from "@kartenn/sdk";
import type { MoveCheck, MoveIntent, MoveState, TreeNode } from "@kartenn/ui";
import { shallowReactive } from "vue";
import { useI18n } from "vue-i18n";
import { useSession } from "../../rights/index.ts";
import { useTime } from "../../time/index.ts";

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

/** How a move ended: in the draft, applied, held, or refused at its place. */
export type MoveOutcome =
  | { kind: "drafted"; draft: Draft; valid: boolean }
  | WriteOutcome<unknown>
  | { kind: "unchanged" }
  | { kind: "refused"; check: Refused };

/** A target refused, with its reason and code. */
export type Refused = Extract<MoveCheck, { ok: false }>;

/** A node that does not move, or a target of no admitted type. */
const INCOMPATIBLE = { ok: false, refusal: "incompatible" } as const;

/** The violation of a simulation that refuses the operation `index` of node `id`. */
function violationOf(simulated: Simulated, index: number, id: string): Violation | undefined {
  const errors = simulated.violations.filter((v) => v.severity === "error");
  return (
    errors.find((v) => v.operation === index) ??
    errors.find((v) => v.subjects.includes(id)) ??
    errors[0]
  );
}

export function useMoves(model: MoveModel) {
  const { t } = useI18n();
  const session = useSession();
  const time = useTime();
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

  /** `canMove` of the tree: for each target hovered or listed, synchronous. */
  function canMove(node: TreeNode, target: TreeNode | null): MoveCheck {
    const subject = model.subject(node);
    const to = target ? model.subject(target) : null;
    if (!subject || (target && !to)) return refusedFor(INCOMPATIBLE, target);
    return said(check(subject, to ?? null, env()), target);
  }

  /** The state of a node: its move in flight, or refused with its code. */
  function moveState(node: TreeNode): MoveState | undefined {
    return states.get(node.id);
  }

  /** The version the operation expects: the host's, else the object read by identifier. */
  async function versionOf(node: MoveNode): Promise<string | null> {
    if (node.version !== undefined) return node.version;
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
    const reason = first?.detail ?? problem?.title ?? t("edit.network");
    const code = first?.rule ?? problem?.code;
    return { ok: false, reason, ...(code ? { code } : {}) };
  }

  /**
   * Moves `intent.node` under `intent.target`, checked again first: to the draft by default,
   * applied now (`now`, with its motive) otherwise. Nothing of the tree is written here.
   */
  async function move(
    intent: MoveIntent,
    options: { now?: boolean; motive?: string; idempotencyKey?: string } = {},
  ): Promise<MoveOutcome> {
    const { node, target } = intent;
    const subject = model.subject(node);
    const to = target ? model.subject(target) : null;
    const verdict = canMove(node, target);
    if (!verdict.ok) return refuse(node.id, verdict);
    if (!subject || states.get(node.id) === "pending") return { kind: "unchanged" };
    if ((subject.parent ?? null) === (to?.id ?? null)) return { kind: "unchanged" };
    const operation = OPERATIONS.get(subject.type);
    if (!operation) return refuse(node.id, refusedFor(INCOMPATIBLE, target));
    states.set(node.id, "pending");
    drafted.delete(node.id);
    const op = operation(subject, await versionOf(subject), to ?? null);
    if (options.now) {
      const outcome = await changesets.submit(
        { motive: options.motive ?? "", operations: [op] },
        options.idempotencyKey ?? crypto.randomUUID(),
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
    const added = await draft.add(op);
    if (added.kind !== "saved") {
      refuse(node.id, refusalOf(added));
      return added as WriteOutcome<unknown>;
    }
    // In the draft: its simulation says whether the move holds; the node stays at its place,
    // marked as moved in the draft, the preview of the draft showing the new place.
    const simulated = await queryClient
      .fetchQuery(drafts.simulate(added.draft))
      .catch(() => undefined);
    if (!simulated) {
      states.delete(node.id);
      drafted.add(node.id);
      return { kind: "drafted", draft: added.draft, valid: true };
    }
    const index = (added.draft.operations?.length ?? 1) - 1;
    const refused = simulated.valid ? undefined : violationOf(simulated, index, node.id);
    if (refused) {
      refuse(node.id, { ok: false, reason: refused.detail, code: refused.rule });
      return { kind: "drafted", draft: added.draft, valid: false };
    }
    states.delete(node.id);
    drafted.add(node.id);
    return { kind: "drafted", draft: added.draft, valid: true };
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
