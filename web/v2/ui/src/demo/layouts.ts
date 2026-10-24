// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import DemoBox from "./DemoBox.vue";

/**
 * A layout of the catalogue filled with `count` placeholders: the props of the state go to
 * the layout, so that each state shows one setting.
 */
export function filled(layout: Component, count: number): FunctionalComponent {
  return (props) =>
    h(layout, props, () =>
      Array.from({ length: count }, (_, i) => h(DemoBox, { label: `Bloc ${i + 1}` })),
    );
}
