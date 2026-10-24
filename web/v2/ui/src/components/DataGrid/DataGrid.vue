<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts" generic="R extends Record<string, unknown>">
// The data grid of kartenn (APG Grid, written by kartenn: Reka has none; design
// « Inventaire », line 20; « Structure avant style »). One CSS grid: the columns are set
// once on the grid, every row (header included) is a subgrid of it, and the header sticks
// to the top of the same scroll container: head and cells align by construction, whatever
// the scroll bar and the elastic columns (REQ-020-34). Keyboard of the pattern (arrows,
// Home, End, Ctrl+Home, Ctrl+End, Page Up, Page Down) over a roving focus; on a header,
// Enter or Space sorts (`aria-sort`) and Alt+Down asks for the filter of the column (the
// host opens it). The states of rows and cells are decided by the host (draft, review,
// pending, error with its code); loading, the rows are skeletons in the cells.
// Virtualisation and the editing keys come with the complete version (020, task 25).
import { computed, nextTick, ref } from "vue";
import { t } from "../../i18n.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Icon from "../Icon/Icon.vue";
import { isRtl, moveInGrid } from "../internal/gridKeys.ts";
import Skeleton from "../Skeleton/Skeleton.vue";
import TextField from "../TextField/TextField.vue";
import {
  columnsTemplate,
  nextSort,
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
    selectable?: boolean;
    selected?: string[];
    editing?: CellRef | undefined;
    cellState?: ((row: R, column: GridColumn) => CellState | undefined) | undefined;
    rowState?: ((row: R) => RowState | undefined) | undefined;
    /** The columns whose filter is set (the host keeps the filters). */
    filtered?: string[];
    /** The rows are coming: skeleton rows in the cells, the grid busy. */
    loading?: boolean;
    loadingRows?: number;
  }>(),
  {
    selected: () => [],
    editing: undefined,
    cellState: undefined,
    rowState: undefined,
    filtered: () => [],
    loadingRows: 5,
  },
);
/** The sort of the grid; a sortable header changes it. */
const sort = defineModel<GridSort | undefined>("sort", { default: undefined });
/** The cell that holds the roving focus (the first header when unset). */
const active = defineModel<CellRef | undefined>("active", { default: undefined });
const emit = defineEmits<{
  /** Alt+Down or a click on the funnel of a filterable header. */
  filter: [column: string];
  /** Enter on a cell of the body. */
  activate: [cell: CellRef];
}>();

const grid = ref<HTMLElement | null>(null);
const template = computed(() => ({ "--k-grid-columns": columnsTemplate(props.columns) }));
const keyOf = (row: R) => String(row[props.rowKey]);
const isSelected = (row: R) => props.selected.includes(keyOf(row));
const ariaSort = (column: GridColumn) =>
  !column.sortable ? undefined : sort.value?.column === column.id ? sort.value.direction : "none";
const isFiltered = (column: GridColumn) => props.filtered.includes(column.id);

/** The keys of the rows the focus moves over: the header (""), then the body. */
const rowKeys = computed(() => ["", ...(props.loading ? [] : props.rows.map(keyOf))]);
const current = computed<CellRef>(
  () => active.value ?? { row: "", column: props.columns[0]?.id ?? "" },
);
const tabindex = (row: string, column: string) =>
  current.value.row === row && current.value.column === column ? 0 : -1;
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

function sortBy(column: GridColumn): void {
  if (column.sortable) sort.value = nextSort(sort.value, column.id);
}

async function focusActive(): Promise<void> {
  await nextTick();
  grid.value
    ?.querySelector<HTMLElement>(
      '[role="columnheader"][tabindex="0"], [role="gridcell"][tabindex="0"]',
    )
    ?.focus();
}

/** The cell under `target`, as the grid keys it. */
function cellOf(target: EventTarget | null): CellRef | null {
  const cell = (target as HTMLElement | null)?.closest<HTMLElement>("[data-column]");
  return cell ? { row: cell.dataset.row ?? "", column: cell.dataset.column! } : null;
}

function onFocusin(event: FocusEvent): void {
  const cell = cellOf(event.target);
  if (cell && (cell.row !== current.value.row || cell.column !== current.value.column))
    active.value = cell;
}

