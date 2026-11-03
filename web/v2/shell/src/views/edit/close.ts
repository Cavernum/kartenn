import type { Operation, Simulated, Violation } from "@kartenn/sdk";

// « Clore… » of a record (021/A31, task 11g; REQ-004-25, 26; REQ-021-37): the closure of an
// object of the core, to the draft or applied at once with its motive, the version served
// with the value read as `expected`, never one read apart. Before it is sent, its simulation
// alone on the referential (`POST /v1/changesets?simulate=true`) says what it would do: the
// rules that refuse it, with their codes, and with the cascade the dependents it closes too.

/**
 * The kinds whose closure may take dependents along (REQ-004-26): the ranges and addresses
 * of a network, the objects of a domain, the children of an entity or a site, the VLAN of a
 * group. A closure of the others closes the object alone.
 */
export const CASCADING = new Set(["domain", "prefix", "entity", "site", "vlan_group"]);

/** The rule of a closure refused because objects of a module still refer to the object. */
export const DEPENDENTS_RULE = "core.object.dependents";

/** The rule of an operation written on a version that is no longer the current one. */
export const CONFLICT_RULE = "changeset.operation.conflict";

/** The operation that closes the object `id` of `kind`, read at `expected`. */
export function closeOperation(
  kind: string,
  id: string,
  expected: string | null,
  cascade: boolean,
): Operation {
  return {
    op: "close",
    kind: kind as Extract<Operation, { op: "close" }>["kind"],
    id: { id },
    ...(expected ? { expected } : {}),
    ...(cascade ? { cascade: true } : {}),
  };
}

/** What the simulation of a closure says, for the panel. */
export interface CloseVerdict {
  /** No violation is an error. */
  valid: boolean;
  /** The errors, then the warnings, each with its code. */
  errors: readonly Violation[];
  warnings: readonly Violation[];
  /** The closures the cascade adds, counted by kind, in the order first met. */
  cascaded: readonly { kind: string; count: number }[];
  /** Whether the refusal may come from dependents: the cascade is not asked, and could be. */
  dependents: boolean;
}

/** The verdict of the simulation of the closure of an object of `kind`. */
export function closeVerdict(kind: string, simulated: Simulated, cascade: boolean): CloseVerdict {
  const violations = simulated.violations as readonly Violation[];
  const errors = violations.filter((v) => v.severity === "error");
  const counts = new Map<string, number>();
  for (const c of simulated.cascaded) counts.set(c.kind, (counts.get(c.kind) ?? 0) + 1);
  return {
    valid: simulated.valid,
    errors,
    warnings: violations.filter((v) => v.severity !== "error"),
    cascaded: [...counts].map(([k, count]) => ({ kind: k, count })),
    dependents:
      !cascade &&
      errors.length > 0 &&
      (CASCADING.has(kind) || errors.some((v) => v.rule === DEPENDENTS_RULE)),
  };
}

/** The type of a kind for the words of the dictionary (`address` → `core.address`). */
export const typeOfKind = (kind: string) => (kind.includes(".") ? kind : `core.${kind}`);
