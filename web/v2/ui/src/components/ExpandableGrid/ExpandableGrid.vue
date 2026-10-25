<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts" generic="R extends Record<string, unknown>">
// A compact data grid that opens large (design « Inventaire », ExpandableGrid; revue du
// 2026-10-25): a list that may be long (the flows a draft breaks, the members of a group)
// kept to a few rows in a narrow place (a panel, a section), its row count above it and
// an « Agrandir » button that opens the same table in a large modal dialog (APG Dialog
// (Modal): focus trapped, Escape closes, the focus back on the button). Both grids are
// `DataGrid`s over the same rows, sort and filters (the host keeps them, as for
// `DataGrid`); the compact one may show fewer columns (`compactColumns`). It never
// truncates silently: whatever is left out is one button away.
import { DialogTrigger } from "reka-ui";
import { computed } from "vue";
import { currentLocale, t } from "../../i18n.ts";
import DataGrid from "../DataGrid/DataGrid.vue";
import type { CellState, GridColumn, GridSort, RowState } from "../DataGrid/grid.ts";
import Dialog from "../Dialog/Dialog.vue";
import IconButton from "../IconButton/IconButton.vue";

const props = withDefaults(
  defineProps<{
    label: string;
    columns: GridColumn[];
    rows: R[];
    rowKey: keyof R & string;
    /** The columns of the compact grid, by id (all of them when unset). */
    compactColumns?: string[] | undefined;
    /** How many rows the compact grid shows before it scrolls. */
    visibleRows?: number;
    cellState?: ((row: R, column: GridColumn) => CellState | undefined) | undefined;
    rowState?: ((row: R) => RowState | undefined) | undefined;
    filtered?: string[];
  }>(),
  {
    compactColumns: undefined,
    visibleRows: 5,
    cellState: undefined,
    rowState: undefined,
    filtered: () => [],
  },
);
/** The sort, shared by both grids. */
const sort = defineModel<GridSort | undefined>("sort", { default: undefined });
/** The large dialog is open. */
const expanded = defineModel<boolean>("expanded", { default: false });
const emit = defineEmits<{ filter: [column: string] }>();
defineSlots<{
  cell?: (scope: { row: R; column: GridColumn; value: unknown }) => unknown;
}>();

const compact = computed(() =>
  props.compactColumns
    ? props.columns.filter((column) => props.compactColumns!.includes(column.id))
    : props.columns,
);
const height = computed(() => ({ "--k-expandable-grid-rows": String(props.visibleRows) }));
const count = computed(() => {
  const n = props.rows.length;
  const word = new Intl.PluralRules(currentLocale()).select(n) === "one" ? "row" : "rows";
  return `${new Intl.NumberFormat(currentLocale()).format(n)} ${t(word)}`;
});
</script>

<template>
  <div class="k-expandable-grid" :style="height">
    <div class="k-expandable-grid__bar">
      <span class="k-expandable-grid__count">{{ count }}</span>
      <Dialog v-model:open="expanded" :title="props.label" :description="count" size="lg" contained>
        <template #trigger>
          <DialogTrigger as-child>
            <IconButton icon="maximize-2" size="sm" :label="`${t('enlarge')} : ${props.label}`" />
          </DialogTrigger>
        </template>
        <DataGrid
          v-model:sort="sort"
          class="k-expandable-grid__full"
          :label="props.label"
          :columns="props.columns"
          :rows="props.rows"
          :row-key="props.rowKey"
          :cell-state="props.cellState"
          :row-state="props.rowState"
          :filtered="props.filtered"
          @filter="emit('filter', $event)"
        >
          <template #cell="scope">
            <slot name="cell" v-bind="scope">{{ scope.value }}</slot>
          </template>
        </DataGrid>
      </Dialog>
    </div>
    <DataGrid
      v-model:sort="sort"
      class="k-expandable-grid__compact"
      :label="props.label"
      :columns="compact"
      :rows="props.rows"
      :row-key="props.rowKey"
      :cell-state="props.cellState"
      :row-state="props.rowState"
      :filtered="props.filtered"
      @filter="emit('filter', $event)"
    >
      <template #cell="scope">
        <slot name="cell" v-bind="scope">{{ scope.value }}</slot>
      </template>
    </DataGrid>
  </div>
</template>

<style>
@layer components {
  /* `--k-expandable-grid-rows` is set on the root by CSSOM (`visibleRows`). */
  .k-expandable-grid {
    display: grid;
    gap: var(--k-space-1);
    min-inline-size: 0;
  }

  /* The count at the start, the button at the end, on one line. */
  .k-expandable-grid__bar {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    justify-content: space-between;
    min-inline-size: 0;
  }

  .k-expandable-grid__count {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-variant-numeric: tabular-nums;
    color: var(--k-text-muted);
  }

  /* The header and a few rows (each with its bottom border, plus the frame of the grid),
   * then the grid scrolls. */
  .k-expandable-grid__compact {
    max-block-size: calc(
      var(--k-header-height) + var(--k-border-width) * 3 +
        (var(--k-row-height) + var(--k-border-width)) * var(--k-expandable-grid-rows)
    );
  }

  /* In the dialog, the grid takes the room left and scrolls in it, its header sticky. */
  .k-expandable-grid__full {
    min-block-size: 0;
  }
}
</style>
