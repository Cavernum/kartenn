// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { filled } from "../../demo/layouts.ts";
import Stack from "./Stack.vue";

const demo = filled(Stack, 3);

export default {
  component: Stack,
  pattern: "— (mise en page verticale)",
  states: [
    { name: "gap-2", description: "Écart 8 px (défaut)", props: {}, demo, wide: true },
    { name: "gap-4", description: "Écart 16 px", props: { gap: "4" }, demo, wide: true },
    { name: "start", description: "Alignés au début", props: { align: "start" }, demo, wide: true },
  ],
} satisfies CatalogueEntry;
