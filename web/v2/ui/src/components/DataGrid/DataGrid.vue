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
// Adaptation to the width (design « Adaptation à la largeur »): the root is a container;
// the grid scrolls in its own region (`scrollbar-gutter: stable`), its key column sticks
// at the inline start, and under a container `md` (`lg`) the columns of priority 2 (3)
// hide, a column chooser above the grid showing them again. The arrows skip what is
// hidden. The class and style of the host go to the root, other attributes to the grid.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useAttrs, watch } from "vue";
import { t } from "../../i18n.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Icon from "../Icon/Icon.vue";
import ColumnChooser from "../internal/ColumnChooser.vue";
import { isRtl, moveInGrid } from "../internal/gridKeys.ts";
import Skeleton from "../Skeleton/Skeleton.vue";
import TextField from "../TextField/TextField.vue";
import {
  columnTemplates,
  nextSort,
  priorityOf,
  renderedColumns,
  type CellRef,
  type CellState,
  type ColumnVisibility,
  type GridColumn,
  type GridSort,
  type RowState,
} from "./grid.ts";

defineOptions({ inheritAttrs: false });

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

/** The user's choice of columns (column chooser). */
const visibility = defineModel<ColumnVisibility>("columnVisibility", { default: () => ({}) });
const attrs = useAttrs();
const rootAttrs = computed(() => ({ class: attrs.class, style: attrs.style }));
const gridAttrs = computed(() =>
  Object.fromEntries(Object.entries(attrs).filter(([key]) => key !== "class" && key !== "style")),
);

const grid = ref<HTMLElement | null>(null);
const root = ref<HTMLElement | null>(null);
/** The columns rendered (the user's hidden ones left out), in order. */
const shown = computed(() => renderedColumns(props.columns, visibility.value));
const template = computed(() => columnTemplates(props.columns, visibility.value));
/** The priority a cell carries for the container queries; none for an always shown one. */
const priority = (column: GridColumn) => {
  const p = priorityOf(props.columns, column, visibility.value);
  return p > 1 ? p : undefined;
};
/** The index of a column among all of them (`aria-colindex`), hidden ones counted. */
const colIndex = (column: GridColumn) => props.columns.indexOf(column) + 1;
/** The columns on screen now: the rendered ones whose header takes room. */
function onScreen(): GridColumn[] {
  const visible = shown.value.filter((column) => {
    const header = grid.value?.querySelector(
      `[role="columnheader"][data-column="${CSS.escape(column.id)}"]`,
    );
    return !!header && header.getClientRects().length > 0;
  });
  return visible.length > 0 ? visible : shown.value;
}
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

const ROW_MARKS = {
  draft: { icon: "draft", word: "inDraft" },
  review: { icon: "eye", word: "inReview" },
  removed: { icon: "minus", word: "removedInDraft" },
} as const;
/** The icon and the hidden word of a row with a state. */
const markOf = (row: R) => ROW_MARKS[props.rowState?.(row) ?? "draft"];

/**
 * The rows that just joined a draft (design « Mouvement »): their background fades from
 * the soft accent once, without moving their neighbours. A row is fresh when its state
 * turns to `draft` after the first render; a row the host hides (filter) and shows again
 * keeps its last state, so it does not flash again. Cleared at the end of its animation.
 */
const fresh = ref(new Set<string>());
const known = new Map<string, RowState | undefined>();
let firstRender = true;
watch(
  () => props.rows.map((row) => [keyOf(row), props.rowState?.(row)] as const),
  (entries) => {
    const next = new Set<string>();
    for (const [key, state] of entries) {
      if (!firstRender && state === "draft" && known.get(key) !== "draft") next.add(key);
      known.set(key, state);
    }
    firstRender = false;
    if (next.size > 0 || fresh.value.size > 0) fresh.value = next;
  },
  { immediate: true },
);
function onAnimationend(key: string, event: AnimationEvent): void {
  if (event.animationName !== "k-row-fresh" || !fresh.value.has(key)) return;
  const next = new Set(fresh.value);
  next.delete(key);
  fresh.value = next;
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
  // The arrows move over the columns on screen only.
  const columns = onScreen();
  const from = {
    row: Math.max(0, rowKeys.value.indexOf(cell.row)),
    column: Math.max(
      0,
      columns.findIndex((c) => c.id === cell.column),
    ),
  };
  const to = moveInGrid(from, event, rowKeys.value.length, columns.length, isRtl(grid.value!));
  if (!to) return;
  event.preventDefault();
  active.value = { row: rowKeys.value[to.row]!, column: columns[to.column]!.id };
  void focusActive();
}

