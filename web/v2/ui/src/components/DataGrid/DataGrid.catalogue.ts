// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { environments, prefixes } from "../../fixtures/index.ts";
import DataGrid from "./DataGrid.vue";
import type { CellState, GridColumn } from "./grid.ts";

const columns: GridColumn[] = [
  { id: "prefix", label: "Préfixe", width: "md", mono: true, sortable: true, filterable: true },
  { id: "label", label: "Libellé", width: "fill", sortable: true },
  { id: "env", label: "Environnement", width: "md", filterable: true },
  { id: "used", label: "Occupation", width: "sm", align: "end", sortable: true },
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

export default {
  component: DataGrid,
  pattern: "Grid (maison)",
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
