<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts" generic="R extends Record<string, unknown>">
// A compact data grid that opens large (design « Inventaire », ExpandableGrid; revue du
// 2026-10-25): a list that may be long (the flows a draft makes nonconforming, the members of a group)
// kept to a few rows in a narrow place (a panel, a section), its row count above it and
// an « Agrandir » button that opens the same table in a large modal dialog (APG Dialog
// (Modal): focus trapped, Escape closes, the focus back on the button). Both grids are
// `DataGrid`s over the same rows, sort and filters (the host keeps them, as for
// `DataGrid`); the compact one may show fewer columns (`compactColumns`). It never
// truncates silently: whatever is left out is one button away. Editable as a `DataGrid`
// (`editable`, `validate`, `commit`), both grids say why a cell or a row is in its state
// and why a cell offers no editor (`stateReason`, `rowReason`, `readonlyReason`).
import { computed } from "vue";
import { currentLocale, t } from "../../i18n.ts";
import DataGrid from "../DataGrid/DataGrid.vue";
import type { CellState, GridColumn, GridCommit, GridSort, RowState } from "../DataGrid/grid.ts";
import Dialog from "../Dialog/Dialog.vue";
import IconButton from "../IconButton/IconButton.vue";
import NoValue from "../NoValue/NoValue.vue";

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
    /** Which cells the user may edit, in both grids (as for `DataGrid`). */
    editable?: ((row: R, column: GridColumn) => boolean) | undefined;
    /** Why a cell of an editable grid offers no editor (as for `DataGrid`). */
    readonlyReason?: ((row: R, column: GridColumn) => string | undefined) | undefined;
    /** Why a cell is in its state, a write held in review (as for `DataGrid`). */
    stateReason?: ((row: R, column: GridColumn) => string | undefined) | undefined;
    /** Why a row is in its state, said by its first cell (as for `DataGrid`). */
    rowReason?: ((row: R) => string | readonly string[] | undefined) | undefined;
    /** The local check of the text of an editor (as for `DataGrid`). */
    validate?: ((row: R, column: GridColumn, value: string) => string | undefined) | undefined;
  }>(),
  {
    compactColumns: undefined,
    visibleRows: 5,
    cellState: undefined,
    rowState: undefined,
    filtered: () => [],
    editable: undefined,
    readonlyReason: undefined,
    stateReason: undefined,
    rowReason: undefined,
    validate: undefined,
  },
);
/** The sort, shared by both grids. */
const sort = defineModel<GridSort | undefined>("sort", { default: undefined });
/** The large dialog is open. */
const expanded = defineModel<boolean>("expanded", { default: false });
const emit = defineEmits<{
  filter: [column: string];
  /** An editor of either grid commits its text (as for `DataGrid`). */
  commit: [commit: GridCommit];
}>();
defineSlots<{
  cell?: (scope: { row: R; column: GridColumn; value: unknown }) => unknown;
}>();

/**
 * What both grids take alike: the rows, their states and why, who edits and the check.
 * Each grid keeps its own focus and editor.
 */
const shared = computed(() => ({
  rows: props.rows,
  rowKey: props.rowKey,
  cellState: props.cellState,
  rowState: props.rowState,
  filtered: props.filtered,
  editable: props.editable,
  readonlyReason: props.readonlyReason,
  stateReason: props.stateReason,
  rowReason: props.rowReason,
  validate: props.validate,
}));

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
/**
 * Above the compact grid: « 3 sur 10 lignes » when it holds more than it shows, so that
 * it never looks finished (relecture du 2026-10-26); the dialog says the whole count.
 */
const shownCount = computed(() =>
  props.rows.length > props.visibleRows
    ? `${new Intl.NumberFormat(currentLocale()).format(props.visibleRows)} ${t("of")} ${count.value}`
    : count.value,
);
</script>

<template>
  <div class="k-expandable-grid" :style="height">
    <div class="k-expandable-grid__bar">
      <span class="k-expandable-grid__count">{{ shownCount }}</span>
      <Dialog v-model:open="expanded" :title="props.label" :description="count" size="lg" contained>
        <template #trigger>
          <IconButton icon="maximize-2" size="sm" :label="`${t('enlarge')} : ${props.label}`" />
        </template>
        <DataGrid
          v-model:sort="sort"
          class="k-expandable-grid__full"
          v-bind="shared"
          :label="props.label"
          :columns="props.columns"
          @filter="emit('filter', $event)"
          @commit="emit('commit', $event)"
        >
          <template #cell="scope">
            <slot name="cell" v-bind="scope"
              ><NoValue v-if="scope.value === null" /><template v-else>{{
                scope.value
              }}</template></slot
            >
          </template>
        </DataGrid>
      </Dialog>
    </div>
    <DataGrid
      v-model:sort="sort"
      class="k-expandable-grid__compact"
      v-bind="shared"
      :label="props.label"
      :columns="compact"
      @filter="emit('filter', $event)"
      @commit="emit('commit', $event)"
    >
      <template #cell="scope">
        <slot name="cell" v-bind="scope"
          ><NoValue v-if="scope.value === null" /><template v-else>{{
            scope.value
          }}</template></slot
        >
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
