// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import Skeleton from "./Skeleton.vue";

export default {
  component: Skeleton,
  pattern: "status (région présente avant son mot)",
  states: [
    { name: "rows", description: "Lignes de table (hauteur de ligne)", props: {}, wide: true },
    {
      name: "text",
      description: "Lignes de texte (fiche)",
      props: { variant: "text", rows: 3 },
      wide: true,
    },
    { name: "block", description: "Bloc (panneau)", props: { variant: "block" }, wide: true },
  ],
} satisfies CatalogueEntry;
