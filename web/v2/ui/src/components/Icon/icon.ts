// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName, IconSize } from "../../icons/index.ts";

/**
 * The props of `Icon`: a decorative icon has no name; an icon that alone carries a
 * meaning (`meaningful`) must have one (`label`), or the code does not compile.
 */
export type IconProps = {
  name: IconName;
  size?: IconSize;
} & ({ meaningful?: false; label?: undefined } | { meaningful: true; label: string });
