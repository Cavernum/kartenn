// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { prefixes } from "../../fixtures/index.ts";
import Stack from "../Stack/Stack.vue";
import TextField from "./TextField.vue";

/** The error of a prefix in CIDR notation, IPv4 (the check of the keyboard state). */
function prefixError(value: string): string | undefined {
  if (value === "") return undefined;
  const parts = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})\/(\d{1,2})$/.exec(value);
  if (!parts) return "Notation CIDR attendue (192.0.2.0/24).";
  const [, a, b, c, d, length] = parts.map(Number) as number[];
  if ([a, b, c, d].some((octet) => octet! > 255) || length! > 32)
    return "Adresse hors de l'espace IPv4.";
  return undefined;
}

/**
 * The fields of the keyboard script (e2e/components/text-field.spec.ts): a prefix checked
 * when it is left (`change`), its error said and linked, then gone once it is right; a
 * name after it, to leave the first by Tab.
 */
const Keyboard = defineComponent({
  setup() {
    const prefix = ref("");
    const error = ref<string | undefined>();
    const name = ref("");
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(TextField, {
          label: "Préfixe",
          mono: true,
          hint: "Notation CIDR.",
          modelValue: prefix.value,
          "onUpdate:modelValue": (value: string) => (prefix.value = value),
          error: error.value,
          onChange: () => (error.value = prefixError(prefix.value)),
        }),
        h(TextField, {
          label: "Nom",
          modelValue: name.value,
          "onUpdate:modelValue": (value: string) => (name.value = value),
        }),
      ]);
  },
});

export default {
  component: TextField,
  pattern: "champ natif étiqueté (aide et erreur par aria-describedby)",
  states: [
    {
      name: "rest",
      description: "Au repos, avec aide",
      props: { label: "Nom", hint: "Visible dans les listes." },
    },
    {
      name: "placeholder",
      description: "Vide, avec icône et exemple",
      props: {
        label: "Recherche",
        icon: "search",
        type: "search",
        placeholder: "Nom, préfixe, adresse",
      },
    },
    {
      name: "mono",
      description: "Chasse fixe (préfixe), obligatoire",
      props: { label: "Préfixe", mono: true, required: true, modelValue: prefixes[1] },
    },
    {
      name: "hover",
      description: "Survol",
      props: { label: "Nom", modelValue: "Serveurs de bases" },
      interaction: "hover",
      target: "input",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { label: "Nom", modelValue: "Serveurs de bases" },
      interaction: "focus",
      target: "input",
    },
    {
      name: "invalid",
      description: "Invalide : erreur liée et aria-invalid",
      props: {
        label: "Préfixe",
        mono: true,
        modelValue: "192.0.2.300/24",
        error: "Adresse hors de l'espace IPv4.",
        hint: "Notation CIDR.",
      },
    },
    {
      name: "pending",
      description: "En attente (vérification en cours)",
      props: { label: "Préfixe", mono: true, modelValue: prefixes[2], pending: true },
    },
    {
      name: "readonly",
      description: "Lecture seule",
      props: {
        label: "Identifiant",
        mono: true,
        readonly: true,
        modelValue: "01JD4K7Q9V2M8X3N5P6R7S8T9W",
      },
    },
    {
      name: "disabled",
      description: "Désactivé",
      props: { label: "Nom", disabled: true, modelValue: "Serveurs de bases" },
    },
    {
      name: "small",
      description: "Petit (24 px en compacte), étiquette cachée",
      props: {
        label: "Filtrer",
        hideLabel: true,
        size: "sm",
        icon: "funnel",
        placeholder: "Filtrer",
      },
    },
    {
      name: "large",
      description: "Grand (32 px en compacte)",
      props: { label: "Nom", size: "lg", modelValue: "Serveurs de bases" },
    },
    {
      name: "missing",
      description: "Obligatoire et vide : l'erreur avant l'aide",
      props: {
        label: "Nom",
        required: true,
        error: "Le nom est obligatoire.",
        hint: "Visible dans les listes.",
      },
    },
    {
      name: "keyboard",
      description: "Au clavier : erreur dite en quittant le champ, liée, puis retirée",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
