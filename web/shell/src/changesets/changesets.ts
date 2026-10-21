// The changesets in the shell (REQ-007-18, design « Écrans et routes du client »): what a
// row and a record show, and which actions a user is offered on a changeset. An aid only:
// the server stays the judge of every action (REQ-007-20).

/** The states of the journal (spec 004). */
export const STATES = [
  "pending",
  "applied",
  "refused",
  "rejected",
  "withdrawn",
  "expired",
  "failed_revalidation",
] as const;

/** A changeset of a list (`GET /v1/changesets`). */
export interface ChangesetRow extends Record<string, unknown> {
  id: string;
  href: string;
  state: string;
  kind: string;
  author: string;
  motive: string;
  submitted_at: string;
}

/** A proposal the caller may validate (`GET /v1/proposals`). */
export interface ProposalRow extends ChangesetRow {
  required: { permission: string; objects: string[] }[];
}

/** A changeset read whole (`GET /v1/changesets/{id}`). */
export interface Changeset extends ChangesetRow {
  valid_from: string;
  valid_to?: string | null;
  applied_at?: string | null;
  expires_at?: string | null;
  validator?: string | null;
  rejection?: string | null;
  policy?: string | null;
  operations: Record<string, unknown>[];
  violations: { rule: string; severity: string; detail: string; subjects: string[] }[];
  diff: {
    property: string;
    before?: string | null;
    after?: string | null;
    count: number;
    masked?: boolean | null;
  }[];
}

/** An action on a changeset, as its route names it (`POST …/{id}/<action>`). */
export type Action = "validation" | "rejection" | "withdrawal" | "undo";

/**
 * The actions offered on `changeset` to the identity `me`: a pending changeset is validated
 * or rejected by someone else, withdrawn by its author; an applied one is undone. Under a
 * past `as_of` (`writes` false), none.
 */
export function actionsOf(
  changeset: Pick<ChangesetRow, "state" | "author">,
  me: string | null,
  writes: boolean,
): Action[] {
  if (!writes || !me) return [];
  if (changeset.state === "pending") {
    return changeset.author === me ? ["withdrawal"] : ["validation", "rejection"];
  }
  return changeset.state === "applied" ? ["undo"] : [];
}

/** The text of a cell of a list of changesets, the caller as `you`. */
export function changesetCell(
  row: Record<string, unknown>,
  key: string,
  me: string | null,
  you: string,
): string {
  const value = row[key];
  if (key === "required" && Array.isArray(value)) {
    return (value as ProposalRow["required"]).map((r) => r.permission).join(", ");
  }
  if (key === "author" && me && value === me) return you;
  if (value === null || value === undefined || value === "") return "—";
  return String(value);
}

/** The operation of an operation (`create_address`…), and its other members as text. */
export function operationText(operation: Record<string, unknown>): { op: string; rest: string } {
  const { op, ...rest } = operation;
  return { op: String(op ?? "—"), rest: JSON.stringify(rest) };
}
