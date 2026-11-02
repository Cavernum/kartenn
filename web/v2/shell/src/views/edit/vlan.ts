import { drafts, queryClient, reads, type components, type Operation } from "@kartenn/sdk";
import { freeLocal, networkVlanOperation } from "../parts/vlan.ts";

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
 * A local name the active draft does not use yet (its operations read again): the names
 * hold for the whole draft, submitted as one changeset. Without a draft, or one that could
 * not be read (the SDK then creates another), the first.
 */
export async function draftLocal(): Promise<number> {
  const id = drafts.active();
  if (id === null) return 0;
  try {
    const draft = await queryClient.fetchQuery({ ...drafts.get(id), staleTime: 0 });
    return freeLocal(draft.data.operations);
  } catch {
    return 0;
  }
}

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
