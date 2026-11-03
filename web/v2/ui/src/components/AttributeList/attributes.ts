// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** One property of an `AttributeList`: its term and its value. */
export interface AttributeItem {
  /** Stable key: the item's `v-for` key and the name of the slot of its value. */
  key: string;
  label: string;
  /** Absent (`undefined`, `null`, `""`) without its slot: said « non renseigné ». */
  value?: string | null | undefined;
  /** The value in fixed pitch (addresses, codes, identifiers). */
  mono?: boolean | undefined;
}
