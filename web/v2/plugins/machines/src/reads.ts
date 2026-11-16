// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What the elements read beside their own lists, and what they compute from the summaries
// of the bodies (H23). The lists and the records name what they cite by those summaries,
// nothing read (tasks 11p10-d3, d4); only an identifier without a summary is still read by
// name, once through the shared cache, a few at a time: that of a filter of the list of the
// machines given by the address of the page.

import { queryClient, reads, type components, type Entry } from "@kartenn/sdk";
import { t } from "./dom.ts";

export type Machine = components["schemas"]["Machine"];
export type MachineCard = components["schemas"]["MachineCard"];
export type Interface = components["schemas"]["Interface"];
export type Cluster = components["schemas"]["Cluster"];
export type Vlan = components["schemas"]["Vlan"];
/**
 * The summary of an object a body cites (H23): `label` and `href` absent together if the
 * caller may not read it or if it was absent then (REQ-006-42).
 */
export type Summary = Pick<components["schemas"]["Summary"], "id" | "label" | "href">;

/**
 * Whether the summary was read: a `label` (an empty one is none) or an `href` (REQ-006-42
 * omits them together); the rule of `SummaryName` of the shell.
 */
export const readable = (summary: Summary): boolean => Boolean(summary.label || summary.href);

/** The kinds of a machine (REQ-009-2). */
export const KINDS = ["physical", "virtual", "container", "appliance"] as const;

/**
 * At most this many reads at once, of names and of the guests of the nodes opened in the
 * tree of hosting: one read per row all at once overflows the server's shedding limit
 * (REQ-006-30) and the reader's quota, refusing the other requests of the page with them.
 * The interfaces of the machine column are read by window instead (`holders.ts`).
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

/**
 * `access 120 · users`, `trunk 120 · users, 130 · servers`: the 802.1Q mode of an interface,
 * its VLAN by the `label` of their summaries (H23), nothing read; a VLAN the caller may not
 * read, or absent then, by `?`.
 */
export function modeOf(iface: Interface): string {
  const vid = (vlan?: Summary | null) => (vlan ? vlan.label || "?" : "");
  switch (iface.vlan.mode) {
    case "access":
      return `${t("mode.access")} ${vid(iface.vlan.access)}`;
    case "trunk":
      return `${t("mode.trunk")} ${iface.vlan.tagged.map(vid).join(", ")}`;
    default:
      return t("mode.none");
  }
}
