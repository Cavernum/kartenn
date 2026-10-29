// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import ShortcutHint from "./ShortcutHint.vue";

/** In a sentence of help, as the aide `?` writes it. */
const Sentence = () =>
  h("p", null, [
    "Pour aller aux préfixes, taper ",
    h(ShortcutHint, { keys: [["g"], ["p"]] }),
    " hors d'un champ.",
  ]);

export default {
  component: ShortcutHint,
  pattern: "— (une combinaison, ou une séquence de combinaisons jointes par « puis »)",
  states: [
    { name: "combination", description: "Une combinaison", props: { keys: ["Mod", "k"] } },
    { name: "single", description: "Une touche", props: { keys: ["/"] } },
    { name: "sequence", description: "Une séquence : g puis p", props: { keys: [["g"], ["p"]] } },
    { name: "sentence", description: "Dans une phrase d'aide", props: {}, demo: Sentence },
  ],
} satisfies CatalogueEntry;
