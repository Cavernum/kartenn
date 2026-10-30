// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { computed, defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { domains, environments } from "../../fixtures/index.ts";
import { filterOptions, type Suggest } from "../Combobox/combobox.ts";
import type { ListOption } from "../Select/select.ts";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import FilterPicker from "./FilterPicker.vue";
import { filtersFromParams, filtersToParams, type Filter, type FilterModel } from "./filters.ts";

/** A key without a label in the language: its key stands for it (REQ-020-21). */
const keyed = (key: string, label: string | null): ListOption =>
  label ? { value: key, label } : { value: key, label: key, mono: true };

/** Owners, read from the API (an entity, its slug): answered after a short latency. */
const owners: ListOption[] = [
  { value: "atelier-nord", label: "Atelier Nord" },
  { value: "atelier-sud", label: "Atelier Sud" },
  { value: "exploitation", label: "Exploitation" },
];
const suggestOwner: Suggest = (query, signal) =>
  new Promise((resolve, reject) => {
    const late = setTimeout(() => resolve(filterOptions(owners, query)), 40);
    signal.addEventListener("abort", () => {
      clearTimeout(late);
      reject(signal.reason);
    });
  });

/**
 * The filters of the list of addresses (route `GET /v1/addresses`, 006 « Filtres »): core
 * fields (domain, value with `within`, usage) and declared properties (environment, owner,
 * a TTL without a label in French, a description), as the 021 draws them from the contract.
 */
export const addressFilters: FilterModel = [
  {
    key: "domain",
    label: "Domaine",
    type: "enum",
    operators: ["eq"],
    options: domains.map((d) => keyed(d.key, d.label)),
  },
  {
    key: "value",
    label: "Adresse",
    type: "address",
    operators: ["eq", "within"],
    params: { within: "within" },
  },
  {
    key: "usage",
    label: "Usage",
    type: "enum",
    operators: ["eq"],
    options: [
      { value: "host", label: "Hôte" },
      { value: "vip", label: "VIP" },
      { value: "gateway", label: "Passerelle" },
      { value: "reserved", label: "Réservée" },
      { value: "dhcp", label: "DHCP" },
    ],
  },
  {
    key: "prop.policy.environment",
    label: "Environnement",
    type: "enum",
    operators: ["eq", "exists", "absent"],
    options: environments.map((e) => keyed(e.key, e.label)),
  },
  {
    key: "prop.core.owner",
    label: "Propriétaire",
    type: "enum",
    operators: ["eq", "exists", "absent"],
    suggest: suggestOwner,
    valueLabel: (key) => owners.find((o) => o.value === key)?.label,
  },
  { key: "prop.dns.ttl", type: "number", operators: ["eq", "between", "exists", "absent"] },
  { key: "created", label: "Création", type: "date", operators: ["after", "before", "between"] },
  {
    key: "prop.core.description",
    label: "Description",
    type: "text",
    operators: ["contains", "exists", "absent"],
  },
];

const applied: Filter[] = [
  { key: "domain", operator: "eq", values: ["dc-nord"] },
  { key: "value", operator: "within", values: ["192.0.2.0/24"] },
  { key: "usage", operator: "eq", values: ["host", "vip"] },
];

const invalid: Filter[] = [
  { key: "created", operator: "after", values: ["2026-13-01"] },
  { key: "value", operator: "within", values: ["192.0.2.0/33"] },
  { key: "prop.dns.ttl", operator: "between", values: ["3600", "60"] },
];

const keys: Filter[] = [
  { key: "prop.dns.ttl", operator: "between", values: ["60", "3600"] },
  { key: "prop.policy.environment", operator: "eq", values: ["form"] },
  { key: "prop.core.description", operator: "absent", values: [] },
];

/**
 * The bar of the keyboard script (e2e/components/filter-picker.spec.ts): its filters read
 * from a query, and the query they write, beside the parameters that are not filters (kept
 * as they are); a field after it to leave it by Tab.
 */
const Keyboard = defineComponent({
  setup() {
    const start = new URLSearchParams(
      "domain=dc-nord&within=192.0.2.0%2F24&valid_at=2026-01-15T10%3A00%3A00%2B01%3A00",
    );
    const read = filtersFromParams(addressFilters, start);
    const filters = ref<Filter[]>(read.filters);
    const query = computed(() =>
      decodeURIComponent(
        filtersToParams(addressFilters, filters.value, new URLSearchParams(read.rest)).toString(),
      ),
    );
    const note = ref("");
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(FilterPicker, {
          model: addressFilters,
          label: "Filtres des adresses",
          shortcut: ["/"],
          modelValue: filters.value,
          "onUpdate:modelValue": (next: Filter[]) => (filters.value = next),
        }),
        h("p", { class: "k-cat-note" }, [
          "URL : ",
          h("code", { "data-query": "" }, `?${query.value}`),
        ]),
        h(TextField, {
          label: "Note",
          modelValue: note.value,
          "onUpdate:modelValue": (next: string) => (note.value = next),
        }),
      ]);
  },
});

export default {
  component: FilterPicker,
  pattern: "Button puis Combobox en séquence (propriété, opérateur, valeur) dans un popover",
  states: [
    {
      name: "rest",
      description: "Filtres appliqués : libellés des valeurs, plusieurs valeurs (« l'un de »)",
      props: { model: addressFilters, modelValue: applied },
      wide: true,
    },
    {
      name: "empty",
      description: "Aucun filtre : le bouton d'ajout seul",
      props: { model: addressFilters, modelValue: [], shortcut: ["/"] },
    },
    {
      name: "draft",
      description: "Brouillon : un filtre commencé puis laissé, en pointillés, hors de l'URL",
      props: {
        model: addressFilters,
        modelValue: [
          applied[0],
          { key: "prop.core.owner", operator: "eq", values: ["atelier-nord"], draft: true },
        ],
      },
      wide: true,
    },
    {
      name: "invalid",
      description: "Lus dans l'URL et hors type : marqués invalides, le motif dit",
      props: { model: addressFilters, modelValue: invalid },
      wide: true,
    },
    {
      name: "keys",
      description: "Sans libellé dans la langue : la clé en chasse fixe (propriété, valeur)",
      props: { model: addressFilters, modelValue: keys },
      wide: true,
    },
    {
      name: "open",
      description: "Ajout : la liste des propriétés ouverte, libellé puis clé",
      props: { model: addressFilters, modelValue: applied, defaultOpen: true },
      wide: true,
      isolated: true,
    },
    {
      name: "values",
      description: "Modification : plusieurs valeurs choisies, chacune retirable",
      props: { model: addressFilters, modelValue: applied, defaultOpen: 2 },
      wide: true,
      isolated: true,
    },
    {
      name: "refused",
      description: "Modification d'un filtre invalide : le motif lié au champ de la valeur",
      props: { model: addressFilters, modelValue: invalid, defaultOpen: 0 },
      wide: true,
      isolated: true,
    },
    {
      name: "between",
      description: "Entre deux bornes : deux champs typés, une borne vide reste ouverte",
      props: { model: addressFilters, modelValue: keys, defaultOpen: 0 },
      wide: true,
      isolated: true,
    },
    {
      name: "keyboard",
      description: "Au clavier : ajout, modification, retrait ; l'URL écrite à chaque filtre",
      props: {},
      demo: Keyboard,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
