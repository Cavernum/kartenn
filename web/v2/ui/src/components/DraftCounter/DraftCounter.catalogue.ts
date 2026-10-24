// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import DraftCounter from "./DraftCounter.vue";

export default {
  component: DraftCounter,
  pattern: "Button (aria-expanded du panneau)",
  states: [
    { name: "pending", description: "Opérations en brouillon", props: { count: 12 } },
    { name: "empty", description: "Compteur à 0", props: { count: 0 } },
    { name: "expanded", description: "Panneau ouvert", props: { count: 3, expanded: true } },
    { name: "focus", description: "Focus visible", props: { count: 3 }, interaction: "focus" },
  ],
} satisfies CatalogueEntry;
