// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Button from "../Button/Button.vue";
import Stack from "../Stack/Stack.vue";
import InlineAlert from "./InlineAlert.vue";

/**
 * The violations of a submission, shown when it is refused (the script,
 * e2e/components/inline-alert.spec.ts): the alert comes in its region, its content a frame
 * later, which is said.
 */
const Announce = defineComponent({
  setup() {
    const refused = ref(false);
    return () =>
      h(Stack, { gap: "2", align: "stretch" }, () => [
        h("span", {}, [
          h(Button, { variant: "primary", onClick: () => (refused.value = !refused.value) }, () =>
            refused.value ? "Retirer l'alerte" : "Appliquer le changeset",
          ),
        ]),
        refused.value
          ? h(InlineAlert, {
              tone: "ko",
              title: "Le changeset est refusé.",
              items: [
                { message: "Le préfixe chevauche un voisin.", code: "prefix.overlaps-sibling" },
              ],
            })
          : null,
      ]);
  },
});

export default {
  component: InlineAlert,
  pattern: "role=status (alert pour une erreur)",
  states: [
    {
      name: "ko",
      description: "Violations sous un formulaire, avec leurs codes",
      props: {
        tone: "ko",
        title: "Le changeset est refusé.",
        items: [
          { message: "Le préfixe chevauche un voisin.", code: "prefix.overlaps-sibling" },
          { message: "Le responsable est inconnu.", code: "core.owner.unknown" },
        ],
      },
      wide: true,
    },
    {
      name: "warn",
      description: "Avertissement",
      props: { tone: "warn", title: "Trois adresses sont hors du préfixe." },
      slot: "Elles restent visibles dans la liste des anomalies.",
      wide: true,
    },
    {
      name: "ok",
      description: "Succès",
      props: { tone: "ok", title: "Le changeset est appliqué." },
      wide: true,
    },
    {
      name: "info",
      description: "Information",
      props: { tone: "info", title: "Lecture au passé : la modification est désactivée." },
      wide: true,
    },
    {
      name: "icon",
      description: "Icône qui en dit plus (cadenas), ton et rôle gardés",
      props: { tone: "warn", icon: "lock", title: "Groupe livré : lecture seule." },
      slot: "Ses rôles suivent la synchronisation.",
      wide: true,
    },
    {
      name: "announce",
      description: "Annoncée quand elle paraît (contenu rendu une image après sa région)",
      props: {},
      demo: Announce,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
