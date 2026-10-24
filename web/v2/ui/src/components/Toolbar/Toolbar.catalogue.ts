// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type PropType } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoColumn from "../../demo/DemoColumn.vue";
import Toolbar from "./Toolbar.vue";
import type { ToolbarAction } from "./toolbar.ts";

const actions: ToolbarAction[] = [
  { id: "create", label: "Créer", icon: "plus", variant: "primary", priority: 3 },
  { id: "edit", label: "Modifier", icon: "pencil", priority: 2 },
  { id: "import", label: "Importer", icon: "arrow-down" },
  { id: "export", label: "Exporter", icon: "arrow-up" },
  { id: "history", label: "Historique", icon: "clock", iconOnly: true, priority: 1 },
  {
    id: "delete",
    label: "Supprimer",
    icon: "trash",
    iconOnly: true,
    forbidden: "prefix.delete",
    priority: 1,
  },
];

/** The toolbar in a column of a given width, or the whole width; the action run is said. */
const demo = defineComponent({
  props: {
    actions: { type: Array as PropType<ToolbarAction[]>, required: true },
    size: { type: String as PropType<"sm" | "md" | "lg" | "full">, default: "full" },
  },
  setup(props) {
    const ran = ref("");
    const bar = () => [
      h(Toolbar, {
        label: "Actions sur les préfixes",
        actions: props.actions,
        onAction: (id: string) => (ran.value = `Action : ${id}`),
      }),
      h("p", { role: "status", class: "k-cat-note" }, ran.value),
    ];
    return () =>
      props.size === "full" ? h("div", bar()) : h(DemoColumn, { size: props.size }, bar);
  },
});

export default {
  component: Toolbar,
  pattern: "Toolbar ; actions repliées dans « … » par priorité",
  states: [
    {
      name: "wide",
      description: "Toutes les actions tiennent",
      props: { actions },
      demo,
      wide: true,
    },
    {
      name: "folded",
      description: "Largeur d'un panneau (420 px) : la fin de la barre passe dans « … »",
      props: { actions, size: "lg" },
      demo,
    },
    {
      name: "narrow",
      description: "240 px : seules les actions prioritaires restent",
      props: { actions, size: "sm" },
      demo,
    },
    {
      name: "forbidden",
      description: "Action interdite : focalisable, infobulle « Il faut … »",
      props: { actions },
      demo,
      wide: true,
      interaction: "focus",
      target: "[data-action='delete']",
    },
  ],
} satisfies CatalogueEntry;
