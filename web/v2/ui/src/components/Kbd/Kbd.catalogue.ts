// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import Kbd from "./Kbd.vue";

export default {
  component: Kbd,
  // A sequence (g then d) is not one combination: `ShortcutHint` writes it.
  pattern: "— (kbd imbriqués d'une combinaison)",
  states: [
    { name: "palette", description: "Palette de commandes", props: { keys: ["Mod", "k"] } },
    { name: "single", description: "Une touche (filtre de la vue)", props: { keys: ["/"] } },
    { name: "named", description: "Touches nommées", props: { keys: ["Mod", "Enter"] } },
    {
      name: "shift",
      description: "Maj+Espace (étendre la sélection de lignes)",
      props: { keys: ["Shift", " "] },
    },
    { name: "arrows", description: "Flèches", props: { keys: ["Alt", "ArrowDown"] } },
    { name: "escape", description: "Échap", props: { keys: ["Escape"] } },
  ],
} satisfies CatalogueEntry;
