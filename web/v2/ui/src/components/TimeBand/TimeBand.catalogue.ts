// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import TimeBand from "./TimeBand.vue";

export default {
  component: TimeBand,
  pattern: "role=status, bouton de retour au présent",
  states: [
    {
      name: "past",
      description: "Lecture au passé",
      props: { at: "2025-11-03T08:30:00Z", timeZone: "Europe/Paris" },
      wide: true,
    },
    {
      name: "other-zone",
      description: "Autre fuseau nommé",
      props: { at: "2025-11-03T08:30:00Z", timeZone: "America/Montreal" },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
