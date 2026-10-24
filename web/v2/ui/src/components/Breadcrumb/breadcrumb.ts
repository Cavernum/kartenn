// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** A crumb: its label, its link (none for the current page), fixed pitch for a key. */
export interface Crumb {
  label: string;
  to?: string;
  mono?: boolean;
}
