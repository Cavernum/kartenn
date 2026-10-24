<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts" generic="R extends Record<string, unknown>">
// The data grid of kartenn (APG Grid, written by kartenn: Reka has none; design
// « Inventaire », line 20). Visual version (task 12): the structure of the pattern (grid,
// rows and cells indexed, sortable headers with `aria-sort`, selection), the states of
// rows and cells decided by the host (draft, review, pending, error with its code), a cell
// being edited. Virtualisation, keyboard and editing come with the complete version
// (task 25); the API is theirs already.
import { computed } from "vue";
import { t } from "../../i18n.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Icon from "../Icon/Icon.vue";
import TextField from "../TextField/TextField.vue";
import {
  columnsTemplate,
  type CellRef,
  type CellState,
  type GridColumn,
  type GridSort,
  type RowState,
} from "./grid.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    columns: GridColumn[];
    rows: R[];
    /** The field of a row that identifies it. */
    rowKey: keyof R & string;
    sort?: GridSort | undefined;
    selectable?: boolean;
    selected?: string[];
    /** The cell that holds the roving focus. */
    active?: CellRef | undefined;
    editing?: CellRef | undefined;
    cellState?: ((row: R, column: GridColumn) => CellState | undefined) | undefined;
    rowState?: ((row: R) => RowState | undefined) | undefined;
  }>(),
  {
    sort: undefined,
    selected: () => [],
    active: undefined,
    editing: undefined,
    cellState: undefined,
    rowState: undefined,
  },
);

const template = computed(() => ({ "--k-grid-columns": columnsTemplate(props.columns) }));
const keyOf = (row: R) => String(row[props.rowKey]);
const isSelected = (row: R) => props.selected.includes(keyOf(row));
const ariaSort = (column: GridColumn) =>
  !column.sortable ? undefined : props.sort?.column === column.id ? props.sort.direction : "none";
const isActive = (row: string, column: string) =>
  props.active ? props.active.row === row && props.active.column === column : false;
const tabindex = (row: string, column: string) => {
  if (props.active) return isActive(row, column) ? 0 : -1;
  // Without an active cell, the first header is the entry point of the grid.
  return row === "" && column === props.columns[0]?.id ? 0 : -1;
};
const isEditing = (row: R, column: GridColumn) =>
  props.editing?.row === keyOf(row) && props.editing.column === column.id;

function stateOf(row: R, column: GridColumn): string | undefined {
  const state = props.cellState?.(row, column);
  return state === undefined ? undefined : typeof state === "string" ? state : "error";
}
const WORDS = { pending: "pending", draft: "inDraft", review: "inReview" } as const;
/** The state said in words, for assistive technologies (never colour alone). */
function stateWord(row: R, column: GridColumn): string | undefined {
  const state = stateOf(row, column);
  return state && state in WORDS ? t(WORDS[state as keyof typeof WORDS]) : undefined;
}
function errorOf(row: R, column: GridColumn): { error: string; code?: string } | undefined {
  const state = props.cellState?.(row, column);
  return typeof state === "object" ? state : undefined;
}
</script>

<template>
  <div
    class="k-data-grid"
    role="grid"
    :aria-label="props.label"
    :aria-rowcount="props.rows.length + 1"
    :aria-colcount="props.columns.length"
    :aria-multiselectable="props.selectable ? 'true' : undefined"
    :style="template"
  >
    <div role="rowgroup" class="k-data-grid__head">
      <div role="row" aria-rowindex="1" class="k-data-grid__row">
        <div
          v-for="(column, c) in props.columns"
          :key="column.id"
          role="columnheader"
          class="k-data-grid__header"
          :data-align="column.align"
          :aria-colindex="c + 1"
          :aria-sort="ariaSort(column)"
          :tabindex="tabindex('', column.id)"
        >
          <span class="k-data-grid__label">{{ column.label }}</span>
          <Icon
            v-if="column.sortable"
            :name="
              props.sort?.column !== column.id
                ? 'arrow-up-down'
                : props.sort.direction === 'ascending'
                  ? 'arrow-up'
                  : 'arrow-down'
            "
            class="k-data-grid__sort"
            :data-active="props.sort?.column === column.id ? 'true' : undefined"
          />
          <Icon v-if="column.filterable" name="funnel" class="k-data-grid__filter" />
        </div>
      </div>
    </div>
    <div role="rowgroup" class="k-data-grid__body">
      <div
        v-for="(row, r) in props.rows"
        :key="keyOf(row)"
        role="row"
        class="k-data-grid__row"
        :aria-rowindex="r + 2"
        :aria-selected="props.selectable ? isSelected(row) : undefined"
        :data-row-state="props.rowState?.(row)"
      >
        <div
          v-for="(column, c) in props.columns"
          :key="column.id"
          role="gridcell"
          class="k-data-grid__cell"
          :class="{ 'k-mono': column.mono }"
          :data-align="column.align"
          :data-state="stateOf(row, column)"
          :data-editing="isEditing(row, column) ? 'true' : undefined"
          :aria-colindex="c + 1"
          :aria-busy="stateOf(row, column) === 'pending' ? 'true' : undefined"
          :aria-invalid="errorOf(row, column) ? 'true' : undefined"
          :tabindex="isEditing(row, column) ? undefined : tabindex(keyOf(row), column.id)"
        >
          <TextField
            v-if="isEditing(row, column)"
            :label="`${column.label} — ${keyOf(row)}`"
            hide-label
            size="md"
            :mono="column.mono ?? false"
            :model-value="String(row[column.id] ?? '')"
            :error="errorOf(row, column)?.error"
            class="k-data-grid__editor"
          />
          <template v-else>
            <template v-if="c === 0 && props.rowState?.(row)">
              <Icon
                :name="props.rowState?.(row) === 'draft' ? 'draft' : 'eye'"
                class="k-data-grid__row-mark"
              />
              <span class="k-visually-hidden"
                >({{ props.rowState?.(row) === "draft" ? t("inDraft") : t("inReview") }})</span
              >
            </template>
            <span class="k-data-grid__value">
              <slot name="cell" :row="row" :column="column" :value="row[column.id]">{{
                row[column.id]
              }}</slot>
            </span>
            <Icon
              v-if="stateOf(row, column) === 'pending'"
              name="loader-circle"
              class="k-spinner k-data-grid__mark"
            />
            <Icon
              v-else-if="stateOf(row, column) === 'error'"
              name="circle-x"
              class="k-data-grid__mark"
            />
            <Icon
              v-else-if="stateOf(row, column) === 'review'"
              name="eye"
              class="k-data-grid__mark"
            />
            <span v-if="stateWord(row, column)" class="k-visually-hidden"
              >({{ stateWord(row, column) }})</span
            >
          </template>
          <div
            v-if="errorOf(row, column)"
            class="k-data-grid__error"
            :data-side="c >= props.columns.length / 2 ? 'end' : 'start'"
            role="alert"
          >
            <span>{{ errorOf(row, column)!.error }}</span>
            <CodeBadge v-if="errorOf(row, column)!.code" :code="errorOf(row, column)!.code!" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
