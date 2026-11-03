// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What the elements read beside their own lists: the names of the objects they cite (an
// owner, a host, a VLAN), read once each through the shared cache, a few at a time; and
// what they compute from it (802.1Q mode).

import { queryClient, reads, type components, type Entry } from "@kartenn/sdk";
import { t } from "./dom.ts";

export type Machine = components["schemas"]["Machine"];
export type MachineCard = components["schemas"]["MachineCard"];
export type Interface = components["schemas"]["Interface"];
export type Cluster = components["schemas"]["Cluster"];
export type Vlan = components["schemas"]["Vlan"];

/** The kinds of a machine (REQ-009-2). */
export const KINDS = ["physical", "virtual", "container", "appliance"] as const;

/**
 * At most this many reads of names and cells at once: a list shows dozens of rows, and one
 * read per row all at once overflows the server's shedding limit (REQ-006-30) and the
 * reader's quota, refusing the other requests of the page with them.
 */
export const READS_AT_ONCE = 4;
let reading = 0;
const queued: (() => void)[] = [];

/** Runs `read` within the limit; `null`, without reading, if its place is gone by its turn. */
export async function limited<T>(live: () => boolean, read: () => Promise<T>): Promise<T | null> {
  while (reading >= READS_AT_ONCE) await new Promise<void>((resolve) => queued.push(resolve));
  reading++;
  try {
    return live() ? await read() : null;
  } finally {
    reading--;
    queued.shift()?.();
  }
}

/** How each type of object cited is named. */
const NAMERS: Record<string, (object: never) => string> = {
  "core.entity": (o: { slug: string }) => o.slug,
  "core.site": (o: { slug: string }) => o.slug,
  "core.vlan": (o: Vlan) => String(o.vid),
  "machines.machine": (o: Machine) => o.name,
  "machines.cluster": (o: Cluster) => o.slug,
};

/** A name: read (`name`), still reading, or unreadable (its identifier is shown, in mono). */
export type Name = { state: "read"; name: string } | { state: "reading" } | { state: "failed" };

/**
 * The names of the objects an element cites, from the shared cache at its dates; one read
 * per object not held yet, a few at a time, then `changed` so that the element draws them.
 */
export class Names {
  private readonly failed = new Set<string>();
  private readonly asked = new Set<string>();

  constructor(
    private readonly changed: () => void,
    private readonly live: () => boolean = () => true,
  ) {}

  of(type: keyof typeof NAMERS, id: string): Name {
    const options = reads.object(type, id);
    const hash = JSON.stringify(options.queryKey);
    const held = queryClient.getQueryData<Entry<never>>(options.queryKey);
    if (held) return { state: "read", name: NAMERS[type]!(held.data) };
    if (this.failed.has(hash)) return { state: "failed" };
    if (!this.asked.has(hash)) {
      this.asked.add(hash);
      void limited(this.live, () => queryClient.fetchQuery(options)).then(
        () => {
          this.asked.delete(hash);
          this.changed();
        },
        () => {
          this.failed.add(hash);
          this.changed();
        },
      );
    }
    return { state: "reading" };
  }
}

/** `access 120`, `trunk 120, 130`: the 802.1Q mode of an interface, its VLAN by their VID. */
export function modeOf(iface: Interface, names: Names): string {
  const vid = (id?: string | null) => {
    if (!id) return "";
    const name = names.of("core.vlan", id);
    return name.state === "read" ? name.name : "?";
  };
  switch (iface.vlan.mode) {
    case "access":
      return `${t("mode.access")} ${vid(iface.vlan.access?.id)}`;
    case "trunk":
      return `${t("mode.trunk")} ${iface.vlan.tagged.map((v) => vid(v.id)).join(", ")}`;
    default:
      return t("mode.none");
  }
}
