<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts" generic="R extends Record<string, unknown>">
// A row of the body of `DataGrid` (task 021-22o-b), its own component: when the grid renders
// (the roving focus moves, a press on a link of a cell focuses it, an editor opens, a page
// comes), a row renders again only when its props change (its row of the model, its key,
// its index) or what it read changed: the grid's shared state through its own key (the stop,
// the editor, the open error, the selection in this row), the host's states of its cells.
// Each state of the row and of its cells is asked of the host once per render (`cells`). The
// host's slots come forwarded by the grid: a host whose slots are compiled leaves the rows
// alone, one that renders its slots anew renders them again with the grid, as Vue does any
// child. The styles are the grid's (`DataGrid.vue`).
import { computed, onMounted, ref, type ComponentPublicInstance } from "vue";
import { t } from "../../i18n.ts";
import Checkbox from "../Checkbox/Checkbox.vue";
import Icon from "../Icon/Icon.vue";
import CellSelect from "../internal/CellSelect.vue";
import { ExtensionCell, type ExtensionSlotProps } from "../internal/extensionCell.ts";
import CellError from "./CellError.vue";
import { SELECT_COLUMN, type GridColumn } from "./grid.ts";
import type { CellView, DataGridRowContext } from "./row.ts";

const props = defineProps<{
  row: R;
  /** The key of the row (`rowKey` of the grid). */
  rowKey: string;
  /** Its index among the rows given. */
  index: number;
  c: DataGridRowContext<R>;
}>();
defineSlots<{
  cell(props: { row: R; column: GridColumn; value: unknown }): unknown;
  editor(props: {
    row: R;
    column: GridColumn;
    value: string;
    update: (value: string) => void;
    label: string;
    invalid: boolean;
    describedBy: string | undefined;
  }): unknown;
  extension(props: ExtensionSlotProps): unknown;
}>();

/** The text of the editor, the grid's. */
const draft = computed({
  get: () => props.c.draft.value,
  set: (value: string) => props.c.setDraft(value),
});
// The grid's shared state, through the key of this row: a focus that moves renders the row
// it leaves and the row it reaches, never the others (a computed that keeps its value does
// not render its reader again).
const stop = computed(() => props.c.stopIn(props.rowKey));
const editing = computed(() => props.c.editingIn(props.rowKey));
const error = computed(() => props.c.errorIn(props.rowKey));
const selected = computed(() =>
  props.c.selectable() ? props.c.isSelected(props.rowKey) : undefined,
);
const fresh = computed(() => props.c.isFresh(props.rowKey));
/** The state of the row, asked once. */
const state = computed(() => props.c.rowState(props.row));
const mark = computed(() => (state.value ? props.c.markOf(state.value) : undefined));
const cells = computed(() =>
  props.c.cellsOf(
    props.row,
    props.index,
    { stop: stop.value, editing: editing.value, error: error.value },
    state.value,
  ),
);

/** The element of the row, for the grid's virtualiser, and for the row itself. */
let el: HTMLElement | null = null;
function setRow(node: Element | ComponentPublicInstance | null): void {
  el = node instanceof HTMLElement ? node : null;
  props.c.measure(node);
}
// The cell an open error's layer stands against: the row's own, once in the document. A
// row that mounts with an error open (scrolled back into the window under the focus, an
// editor's error) renders once more after its mount, for its layer to find it.
let isMounted = false;
const placed = ref(false);
onMounted(() => {
  isMounted = true;
  if (cells.value.some((cell) => cell.errorOpen)) placed.value = true;
});
function anchorOf(cell: CellView): HTMLElement | null {
  if (!cell.errorOpen || !(isMounted || placed.value)) return null;
  return el?.querySelector<HTMLElement>(`[data-column="${CSS.escape(cell.column.id)}"]`) ?? null;
}
</script>

