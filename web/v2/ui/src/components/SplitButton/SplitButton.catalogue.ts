// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type PropType } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoColumn from "../../demo/DemoColumn.vue";
import SplitButton from "./SplitButton.vue";
import type { SplitAction } from "./split.ts";

const APPLY: SplitAction = { id: "apply", label: "Appliquer", icon: "circle-check" };
const REVIEW: SplitAction = { id: "review", label: "Soumettre en revue", icon: "send" };
const SAVE: SplitAction = { id: "save", label: "Enregistrer le brouillon", icon: "draft" };
const actions = [APPLY, REVIEW, SAVE];
const held = [
  { ...APPLY, disabled: true, reason: "Une politique d'approbation retient ce changeset." },
  REVIEW,
  SAVE,
];
const forbidden = [{ ...APPLY, forbidden: "changeset.approval.self" }, REVIEW, SAVE];

/** The button at the width of the panel; the action run is said under it. */
const demo = defineComponent({
  props: {
    actions: { type: Array as PropType<SplitAction[]>, required: true },
    action: { type: String, default: "" },
    loading: Boolean,
    defaultOpen: Boolean,
  },
  setup(props) {
    const action = ref(props.action);
    const ran = ref("");
    return () =>
      h(DemoColumn, () => [
        h(SplitButton, {
          actions: props.actions,
          action: action.value,
          "onUpdate:action": (v: string) => (action.value = v),
          loading: props.loading,
          defaultOpen: props.defaultOpen,
          shortcut: ["Control", "Enter"],
          onRun: (id: string) => (ran.value = `Lancée : ${id}`),
        }),
        h("p", { role: "status", class: "k-cat-note" }, ran.value),
      ]);
  },
});

export default {
  component: SplitButton,
  pattern: "Button + Menu Button (entrées désactivées avec motif)",
  states: [
    { name: "apply", description: "Action par défaut : Appliquer", props: { actions }, demo },
    {
      name: "chosen",
      description: "Action choisie au chevron : Soumettre en revue",
      props: { actions, action: "review" },
      demo,
    },
    {
      name: "held",
      description: "Appliquer retenu par une politique : l'action par défaut est la revue",
      props: { actions: held, action: "review" },
      demo,
    },
    {
      name: "forbidden",
      description: "Action choisie interdite : grisée, focalisable, infobulle « Il faut … »",
      props: { actions: forbidden, action: "apply" },
      demo,
      interaction: "focus",
    },
    { name: "loading", description: "Action en cours", props: { actions, loading: true }, demo },
    {
      name: "open",
      description: "Chevron ouvert : entrée interdite avec son motif",
      props: { actions: forbidden, action: "review", defaultOpen: true },
      demo,
      isolated: true,
    },
  ],
} satisfies CatalogueEntry;
