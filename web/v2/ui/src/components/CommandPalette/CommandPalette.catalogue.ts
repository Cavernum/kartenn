// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import CommandPalette from "./CommandPalette.vue";
import type { PaletteGroup } from "./palette.ts";

const groups: PaletteGroup[] = [
  {
    label: "Commandes",
    items: [
      { id: "create", label: "Créer un préfixe", icon: "plus", shortcut: ["c"] },
      { id: "draft", label: "Ouvrir le brouillon", icon: "draft", shortcut: [["g"], ["b"]] },
    ],
  },
  {
    label: "Navigation",
    items: [
      { id: "prefixes", label: "Préfixes", icon: "prefix", shortcut: [["g"], ["p"]] },
      { id: "changesets", label: "Changesets", icon: "changeset", shortcut: [["g"], ["c"]] },
    ],
  },
  {
    label: "Récents",
    items: [
      { id: "p1", label: "Serveurs de bases", icon: "prefix", detail: prefixes[1] },
      { id: "p2", label: "Postes du siège", icon: "prefix", detail: prefixes[4] },
    ],
  },
];

export default {
  component: CommandPalette,
  pattern: "Combobox dans un Dialog modal",
  states: [
    {
      name: "open",
      description: "Ouverte : commandes, navigation, objets récents",
      props: { groups, open: true },
      isolated: true,
    },
    {
      name: "empty",
      description: "Aucune entrée",
      props: { groups: [], query: "zzz", open: true },
      isolated: true,
    },
    {
      name: "loading",
      description: "Chargement des objets",
      props: { groups: [], query: "serv", loading: true, open: true },
      isolated: true,
    },
  ],
} satisfies CatalogueEntry;
