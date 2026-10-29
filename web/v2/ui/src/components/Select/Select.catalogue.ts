// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import Select from "./Select.vue";
import type { ListOption } from "./select.ts";

/** Usages of an address (a closed list of the contract): key as value, label shown. */
const usages: ListOption[] = [
  { value: "host", label: "Hôte" },
  { value: "gateway", label: "Passerelle" },
  { value: "vip", label: "VIP" },
  { value: "reserved", label: "Réservée" },
  { value: "dhcp", label: "DHCP" },
];

/** Groups of VLAN anchored on a site: key in fixed pitch, range of VID as detail. */
const groups: ListOption[] = [
  { value: "campus-ouest", label: "campus-ouest", mono: true, detail: "100–199" },
  { value: "campus-est", label: "campus-est", mono: true, detail: "200–299" },
  { value: "labo", label: "labo", mono: true, detail: "900–949", disabled: true },
];

/** Types of flow of the policy matrix (E10). */
const flowTypes: ListOption[] = [
  { value: "internal", label: "Interne" },
  { value: "admin", label: "Administration" },
  { value: "backup", label: "Sauvegarde" },
];

/**
 * The select of the keyboard script (APG Select-Only Combobox;
 * e2e/components/select.spec.ts): a usage, what is chosen said under it, a field after it
 * to leave it by Tab.
 */
const Keyboard = defineComponent({
  setup() {
    const usage = ref<string | undefined>("host");
    const owner = ref("");
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(Select, {
          label: "Usage",
          options: usages,
          modelValue: usage.value,
          "onUpdate:modelValue": (value: string | undefined) => (usage.value = value),
        }),
        h(
          "p",
          { role: "status", class: "k-cat-note" },
          `Usage choisi : ${usages.find((u) => u.value === usage.value)?.label ?? "aucun"}`,
        ),
        h(TextField, {
          label: "Propriétaire",
          modelValue: owner.value,
          "onUpdate:modelValue": (value: string) => (owner.value = value),
        }),
      ]);
  },
});

export default {
  component: Select,
  pattern: "Select-Only Combobox (bouton combobox, liste listbox)",
  states: [
    {
      name: "rest",
      description: "Au repos, une valeur choisie, avec aide",
      props: {
        label: "Usage",
        options: usages,
        modelValue: "host",
        hint: "Liste fermée du contrat.",
      },
    },
    {
      name: "placeholder",
      description: "Aucune valeur, obligatoire",
      props: { label: "Usage", options: usages, placeholder: "Choisir un usage", required: true },
    },
    {
      name: "mono",
      description: "Clés en chasse fixe, plage en détail",
      props: { label: "Groupe", options: groups, modelValue: "campus-ouest", mono: true },
    },
    {
      name: "hover",
      description: "Survol",
      props: { label: "Usage", options: usages, modelValue: "host" },
      interaction: "hover",
      target: "button",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { label: "Usage", options: usages, modelValue: "host" },
      interaction: "focus",
      target: "button",
    },
    {
      name: "open",
      description: "Ouvert : l'option choisie cochée, détail en chasse fixe, option désactivée",
      props: {
        label: "Groupe",
        options: groups,
        modelValue: "campus-ouest",
        mono: true,
        defaultOpen: true,
      },
      isolated: true,
    },
    {
      name: "invalid",
      description: "Invalide : erreur liée et aria-invalid",
      props: {
        label: "Usage",
        options: usages,
        placeholder: "Choisir un usage",
        required: true,
        error: "L'usage est obligatoire.",
        hint: "Liste fermée du contrat.",
      },
    },
    {
      name: "readonly",
      description: "Lecture seule : focalisable, ne s'ouvre pas",
      props: { label: "Usage", options: usages, modelValue: "gateway", readonly: true },
    },
    {
      name: "disabled",
      description: "Désactivé",
      props: { label: "Usage", options: usages, modelValue: "host", disabled: true },
    },
    {
      name: "small",
      description: "Petit (24 px en compacte), étiquette cachée",
      props: { label: "Usage", options: usages, modelValue: "host", hideLabel: true, size: "sm" },
    },
    {
      name: "large",
      description: "Grand (32 px en compacte)",
      props: { label: "Usage", options: usages, modelValue: "host", size: "lg" },
    },
    {
      name: "toolbar",
      description: "Dans une barre : l'étiquette écrite avant la valeur, icône",
      props: {
        label: "Type de flux",
        options: flowTypes,
        modelValue: "internal",
        inlineLabel: true,
        icon: "flow",
      },
    },
    {
      name: "keyboard",
      description: "Au clavier : ouvrir, parcourir, choisir, fermer sans choisir",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
