import type { components, Operation } from "@kartenn/sdk";
import type { State } from "@kartenn/ui";

// The VLAN of a network (spec 026, REQ-026-6; 008 REQ-008-17): the property `core.vlan` of a
// prefix of nature `network`, a reference to a VLAN. Its column in the list of prefixes reads
// the properties of each row shown, a few at a time; its tab on the record of the network
// sets it and removes it by one operation on the property.

export type Resolved = components["schemas"]["Resolved"];

/** The state each status of a VLAN shows (maquette E12): its colour, icon and words. */
export const VLAN_STATES: Readonly<Record<string, State>> = {
  active: "ok",
  reserved: "neutral",
  deprecated: "warn",
};

/** The state each run of VIDs of a group shows (maquette E12, tranches). */
export const SLICE_STATES: Readonly<Record<string, State>> = {
  used: "ok",
  reserved: "neutral",
  free: "info",
};

/** The property of a network that names its VLAN (ex-`vlan.vlan`, REQ-026-2). */
export const VLAN_PROPERTY = "core.vlan";

/** The identifier of the VLAN a value of `core.vlan` names: its text, or its `id`. */
export function vlanOfValue(value: unknown): string | null {
  if (typeof value === "string" && value !== "") return value;
  if (typeof value === "object" && value !== null) {
    const id = (value as { id?: unknown }).id;
    if (typeof id === "string") return id;
  }
  return null;
}

/**
 * What the resolved properties of a network say of its VLAN: one by its identifier, none,
 * or masked — the reader may not read the property (003), which is never « no VLAN » and
 * never offers a write that would overwrite it.
 */
export type NetworkVlanState = { kind: "vlan"; id: string } | { kind: "none" } | { kind: "masked" };

export function vlanOfProperties(resolved: Resolved): NetworkVlanState {
  if (resolved.masked.includes(VLAN_PROPERTY)) return { kind: "masked" };
  const properties = resolved.properties as Record<string, unknown>;
  const id = vlanOfValue(properties[VLAN_PROPERTY]);
  return id ? { kind: "vlan", id } : { kind: "none" };
}

/**
 * A VLAN a value of `core.vlan` names: an identifier, or the local name of the VLAN an
 * earlier operation of the same changeset creates (021/A13, `{"local": n}`).
 */
export type VlanValue = string | { local: number };

/**
 * The operation that sets the VLAN of a network (`vlan` an identifier, or a local name), or
 * removes it (`null`): the property by the identifier of its declaration.
 */
export function networkVlanOperation(
  prefix: string,
  property: string,
  vlan: VlanValue | null,
): Operation {
  const object = { id: prefix };
  return vlan === null
    ? { op: "unassign_property", kind: "prefix", object, property: { id: property } }
    : { op: "assign_property", kind: "prefix", object, property: { id: property }, values: [vlan] };
}

/** Whether `operation` (of a draft) writes the property `property` of the network `prefix`. */
function writesNetworkVlan(
  operation: Operation,
  kind: "assign_property" | "unassign_property",
  prefix: string,
  property: string | undefined,
): boolean {
  const op = operation as { op?: unknown; object?: { id?: unknown }; property?: { id?: unknown } };
  return (
    property !== undefined &&
    op.op === kind &&
    op.object?.id === prefix &&
    op.property?.id === property
  );
}

/** Whether `operation` (of a draft) removes the VLAN of the network `prefix`. */
export function isNetworkVlanRemoval(
  operation: Operation,
  prefix: string,
  property: string | undefined,
): boolean {
  return writesNetworkVlan(operation, "unassign_property", prefix, property);
}

/** Whether `operation` (of a draft) sets a VLAN on the network `prefix`. */
export function isNetworkVlanAssignment(
  operation: Operation,
  prefix: string,
  property: string | undefined,
): boolean {
  return writesNetworkVlan(operation, "assign_property", prefix, property);
}

/**
 * A local name no operation of `operations` uses yet (one more than the greatest one met,
 * at any depth: a member `local`, or a reference `{"local": n}`): the names hold for the
 * whole changeset, so the operations added to a draft take one of their own (REQ-004-4).
 */
export function freeLocal(operations: readonly unknown[]): number {
  let most = -1;
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) value.forEach(visit);
    else if (typeof value === "object" && value !== null) {
      for (const [name, member] of Object.entries(value)) {
        if (name === "local" && typeof member === "number") most = Math.max(most, member);
        else visit(member);
      }
    }
  };
  visit(operations);
  return most + 1;
}

/**
 * At most this many reads of the column at once: a list shows dozens of rows, and one read
 * per row all at once overflows the shedding limit of the server (REQ-006-30) and the
 * reader's quota, refusing the other reads of the page with them.
 */
export const COLUMN_READS = 4;
let reading = 0;
const waiting: (() => void)[] = [];

/** Runs `read` within the limit; a read whose row left the page meanwhile is not sent. */
export async function limited<T>(
  signal: AbortSignal | undefined,
  read: () => Promise<T>,
): Promise<T> {
  while (reading >= COLUMN_READS) await new Promise<void>((resolve) => waiting.push(resolve));
  reading++;
  try {
    signal?.throwIfAborted();
    return await read();
  } finally {
    reading--;
    waiting.shift()?.();
  }
}
