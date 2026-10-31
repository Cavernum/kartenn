// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import Tabs from "./Tabs.vue";
import type { TabItem } from "./tabs.ts";

const tabs: TabItem[] = [
  { value: "attributes", label: "Attributs" },
  { value: "addresses", label: "Adresses", count: 42 },
  { value: "history", label: "Historique", icon: "clock" },
  { value: "vlan", label: "VLAN", state: "loading" },
  { value: "flows", label: "Flux", state: "error" },
];

/** The tabs with a sentence in each panel. */
const demo: FunctionalComponent = (props) =>
  h(
    Tabs as Component,
    props,
    Object.fromEntries(tabs.map((tab) => [tab.value, () => h("p", `Panneau « ${tab.label} ».`)])),
  );

/**
 * The tabs of the keyboard script (APG Tabs, manual activation; e2e/components/tabs.spec.ts):
 * a field before them, the tab shown said under them.
 */
const Keyboard = defineComponent({
  setup() {
    const before = ref("");
    const tab = ref("attributes");
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(TextField, {
          label: "Préfixe",
          modelValue: before.value,
          "onUpdate:modelValue": (value: string) => (before.value = value),
        }),
        h(
          Tabs as Component,
          {
            tabs,
            label: "Fiche du préfixe",
            modelValue: tab.value,
            "onUpdate:modelValue": (value: string) => (tab.value = value),
          },
          Object.fromEntries(tabs.map((t) => [t.value, () => h("p", `Panneau « ${t.label} ».`)])),
        ),
      ]);
  },
});

export default {
  component: Tabs,
  pattern: "Tabs (activation manuelle)",
  states: [
    {
      name: "first",
      description: "Premier onglet actif ; onglets de plugin en chargement et en erreur",
      props: { tabs, label: "Fiche du préfixe" },
      demo,
      wide: true,
    },
    {
      name: "second",
      description: "Un autre onglet actif, avec compteur",
      props: { tabs, label: "Fiche du préfixe", modelValue: "addresses" },
      demo,
      wide: true,
    },
    {
      name: "hover",
      description: "Survol d'un onglet",
      props: { tabs, label: "Fiche du préfixe" },
      demo,
      wide: true,
      interaction: "hover",
      target: "[role=tab]:nth-child(3)",
    },
    {
      name: "focus",
      description: "Focus visible sur l'onglet actif",
      props: { tabs, label: "Fiche du préfixe" },
      demo,
      wide: true,
      interaction: "focus",
      target: "[role=tab]",
    },
    {
      name: "keyboard",
      description:
        "Au clavier : les flèches, Origine et Fin déplacent le focus, Entrée ou Espace active l'onglet (activation manuelle)",
      props: {},
      demo: Keyboard,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
