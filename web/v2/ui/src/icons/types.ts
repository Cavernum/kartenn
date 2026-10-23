// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** An icon as Lucide describes it: the children of a 24 × 24 SVG, stroked, unfilled. */
export type IconNode = readonly (readonly [
  tag: string,
  attrs: Readonly<Record<string, string | number | undefined>>,
])[];
