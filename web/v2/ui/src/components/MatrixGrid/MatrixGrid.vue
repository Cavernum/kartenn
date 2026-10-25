<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A two-way table (APG Grid; design « Inventaire », line 34; REQ-020-35): the policy
// matrix, the permissions by role. Row and column headers are of the same make, and both
// stay in view when the matrix scrolls (the row of column headers sticks to the top, the
// column of row headers to the inline start, the corner to both). One CSS grid, rows in
// `subgrid` (« Structure avant style »). Keyboard of the pattern over a roving focus,
// headers included; Enter or Space on a cell asks the host to act (`activate`).
// Virtualisation beyond 50 × 50 comes with the screens that need it.
// Crosshair (revue du 2026-10-25): the hovered cell, else the active one, lights its row
// and its column, headers included (`data-cross`), so that one sees which entries of both
// axes it joins; the active cell keeps its accent edge (a shape, not a colour alone). With
// `axes`, the matrix says the direction of what it holds (a flow: rows are the source,
// columns the destination): the corner reads « rows ↓ » and « columns → », and the
// crosshair carries small arrowheads sitting on borders (revue du 2026-10-25, second
// pass): one on the inline-end border of the source row header, pointing into the grid;
// one on the block-end border of the destination column header, pointing down into its
// column; and two on the shown cell, on its inline-start border (pointing along the row,
// from the source) and on its block-start border (pointing down the column, from the
// destination header): the two paths meet in the cell. They are empty pseudo-elements
// (`data-flow`), outside the accessibility tree: the name of the grid and the corner say
// the direction in words.
import { computed, nextTick, ref, shallowRef } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import { isRtl, moveInGrid } from "../internal/gridKeys.ts";
import type { MatrixAxis, MatrixCell, MatrixCellState } from "./matrix.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    /** The text of the corner: what the rows are, then the columns (« Source → Destination »). */
    corner: string;
    rows: MatrixAxis[];
    columns: MatrixAxis[];
    value?: ((row: MatrixAxis, column: MatrixAxis) => string | undefined) | undefined;
    cellState?: ((row: MatrixAxis, column: MatrixAxis) => MatrixCellState | undefined) | undefined;
    /** What the rows and the columns are, for a directed matrix (« Source », « Destination »). */
    axes?: { rows: string; columns: string } | undefined;
  }>(),
  { value: undefined, cellState: undefined, axes: undefined },
);
/** The cell that holds the roving focus (the corner when unset). */
const active = defineModel<MatrixCell | undefined>("active", { default: undefined });
const emit = defineEmits<{ activate: [cell: MatrixCell] }>();

const grid = ref<HTMLElement | null>(null);
const count = computed(() => ({ "--k-matrix-count": String(props.columns.length) }));
const rowIds = computed(() => ["", ...props.rows.map((r) => r.id)]);
const columnIds = computed(() => ["", ...props.columns.map((c) => c.id)]);
const current = computed<MatrixCell>(() => active.value ?? { row: "", column: "" });
const tabindex = (row: string, column: string) =>
  current.value.row === row && current.value.column === column ? 0 : -1;
const isActive = (row: string, column: string) =>
  active.value?.row === row && active.value.column === column;

const WORDS = { draft: "inDraft", review: "inReview" } as const;

/** The cell under the pointer, if any: it leads the crosshair over the active one. */
const hover = shallowRef<MatrixCell | null>(null);
const cross = computed<MatrixCell>(() => hover.value ?? current.value);
/** Whether a cell or a header lies on the crosshair (`""` is a header of the other axis). */
const onCross = (row: string, column: string): "true" | undefined =>
  (row !== "" && row === cross.value.row) || (column !== "" && column === cross.value.column)
    ? "true"
    : undefined;
/** The crosshair stands on a cell of the body: its headers carry the arrows of direction. */
const crossCell = computed(() => cross.value.row !== "" && cross.value.column !== "");
/**
 * The arrowhead of direction of a header or a cell (`data-flow`), for a directed matrix
 * whose crosshair stands on a cell: the source row header, the destination column header,
 * the cell shown.
 */
