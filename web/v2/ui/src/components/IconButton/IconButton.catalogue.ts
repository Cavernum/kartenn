// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import IconButton from "./IconButton.vue";

export default {
  component: IconButton,
  pattern: "Button (nom accessible obligatoire)",
  states: [
    {
      name: "ghost",
      description: "Discret (barres d'outils, lignes)",
      props: { icon: "ellipsis", label: "Actions" },
    },
    {
      name: "secondary",
      description: "Secondaire",
      props: { icon: "refresh-cw", label: "Recharger", variant: "secondary" },
    },
    {
      name: "primary",
      description: "Principal",
      props: { icon: "plus", label: "Créer", variant: "primary" },
    },
    {
      name: "hover",
      description: "Survol : son nom en infobulle, après un délai",
      props: { icon: "pencil", label: "Modifier" },
      interaction: "hover",
    },
    {
      name: "focus",
      description: "Focus visible : son nom en infobulle, aussitôt",
      props: { icon: "pencil", label: "Modifier" },
      interaction: "focus",
    },
    {
      name: "shortcut",
      description: "Raccourci dans l'infobulle (aria-keyshortcuts)",
      props: { icon: "panel-left", label: "Replier la barre latérale", shortcut: ["["] },
      interaction: "focus",
    },
    {
      name: "forbidden",
      description: "Interdit : son nom puis « Il faut … » en infobulle, la raison en description",
      props: {
        icon: "trash",
        label: "Supprimer",
        variant: "secondary",
        forbidden: "prefix.delete",
      },
    },
    {
      name: "pressed",
      description: "Bascule enfoncée",
      props: { icon: "panel-left", label: "Barre latérale", pressed: true },
    },
    {
      name: "loading",
      description: "En cours",
      props: { icon: "refresh-cw", label: "Recharger", variant: "secondary", loading: true },
    },
    {
      name: "disabled",
      description: "Désactivé",
      props: { icon: "trash", label: "Supprimer", variant: "secondary", disabled: true },
    },
    {
      name: "small",
      description: "Petit (24 px en compacte)",
      props: { icon: "x", label: "Fermer", size: "sm", variant: "secondary" },
    },
    {
      name: "large",
      description: "Grand (32 px en compacte)",
      props: { icon: "x", label: "Fermer", size: "lg", variant: "secondary" },
    },
  ],
} satisfies CatalogueEntry;
