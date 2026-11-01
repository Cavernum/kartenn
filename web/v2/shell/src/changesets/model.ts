import type { components, Operation } from "@kartenn/sdk";
import type { DiffChange, DiffGroup, DiffKind, State } from "@kartenn/ui";
import type { Rights } from "../rights/index.ts";

// The changesets in the shell (REQ-021-10; design 021 « Écrans » : Changesets; maquette E4):
// what a state shows, which permissions the operations of a changeset or a draft ask for,
// the review actions a reader is offered — refused ones kept, with the permission lacking
// or the reason (REQ-021-37) — and the differential grouped by object for `DiffList`. An
// aid only: the server stays the judge of every decision.

export type Changeset = components["schemas"]["ChangesetDto"];
export type ChangesetItem = components["schemas"]["ChangesetItem"];
export type ProposalItem = components["schemas"]["ProposalItem"];
export type DiffEntry = components["schemas"]["DiffEntry"];

/** The states of the journal (spec 004), in the order of the filter. */
export const STATES = [
  "pending",
  "applied",
  "rejected",
  "withdrawn",
  "expired",
  "refused",
  "failed_revalidation",
] as const;

/** The kinds of a changeset (`direct`, `approval`, `proposal`). */
export const KINDS = ["direct", "approval", "proposal"] as const;

/** The badge of a state: its colour and icon by meaning, its label by the catalogs. */
export function stateBadge(state: string): State {
  switch (state) {
    case "pending":
      return "review";
    case "applied":
      return "ok";
    case "rejected":
    case "refused":
    case "failed_revalidation":
      return "ko";
    case "expired":
      return "warn";
    default:
      return "neutral";
  }
}

/** The kinds of the core an operation may name, bare as in the operations (`prefix`). */
const CORE_KINDS = new Set([
  "domain",
  "prefix",
  "range",
  "address",
  "entity",
  "site",
  "vlan",
  "vlan_group",
]);

/**
 * The qualified type an operation writes (`core.prefix`), when it says it: `create_<kind>`,
 * `update_<kind>`, `move_<kind>`, or its `kind` member (`close`, `rename`, `restore`, the
 * properties); `null` otherwise (an operation of the rights, of a module): the server alone
 * knows what it asks for.
 */
export function typeOfOperation(operation: Operation | Record<string, unknown>): string | null {
  const op = operation as Record<string, unknown>;
  if (op.op === "within" && typeof op.operation === "object" && op.operation !== null) {
    return typeOfOperation(op.operation as Record<string, unknown>);
  }
  const named = /^(?:create|update|move)_([a-z_]+)$/.exec(String(op.op ?? ""))?.[1];
  const kind = named ?? (typeof op.kind === "string" ? op.kind : undefined);
  return kind && CORE_KINDS.has(kind) ? `core.${kind}` : null;
}

/** What a reviewer does with the objects of a changeset (024, REQ-024-9). */
export type Verb = "apply" | "approval.self" | "edit";

/** The permissions of `verb` on the types the operations name, in order, each once. */
export function permissionsOf(
  operations: readonly (Operation | Record<string, unknown>)[],
  verb: Verb,
): string[] {
  const types = operations.map(typeOfOperation).filter((t): t is string => t !== null);
  return [...new Set(types)].map((type) => `${type}.${verb}`);
}

/** The first permission of `verb` the operations ask for that `rights` lacks, if one. */
export function lacking(
  rights: Rights,
  operations: readonly (Operation | Record<string, unknown>)[],
  verb: Verb,
): string | undefined {
  return permissionsOf(operations, verb).find((permission) => !rights.has(permission));
}

/** A review action of the record of a changeset. */
export interface ReviewAction {
  id: "validation" | "rejection" | "withdrawal" | "undo";
  /** The permission lacking (REQ-021-37), named as served. */
  forbidden?: string;
  /** Why it is refused otherwise: its key in the catalogs. */
  reason?: "author" | "past" | "held";
}

/**
 * Whether a policy holds a pending changeset (`kind = approval`, its `policy`): its author
 * may not apply it, whatever their rights (spec 024, REQ-024-12; `self_applicable` of
 * `kartenn-service/src/review.rs`). A review a module requires is not served: the server
 * alone refuses it then.
 */
export function held(changeset: Pick<Changeset, "kind" | "policy">): boolean {
  return changeset.kind === "approval" || !!changeset.policy;
}

export interface ReviewContext {
  /** The identity of the reader. */
  me: string | null;
  rights: Rights;
  /** At the present: nothing is decided under a past date. */
  present: boolean;
}

/**
 * The review actions of a changeset for its reader (REQ-021-10, maquette E4; spec 024,
 * REQ-024-11, 12): pending, its validation — by someone else with `<type>.apply` on what it
 * writes; by its author with `<type>.approval.self` on all of it when no policy holds it
 * (else refused, `held`) — its rejection (by someone else) and its withdrawal (by its
 * author only); applied, its undo (`<type>.edit`). Every action refused stays, with the
 * permission lacking or its reason (REQ-021-37).
 */