function flowOf(row: string, column: string): "source" | "destination" | "cell" | undefined {
  if (!props.axes || !crossCell.value) return undefined;
  if (column === "" && row === cross.value.row) return "source";
  if (row === "" && column === cross.value.column) return "destination";
  if (row === cross.value.row && column === cross.value.column) return "cell";
  return undefined;
}

function onPointerover(event: PointerEvent): void {
  const cell = cellOf(event.target);
  const next = cell && cell.row !== "" && cell.column !== "" ? cell : null;
  if (next?.row !== hover.value?.row || next?.column !== hover.value?.column) hover.value = next;
}

async function focusActive(): Promise<void> {
  await nextTick();
  grid.value?.querySelector<HTMLElement>('[data-column][tabindex="0"]')?.focus();
}

function cellOf(target: EventTarget | null): MatrixCell | null {
  const cell = (target as HTMLElement | null)?.closest<HTMLElement>("[data-column]");
  return cell ? { row: cell.dataset.row ?? "", column: cell.dataset.column ?? "" } : null;
}

function onFocusin(event: FocusEvent): void {
  const cell = cellOf(event.target);
  if (cell && (cell.row !== current.value.row || cell.column !== current.value.column))
    active.value = cell;
}

function onKeydown(event: KeyboardEvent): void {
  const cell = current.value;
  if ((event.key === "Enter" || event.key === " ") && cell.row !== "" && cell.column !== "") {
    event.preventDefault();
    emit("activate", cell);
    return;
  }
  const from = {
    row: Math.max(0, rowIds.value.indexOf(cell.row)),
    column: Math.max(0, columnIds.value.indexOf(cell.column)),
  };
  const to = moveInGrid(
    from,
    event,
    rowIds.value.length,
    columnIds.value.length,
    isRtl(grid.value!),
  );
  if (!to) return;
  event.preventDefault();
  active.value = { row: rowIds.value[to.row]!, column: columnIds.value[to.column]! };
  void focusActive();
}
</script>

<template>
  <div
    ref="grid"
    class="k-matrix"
    role="grid"
    :aria-label="props.label"
    :aria-rowcount="props.rows.length + 1"
    :aria-colcount="props.columns.length + 1"
    :style="count"
    @keydown="onKeydown"
    @focusin="onFocusin"
    @pointerover="onPointerover"
    @pointerleave="hover = null"
  >
    <div role="rowgroup" class="k-matrix__head">
      <div role="row" aria-rowindex="1" class="k-matrix__row">
        <div
          role="columnheader"
          class="k-matrix__corner"
          data-row=""
          data-column=""
          aria-colindex="1"
          :tabindex="tabindex('', '')"
        >
          <template v-if="props.axes">
            <span class="k-matrix__axes" aria-hidden="true">
              <span class="k-matrix__axis" data-axis="columns"
                >{{ props.axes.columns }}<Icon name="arrow-right" class="k-matrix__flow"
              /></span>
              <span class="k-matrix__axis" data-axis="rows"
                >{{ props.axes.rows }}<Icon name="arrow-down" class="k-matrix__flow"
              /></span>
            </span>
            <span class="k-visually-hidden">{{ props.corner }}</span>
          </template>
          <template v-else>{{ props.corner }}</template>
        </div>
        <div
          v-for="(column, c) in props.columns"
          :key="column.id"
          role="columnheader"
          class="k-matrix__header"
          :class="{ 'k-mono': column.mono }"
          data-row=""
          :data-column="column.id"
          :data-active="isActive('', column.id) ? 'true' : undefined"
          :data-cross="onCross('', column.id)"
          :data-flow="flowOf('', column.id)"
          :aria-colindex="c + 2"
          :tabindex="tabindex('', column.id)"
        >
          <span class="k-matrix__text">{{ column.label }}</span>
        </div>
      </div>
    </div>
    <div role="rowgroup" class="k-matrix__body">
      <div
        v-for="(row, r) in props.rows"
        :key="row.id"
        role="row"
        class="k-matrix__row"
        :aria-rowindex="r + 2"
      >
        <div
          role="rowheader"
          class="k-matrix__header k-matrix__row-header"
          :class="{ 'k-mono': row.mono }"
          :data-row="row.id"
          data-column=""
          :data-active="isActive(row.id, '') ? 'true' : undefined"
          :data-cross="onCross(row.id, '')"
          :data-flow="flowOf(row.id, '')"
          aria-colindex="1"
          :tabindex="tabindex(row.id, '')"
        >
          <span class="k-matrix__text">{{ row.label }}</span>
        </div>
        <div
          v-for="(column, c) in props.columns"
          :key="column.id"
          role="gridcell"
          class="k-matrix__cell"
          :data-row="row.id"
          :data-column="column.id"
          :data-state="props.cellState?.(row, column)"
          :data-active="isActive(row.id, column.id) ? 'true' : undefined"
          :data-cross="onCross(row.id, column.id)"
          :data-flow="flowOf(row.id, column.id)"
          :aria-colindex="c + 2"
          :tabindex="tabindex(row.id, column.id)"
        >
          <slot
            name="cell"
            :row="row"
            :column="column"
            :value="props.value?.(row, column)"
            :state="props.cellState?.(row, column)"
            >{{ props.value?.(row, column) }}</slot
          >
          <template v-if="props.cellState?.(row, column)">
            <Icon
              :name="props.cellState(row, column) === 'draft' ? 'draft' : 'eye'"
              class="k-matrix__mark"
            />
            <span class="k-visually-hidden">({{ t(WORDS[props.cellState(row, column)!]) }})</span>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<style>
