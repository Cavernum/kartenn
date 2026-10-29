// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Cluster from "../Cluster/Cluster.vue";
import Stack from "../Stack/Stack.vue";
import EnvChip from "../EnvChip/EnvChip.vue";
import IconButton from "../IconButton/IconButton.vue";
import Select from "../Select/Select.vue";
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
        h(Select, {
          size,
          label: `Usage ${size}`,
          hideLabel: true,
          options: [{ value: "host", label: "Hôte" }],
          modelValue: "host",
        }),
        h(IconButton, { size, icon: "ellipsis", label: `Actions ${size}`, variant: "secondary" }),
        h(EnvChip, { value: "prod", label: "Production", hue: 1 }),
        h(StatusBadge, { state: "ok", label: "Conforme" }),
      ]),
    ),
  );

/**
 * The buttons of the keyboard script (APG Button; e2e/components/button.spec.ts): an
 * action, a toggle, a busy one, a disabled one, a refused one and an icon button, in the
 * order of the tabs; what each activation ran is said under them.
 */
const Keyboard = defineComponent({
  setup() {
    const said = ref("");
    const filters = ref(false);
    let saves = 0;
    const ran = (what: string) => () => (said.value = `Activé : ${what}`);
    return () =>
      h(Stack, { gap: "2", align: "start" }, () => [
        h(Cluster, { gap: "2" }, () => [
          h(
            Button,
            {
              variant: "primary",
              onClick: () => (said.value = `Activé : Enregistrer (${++saves})`),
            },
            () => "Enregistrer",
          ),
          h(
            Button,
            {
              icon: "funnel",
              pressed: filters.value,
              onClick: () => {
                filters.value = !filters.value;
                ran(filters.value ? "Filtres montrés" : "Filtres cachés")();
              },
            },
            () => "Filtres",
          ),
          h(Button, { loading: true, onClick: ran("Recharger") }, () => "Recharger"),
          h(Button, { disabled: true, onClick: ran("Supprimer") }, () => "Supprimer"),
          h(
            Button,
            { icon: "pencil", forbidden: "prefix.edit", onClick: ran("Modifier") },
            () => "Modifier",
          ),
          h(IconButton, { icon: "copy", label: "Copier", onClick: ran("Copier") }),
        ]),
        h("p", { role: "status", class: "k-cat-note" }, said.value),
      ]);
  },
});

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
      name: "forbidden",
      description: "Interdit par les droits : focalisable, aria-disabled, « Il faut … » au focus",
      props: { icon: "pencil", forbidden: "prefix.edit" },
      slot: "Modifier",
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
      description:
        "Bouton, champ, liste de choix, bouton-icône et pastilles côte à côte, par taille",
      props: {},
      demo: Alignment,
      wide: true,
    },
    {
      name: "keyboard",
      description: "Au clavier : action, bascule, occupé, désactivé, interdit, bouton-icône",
      props: {},
      demo: Keyboard,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
