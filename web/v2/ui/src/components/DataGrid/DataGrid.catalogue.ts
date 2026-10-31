// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoFrame from "../../demo/DemoFrame.vue";
import { settleDemo } from "../../demo/settle.ts";
import { environments, prefixes } from "../../fixtures/index.ts";
import EnvChip from "../EnvChip/EnvChip.vue";
import KeyLabel from "../KeyLabel/KeyLabel.vue";
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

/**
 * Cells holding a widget: an environment chip and a key label, out of the tab order,
 * reached by Enter (APG Grid). Their focus ring must stand whole inside the cell.
 */
const Widgets = () =>
  h(
    DataGrid as Component,
    { ...base, rows: rows.slice(0, 2) },
    {
      cell: ({ row, column }: { row: Row; column: GridColumn }) => {
        const env = environments.find((e) => e.label === row.env) ?? environments[0];
        if (column.id === "env")
          return h(EnvChip, { value: env.key, label: env.label, hue: env.hue });
        if (column.id === "owner")
          return h(KeyLabel, { value: "reseau", label: row.owner, showKey: true });
        return String(row[column.id as keyof Row]);
      },
    },
  );

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

/**
 * Columns of priority 2 and 3 in a grid of 420 px (a container under `md`): they hide, the
 * key column sticks, « Colonnes » shows them again (design « Adaptation à la largeur »).
 */
const prioritized: GridColumn[] = columns.map((c) =>
  c.id === "env" || c.id === "owner"
    ? { ...c, priority: 3 }
    : c.id === "used"
      ? { ...c, priority: 2 }
      : c,
);
const visibility = ref<Record<string, boolean>>({});
const narrow = () =>
  h("div", { style: { maxInlineSize: "var(--k-size-panel-lg)" } }, [
    h(DataGrid as Component, {
      ...base,
      columns: prioritized,
      columnVisibility: visibility.value,
      "onUpdate:columnVisibility": (next: Record<string, boolean>) => (visibility.value = next),
    }),
  ]);

/**
 * A simple list (a key, three attributes) in 420 px, a container under `sm`: its rows
 * stack, every column shown, the header a strip of sort and filter controls.
 */
const listColumns: GridColumn[] = prioritized.filter((c) => c.id !== "env");
const stacked = () =>
  h("div", { style: { maxInlineSize: "var(--k-size-panel-lg)" } }, [
    h(DataGrid as Component, {
      ...base,
      columns: listColumns,
      stack: true,
      selectable: true,
      selected: [prefixes[1]],
      sort: { column: "prefix", direction: "ascending" },
      filtered: ["prefix"],
      rowState: (row: Row) => (row.prefix === prefixes[2] ? "draft" : undefined),
    }),
  ]);

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
      name: "cell-error-hover",
      description:
        "Cellule invalide survolée : son message (code et texte) dans une couche jointe sous la cellule, qui garde sa taille",
      props: { ...base, cellState },
      wide: true,
      interaction: "hover",
      target: "[aria-invalid='true']",
    },
    {
      name: "cell-error-focus",
      description:
        "Cellule invalide au focus du clavier : le même message, fermé par Échap jusqu'au départ du focus",
      props: { ...base, cellState, active: { row: prefixes[1], column: "owner" } },
      wide: true,
      interaction: "focus",
      target: "[aria-invalid='true']",
    },
    {
      name: "row-states",
      description:
        "Lignes en brouillon, en revue, retirée dans le brouillon (barrée) ; une ligne qui entre dans un brouillon s'éclaire puis s'estompe",
      props: {
        ...base,
        rowState: (row: Row) =>
          row.prefix === prefixes[1]
            ? "draft"
            : row.prefix === prefixes[3]
              ? "review"
              : row.prefix === prefixes[4]
                ? "removed"
                : undefined,
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
      name: "settle",
      description:
        "Du squelette aux données : les lignes prennent la place de leurs squelettes, rien ne bouge (CLS 0)",
      props: {},
      demo: settleDemo((loading) =>
        h(DataGrid as Component, { ...base, loading, loadingRows: rows.length }),
      ),
      wide: true,
    },
    {
      name: "widget-env",
      description:
        "Pastille d'environnement d'une cellule au focus (Entrée) : son anneau entier, la cellule ne le coupe pas",
      props: {},
      demo: Widgets,
      interaction: "focus",
      target: ".k-env-chip-frame",
      wide: true,
    },
    {
      name: "widget-key",
      description: "Libellé et clé d'une cellule au focus : son anneau entier, sa bulle",
      props: {},
      demo: Widgets,
      interaction: "focus",
      target: ".k-key-label__text",
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
      name: "priorities",
      description:
        "Grille de 420 px : colonnes de priorité 2 et 3 masquées, colonne de clé collante, « Colonnes » les rend",
      props: {},
      demo: narrow,
      wide: true,
    },
    {
      name: "stacked",
      description:
        "Liste simple (stack) dans 420 px : lignes empilées, la clé puis chaque attribut sous son libellé, toutes les colonnes montrées",
      props: {},
      demo: stacked,
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