<template>
  <div
    :ref="setRow"
    role="row"
    class="k-data-grid__row"
    :data-index="props.index"
    :aria-rowindex="props.c.rowOffset() + props.index + 2"
    :aria-selected="selected"
    :data-row-state="state"
    :data-fresh="fresh ? 'true' : undefined"
    @animationend="props.c.onAnimationend(props.rowKey, $event)"
  >
    <div
      v-if="props.c.checkboxes.value"
      role="gridcell"
      class="k-data-grid__cell k-data-grid__select"
      :data-row="props.rowKey"
      :data-column="SELECT_COLUMN"
      aria-colindex="1"
      :tabindex="stop === SELECT_COLUMN ? 0 : -1"
      @click.capture="props.c.noteShift"
    >
      <Checkbox
        :label="`${t('selectRow')} ${props.rowKey}`"
        hide-label
        size="sm"
        :model-value="!!selected"
        @update:model-value="props.c.onBox(props.rowKey)"
      />
    </div>
    <div
      v-for="cell in cells"
      :key="cell.column.id"
      role="gridcell"
      class="k-data-grid__cell"
      :class="{ 'k-mono': cell.column.mono }"
      :data-row="props.rowKey"
      :data-column="cell.column.id"
      :data-align="cell.column.align"
      :data-priority="props.c.priority(cell.column)"
      :data-width="props.c.widthOf(cell.column)"
      :style="props.c.widthStyle(cell.column)"
      :data-wrap="cell.column.wrap ? 'true' : undefined"
      :data-label="
        props.c.stack() && cell.c > 0 && !cell.column.actions ? cell.column.label : undefined
      "
      :data-actions="cell.column.actions ? 'true' : undefined"
      :data-state="cell.state"
      :data-editing="cell.editing ? 'true' : undefined"
      :aria-colindex="props.c.colIndex(cell.column)"
      :aria-readonly="cell.readonly ? 'true' : undefined"
      :aria-busy="cell.state === 'pending' ? 'true' : undefined"
      :aria-invalid="cell.message ? 'true' : undefined"
      :aria-describedby="cell.describedBy"
      :data-error-open="cell.errorOpen ? 'true' : undefined"
      :tabindex="cell.tabindex"
      @pointerenter="props.c.onCellEnter(props.row, cell.column, $event)"
      @pointerleave="props.c.onCellLeave(props.index, cell.c, $event)"
    >
      <div v-if="cell.editing && props.c.has('editor')" class="k-data-grid__editor">
        <slot
          name="editor"
          :row="props.row"
          :column="cell.column"
          :value="draft"
          :update="(value: string) => (draft = value)"
          :label="props.c.editorLabel(props.row, cell.column)"
          :invalid="!!cell.message"
          :described-by="cell.errorId"
        />
      </div>
      <div
        v-else-if="cell.editing"
        class="k-field-frame k-data-grid__editor"
        data-size="md"
        :data-invalid="cell.message ? 'true' : undefined"
      >
        <CellSelect
          v-if="props.c.editorOptions.value"
          :options="props.c.editorOptions.value"
          :value="draft"
          :label="props.c.editorLabel(props.row, cell.column)"
          :mono="cell.column.mono"
          :invalid="!!cell.message"
          :described-by="cell.errorId"
          @update="(value: string) => (draft = value)"
          @done="props.c.choose"
        />
        <input
          v-else
          v-model="draft"
          class="k-field-frame__input"
          :class="{ 'k-mono': cell.column.mono }"
          type="text"
          spellcheck="false"
          :aria-label="props.c.editorLabel(props.row, cell.column)"
          :aria-invalid="cell.message ? 'true' : undefined"
          :aria-describedby="cell.errorId"
        />
      </div>
      <template v-else>
        <template v-if="cell.c === 0 && mark">
          <Icon :name="mark.icon" class="k-data-grid__row-mark" />
          <span class="k-visually-hidden">({{ mark.word }})</span>
        </template>
        <span class="k-data-grid__value">
          <ExtensionCell
            v-if="cell.column.element"
            :tag="cell.column.element"
            :row="props.row"
            :column="cell.column"
            :extra="cell.extra"
            ><template v-if="props.c.has('extension')" #default="given"
              ><slot name="extension" v-bind="given" /></template
          ></ExtensionCell>
          <slot
            v-else
            name="cell"
            :row="props.row"
            :column="cell.column"
            :value="props.row[cell.column.id]"
          />
        </span>
        <Icon
          v-if="cell.state === 'pending'"
          name="loader-circle"
          class="k-spinner k-data-grid__mark"
        />
        <Icon v-else-if="cell.state === 'error'" name="circle-x" class="k-data-grid__mark" />
        <Icon v-else-if="cell.state === 'review'" name="eye" class="k-data-grid__mark" />
        <span v-if="cell.word" class="k-visually-hidden">({{ cell.word }})</span>
      </template>
      <CellError
        v-if="cell.message"
        :id="cell.errorId!"
        :error="cell.message.error"
        :code="cell.message.code"
        :anchor="anchorOf(cell)"
        @leave="props.c.errorLeave()"
        @dismiss="props.c.errorDismiss()"
        @back="props.c.errorBack(props.index, cell.c)"
        @blur="props.c.errorBlur()"
      />
    </div>
  </div>
</template>
