// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoBox from "../../demo/DemoBox.vue";
import Grid from "../Grid/Grid.vue";
import Heading from "./Heading.vue";

/** A long name in the first of three columns: it wraps in its column, never overflows. */
const narrow: FunctionalComponent = (props) =>
  h(Grid, { columns: 3 }, () => [
    h(
      Heading as Component,
      props,
      () => "srv-2001-db8-0-1-supervision-collecteur-principal-nord-01.example.net",
    ),
    h(DemoBox, { label: "Colonne 2" }),
    h(DemoBox, { label: "Colonne 3" }),
  ]);

export default {
  component: Heading,
  pattern: "heading (h1 à h6 ; niveau et taille séparés)",
  states: [
    {
      name: "page",
      description: "Titre de page : niveau 1, taille de la page (défaut du niveau 1)",
      props: { level: 1 },
      slot: "Machines",
    },
    {
      name: "section",
      description: "Titre de section : niveau 2, taille d'une section (défaut sous le niveau 1)",
      props: { level: 2 },
      slot: "Interfaces",
    },
    {
      name: "level-apart",
      description: "Niveau 3 à la taille de la page : le niveau suit le plan, la taille la vue",
      props: { level: 3, size: "page" },
      slot: "Hébergement",
    },
    {
      name: "long",
      description: "Nom sans espace plus long que sa colonne : il va à la ligne",
      props: { level: 1 },
      demo: narrow,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
