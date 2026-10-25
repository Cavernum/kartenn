// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoFrame from "../../demo/DemoFrame.vue";
import { environments, prefixes } from "../../fixtures/index.ts";
import DataGrid from "./DataGrid.vue";
import type { CellState, GridColumn } from "./grid.ts";

const columns: GridColumn[] = [
  { id: "prefix", label: "Préfixe", width: "md", mono: true, sortable: true, filterable: true },
  { id: "label", label: "Libellé", width: "fill", sortable: true },
  { id: "env", label: "Environnement", width: "md", filterable: true },
  { id: "used", label: "Occupation", width: "md", align: "end", sortable: true },
  { id: "owner", label: "Responsable", width: "md" },
];

const LABELS = ["Serveurs de bases", "Postes du siège", "Supervision", "Sauvegardes", "Invités"];
const rows = prefixes.slice(0, 5).map((prefix, i) => ({
  prefix,
  label: LABELS[i]!,
  env: environments[i % 3]!.label,
  used: `${[12, 87, 45, 3, 100][i]} %`,
  owner: ["Équipe réseau", "Équipe postes", "Exploitation"][i % 3]!,
}));
type Row = (typeof rows)[number];

const base = { label: "Préfixes du domaine", columns, rows, rowKey: "prefix" };

/** The states of the contract of in-place editing, one cell each. */
const states: Record<string, CellState> = {
  [`${prefixes[0]}:label`]: "pending",
  [`${prefixes[1]}:owner`]: { error: "Responsable inconnu.", code: "core.owner.unknown" },
  [`${prefixes[3]}:owner`]: "draft",
  [`${prefixes[4]}:env`]: "review",
};
const cellState = (row: Row, column: GridColumn) => states[`${row.prefix}:${column.id}`];

/** The host keeps the filters: Alt+Down (or the funnel) on a header sets or clears one. */
const filtering = defineComponent({
  setup() {
    const filtered = ref<string[]>(["prefix"]);
    return () =>
      h(DataGrid as Component, {
        ...base,
        filtered: filtered.value,
        onFilter: (column: string) =>
          (filtered.value = filtered.value.includes(column)
            ? filtered.value.filter((c) => c !== column)
            : [...filtered.value, column]),
      });
  },
});

/** Many rows in a frame of fixed height: the header sticks, head and cells stay aligned. */
const many = Array.from({ length: 30 }, (_, i) => ({
  prefix: `192.0.2.${i * 8}/29`,
  label: LABELS[i % LABELS.length]!,
  env: environments[i % 3]!.label,
  used: `${(i * 37) % 101} %`,
  owner: ["Équipe réseau", "Équipe postes", "Exploitation"][i % 3]!,
}));
const sticky = () => h(DemoFrame, () => h(DataGrid as Component, { ...base, rows: many }));

export default {
  component: DataGrid,
  pattern: "Grid (maison) ; une seule grille, lignes en subgrid ; tri et filtre de colonne",
  states: [
    {
      name: "rest",
      description: "Tri sur le préfixe, colonnes filtrables, focus sur la première cellule",
      props: { ...base, sort: { column: "prefix", direction: "ascending" } },
      wide: true,
    },
    {
      name: "selected",
      description: "Lignes sélectionnées (aria-selected), multisélection",
      props: { ...base, selectable: true, selected: [prefixes[1], prefixes[2]] },
      wide: true,
    },
    {
      name: "cell-states",
      description: "Cellules en brouillon, en attente, en revue, invalide (motif et code)",
      props: { ...base, cellState },
      wide: true,
    },
    {
      name: "row-states",
      description: "Lignes en brouillon et en revue",
      props: {
        ...base,
        rowState: (row: Row) =>
          row.prefix === prefixes[1] ? "draft" : row.prefix === prefixes[3] ? "review" : undefined,
      },
      wide: true,
    },
    {
      name: "wrap",
      description: "Colonne de message (wrap) : le texte va à la ligne, jamais coupé",
      props: {
        ...base,
        columns: columns.map((c) => (c.id === "label" ? { ...c, wrap: true } : c)),
        rows: rows.map((row, i) =>
          i === 1
            ? {
                ...row,
                label:
                  "Postes du siège, deuxième étage : la plage sera rendue au domaine après la migration des imprimantes.",
              }
            : row,
        ),
      },
      wide: true,
    },
    {
      name: "loading",
      description:
        "Chargement : squelettes animés dans les cellules (immobiles sous mouvement réduit)",
      props: { ...base, loading: true },
      wide: true,
    },
    {
      name: "filtered",
      description:
        "Filtre posé sur le préfixe ; Alt+Bas sur un en-tête filtrable demande son filtre",
      props: {},
      demo: filtering,
      wide: true,
    },
    {
      name: "sticky",
      description: "Trente lignes dans un cadre : l'en-tête reste collé, aligné sur les cellules",
      props: {},
      demo: sticky,
      wide: true,
    },
    {
      name: "editing",
      description: "Cellule en édition (éditeur en place)",
      props: {
        ...base,
        editing: { row: prefixes[2], column: "label" },
        active: { row: prefixes[2], column: "label" },
      },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
