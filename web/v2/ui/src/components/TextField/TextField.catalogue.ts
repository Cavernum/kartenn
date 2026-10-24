// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import TextField from "./TextField.vue";

export default {
  component: TextField,
  pattern: "champ natif étiqueté (aide et erreur par aria-describedby)",
  states: [
    {
      name: "rest",
      description: "Au repos, avec aide",
      props: { label: "Nom", hint: "Visible dans les listes." },
    },
    {
      name: "placeholder",
      description: "Vide, avec icône et exemple",
      props: {
        label: "Recherche",
        icon: "search",
        type: "search",
        placeholder: "Nom, préfixe, adresse",
      },
    },
    {
      name: "mono",
      description: "Chasse fixe (préfixe), obligatoire",
      props: { label: "Préfixe", mono: true, required: true, modelValue: prefixes[1] },
    },
    {
      name: "hover",
      description: "Survol",
      props: { label: "Nom", modelValue: "Serveurs de bases" },
      interaction: "hover",
      target: "input",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { label: "Nom", modelValue: "Serveurs de bases" },
      interaction: "focus",
      target: "input",
    },
    {
      name: "invalid",
      description: "Invalide : erreur liée et aria-invalid",
      props: {
        label: "Préfixe",
        mono: true,
        modelValue: "192.0.2.300/24",
        error: "Adresse hors de l'espace IPv4.",
        hint: "Notation CIDR.",
      },
    },
    {
      name: "pending",
      description: "En attente (vérification en cours)",
      props: { label: "Préfixe", mono: true, modelValue: prefixes[2], pending: true },
    },
    {
      name: "readonly",
      description: "Lecture seule",
      props: {
        label: "Identifiant",
        mono: true,
        readonly: true,
        modelValue: "01JD4K7Q9V2M8X3N5P6R7S8T9W",
      },
    },
    {
      name: "disabled",
      description: "Désactivé",
      props: { label: "Nom", disabled: true, modelValue: "Serveurs de bases" },
    },
    {
      name: "small",
      description: "Petit (24 px en compacte), étiquette cachée",
      props: {
        label: "Filtrer",
        hideLabel: true,
        size: "sm",
        icon: "funnel",
        placeholder: "Filtrer",
      },
    },
    {
      name: "large",
      description: "Grand (32 px en compacte)",
      props: { label: "Nom", size: "lg", modelValue: "Serveurs de bases" },
    },
  ],
} satisfies CatalogueEntry;