export function reviewActions(changeset: Changeset, context: ReviewContext): ReviewAction[] {
  const { me, rights, present } = context;
  const author = me !== null && changeset.author === me;
  const past = present ? {} : { reason: "past" as const };
  if (changeset.state === "pending") {
    const lack = lacking(rights, changeset.operations, author ? "approval.self" : "apply");
    const decide = !present
      ? past
      : author && held(changeset)
        ? { reason: "held" as const }
        : lack
          ? { forbidden: lack }
          : {};
    // Its author: « Valider » (or its refusal), no « Rejeter », « Retirer » (E4).
    return [
      { id: "validation", ...decide },
      ...(author ? [] : [{ id: "rejection" as const, ...decide }]),
      { id: "withdrawal", ...(!present ? past : author ? {} : { reason: "author" as const }) },
    ];
  }
  if (changeset.state === "applied") {
    const edit = lacking(rights, changeset.operations, "edit");
    return [{ id: "undo", ...(!present ? past : edit ? { forbidden: edit } : {}) }];
  }
  return [];
}

/** A value of the differential as text: a string as it is, anything else as JSON. */
function shown(value: unknown, masked: string): string | undefined {
  if (value === masked) return masked;
  if (value === null || value === undefined) return undefined;
  return typeof value === "string" ? value : JSON.stringify(value);
}

function kindOf(before: string | undefined, after: string | undefined): DiffKind {
  if (before === undefined) return "added";
  if (after === undefined) return "removed";
  return "changed";
}

export interface DiffWords {
  /** The label of a property key, `null` when only its key is known. */
  property: (key: string) => string | null;
  /** What a masked value shows (« masqué »). */
  masked: string;
  /** The title of the object of a group: its name when known, else its identifier. */
  object: (id: string) => string;
}

/**
 * The entries of the differential grouped by object, in the order they come (REQ-003-19,
 * `DiffList`): one group per object, a line per property — added, removed or changed,
 * by its values — the group added or removed when all its lines are, changed otherwise.
 */
export function diffGroups(entries: readonly DiffEntry[], words: DiffWords): DiffGroup[] {
  const groups = new Map<string, DiffGroup>();
  for (const entry of entries) {
    const id = entry.object.id;
    let group = groups.get(id);
    if (!group) {
      const title = words.object(id);
      // The identifier beside a name only: never twice.
      group = {
        id,
        title,
        ...(title !== id ? { objectKey: id } : {}),
        kind: "changed",
        changes: [],
      };
      groups.set(id, group);
    }
    const before = entry.masked ? words.masked : shown(entry.before, words.masked);
    const after = entry.masked ? words.masked : shown(entry.after, words.masked);
    const label = words.property(entry.property);
    const change: DiffChange = {
      attribute: label ? `${label} (${entry.property})` : entry.property,
      kind: kindOf(before, after),
      ...(before !== undefined ? { before } : {}),
      ...(after !== undefined ? { after } : {}),
    };
    group.changes.push(change);
  }
  return settled(groups);
}

/** Each group added or removed when all its lines are, changed otherwise. */
function settled(groups: Map<string, DiffGroup>): DiffGroup[] {
  for (const group of groups.values()) {
    const kinds = new Set(group.changes.map((change) => change.kind));
    group.kind = kinds.size === 1 ? [...kinds][0]! : "changed";
  }
  return [...groups.values()];
}

export type DiffCount = components["schemas"]["DiffCountDto"];

/** A value of the counted differential, served as JSON text: a string as it is. */
function counted(text: string | null | undefined): string | undefined {
  if (text === null || text === undefined) return undefined;
  try {
    const value: unknown = JSON.parse(text);
    return value === null ? undefined : typeof value === "string" ? value : JSON.stringify(value);
  } catch {
    return text;
  }
}

/**
 * The counted differential (REQ-003-19) grouped by property: a line per transition, how
 * many objects see it (`objects`), masked values said. What a changeset not applied yet
 * shows: its objects are written only once it applies.
 */
export function countedGroups(
  counts: readonly DiffCount[],
  words: Omit<DiffWords, "object"> & { objects: (n: number) => string },
): DiffGroup[] {
  const groups = new Map<string, DiffGroup>();
  for (const count of counts) {
    let group = groups.get(count.property);
    if (!group) {
      const label = words.property(count.property);
      group = {
        id: count.property,
        title: label ?? count.property,
        ...(label ? { objectKey: count.property } : {}),
        kind: "changed",
        changes: [],
      };
      groups.set(count.property, group);
    }
    const before = count.masked ? words.masked : counted(count.before);
    const after = count.masked ? words.masked : counted(count.after);
    group.changes.push({
      attribute: words.objects(count.count),
      kind: kindOf(before, after),
      ...(before !== undefined ? { before } : {}),
      ...(after !== undefined ? { after } : {}),
    });
  }
  return settled(groups);
}
