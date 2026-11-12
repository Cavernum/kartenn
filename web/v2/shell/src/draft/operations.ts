import type { Operation } from "@kartenn/sdk";

// An operation of a changeset or a draft as a line of a list (REQ-021-10, 11; maquette E5):
// its code, the object it names — the value it creates, else the identifier of the object
// it writes — and its other members, in the order of the contract, as `name: value`.

export interface OperationLine {
  /** The code of the operation (`update_prefix`), shown in fixed pitch. */
  op: string;
  /** What it names: the value created (`192.0.2.0/24`), a slug, a key, an identifier. */
  subject: string | null;
  /**
   * Its other members, `name: value`, each value as text (`null`: absent, `NoValue`);
   * `masked`: the values the reader may not read, said « masqué » (REQ-005-21).
   */
  members: { name: string; value: string | null; masked?: true }[];
}

/** The members that name what an operation creates or writes, in order of preference. */
const SUBJECTS = ["prefix", "address", "first", "slug", "key", "vid", "name", "id", "object"];
/**
 * Members said elsewhere or of no use to read (the version read, the local name; `masked`,
 * said by the values it hides).
 */
const HIDDEN = new Set(["op", "expected", "local", "masked"]);

/**
 * A member value as text: a reference by its identifier (a local name as `#n`), a list of
 * values joined, JSON otherwise; absent, `null`, said by the line (`NoValue`).
 */
export function memberText(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  // The values of an assignment: each as text (`#0` for the VLAN an earlier operation creates),
  // an item absent as its JSON (`null`).
  if (Array.isArray(value) && value.length > 0)
    return value.map((item) => memberText(item) ?? "null").join(", ");
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
  // A range by its two ends; an end absent is not written (absent members are left out).
  const first = op.first != null && op.last != null;
  const subject = subjectName
    ? first && subjectName === "first"
      ? `${memberText(op.first)}–${memberText(op.last)}`
      : memberText(op[subjectName])
    : null;
  // A sensitive value served masked (`masked: true`, `values: null`): its values said
  // « masqué » as the differential says them, never absent, and never shown beside the
  // word were they served all the same (REQ-005-21, task 14g).
  const masked = op.masked === true;
  const members: OperationLine["members"] = Object.entries(op)
    .filter(
      ([name, value]) =>
        !HIDDEN.has(name) &&
        name !== subjectName &&
        !(first && name === "last") &&
        !(masked && name === "values") &&
        value !== undefined &&
        value !== null,
    )
    .map(([name, value]) => ({ name, value: memberText(value) }));
  if (masked) members.push({ name: "values", value: null, masked: true });
  return { op: String(op.op ?? "—"), subject, members };
}

type AnyOperation = Operation | Record<string, unknown>;

/** Whether an operation is a guard (`expect`): kept with the one it guards, never listed. */
export const isGuard = (operation: AnyOperation): boolean =>
  (operation as Record<string, unknown>).op === "expect";

/** The object an operation writes or guards, by identifier (`id`, else `object`). */
export function objectOf(operation: AnyOperation): string | null {
  const op = operation as Record<string, unknown>;
  const ref = (op.id ?? op.object) as { id?: unknown } | undefined;
  return typeof ref?.id === "string" ? ref.id : null;
}

/**
 * The operations of a draft as its list shows them, guards left out (Q-021-21): each with
 * its index in the draft and the indexes of the guards it carries — those of its object no
 * earlier operation listed carries (their violations are said on it).
 */
export function listedOperations<T extends AnyOperation>(
  operations: readonly T[],
): { operation: T; index: number; guards: number[] }[] {
  const out: { operation: T; index: number; guards: number[] }[] = [];
  const carried = new Set<number>();
  operations.forEach((operation, index) => {
    if (isGuard(operation)) return;
    const object = objectOf(operation);
    const guards: number[] = [];
    operations.forEach((other, at) => {
      if (object !== null && isGuard(other) && !carried.has(at) && objectOf(other) === object) {
        guards.push(at);
        carried.add(at);
      }
    });
    out.push({ operation, index, guards });
  });
  return out;
}

/**
 * The operations of a draft without the one at `index`, and without a guard left with no
 * operation on its object (Q-021-21: a guard goes with what it guards).
 */
export function withoutOperation<T extends AnyOperation>(
  operations: readonly T[],
  index: number,
): T[] {
  const kept = operations.filter((_, at) => at !== index);
  const written = new Set(kept.filter((op) => !isGuard(op)).map(objectOf));
  return kept.filter((op) => !isGuard(op) || written.has(objectOf(op)));
}
