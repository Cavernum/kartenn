// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import BarList from "./BarList.vue";

const percent = (value: number) => `${value} %`;
const groups = [
  { key: "campus-ouest", label: "campus-ouest", value: 87 },
  { key: "campus-est", label: "campus-est", value: 41 },
  { key: "labo", label: "labo", value: 6 },
];

export default {
  component: BarList,
  pattern: "table (en-têtes de ligne, valeurs), barres décoratives",
  states: [
    {
      name: "percent",
      description: "Occupation de groupes, échelle à 100 %",
      props: {
        label: "Occupation des groupes de VLAN",
        items: groups,
        max: 100,
        mono: true,
        format: percent,
        pointLabel: "Groupe",
        valueLabel: "Occupation",
      },
    },
    {
      name: "counts",
      description: "Comptes, échelle sur la plus grande valeur",
      props: {
        label: "Objets par sorte",
        items: [
          { key: "prefix", label: "Préfixes", value: 1208 },
          { key: "range", label: "Plages", value: 342 },
          { key: "entity", label: "Entités", value: 64 },
        ],
      },
    },
    {
      name: "over",
      description: "Une valeur au-delà de l'échelle : barre pleine, coupée à sa fin",
      props: {
        label: "Occupation des groupes de VLAN",
        items: [{ key: "dmz", label: "dmz", value: 104 }, ...groups.slice(1)],
        max: 100,
        mono: true,
        format: percent,
      },
    },
  ],
} satisfies CatalogueEntry;
