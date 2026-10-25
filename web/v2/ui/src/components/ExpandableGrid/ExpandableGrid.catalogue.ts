// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { computed, defineComponent, h, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import type { GridColumn, GridSort } from "../DataGrid/grid.ts";
import ExpandableGrid from "./ExpandableGrid.vue";

/** Flows a draft would break: invented hosts of two environments, documentation ports. */
const SOURCES = ["rece-batch", "rece-ci", "rece-web", "rece-etl", "rece-mon"];
const TARGETS = ["prod-api", "prod-registry", "prod-db", "prod-cache", "prod-bus"];
const SERVICES = ["tcp/443", "tcp/5000", "tcp/5432", "tcp/6379", "tcp/5672"];
const flows = Array.from({ length: 25 }, (_, i) => ({
  key: `f${i}`,
  source: SOURCES[i % 5]!,
  destination: TARGETS[Math.floor(i / 5)]!,
  service: SERVICES[Math.floor(i / 5)]!,
  rule: i % 3 === 0 ? "flow.verdict.default" : "flow.verdict.denied",
}));
type Flow = (typeof flows)[number];

const columns: GridColumn[] = [
  { id: "source", label: "Source", width: "md", mono: true, sortable: true },
  { id: "destination", label: "Destination", width: "fill", mono: true, sortable: true },
  { id: "service", label: "Service", width: "sm", mono: true },
  { id: "rule", label: "Règle", width: "lg", filterable: true },
];
const base = {
  label: "Flux cassés",
  columns,
  rows: flows,
  rowKey: "key",
  compactColumns: ["source", "destination"],
};

/** The host sorts the rows (as for DataGrid); the rule shows as a copyable code. */
const sorted = (expanded: boolean) =>
  defineComponent({
    setup() {
      const sort = ref<GridSort>({ column: "source", direction: "ascending" });
      const open = ref(expanded);
      const rows = computed(() =>
        [...flows].sort((a, b) => {
          const column = sort.value.column as keyof Flow;
          const order = String(a[column]).localeCompare(String(b[column]));
          return sort.value.direction === "ascending" ? order : -order;
        }),
      );
      return () =>
        h(
          ExpandableGrid as Component,
          {
            ...base,
            rows: rows.value,
            sort: sort.value,
            "onUpdate:sort": (next: GridSort) => (sort.value = next),
            expanded: open.value,
            "onUpdate:expanded": (next: boolean) => (open.value = next),
          },
          {
            cell: ({ column, value }: { column: GridColumn; value: unknown }) =>
              column.id === "rule" ? h(CodeBadge, { code: String(value) }) : String(value),
          },
        );
    },
  });

export default {
  component: ExpandableGrid,
  pattern: "Grid compacte + Button qui ouvre un Dialog (Modal) avec la même grille",
  states: [
    {
      name: "compact",
      description: "25 flux, cinq lignes visibles, deux colonnes ; « Agrandir » ouvre le reste",
      props: {},
      demo: sorted(false),
    },
    {
      name: "few",
      description: "Trois lignes : la grille garde sa hauteur naturelle",
      props: { ...base, rows: flows.slice(0, 3) },
    },
    {
      name: "expanded",
      description: "Agrandie : boîte de dialogue large, toutes les colonnes, tri et filtre",
      props: {},
      demo: sorted(true),
      isolated: true,
    },
  ],
} satisfies CatalogueEntry;
