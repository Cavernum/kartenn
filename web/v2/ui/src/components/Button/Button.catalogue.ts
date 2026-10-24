// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Cluster from "../Cluster/Cluster.vue";
import Stack from "../Stack/Stack.vue";
import EnvChip from "../EnvChip/EnvChip.vue";
import IconButton from "../IconButton/IconButton.vue";
import StatusBadge from "../StatusBadge/StatusBadge.vue";
import TextField from "../TextField/TextField.vue";
import Button from "./Button.vue";

/** The controls side by side, size by size: one baseline, one height (REQ-020-17). */
const Alignment = () =>
  h(Stack, { gap: "3", align: "start" }, () =>
    (["sm", "md", "lg"] as const).map((size) =>
      h(Cluster, { gap: "2", wrap: false }, () => [
        h(Button, { size, variant: "primary" }, () => "Appliquer"),
        h(Button, { size, icon: "plus" }, () => "Ajouter"),
        h(TextField, { size, label: `Recherche ${size}`, hideLabel: true, placeholder: "Préfixe" }),
        h(IconButton, { size, icon: "ellipsis", label: `Actions ${size}`, variant: "secondary" }),
        h(EnvChip, { value: "prod", label: "Production", hue: 1 }),
        h(StatusBadge, { state: "ok", label: "Conforme" }),
      ]),
    ),
  );

/** The four variants at rest, side by side. */
const Variants = () =>
  h(Cluster, { gap: "2" }, () =>
    (["primary", "secondary", "ghost", "danger"] as const).map((variant) =>
      h(Button, { variant }, () => variant),
    ),
  );

export default {
  component: Button,
  pattern: "Button",
  states: [
    { name: "variants", description: "Les quatre variantes, au repos", props: {}, demo: Variants },
    { name: "primary", description: "Principal", props: { variant: "primary" }, slot: "Appliquer" },
    {
      name: "secondary-icon",
      description: "Secondaire avec icône",
      props: { icon: "plus" },
      slot: "Ajouter un préfixe",
    },
    {
      name: "hover",
      description: "Survol",
      props: { variant: "primary" },
      slot: "Appliquer",
      interaction: "hover",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: {},
      slot: "Annuler",
      interaction: "focus",
    },
    {
      name: "pressed",
      description: "Bouton bascule enfoncé (aria-pressed)",
      props: { pressed: true, icon: "funnel" },
      slot: "Filtres",
    },
    {
      name: "shortcut",
      description: "Raccourci affiché (aria-keyshortcuts)",
      props: { variant: "primary", shortcut: ["Mod", "Enter"] },
      slot: "Appliquer",
    },
    {
      name: "loading",
      description: "En cours : focalisable, aria-busy",
      props: { variant: "primary", loading: true },
      slot: "Appliquer",
    },
    {
      name: "disabled",
      description: "Désactivé",
      props: { variant: "primary", disabled: true },
      slot: "Appliquer",
    },
    {
      name: "danger",
      description: "Destructif",
      props: { variant: "danger", icon: "trash" },
      slot: "Supprimer",
    },
    { name: "small", description: "Petit (24 px)", props: { size: "sm" }, slot: "Modifier" },
    { name: "large", description: "Grand (32 px)", props: { size: "lg" }, slot: "Modifier" },
    {
      name: "alignment",
      description: "Bouton, champ, bouton-icône et pastilles côte à côte, par taille",
      props: {},
      demo: Alignment,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