@layer components {
  .k-data-grid {
    --k-grid-columns: var(--k-size-col-md);

    position: relative;
    overflow: auto;
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-data-grid__head {
    position: sticky;
    inset-block-start: 0;
    z-index: var(--k-z-sticky);
    background-color: var(--k-surface-sunken);
  }

  /* Head and body share one template and one width. */
  .k-data-grid__head,
  .k-data-grid__body {
    inline-size: max-content;
    min-inline-size: 100%;
  }

  .k-data-grid__row {
    display: grid;
    grid-template-columns: var(--k-grid-columns);
    min-block-size: var(--k-row-height);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-data-grid__head .k-data-grid__row {
    min-block-size: var(--k-header-height);
  }

  .k-data-grid__body .k-data-grid__row:hover {
    background-color: var(--k-hover);
  }

  /* Selected: the soft accent and the inner edge, never the tint alone (WCAG 1.4.1). */
  .k-data-grid__row[aria-selected="true"],
  .k-data-grid__body .k-data-grid__row[aria-selected="true"]:hover {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  /* Draft and review rows: an icon and a hidden word in their first cell. */
  .k-data-grid__row-mark {
    flex: none;
    color: var(--k-accent);
  }

  .k-data-grid__row[data-row-state="review"] .k-data-grid__row-mark {
    color: var(--k-state-review-fg);
  }

  .k-data-grid__row[data-row-state="review"] {
    background-color: var(--k-state-review-bg);
  }

  .k-data-grid__header,
  .k-data-grid__cell {
    position: relative;
    display: flex;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
    padding-inline: var(--k-cell-padding-inline);
    white-space: nowrap;
  }

  .k-data-grid__header {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text-muted);
  }

  .k-data-grid__header[data-align="end"],
  .k-data-grid__cell[data-align="end"] {
    justify-content: end;
    font-variant-numeric: tabular-nums;
  }

  .k-data-grid__header:focus-visible,
  .k-data-grid__cell:focus-visible {
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-data-grid__sort {
    color: var(--k-text-muted);
  }

  .k-data-grid__sort[data-active] {
    color: var(--k-accent);
  }

  .k-data-grid__filter {
    margin-inline-start: auto;
    color: var(--k-text-muted);
  }

  .k-data-grid__value {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .k-data-grid__cell[data-state="draft"] {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-data-grid__cell[data-state="review"] {
    background-color: var(--k-state-review-bg);
  }

  .k-data-grid__cell[data-state="review"] .k-data-grid__mark {
    color: var(--k-state-review-fg);
  }

  .k-data-grid__cell[data-state="pending"] {
    color: var(--k-text-muted);
  }

  .k-data-grid__cell[data-state="error"] {
    background-color: var(--k-state-ko-bg);
    outline: var(--k-border-width) solid var(--k-state-ko-fg);
    outline-offset: calc(var(--k-border-width) * -1);
  }

  .k-data-grid__cell[data-state="error"] .k-data-grid__mark {
    color: var(--k-state-ko-fg);
  }

  .k-data-grid__mark {
    flex: none;
    margin-inline-start: auto;
  }

  /* The editor takes the whole cell, at the height of the row. */
  .k-data-grid__error[data-side="end"] {
    inset-inline: auto 0;
  }

  .k-data-grid__cell[data-editing] {
    padding-inline: 0;
  }

  .k-data-grid__editor {
    flex: 1;
  }

  /* The error of a cell, under it, over the next row. */
  .k-data-grid__error {
    position: absolute;
    inset-block-start: 100%;
    inset-inline-start: 0;
    z-index: var(--k-z-overlay);
    display: flex;
    flex-wrap: nowrap;
    gap: var(--k-space-2);
    align-items: center;
    inline-size: max-content;
    max-inline-size: var(--k-size-panel-lg);
    padding: var(--k-space-1) var(--k-space-2);
    font-family: var(--k-font-sans);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-state-ko-fg);
    white-space: normal;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-state-ko-fg);
    border-radius: var(--k-radius-md);
  }
}
</style>
