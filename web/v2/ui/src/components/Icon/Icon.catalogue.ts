// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import Icon from "./Icon.vue";

export default {
  component: Icon,
  pattern: "img (porteuse de sens) ou aria-hidden (décorative)",
  states: [
    { name: "decorative", description: "Décorative, 16 px", props: { name: "prefix" } },
    {
      name: "decorative-20",
      description: "Décorative, 20 px",
      props: { name: "prefix", size: 20 },
    },
    {
      name: "meaningful",
      description: "Porteuse de sens, nommée",
      props: { name: "circle-check", meaningful: true, label: "Conforme" },
    },
  ],
} satisfies CatalogueEntry;
