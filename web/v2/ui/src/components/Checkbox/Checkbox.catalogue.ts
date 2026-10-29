// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { computed, defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Stack from "../Stack/Stack.vue";
import Checkbox from "./Checkbox.vue";

const ROWS = ["192.0.2.10", "192.0.2.12", "192.0.2.72"] as const;

/**
 * The boxes of the keyboard script (APG Checkbox, mixed; e2e/components/checkbox.spec.ts):
 * a box for all the rows, mixed while some only are chosen, which checks them all, then
 * none; a box per row (hidden labels, as in a grid); a disabled one, out of the tabs.
 */
const Keyboard = defineComponent({
  setup() {
    const chosen = ref<string[]>([ROWS[0]]);
    const all = computed<boolean | "mixed">(() =>
      chosen.value.length === 0 ? false : chosen.value.length === ROWS.length ? true : "mixed",
    );
    return () =>
      h(Stack, { gap: "0" }, () => [
        h(Checkbox, {
          label: `Tout sélectionner (${ROWS.length})`,
          modelValue: all.value,
          "onUpdate:modelValue": (on: boolean | "mixed") => (chosen.value = on ? [...ROWS] : []),
        }),
        ...ROWS.map((row) =>
          h(Checkbox, {
            label: `Sélectionner ${row}`,
            modelValue: chosen.value.includes(row),
            "onUpdate:modelValue": (on: boolean | "mixed") =>
              (chosen.value = on
                ? ROWS.filter((r) => r === row || chosen.value.includes(r))
                : chosen.value.filter((r) => r !== row)),
          }),
        ),
        h(Checkbox, { label: "Anycast", disabled: true }),
      ]);
  },
});

export default {
  component: Checkbox,
  pattern: "Checkbox (deux et trois états)",
  states: [
    { name: "unchecked", description: "Non cochée", props: { label: "Point à point" } },
    { name: "checked", description: "Cochée", props: { label: "Anycast", modelValue: true } },
    {
      name: "mixed",
      description: "Mixte (une partie des lignes) : indeterminate, exposé « mixed »",
      props: { label: "Tout sélectionner (1 284)", modelValue: "mixed" },
    },
    {
      name: "hint",
      description: "Avec aide, liée par aria-describedby",
      props: { label: "Anycast", hint: "La même adresse annoncée en plusieurs lieux." },
    },
    {
      name: "hover",
      description: "Survol",
      props: { label: "Point à point" },
      interaction: "hover",
      target: "input",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { label: "Point à point", modelValue: true },
      interaction: "focus",
      target: "input",
    },
    {
      name: "disabled",
      description: "Désactivée (hors de l'ordre de tabulation)",
      props: { label: "Point à point", disabled: true },
    },
    {
      name: "disabled-checked",
      description: "Désactivée et cochée",
      props: { label: "Anycast", disabled: true, modelValue: true },
    },
    {
      name: "row",
      description: "Case d'une ligne : étiquette cachée, cible de 24 px",
      props: { label: "Sélectionner 192.0.2.10", hideLabel: true, size: "sm" },
    },
    {
      name: "small",
      description: "Petite (24 px en compacte), mixte",
      props: { label: "Tout sélectionner (3)", size: "sm", modelValue: "mixed" },
    },
    {
      name: "keyboard",
      description: "Au clavier : Espace, case mixte des lignes, case désactivée",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
