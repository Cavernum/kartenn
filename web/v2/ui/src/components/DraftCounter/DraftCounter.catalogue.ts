// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type PropType } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DraftCounter from "./DraftCounter.vue";
import type { DraftSummary } from "./draft.ts";

const drafts: DraftSummary[] = [
  { id: "d1", label: "Salle B", count: 3, current: true },
  { id: "d2", label: "Reprise des VIP", count: 12 },
  { id: "d3", label: "Brouillon du 14 janvier", count: 1 },
];

/**
 * The counter toggles the panel (here, its state alone: no panel, hence no
 * `aria-controls`; the SidePanel page shows it with its panel).
 */
const demo = defineComponent({
  props: {
    count: { type: Number, required: true },
    expanded: Boolean,
    drafts: { type: Array as PropType<DraftSummary[]>, default: undefined },
    defaultOpen: Boolean,
  },
  setup(props) {
    const expanded = ref(props.expanded);
    return () =>
      h(DraftCounter, {
        count: props.count,
        expanded: expanded.value,
        drafts: props.drafts,
        defaultOpen: props.defaultOpen,
        onToggle: () => (expanded.value = !expanded.value),
      });
  },
});

export default {
  component: DraftCounter,
  pattern: "Button (bascule du panneau, aria-expanded) et Menu Button (liste des brouillons)",
  states: [
    { name: "pending", description: "Opérations en brouillon", props: { count: 12 }, demo },
    { name: "empty", description: "Compteur à 0", props: { count: 0 }, demo },
    {
      name: "expanded",
      description: "Panneau ouvert : l'icône devient une croix (le prochain appui le ferme)",
      props: { count: 3, expanded: true },
      demo,
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { count: 3 },
      demo,
      interaction: "focus",
    },
    {
      name: "drafts",
      description: "Avec la liste des brouillons de l'utilisateur (chevron)",
      props: { count: 3, drafts },
      demo,
    },
    {
      name: "drafts-open",
      description: "Liste des brouillons ouverte",
      props: { count: 3, drafts, defaultOpen: true },
      demo,
      isolated: true,
    },
  ],
} satisfies CatalogueEntry;
