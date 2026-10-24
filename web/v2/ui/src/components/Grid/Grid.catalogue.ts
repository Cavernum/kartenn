// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { filled } from "../../demo/layouts.ts";
import Grid from "./Grid.vue";

const demo = filled(Grid, 6);

export default {
  component: Grid,
  pattern: "— (grille de mise en page)",
  states: [
    {
      name: "columns-3",
      description: "Trois colonnes égales",
      props: { columns: 3 },
      demo,
      wide: true,
    },
    {
      name: "auto",
      description: "Autant de panneaux que la largeur en tient",
      props: {},
      demo,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
