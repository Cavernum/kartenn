// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import StatusBadge from "../StatusBadge/StatusBadge.vue";
import DiffValue from "./DiffValue.vue";

/** In a column as narrow as a value of an attribute list: the sign stays with its value. */
const narrow = (kind: "added" | "removed", text: string) => () =>
  h("div", { style: { maxInlineSize: "var(--k-size-col-lg)" } }, [
    h(DiffValue, { kind }, () => text),
  ]);

export default {
  component: DiffValue,
  pattern: "— (ins / del, signe et mot caché)",
  states: [
    {
      name: "added",
      description: "Valeur ajoutée : « + » attaché à la valeur, qui va à la ligne dans sa boîte",
      props: {},
      demo: narrow("added", "Lecture des API de production depuis la recette"),
    },
    {
      name: "removed",
      description: "Valeur retirée : « − »",
      props: {},
      demo: narrow("removed", "Accès historique des sauvegardes"),
    },
    {
      name: "code",
      description: "Valeur ajoutée en code copiable (CodeBadge)",
      props: {},
      demo: () => h(DiffValue, { kind: "added" }, () => h(CodeBadge, { code: "api-lecture" })),
    },
    {
      name: "changed",
      description: "Avant → après (mots cachés « avant », « après »)",
      props: {},
      demo: () =>
        h(
          DiffValue,
          { kind: "changed" },
          {
            before: () => h(StatusBadge, { state: "neutral", label: "Refus par défaut" }),
            default: () => h(StatusBadge, { state: "warn", label: "Sous motif" }),
          },
        ),
    },
  ],
} satisfies CatalogueEntry;
