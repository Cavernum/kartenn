// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoBox from "../../demo/DemoBox.vue";
import Split from "./Split.vue";

const demo: FunctionalComponent = (props) =>
  h(Split as Component, props, {
    side: () => h(DemoBox, { label: "Côté" }),
    default: () => h(DemoBox, { label: "Principal" }),
  });

export default {
  component: Split,
  pattern: "— (côté de largeur fixe et partie principale)",
  states: [
    { name: "start", description: "Côté au début, 320 px", props: {}, demo, wide: true },
    {
      name: "end-sm",
      description: "Côté à la fin, 240 px",
      props: { side: "end", sideSize: "sm" },
      demo,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
