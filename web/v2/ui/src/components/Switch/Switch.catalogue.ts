// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Stack from "../Stack/Stack.vue";
import Switch from "./Switch.vue";

/**
 * The switches of the keyboard script (APG Switch; e2e/components/switch.spec.ts): a
 * setting that takes effect at once, said under it; a disabled one, out of the tabs.
 */
const Keyboard = defineComponent({
  setup() {
    const reserved = ref(false);
    return () =>
      h(Stack, { gap: "1", align: "start" }, () => [
        h(Switch, {
          label: "Montrer les adresses réservées",
          modelValue: reserved.value,
          "onUpdate:modelValue": (on: boolean) => (reserved.value = on),
        }),
        h(Switch, { label: "Montrer les préfixes vides", disabled: true }),
        h(
          "p",
          { role: "status", class: "k-cat-note" },
          reserved.value ? "Adresses réservées montrées" : "Adresses réservées cachées",
        ),
      ]);
  },
});

export default {
  component: Switch,
  pattern: "Switch (réglage binaire pris aussitôt)",
  states: [
    { name: "off", description: "Arrêt", props: { label: "Montrer les adresses réservées" } },
    {
      name: "on",
      description: "Marche : pastille au bout, piste pleine",
      props: { label: "Montrer les adresses réservées", modelValue: true },
    },
    {
      name: "hint",
      description: "Avec aide, liée par aria-describedby",
      props: {
        label: "Montrer les préfixes vides",
        hint: "Un préfixe sans adresse ni plage.",
      },
    },
    {
      name: "hover",
      description: "Survol",
      props: { label: "Montrer les adresses réservées" },
      interaction: "hover",
      target: "button",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { label: "Montrer les adresses réservées", modelValue: true },
      interaction: "focus",
      target: "button",
    },
    {
      name: "disabled",
      description: "Désactivée (hors de l'ordre de tabulation)",
      props: { label: "Montrer les adresses réservées", disabled: true },
    },
    {
      name: "disabled-on",
      description: "Désactivée, en marche",
      props: { label: "Montrer les adresses réservées", disabled: true, modelValue: true },
    },
    {
      name: "small",
      description: "Petite (24 px en compacte)",
      props: { label: "Montrer les adresses réservées", size: "sm", modelValue: true },
    },
    {
      name: "keyboard",
      description: "Au clavier : Espace et Entrée, effet dit aussitôt, bascule désactivée",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
