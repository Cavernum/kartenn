// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoFrame from "../../demo/DemoFrame.vue";
import StatusBadge from "../StatusBadge/StatusBadge.vue";
import MatrixGrid from "./MatrixGrid.vue";
import type { MatrixAxis, MatrixCellState } from "./matrix.ts";

const ZONES: MatrixAxis[] = [
  { id: "servers", label: "Serveurs" },
  { id: "desks", label: "Postes du siège" },
  { id: "monitoring", label: "Supervision" },
  { id: "backup", label: "Sauvegardes" },
  { id: "guests", label: "Invités" },
];
/** Allowed when the source comes before the destination, or to the monitoring. */
const allowed = (row: MatrixAxis, column: MatrixAxis, axis: readonly MatrixAxis[]) =>
  column.id === "monitoring" || axis.indexOf(row) <= axis.indexOf(column);

/** A cell: its state in words, colour and icon (StatusBadge). */
const cell = (axis: readonly MatrixAxis[]) => ({
  cell: ({ row, column }: { row: MatrixAxis; column: MatrixAxis }) =>
    allowed(row, column, axis)
      ? h(StatusBadge, { state: "ok", label: "Autorisé" })
      : h(StatusBadge, { state: "ko", label: "Refusé" }),
});

const policy: FunctionalComponent<Record<string, unknown>> = (props) =>
  h(
    MatrixGrid as Component,
    {
      label: "Matrice de politique : source en ligne, destination en colonne",
      corner: "Source → destination",
      axes: { rows: "Source", columns: "Destination" },
      ...props,
    },
    cell(ZONES),
  );

const STATES: Record<string, MatrixCellState> = {
  "desks:backup": "draft",
  "guests:servers": "review",
};

/** A matrix larger than its frame: both headers stay while it scrolls. */
const LARGE: MatrixAxis[] = Array.from({ length: 24 }, (_, i) => ({
  id: `zone-${i + 1}`,
  label: `zone-${String(i + 1).padStart(2, "0")}`,
  mono: true,
}));
const large: FunctionalComponent = () =>
  h(DemoFrame, () =>
    h(
      MatrixGrid,
      { label: "Matrice des zones", corner: "Source → destination", rows: LARGE, columns: LARGE },
      cell(LARGE),
    ),
  );

export default {
  component: MatrixGrid,
  pattern: "Grid (données) ; en-têtes de lignes et de colonnes figés",
  states: [
    {
      name: "rest",
      description:
        "Matrice dirigée, cellule active : croix sur sa ligne et sa colonne, flèches de sens",
      props: { rows: ZONES, columns: ZONES, active: { row: "desks", column: "servers" } },
      demo: policy,
      wide: true,
    },
    {
      name: "cell-states",
      description: "Cellules en brouillon et en revue (icône et mot caché)",
      props: {
        rows: ZONES,
        columns: ZONES,
        cellState: (row: MatrixAxis, column: MatrixAxis) => STATES[`${row.id}:${column.id}`],
      },
      demo: policy,
      wide: true,
    },
    {
      name: "crosshair",
      description: "Cellule survolée : la croix la suit, la cellule active garde son liseré",
      props: { rows: ZONES, columns: ZONES, active: { row: "desks", column: "servers" } },
      demo: policy,
      wide: true,
      interaction: "hover",
      target: "[data-row='backup'][data-column='monitoring']",
    },
    {
      name: "cell-focus",
      description: "Focus clavier sur une cellule : la croix suit le focus",
      props: { rows: ZONES, columns: ZONES, active: { row: "guests", column: "backup" } },
      demo: policy,
      wide: true,
      interaction: "focus",
      target: "[data-row='guests'][data-column='backup']",
    },
    {
      name: "large",
      description: "24 × 24 dans un cadre : en-têtes figés dans les deux sens au défilement",
      props: {},
      demo: large,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
