// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import Menu from "./Menu.vue";
import type { MenuItem } from "./menu.ts";

const items: MenuItem[] = [
  { kind: "label", label: "Préfixe" },
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"] },
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { id: "history", label: "Historique", icon: "clock" },
  {
    id: "move",
    label: "Déplacer vers…",
    icon: "move",
    disabled: true,
    reason: "Parent déduit de l'inclusion",
  },
  { kind: "separator" },
  { id: "delete", label: "Supprimer", icon: "trash", danger: true },
  { id: "purge", label: "Purger l'historique", icon: "trash", forbidden: "prefix.purge" },
];

export default {
  component: Menu,
  pattern: "Menu Button, Menu",
  states: [
    { name: "closed", description: "Fermé", props: { label: "Actions", items } },
    {
      name: "icon-only",
      description: "Bouton-icône (lignes, barres d'outils)",
      props: { label: "Actions sur le préfixe", items, iconOnly: true },
    },
    {
      name: "open",
      description:
        "Ouvert : raccourci à droite, entrées désactivées (motif, permission) focalisables, entrée destructive",
      props: { label: "Actions", items, defaultOpen: true },
      isolated: true,
    },
    {
      name: "radio",
      description:
        "Entrées radio (menuitemradio dans un groupe) : l'une d'un ensemble est cochée, aria-checked",
      props: {
        label: "Brouillons",
        items: [
          { kind: "label", label: "Brouillons" },
          { kind: "radio", id: "d1", label: "Salle B (3)", checked: true },
          { kind: "radio", id: "d2", label: "Reprise des VIP (12)", checked: false },
        ] satisfies MenuItem[],
        defaultOpen: true,
      },
      isolated: true,
    },
  ],
} satisfies CatalogueEntry;
