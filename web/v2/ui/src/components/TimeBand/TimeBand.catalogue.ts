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
      description: "Date de validité seule (valid_at) : brouillon daté possible",
      props: { validAt: "2025-11-03T08:30:00Z", timeZone: "Europe/Paris" },
      wide: true,
    },
    {
      name: "known",
      description: "État connu à une date passée (as_of) : lecture seule",
      props: { asOf: "2025-12-01T07:00:00Z", timeZone: "Europe/Paris" },
      wide: true,
    },
    {
      name: "both",
      description: "Les deux dates : lecture seule",
      props: {
        validAt: "2025-11-03T08:30:00Z",
        asOf: "2025-12-01T07:00:00Z",
        timeZone: "Europe/Paris",
      },
      wide: true,
    },
    {
      name: "other-zone",
      description: "Autre fuseau nommé",
      props: { validAt: "2025-11-03T08:30:00Z", timeZone: "America/Montreal" },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
