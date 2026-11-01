import { queryClient, reads, type components } from "@kartenn/sdk";

// The creation of a VLAN in the edit panel (026, REQ-008-14, 20; maquette E12): the VID
// proposed is the next free one of the group chosen. Creating a VLAN for a network (and
// setting it on it) waits for an arbitration (task 021-17: a value of a property names no
// object created earlier in the same changeset).

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
