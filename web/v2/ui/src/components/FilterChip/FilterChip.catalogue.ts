// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import Cluster from "../Cluster/Cluster.vue";
import FilterChip from "./FilterChip.vue";

/** A row of filters, as above a list. */
const Row = () =>
  h(Cluster, { gap: "2" }, () => [
    h(FilterChip, { property: "Environnement", operator: "est", value: "Production" }),
    h(FilterChip, {
      property: "Préfixe",
      operator: "dans",
      value: prefixes[0],
      mono: true,
    }),
    h(FilterChip, { property: "Responsable", operator: "contient", state: "draft" }),
  ]);

export default {
  component: FilterChip,
  pattern: "Button (ouvre le sélecteur) et Button (retirer)",
  states: [
    {
      name: "applied",
      description: "Filtre appliqué",
      props: { property: "Environnement", operator: "est", value: "Production" },
    },
    {
      name: "mono",
      description: "Valeur en chasse fixe",
      props: { property: "Préfixe", operator: "dans", value: prefixes[0], mono: true },
    },
    {
      name: "key",
      description: "Propriété sans libellé dans la langue : sa clé en chasse fixe",
      props: {
        property: "prop.dns.ttl",
        propertyMono: true,
        operator: "entre",
        value: "60 et 3600",
        mono: true,
      },
    },
    {
      name: "draft",
      description: "Brouillon de filtre (valeur à choisir)",
      props: { property: "Responsable", operator: "contient", state: "draft" },
    },
    {
      name: "invalid",
      description: "Invalide : motif dit aux technologies d'assistance",
      props: {
        property: "Préfixe",
        operator: "dans",
        value: "192.0.2.0/33",
        mono: true,
        state: "invalid",
        reason: "Longueur de préfixe hors de 0 à 32.",
      },
    },
    {
      name: "hover",
      description: "Survol",
      props: { property: "Environnement", operator: "est", value: "Production" },
      interaction: "hover",
    },
    {
      name: "focus",
      description: "Focus visible sur le bouton de retrait",
      props: { property: "Environnement", operator: "est", value: "Production" },
      interaction: "focus",
      target: "button[aria-label]",
    },
    { name: "row", description: "Une rangée de filtres", props: {}, demo: Row, wide: true },
  ],
} satisfies CatalogueEntry;
