// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import DatePill from "./DatePill.vue";

export default {
  component: DatePill,
  pattern: "Button (ouvre le choix de la période)",
  states: [
    { name: "present", description: "Présent", props: { timeZone: "Europe/Paris" } },
    {
      name: "past",
      description: "Hors présent : couleurs de la bande, fuseau nommé",
      props: { at: "2025-11-03T08:30:00Z", timeZone: "Europe/Paris" },
    },
    {
      name: "hover",
      description: "Survol",
      props: { timeZone: "Europe/Paris" },
      interaction: "hover",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { at: "2025-11-03T08:30:00Z", timeZone: "Europe/Paris" },
      interaction: "focus",
    },
  ],
} satisfies CatalogueEntry;
