// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Grid from "../Grid/Grid.vue";
import KeyLabel from "../KeyLabel/KeyLabel.vue";
import AttributeList from "./AttributeList.vue";
import type { AttributeItem } from "./attributes.ts";

const machine: AttributeItem[] = [
  { key: "kind", label: "Genre" },
  { key: "address", label: "Adresse de gestion", value: "192.0.2.10", mono: true },
  { key: "mac", label: "Adresse MAC", value: "00-00-5E-00-53-01", mono: true },
  { key: "site", label: "Site", value: "Site A" },
];

/** The kind through its slot: a key and its label, as a record shows it. */
const withSlot: FunctionalComponent = (props) =>
  h(AttributeList as Component, props, {
    kind: () => h(KeyLabel, { value: "virtual", label: "Machine virtuelle" }),
  });

/** Two blocks side by side in a grid of tiles: the n-th property on the same line. */
const tiles: FunctionalComponent = () =>
  h(Grid, { columns: "auto", gap: "3", class: "k-attribute-list-tiles" }, () => [
    h(AttributeList as Component, {
      items: [
        { key: "state", label: "État", value: "Soumis" },
        { key: "author", label: "Auteur", value: "Opérateur réseau" },
        { key: "submitted", label: "Soumis le", value: "2026-11-03 09:12", mono: true },
      ],
    }),
    h(AttributeList as Component, {
      items: [
        { key: "valid", label: "Valide à partir du", value: "2026-11-04 00:00", mono: true },
        { key: "retro", label: "Rétroactif", value: "Non" },
        { key: "policy", label: "Politique" },
      ],
    }),
  ]);

const many: AttributeItem[] = [
  ...machine.slice(1),
  { key: "owner", label: "Responsable", value: "Équipe infrastructure" },
  { key: "host", label: "Hôte", value: "hv-nord-01", mono: true },
  { key: "cluster", label: "Grappe" },
  { key: "os", label: "Système", value: "Linux" },
  { key: "vcpu", label: "Processeurs", value: "4" },
];

export default {
  component: AttributeList,
  pattern: "liste de descriptions (dl : dt, dd)",
  states: [
    {
      name: "default",
      description: "Clé et valeur, une valeur par son emplacement (clé et libellé), chasse fixe",
      props: { items: machine },
      demo: withSlot,
    },
    {
      name: "empty",
      description:
        "Valeurs absentes : un tiret montré, « non renseigné » pour les lecteurs d'écran",
      props: {
        items: [
          { key: "owner", label: "Responsable" },
          { key: "host", label: "Hôte", value: null },
          { key: "site", label: "Site", value: "Site A" },
        ],
      },
    },
    {
      name: "long",
      description: "Valeur longue sans espace : elle va à la ligne dans sa colonne",
      props: {
        items: [
          { key: "address", label: "Adresse", value: "2001:db8:0:1:0:0:0:abcd/128", mono: true },
          {
            key: "name",
            label: "Nom",
            value: "srv-supervision-collecteur-principal-nord-01.example.net",
            mono: true,
          },
        ],
      },
    },
    {
      name: "columns",
      description: "En colonnes : autant de paires clé, valeur que la largeur en tient",
      props: { items: many, columns: true },
      wide: true,
    },
    {
      name: "tiles",
      description: "Blocs côte à côte (k-attribute-list-tiles) : leurs lignes partagées",
      props: {},
      demo: tiles,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
