import { queryClient, reads, type components, type Operation, type Violation } from "@kartenn/sdk";
import type { CollectionId } from "../collections.ts";
import { freeLocal, networkVlanOperation, VLAN_PROPERTY } from "../parts/vlan.ts";
import { editRight, fieldOfPointer, propertyRight } from "./fields.ts";

// The creation of a VLAN in the edit panel (026, REQ-008-14, 20; maquette E12): the VID
// proposed is the next free one of the group chosen. « Créer un VLAN pour ce réseau »
// (REQ-026-6, 021/A13): the creation, then the property `core.vlan` of the network set to
// it by its local name, two operations of one changeset — to the draft, or applied at once.

type NextFree = components["schemas"]["VlanNextFree"];

/**
 * The next free VID of a group at the present, `null` if the group has none; a refused read
 * throws, for the panel to say it by `problems/`.
 */
export async function nextFreeVid(group: string): Promise<number | null> {
  const read = reads.sub<NextFree>("vlan_group", group, "next-free");
  const free = await queryClient.fetchQuery({ ...read, staleTime: 0 });
  return free.data.vid ?? null;
}

/** The network a VLAN created is set on: its identifier, its name, the declaration of `core.vlan`. */
export interface VlanNetwork {
  id: string;
  name: string;
  /** The identifier of the declaration of `core.vlan`, which the operation names. */
  property: string;
}

/**
 * The two operations of « Créer un VLAN pour ce réseau »: the creation of the VLAN under the
 * local name `local`, then `core.vlan` of the network set to that name.
 */
export function vlanForNetwork(
  creation: Operation,
  network: VlanNetwork,
  local: number,
): [Operation, Operation] {
  return [
    { ...creation, local } as Operation,
    networkVlanOperation(network.id, network.property, { local }),
  ];
}

/**
 * The two operations made from those of the draft they are added to (the SDK reads it, again
 * after a 412): under a local name free in it, the names holding for the whole draft,
 * submitted as one changeset; a new draft, the first.
 */
export const vlanForNetworkInDraft =
  (creation: Operation, network: VlanNetwork) => (current: readonly Operation[]) =>
    vlanForNetwork(creation, network, freeLocal(current));

/**
 * The edit permissions the setting of `core.vlan` on a network needs, beside those of what
 * the action writes: the network, and the property (the approval of each to apply).
 */
export const NETWORK_VLAN_EDITS: readonly string[] = [
  editRight("core.prefix"),
  propertyRight(VLAN_PROPERTY),
];

/**
 * Whether a violation of « Créer un VLAN pour ce réseau » is about the second operation, the
 * setting of `core.vlan` (`prop.assignment.invalid_value`, `prop.assignment.unknown_local`),
 * by its index or its pointer: said on the field of the network.
 */
export const ofAssignment = (violation: {
  operation?: number | null;
  pointer?: string | null;
}): boolean =>
  violation.operation === 1 || /^\/operations\/1(?:\/|$)/.test(violation.pointer ?? "");

type FieldViolation = Pick<Violation, "rule" | "detail" | "pointer" | "operation">;

/**
 * The errors of the fields of a creation refused (422): each violation on the field its
 * pointer names (`names`, the fields shown), with its code; for a VLAN created for a network
 * (`network`), the setting of `core.vlan` on the field `network`.
 */
export function fieldErrors(
  collection: CollectionId,
  names: ReadonlySet<string>,
  violations: readonly FieldViolation[],
  network: boolean,
): Record<string, string> {
  const errors: Record<string, string> = {};
  for (const violation of violations) {
    const name =
      network && ofAssignment(violation)
        ? "network"
        : fieldOfPointer(collection, violation.pointer);
    if (!name || !(names.has(name) || (network && name === "network"))) continue;
    const said = `${violation.detail} (${violation.rule})`;
    errors[name] = errors[name] ? `${errors[name]} ${said}` : said;
  }
  return errors;
}
