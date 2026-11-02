import type { Operation } from "@kartenn/sdk";

// An operation of a changeset or a draft as a line of a list (REQ-021-10, 11; maquette E5):
// its code, the object it names — the value it creates, else the identifier of the object
// it writes — and its other members, in the order of the contract, as `name: value`.

export interface OperationLine {
  /** The code of the operation (`update_prefix`), shown in fixed pitch. */
  op: string;
  /** What it names: the value created (`192.0.2.0/24`), a slug, a key, an identifier. */
  subject: string | null;
  /** Its other members, `name: value`, each value as text. */
  members: { name: string; value: string }[];
}

/** The members that name what an operation creates or writes, in order of preference. */
const SUBJECTS = ["prefix", "address", "first", "slug", "key", "vid", "name", "id", "object"];
/** Members said elsewhere or of no use to read (the version read, the local name). */
const HIDDEN = new Set(["op", "expected", "local"]);

/**
 * A member value as text: a reference by its identifier (a local name as `#n`), a list of
 * values joined, JSON otherwise.
 */
export function memberText(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  // The values of an assignment: each as text (`#0` for the VLAN an earlier operation creates).
  if (Array.isArray(value) && value.length > 0) return value.map(memberText).join(", ");
  if (typeof value === "object" && !Array.isArray(value)) {
    const ref = value as Record<string, unknown>;
    if (typeof ref.id === "string" && Object.keys(ref).length === 1) return ref.id;
    if (typeof ref.local === "number" && Object.keys(ref).length === 1) return `#${ref.local}`;
  }
  return JSON.stringify(value);
}

export function operationLine(operation: Operation | Record<string, unknown>): OperationLine {
  const op = operation as Record<string, unknown>;
  if (op.op === "within" && typeof op.operation === "object" && op.operation !== null) {
    const inner = operationLine(op.operation as Record<string, unknown>);
    return {
      ...inner,
      members: [...inner.members, { name: "period", value: memberText(op.period) }],
    };
  }
  const subjectName = SUBJECTS.find((name) => op[name] !== undefined && op[name] !== null);
  const first = op.first !== undefined && op.last !== undefined;
  const subject = subjectName
    ? first && subjectName === "first"
      ? `${memberText(op.first)}–${memberText(op.last)}`
      : memberText(op[subjectName])
    : null;
  const members = Object.entries(op)
    .filter(
      ([name, value]) =>
        !HIDDEN.has(name) &&
        name !== subjectName &&
        !(first && name === "last") &&
        value !== undefined &&
        value !== null,
    )
    .map(([name, value]) => ({ name, value: memberText(value) }));
  return { op: String(op.op ?? "—"), subject, members };
}
