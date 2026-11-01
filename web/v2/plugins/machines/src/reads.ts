// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What the elements read beside their own lists: the names of the objects they cite (an
// owner, a host, a VLAN), read once each through the shared cache, a few at a time; and
// what they compute from it (802.1Q mode, an address within a network).

import { queryClient, reads, type components, type Entry } from "@kartenn/sdk";
import { t } from "./dom.ts";

export type Machine = components["schemas"]["Machine"];
export type MachineCard = components["schemas"]["MachineCard"];
export type MachineAddress = components["schemas"]["MachineAddress"];
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

/** An address as a number and its family; `null` if it is not one. */
function parse(value: string): { family: 4 | 6; value: bigint } | null {
  if (value.includes(".") && !value.includes(":")) {
    const parts = value.split(".");
    if (parts.length !== 4 || parts.some((p) => !/^\d{1,3}$/.test(p) || Number(p) > 255)) {
      return null;
    }
    return { family: 4, value: parts.reduce((n, p) => (n << 8n) | BigInt(p), 0n) };
  }
  // IPv6, with `::` and a dotted IPv4 tail (RFC 4291 §2.2).
  let text = value.toLowerCase();
  const tail = /(\d+\.\d+\.\d+\.\d+)$/.exec(text);
  if (tail) {
    const v4 = parse(tail[1]!);
    if (!v4) return null;
    const high = Number(v4.value >> 16n).toString(16);
    const low = Number(v4.value & 0xffffn).toString(16);
    text = `${text.slice(0, -tail[1]!.length)}${high}:${low}`;
  }
  const halves = text.split("::");
  if (halves.length > 2) return null;
  const groups = (half: string) => (half ? half.split(":") : []);
  const head = groups(halves[0]!);
  const rest = halves.length === 2 ? groups(halves[1]!) : [];
  const missing = 8 - head.length - rest.length;
  if (missing < 0 || (halves.length === 1 && missing !== 0)) return null;
  const all = [...head, ...Array<string>(missing).fill("0"), ...rest];
  if (all.some((g) => !/^[0-9a-f]{1,4}$/.test(g))) return null;
  return { family: 6, value: all.reduce((n, g) => (n << 16n) | BigInt(`0x${g}`), 0n) };
}

/** Whether `address` lies in `prefix` (`192.0.2.0/24`, `2001:db8::/32`). */
export function within(address: string, prefix: string): boolean {
  const [network, length] = prefix.split("/");
  const a = parse(address);
  const p = network === undefined ? null : parse(network);
  if (!a || !p || a.family !== p.family) return false;
  const width = a.family === 4 ? 32 : 128;
  const bits = length === undefined ? width : Number(length);
  if (!Number.isInteger(bits) || bits < 0 || bits > width) return false;
  const shift = BigInt(width - bits);
  return a.value >> shift === p.value >> shift;
}
