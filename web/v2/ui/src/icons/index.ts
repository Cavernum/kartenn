// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/ui/icons` (REQ-020-15): the closed list of icons, Lucide and business ones,
 * by name. `Icon` renders them at 16 or 20 px with a stroke of 1.5.
 */

import { lucideIcons } from "./lucide.ts";
import { metierIcons } from "./metier.ts";
import type { IconNode } from "./types.ts";

export type { IconNode } from "./types.ts";
export { lucideIcons } from "./lucide.ts";
export { metierIcons } from "./metier.ts";

export const icons = { ...lucideIcons, ...metierIcons } as const satisfies Record<string, IconNode>;

/** The name of an icon of the closed list. */
export type IconName = keyof typeof icons;

/** The two sizes of an icon, in CSS pixels (tokens --k-icon-size, --k-icon-size-lg). */
export type IconSize = 16 | 20;