// A container query may hide the column of the cell that holds the roving focus: the stop
// goes back to the key column of its row, so that Tab still reaches the grid.
let observer: ResizeObserver | undefined;
onMounted(() => {
  if (typeof ResizeObserver === "undefined" || !root.value) return;
  observer = new ResizeObserver(() => {
    const cell = active.value;
    if (cell && !onScreen().some((c) => c.id === cell.column))
      active.value = { row: cell.row, column: props.columns[0]?.id ?? "" };
  });
  observer.observe(root.value);
});
onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <div ref="root" class="k-data-grid-frame" v-bind="rootAttrs">
    <ColumnChooser v-model="visibility" :columns="props.columns" :grid="grid" />
    <div
      ref="grid"
      v-bind="gridAttrs"
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
            v-for="column in shown"
            :key="column.id"
            role="columnheader"
            class="k-data-grid__header"
            :data-column="column.id"
            data-row=""
            :data-align="column.align"
            :data-priority="priority(column)"
            :data-sortable="column.sortable ? 'true' : undefined"
            :aria-colindex="colIndex(column)"
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
            v-for="(column, c) in shown"
            :key="column.id"
            role="gridcell"
            class="k-data-grid__cell"
            :data-align="column.align"
            :data-priority="priority(column)"
            :aria-colindex="colIndex(column)"
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
          :data-fresh="fresh.has(keyOf(row)) ? 'true' : undefined"
          @animationend="onAnimationend(keyOf(row), $event)"
        >
          <div
            v-for="(column, c) in shown"
            :key="column.id"
            role="gridcell"
            class="k-data-grid__cell"
            :class="{ 'k-mono': column.mono }"
            :data-row="keyOf(row)"
            :data-column="column.id"
            :data-align="column.align"
            :data-priority="priority(column)"
            :data-wrap="column.wrap ? 'true' : undefined"
            :data-state="stateOf(row, column)"
            :data-editing="isEditing(row, column) ? 'true' : undefined"
            :aria-colindex="colIndex(column)"
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
                <Icon :name="markOf(row).icon" class="k-data-grid__row-mark" />
                <span class="k-visually-hidden">({{ t(markOf(row).word) }})</span>
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
              :data-side="c >= shown.length / 2 ? 'end' : 'start'"
              role="alert"
            >
              <span>{{ errorOf(row, column)!.error }}</span>
              <CodeBadge v-if="errorOf(row, column)!.code" :code="errorOf(row, column)!.code!" />
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
@layer components {
  /* The root: a container that takes the width it is given, the chooser above the grid,
   * the grid in the room left (a host's height bound applies here). */
  .k-data-grid-frame {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
    min-block-size: 0;
    container-type: inline-size;
  }

  .k-data-grid-frame > .k-column-chooser {
    padding-block-end: var(--k-space-1);
  }

  .k-data-grid-frame > .k-data-grid {
    grid-row: 2;
  }

  /* One grid: the columns are set here, once; rows and row groups are subgrids of it. It
   * scrolls in itself, a region named by its label (WCAG 1.4.10, note 2). */
  .k-data-grid {
    --k-grid-columns: var(--k-size-col-md);
    --k-grid-columns-below-lg: var(--k-grid-columns);
    --k-grid-columns-below-md: var(--k-grid-columns);

    position: relative;
    display: grid;
    grid-template-columns: var(--k-grid-columns);
    align-content: start;
    min-inline-size: 0;
    overflow: auto;
    scrollbar-gutter: stable;
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
  /* Over the key column, which sticks too. */
  .k-data-grid__head {
    position: sticky;
    inset-block-start: 0;
    z-index: calc(var(--k-z-sticky) + 1);
    background-color: var(--k-surface-sunken);
  }

  .k-data-grid__row {
    min-block-size: var(--k-row-height);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  /* Opaque rows: the sticky key cell takes their background, whatever their state. */
  .k-data-grid__head .k-data-grid__row {
    background-color: inherit;
  }

  .k-data-grid__body .k-data-grid__row {
    background-color: var(--k-surface);
  }

  /* The key column sticks at the inline start while the grid scrolls sideways. */
  .k-data-grid__row > :first-child {
    position: sticky;
    inset-inline-start: 0;
    z-index: var(--k-z-sticky);
    background-color: inherit;
  }

  .k-data-grid__row[aria-selected="true"] > :first-child {
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-data-grid__row > [data-state="error"]:first-child {
    z-index: var(--k-z-overlay);
  }

  /* Columns of priority 3 (2) leave a container under `lg` (`md`): the template drops
   * them (set by CSSOM), the cells hide (design « Adaptation à la largeur »). */
  @container (width < 960px) {
    .k-data-grid {
      grid-template-columns: var(--k-grid-columns-below-lg);
    }

    .k-data-grid [data-priority="3"] {
      display: none;
    }
  }

  @container (width < 640px) {
    .k-data-grid {
      grid-template-columns: var(--k-grid-columns-below-md);
    }

    .k-data-grid [data-priority="2"] {
      display: none;
    }
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

  /* Removed in a draft: struck through and muted, in its place until the draft applies. */
  .k-data-grid__row[data-row-state="removed"] .k-data-grid__row-mark {
    color: var(--k-diff-removed);
  }

  .k-data-grid__row[data-row-state="removed"] .k-data-grid__value {
    color: var(--k-text-muted);
    text-decoration-line: line-through;
  }

  /* Motion (design « Mouvement »): hover, selection and states change the background by a
   * colour fade; a row that just joined a draft starts on the soft accent, which fades
   * out over `slow`. Nothing moves; sort, filter and refresh are never animated. */
  .k-data-grid__body .k-data-grid__row {
    transition-property: background-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-data-grid__row[data-fresh] {
    animation-name: k-row-fresh;
    animation-duration: var(--k-duration-slow);
    animation-timing-function: var(--k-ease-standard);
  }

  @keyframes k-row-fresh {
    from {
      background-color: var(--k-accent-soft);
    }
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

  /* A message wraps, never clipped: the row grows to hold it. */
  .k-data-grid__cell[data-wrap] {
    padding-block: var(--k-space-1);
    white-space: normal;
  }

  .k-data-grid__cell[data-wrap] > .k-data-grid__value {
    overflow-wrap: anywhere;
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

  /* The sentence wraps; the code beside it stays whole (024/M16). */
  .k-data-grid__error > span:first-child {
    min-inline-size: 0;
  }

  .k-data-grid__error > .k-code-badge {
    flex: none;
  }
}
</style>