function onKeydown(event: KeyboardEvent): void {
  // The editor of a cell keeps its keys.
  if ((event.target as HTMLElement).closest("input, textarea")) return;
  const cell = current.value;
  const column = props.columns.find((c) => c.id === cell.column);
  if (cell.row === "" && column) {
    if (event.altKey && event.key === "ArrowDown") {
      if (column.filterable) emit("filter", column.id);
      event.preventDefault();
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      sortBy(column);
      event.preventDefault();
      return;
    }
  } else if (event.key === "Enter") {
    emit("activate", cell);
    event.preventDefault();
    return;
  }
  const from = {
    row: Math.max(0, rowKeys.value.indexOf(cell.row)),
    column: Math.max(
      0,
      props.columns.findIndex((c) => c.id === cell.column),
    ),
  };
  const to = moveInGrid(
    from,
    event,
    rowKeys.value.length,
    props.columns.length,
    isRtl(grid.value!),
  );
  if (!to) return;
  event.preventDefault();
  active.value = { row: rowKeys.value[to.row]!, column: props.columns[to.column]!.id };
  void focusActive();
}
</script>

<template>
  <div
    ref="grid"
    class="k-data-grid"
    role="grid"
    :aria-label="props.label"
    :aria-rowcount="(props.loading ? props.loadingRows : props.rows.length) + 1"
    :aria-colcount="props.columns.length"
    :aria-multiselectable="props.selectable ? 'true' : undefined"
    :aria-busy="props.loading ? 'true' : undefined"
    :style="template"
    @keydown="onKeydown"
    @focusin="onFocusin"
  >
    <div role="rowgroup" class="k-data-grid__head">
      <div role="row" aria-rowindex="1" class="k-data-grid__row">
        <div
          v-for="(column, c) in props.columns"
          :key="column.id"
          role="columnheader"
          class="k-data-grid__header"
          :data-column="column.id"
          data-row=""
          :data-align="column.align"
          :data-sortable="column.sortable ? 'true' : undefined"
          :aria-colindex="c + 1"
          :aria-sort="ariaSort(column)"
          :tabindex="tabindex('', column.id)"
          @click="sortBy(column)"
        >
          <span class="k-data-grid__label">{{ column.label }}</span>
          <Icon
            v-if="column.sortable"
            :name="
              sort?.column !== column.id
                ? 'arrow-up-down'
                : sort.direction === 'ascending'
                  ? 'arrow-up'
                  : 'arrow-down'
            "
            class="k-data-grid__sort"
            :data-active="sort?.column === column.id ? 'true' : undefined"
          />
          <span
            v-if="column.filterable"
            class="k-data-grid__filter"
            :data-active="isFiltered(column) ? 'true' : undefined"
            @click.stop="emit('filter', column.id)"
          >
            <Icon name="funnel" />
            <span v-if="isFiltered(column)" class="k-visually-hidden">({{ t("filtered") }})</span>
          </span>
        </div>
      </div>
    </div>
    <div v-if="props.loading" role="rowgroup" class="k-data-grid__body">
      <div
        v-for="r in props.loadingRows"
        :key="r"
        role="row"
        class="k-data-grid__row"
        :aria-rowindex="r + 1"
      >
        <div
          v-for="(column, c) in props.columns"
          :key="column.id"
          role="gridcell"
          class="k-data-grid__cell"
          :data-align="column.align"
          :aria-colindex="c + 1"
        >
          <span v-if="r === 1 && c === 0" class="k-visually-hidden">{{ t("loading") }}</span>
          <Skeleton variant="cell" :rows="r" :columns="c" />
        </div>
      </div>
    </div>
    <div v-else role="rowgroup" class="k-data-grid__body">
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
          :data-row="keyOf(row)"
          :data-column="column.id"
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
  /* One grid: the columns are set here, once; rows and row groups are subgrids of it. */
  .k-data-grid {
    --k-grid-columns: var(--k-size-col-md);

    position: relative;
    display: grid;
    grid-template-columns: var(--k-grid-columns);
    align-content: start;
    min-inline-size: 0;
    overflow: auto;
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-data-grid__head,
  .k-data-grid__body,
  .k-data-grid__row {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
  }

  /* The header sticks in the scroll container of the rows (the grid itself). */
  .k-data-grid__head {
    position: sticky;
    inset-block-start: 0;
    z-index: var(--k-z-sticky);
    background-color: var(--k-surface-sunken);
  }

  .k-data-grid__row {
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

  .k-data-grid__header[data-sortable] {
    cursor: pointer;
  }

  .k-data-grid__header[data-sortable]:hover {
    color: var(--k-text);
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

  .k-data-grid__label {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .k-data-grid__sort {
    flex: none;
    color: var(--k-text-muted);
  }

  .k-data-grid__sort[data-active] {
    color: var(--k-accent);
  }

  .k-data-grid__filter {
    display: inline-flex;
    flex: none;
    margin-inline-start: auto;
    color: var(--k-text-muted);
    cursor: pointer;
  }

  .k-data-grid__filter[data-active] {
    color: var(--k-accent);
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
