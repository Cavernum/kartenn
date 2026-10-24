// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import InlineAlert from "./InlineAlert.vue";

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
  ],
} satisfies CatalogueEntry;
