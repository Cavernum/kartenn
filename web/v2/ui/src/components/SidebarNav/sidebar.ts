// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";

/** An entry of the sidebar: a page, its icon, a counter read from the event stream. */
export interface SidebarEntry {
  id: string;
  label: string;
  icon: IconName;
  to: string;
  badge?: number;
}

/** A space of the sidebar (spec 021, « Cadre »): a disclosure of entries. */
export interface SidebarGroup {
  id: string;
  label: string;
  entries: SidebarEntry[];
}
