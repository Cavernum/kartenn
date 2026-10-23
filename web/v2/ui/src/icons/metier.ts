// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * The business icons of kartenn (REQ-020-15, design « Icônes »), absent from Lucide and
 * drawn on its grid: 24 units, stroke 1.5 (set by `Icon`), round caps and joins, no
 * fill, 2 units of padding. Proposals, reviewed by the user on the sheet of the
 * catalogue (`/_ui/icones`) beside their Lucide neighbours (Q-020-7).
 */

import type { IconNode } from "./types.ts";

export const metierIcons = {
  /** Prefix: an address block whose network part is marked off by a slash. */
  prefix: [
    ["rect", { x: "3", y: "7", width: "18", height: "10", rx: "2" }],
    ["path", { d: "M12 7v10" }],
    ["path", { d: "m6 14.5 3-5" }],
  ],
  /** Range: two bounds and the span between them. */
  range: [
    ["path", { d: "M4 6v12" }],
    ["path", { d: "M20 6v12" }],
    ["path", { d: "M8 12h8" }],
    ["path", { d: "m10.5 9.5-2.5 2.5 2.5 2.5" }],
    ["path", { d: "m13.5 9.5 2.5 2.5-2.5 2.5" }],
  ],
  /** Address: a single point in the block. */
  address: [
    ["rect", { x: "3", y: "7", width: "18", height: "10", rx: "2" }],
    ["circle", { cx: "8", cy: "12", r: "1.5" }],
    ["path", { d: "M12.5 12h4.5" }],
  ],
  /** Addressing domain: an address space holding others. */
  domain: [
    ["rect", { x: "3", y: "3", width: "18", height: "18", rx: "2" }],
    ["rect", { x: "7", y: "7", width: "10", height: "10", rx: "1" }],
    ["path", { d: "M12 10v4" }],
    ["path", { d: "M10 12h4" }],
  ],
  /** VLAN: lanes of one medium, the middle one tagged. */
  vlan: [
    ["path", { d: "M3 6h18" }],
    ["path", { d: "M3 18h18" }],
    ["path", { d: "M3 12h5" }],
    ["path", { d: "M16 12h5" }],
    ["rect", { x: "8", y: "9.5", width: "8", height: "5", rx: "1" }],
  ],
  /** Interface: the socket of a port. */
  interface: [
    ["rect", { x: "3", y: "5", width: "18", height: "14", rx: "2" }],
    ["path", { d: "M8 19v-3h8v3" }],
    ["path", { d: "M8 9v2" }],
    ["path", { d: "M12 9v2" }],
    ["path", { d: "M16 9v2" }],
  ],
  /** Machine in a cluster: one node above, two below, joined. */
  cluster: [
    ["rect", { x: "9", y: "3", width: "6", height: "5", rx: "1" }],
    ["rect", { x: "3", y: "16", width: "6", height: "5", rx: "1" }],
    ["rect", { x: "15", y: "16", width: "6", height: "5", rx: "1" }],
    ["path", { d: "M12 8v4" }],
    ["path", { d: "M6 16v-2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v2" }],
  ],
  /** Flow: from one endpoint to another. */
  flow: [
    ["circle", { cx: "5", cy: "12", r: "2" }],
    ["circle", { cx: "19", cy: "12", r: "2" }],
    ["path", { d: "M7 12h9.5" }],
    ["path", { d: "m13.5 9 3 3-3 3" }],
  ],
  /** Environment: the chip of the interface, a dot and a label. */
  environment: [
    ["rect", { x: "3", y: "7", width: "18", height: "10", rx: "5" }],
    ["circle", { cx: "8.5", cy: "12", r: "1.5" }],
    ["path", { d: "M12.5 12h4" }],
  ],
  /** Policy exception: a guarded rule with a mark of deviation. */
  exception: [
    ["path", { d: "M12 3 5 6v5c0 4.5 3 7.7 7 9 4-1.3 7-4.5 7-9V6z" }],
    ["path", { d: "M12 8v4" }],
    ["path", { d: "M12 15.5h.01" }],
  ],
  /** Changeset: a document of additions and removals. */
  changeset: [
    ["path", { d: "M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" }],
    ["path", { d: "M14 3v5h5" }],
    ["path", { d: "M9 12h4" }],
    ["path", { d: "M11 10v4" }],
    ["path", { d: "M9 17h4" }],
  ],
  /** Draft: a page being written. */
  draft: [
    ["path", { d: "M12 21H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7l5 5v3" }],
    ["path", { d: "M14 3v5h5" }],
    ["path", { d: "M20.4 14.6a1.5 1.5 0 0 0-2.1-2.1L13 17.8V20h2.2z" }],
  ],
  /** Policy matrix: sources by destinations, one cell decided. */
  matrix: [
    ["rect", { x: "3", y: "3", width: "18", height: "18", rx: "2" }],
    ["path", { d: "M3 9h18" }],
    ["path", { d: "M9 3v18" }],
    ["path", { d: "m12.5 15 2 2 3.5-3.5" }],
  ],
} as const satisfies Record<string, IconNode>;
