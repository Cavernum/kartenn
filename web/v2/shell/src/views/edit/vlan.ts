import {
  changesets,
  queryClient,
  reads,
  type components,
  type Entry,
  type WriteOutcome,
} from "@kartenn/sdk";
import { rowsOf } from "../data.ts";
import { networkVlanOperation } from "../parts/vlan.ts";
import { EDITS, type Values } from "./fields.ts";

// The creation of a VLAN in the edit panel (026, REQ-008-14, 17, 20; maquette E12): the VID
// proposed is the next free one of the group chosen; created for a network, the VLAN is
// applied first, read back by its group and VID, then set on the network by its property
// `core.vlan` — a value of a property names an existing object, never one created earlier
// in the same changeset (`RawValue` has no local name), so neither step goes to a draft.

type NextFree = components["schemas"]["VlanNextFree"];

/** The next free VID of a group at the present, `null` if none or unread. */
export async function nextFreeVid(group: string): Promise<number | null> {
  const read = reads.sub<NextFree>("vlan_group", group, "next-free");
  const free = await queryClient
    .fetchQuery({ ...read, staleTime: 0 })
    .catch(() => null as Entry<NextFree> | null);
  return free?.data.vid ?? null;
}

/** What the creation of a VLAN for a network came to, and the VLAN created, if it was. */
export interface NetworkOutcome {
  /** The first refusal or hold, else the setting of the property. */
  outcome: WriteOutcome<unknown>;
  /** Whether the VLAN was created (a refusal then is the setting's). */
  created: boolean;
}

/**
 * Creates the VLAN of `values` at once (`?approval=self`), then sets it on `network` by the
 * declaration `property` of `core.vlan`, at once too.
 */
export async function createForNetwork(options: {
  values: Values;
  motive: string;
  network: string;
  property: string;
  idempotencyKey: string;
}): Promise<NetworkOutcome> {
  const { values, motive } = options;
  const operation = EDITS.vlans.create(values);
  const created = await changesets.submit(
    { motive, operations: [operation] },
    options.idempotencyKey,
    { approvalSelf: true },
  );
  if (created.kind !== "applied") return { outcome: created, created: false };
  // The answer names the changeset: the VLAN is read back by its group and VID.
  const list = await queryClient
    .fetchInfiniteQuery({
      ...reads.list("/v1/vlans", { group: String(values.group), vid: String(values.vid) }),
      staleTime: 0,
    })
    .catch(() => undefined);
  const vlan = rowsOf(list)[0];
  if (!vlan) return { outcome: { kind: "failed", input: {}, problem: null }, created: true };
  const set = await changesets.submit(
    {
      motive,
      operations: [networkVlanOperation(options.network, options.property, vlan.id)],
    },
    crypto.randomUUID(),
    { approvalSelf: true },
  );
  return { outcome: set, created: true };
}
