// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";

/** An entry of the palette: a command, a page or a recent object. */
export interface PaletteItem {
  id: string;
  label: string;
  icon?: IconName;
  /** A detail in fixed pitch (a prefix, a key), after the label. */
  detail?: string;
  shortcut?: string[];
}

/** A group of entries (Commands, Navigation, Recent). */
export interface PaletteGroup {
  label: string;
  items: PaletteItem[];
}
