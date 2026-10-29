// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { addresses, prefixes } from "../../fixtures/index.ts";
import type { ListOption } from "../Select/select.ts";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import Combobox from "./Combobox.vue";
import { filterOptions, type Suggest } from "./combobox.ts";

/** Groups of VLAN (a read of the API): key in fixed pitch, range of VID as detail. */
const groups: ListOption[] = [
  { value: "campus-ouest", label: "campus-ouest", mono: true, detail: "100–199" },
  { value: "campus-est", label: "campus-est", mono: true, detail: "200–299" },
  { value: "campus-nord", label: "campus-nord", mono: true, detail: "300–399" },
  { value: "labo", label: "labo", mono: true, detail: "900–949" },
];

/** Objects of a global search: prefixes and addresses, their kind as icon. */
const objects: ListOption[] = [
  ...prefixes.map((p) => ({ value: p, label: p, mono: true, icon: "prefix" as const })),
  ...addresses.map((a) => ({ value: a, label: a, mono: true, icon: "address" as const })),
];

const local =
  (list: ListOption[]): Suggest =>
  async (query) =>
    filterOptions(list, query);
/** An answer that never comes: the list stays loading. */
const pending: Suggest = () => new Promise<ListOption[]>(() => {});
const failing: Suggest = async () => {
  throw new Error("urn:kartenn:problem:unavailable");
};

/**
 * The combobox of the keyboard script (APG Combobox; e2e/components/combobox.spec.ts):
 * groups answered after a short latency, the calls counted (one per pause of the
 * typing), what is taken or searched said under it, a field after it to leave it by Tab.
 */
const Keyboard = defineComponent({
  setup() {
    const text = ref("");
    const value = ref<string | undefined>();
    const calls = ref(0);
    const said = ref("Aucun groupe choisi");
    const owner = ref("");
    const suggest: Suggest = (query, signal) => {
      calls.value += 1;
      return new Promise((resolve, reject) => {
        const late = setTimeout(() => resolve(filterOptions(groups, query)), 40);
        signal.addEventListener("abort", () => {
          clearTimeout(late);
          reject(signal.reason);
        });
      });
    };
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(Combobox, {
          label: "Groupe",
          mono: true,
          suggest,
          modelValue: text.value,
          "onUpdate:modelValue": (next: string) => (text.value = next),
          value: value.value,
          "onUpdate:value": (next: string | undefined) => (value.value = next),
          onSelect: (option: ListOption) => (said.value = `Groupe choisi : ${option.value}`),
          onSubmit: (query: string) => (said.value = `Texte gardé : ${query}`),
        }),
        h("p", { class: "k-cat-note", "data-calls": calls.value }, `Appels : ${calls.value}`),
        h("p", { role: "status", class: "k-cat-note" }, said.value),
        h(TextField, {
          label: "Propriétaire",
          modelValue: owner.value,
          "onUpdate:modelValue": (next: string) => (owner.value = next),
        }),
      ]);
  },
});

export default {
  component: Combobox,
  pattern: "Combobox (aria-autocomplete=list, sélection manuelle, aria-activedescendant)",
  states: [
    {
      name: "rest",
      description: "Au repos, avec aide",
      props: {
        label: "Groupe",
        mono: true,
        suggest: local(groups),
        placeholder: "campus-…",
        hint: "Groupes ancrés sur le site du réseau.",
      },
    },
    {
      name: "chosen",
      description: "Une suggestion prise",
      props: {
        label: "Groupe",
        mono: true,
        suggest: local(groups),
        modelValue: "campus-ouest",
        value: "campus-ouest",
      },
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { label: "Groupe", mono: true, suggest: local(groups) },
      interaction: "focus",
      target: "input",
    },
    {
      name: "open",
      description: "Ouvert : les suggestions du texte, aucune mise en avant",
      props: {
        label: "Groupe",
        mono: true,
        suggest: local(groups),
        modelValue: "campus",
        defaultOpen: true,
      },
      isolated: true,
    },
    {
      name: "loading",
      description: "Chargement des suggestions : indicateur, dit par la région polite",
      props: {
        label: "Groupe",
        mono: true,
        suggest: pending,
        modelValue: "campus",
        defaultOpen: true,
      },
      isolated: true,
    },
    {
      name: "empty",
      description: "Aucune suggestion pour le texte",
      props: {
        label: "Groupe",
        mono: true,
        suggest: local(groups),
        modelValue: "dmz",
        defaultOpen: true,
      },
      isolated: true,
    },
    {
      name: "failed",
      description: "Suggestions indisponibles (l'appel a échoué), le texte reste",
      props: {
        label: "Groupe",
        mono: true,
        suggest: failing,
        modelValue: "campus",
        defaultOpen: true,
      },
      isolated: true,
    },
    {
      name: "invalid",
      description: "Invalide : erreur liée et aria-invalid",
      props: {
        label: "Groupe",
        mono: true,
        suggest: local(groups),
        modelValue: "dmz",
        required: true,
        error: "Aucun groupe dmz sur ce site.",
        hint: "Groupes ancrés sur le site du réseau.",
      },
    },
    {
      name: "disabled",
      description: "Désactivé",
      props: {
        label: "Groupe",
        mono: true,
        suggest: local(groups),
        modelValue: "campus-ouest",
        disabled: true,
      },
    },
    {
      name: "search",
      description: "Recherche globale : petit, icône, étiquette cachée, texte libre (Entrée)",
      props: {
        label: "Rechercher",
        hideLabel: true,
        size: "sm",
        icon: "search",
        placeholder: "Adresse, préfixe",
        suggest: local(objects),
      },
    },
    {
      name: "keyboard",
      description: "Au clavier : saisie débrayée, Alt+Bas, Bas, Entrée, Échap",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
