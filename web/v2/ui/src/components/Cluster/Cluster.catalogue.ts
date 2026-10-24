// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { filled } from "../../demo/layouts.ts";
import Cluster from "./Cluster.vue";

const demo = filled(Cluster, 4);

export default {
  component: Cluster,
  pattern: "— (mise en page en rangée)",
  states: [
    { name: "gap-2", description: "Écart 8 px (défaut)", props: {}, demo, wide: true },
    { name: "gap-1", description: "Écart 4 px", props: { gap: "1" }, demo, wide: true },
    { name: "end", description: "Alignés à la fin", props: { justify: "end" }, demo, wide: true },
  ],
} satisfies CatalogueEntry;