@layer components {
  /* One grid: the column of row headers, then one track per column of the matrix. */
  .k-matrix {
    display: grid;
    grid-template-columns:
      minmax(var(--k-size-col-md), max-content)
      repeat(var(--k-matrix-count), minmax(max-content, 1fr));
    align-content: start;
    min-inline-size: 0;
    max-block-size: 100%;
    overflow: auto;
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-matrix__head,
  .k-matrix__body,
  .k-matrix__row {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
  }

  /* The row of column headers sticks to the top, above the column of row headers. */
  .k-matrix__head {
    position: sticky;
    inset-block-start: 0;
    z-index: calc(var(--k-z-sticky) + 1);
  }

  .k-matrix__row {
    min-block-size: var(--k-row-height);
  }

  .k-matrix__head .k-matrix__row {
    min-block-size: var(--k-header-height);
  }

  .k-matrix__corner,
  .k-matrix__header,
  .k-matrix__cell {
    display: flex;
    gap: var(--k-space-1);
    align-items: center;
    /* A column is as wide as its widest cell (a state and its icon are never clipped), and
     * at least `--k-size-col-sm`; the matrix scrolls when they no longer fit. */
    min-inline-size: var(--k-size-col-sm);
    padding-inline: var(--k-cell-padding-inline);
    white-space: nowrap;
    border-block-end: var(--k-border-width) solid var(--k-border);
    border-inline-end: var(--k-border-width) solid var(--k-border);
  }

  .k-matrix__corner,
  .k-matrix__header {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
  }

  .k-matrix__header.k-mono {
    font-size: var(--k-text-mono);
  }

  /* The corner and the row headers stick to the inline start. */
  .k-matrix__corner,
  .k-matrix__row-header {
    position: sticky;
    inset-inline-start: 0;
    z-index: var(--k-z-sticky);
    border-inline-end-color: var(--k-border-strong);
  }

  .k-matrix__corner {
    color: var(--k-text);
  }

  .k-matrix__head .k-matrix__header,
  .k-matrix__corner {
    border-block-end-color: var(--k-border-strong);
  }

  .k-matrix__text {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Aligned on the start, as the headers. */
  .k-matrix__cell {
    font-variant-numeric: tabular-nums;
  }

  .k-matrix__corner:focus-visible,
  .k-matrix__header:focus-visible,
  .k-matrix__cell:focus-visible {
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  /* The crosshair: its cells lightly, its headers more, in the text colour. */
  .k-matrix__cell[data-cross] {
    background-color: var(--k-hover);
  }

  .k-matrix__header[data-cross] {
    color: var(--k-text);
    background-color: var(--k-accent-soft);
  }

  /* The arrowheads of direction: small accent triangles, 8 px wide and 6 px deep (the
   * width of `--k-space-2`, the depth of `--k-space-1` + `--k-space-0-5`), centred on a
   * border. One pointing along the row (to the inline end) on the end border of the
   * source row header and on the start border of the shown cell; one pointing down on
   * the bottom border of the destination column header and on the top border of the
   * shown cell. Half of each lies over the neighbour, painted after it (positioned). */
  .k-matrix__head .k-matrix__header[data-flow],
  .k-matrix__cell[data-flow] {
    position: relative;
  }

  .k-matrix [data-flow]::before,
  .k-matrix [data-flow]::after {
    position: absolute;
    pointer-events: none;
    background-color: var(--k-accent);
  }

  /* Along the row: on the end border of the source, on the start border of the cell. */
  .k-matrix__row-header[data-flow]::after,
  .k-matrix__cell[data-flow]::before {
    content: "";
    inset-block-start: calc(50% - var(--k-space-1));
    inline-size: calc(var(--k-space-1) + var(--k-space-0-5));
    block-size: var(--k-space-2);
    clip-path: polygon(0 0, 100% 50%, 0 100%);
  }

  .k-matrix__row-header[data-flow]::after {
    inset-inline-end: calc((var(--k-space-1) + var(--k-space-0-5)) / -2);
  }

  .k-matrix__cell[data-flow]::before {
    inset-inline-start: calc((var(--k-space-1) + var(--k-space-0-5)) / -2);
  }

  .k-matrix__row-header[data-flow]:dir(rtl)::after,
  .k-matrix__cell[data-flow]:dir(rtl)::before {
    clip-path: polygon(100% 0, 0 50%, 100% 100%);
  }

  /* Down the column: on the bottom border of the destination, on the top border of the cell. */
  .k-matrix__head .k-matrix__header[data-flow]::after,
  .k-matrix__cell[data-flow]::after {
    content: "";
    inset-inline-start: calc(50% - var(--k-space-1));
    inline-size: var(--k-space-2);
    block-size: calc(var(--k-space-1) + var(--k-space-0-5));
    clip-path: polygon(0 0, 100% 0, 50% 100%);
  }

  .k-matrix__head .k-matrix__header[data-flow]::after {
    inset-block-end: calc((var(--k-space-1) + var(--k-space-0-5)) / -2);
  }

  .k-matrix__cell[data-flow]::after {
    inset-block-start: calc((var(--k-space-1) + var(--k-space-0-5)) / -2);
  }

  /* The arrows of the corner, in words' colour. */
  .k-matrix__flow {
    flex: none;
  }

  .k-matrix__axis[data-axis="columns"] > .k-matrix__flow:dir(rtl) {
    scale: -1 1;
  }

  /* The corner of a directed matrix: the columns to the end on top, the rows below. */
  .k-matrix__axes {
    display: grid;
    flex: 1;
    gap: var(--k-space-0-5);
    padding-block: var(--k-space-0-5);
  }

  .k-matrix__axis {
    display: inline-flex;
    gap: var(--k-space-0-5);
    align-items: center;
  }

  .k-matrix__axis[data-axis="columns"] {
    justify-self: end;
  }

  .k-matrix__axis > .k-matrix__flow {
    color: var(--k-text-muted);
  }

  /* The active cell: the inner accent edge, and its headers named in the same way. */
  .k-matrix__cell[data-active],
  .k-matrix__header[data-active] {
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-matrix__cell[data-state="draft"] {
    background-color: var(--k-accent-soft);
  }

  .k-matrix__cell[data-state="review"] {
    background-color: var(--k-state-review-bg);
  }

  .k-matrix__mark {
    flex: none;
    color: var(--k-accent);
  }

  .k-matrix__cell[data-state="review"] .k-matrix__mark {
    color: var(--k-state-review-fg);
  }
}
</style>
