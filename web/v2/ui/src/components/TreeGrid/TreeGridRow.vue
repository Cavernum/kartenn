<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A row of a node of `TreeGrid` (task 021-22e-b), its own component: as the tree scrolls,
// the tree renders its list of rows, and a row renders again only when its props change
// (the row of the model it shows, its index: a recycled row) or what it read changed (the
// focus, the editor, the host's states). The roving stop and the selection it reads through
// its own key, in computeds that keep their value (task 021-22e-c): a step of the focus, a
// selection that moves render the rows whose answer changes, never the others. Each state
// of the row and of its cells is asked of the host once per render (`state`, `cells`), not
// once per attribute. The styles are the tree's (`TreeGrid.vue`).
import { computed, onMounted, onUpdated, ref, type FunctionalComponent, type Slot } from "vue";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import CellError from "../DataGrid/CellError.vue";
import type { GridColumn } from "../DataGrid/grid.ts";
import Icon from "../Icon/Icon.vue";
import IconButton from "../IconButton/IconButton.vue";
import CellSelect from "../internal/CellSelect.vue";
import { ExtensionCell } from "../internal/extensionCell.ts";
import NoValue from "../NoValue/NoValue.vue";
import type { NodeRow, TreeGridRowContext } from "./row.ts";
import { hasChildren } from "./tree.ts";

const props = defineProps<{
  row: NodeRow;
  index: number;
  c: TreeGridRowContext;
  /** Changes at each render of the tree while the host gives the slot `extension`. */
  tick?: number | undefined;
}>();

/** The host's slot, rendered in the cell of an extension. */
const SlotOut: FunctionalComponent<{ render: Slot; given: object }> = (p) => p.render(p.given);

const node = computed(() => props.row.node);
/** The text of the editor, the tree's. */
const draft = computed({
  get: () => props.c.draft.value,
  set: (value: string) => props.c.setDraft(value),
});
/** The states of the row, each asked once. */
const state = computed(() => props.c.nodeState(props.row.node));
const move = computed(() => props.c.moveOf(props.row.node));
const refused = computed(() => props.c.refusal(props.row.node));
const words = computed(() => props.c.rowWords(props.row.node));
const addLabel = computed(() => props.c.addLabel(props.row.node));
// The tree's shared state, through the key of this row.
const stop = computed(() => props.c.stopIn(props.row.key));
const selected = computed(() =>
  props.c.selected.value === undefined ? undefined : props.row.key === props.c.selected.value,
);

/** What a cell shows, asked once: its state, error, editor, description, tab stop. */
function cellView(column: GridColumn) {
  const { c, row, index } = props;
  const editing = c.isEditing(row.key, column.id);
  const message = c.messageOf(row.node, column);
  const cellState = column.id === "" ? undefined : c.stateOf(row.node, column);
  return {
    column,
    editing,
    message,
    state: cellState,
    word: c.stateWord(cellState),
    readonly: c.editable() && !c.canEdit(row.node, column),
    describedBy: message ? c.errorId(index, column.id) : c.reasonOf(row.node, column),
    errorId: message ? c.errorId(index, column.id) : undefined,
    anchor: message ? c.errorAnchor(row.node, column) : null,
    tabindex: editing ? undefined : stop.value === column.id ? 0 : -1,
  };
}
const head = computed(() => cellView(props.c.treeColumn.value));
const cells = computed(() => props.c.shown.value.map((column) => cellView(column)));

// Rendered alone (a state of its own changed), its widgets leave the tab order as the
// tree's do at each of its renders.
const el = ref<HTMLElement | null>(null);
onMounted(() => props.c.tame(el.value));
onUpdated(() => props.c.tame(el.value));
</script>

<template>
  <div
    ref="el"
    role="row"
    class="k-tree-grid__row"
    :data-index="props.index"
    :data-node="props.row.key"
    :aria-level="props.row.level"
    :aria-setsize="props.row.setSize"
    :aria-posinset="props.row.posInSet"
    :aria-rowindex="props.index + 2"
    :aria-expanded="hasChildren(node) ? props.c.isExpanded(props.row.key) : undefined"
    :aria-selected="selected"
    :aria-busy="move === 'pending' || state === 'loading-children' ? 'true' : undefined"
    :data-state="state"
    :data-move="move"
    :data-drop="props.c.dropOf(node)"
    :draggable="props.c.moves.movable(props.row.key) ? 'true' : undefined"
    @click="props.c.onRowClick(props.row, $event)"
    @dragstart="props.c.moves.onDragStart(props.row.key, $event)"
    @dragover="props.c.moves.onDragOver(props.row.key, $event)"
    @dragenter="props.c.moves.onDragOver(props.row.key, $event)"
    @drop="props.c.moves.onDrop(props.row.key, $event)"
    @dragend="props.c.moves.onDragEnd()"
  >
    <div
      role="gridcell"
      class="k-tree-grid__cell k-tree-grid__node"
      :data-row="props.row.key"
      data-column=""
      aria-colindex="1"
      :data-editing="head.editing ? 'true' : undefined"
      :aria-readonly="head.readonly ? 'true' : undefined"
      :aria-invalid="head.message ? 'true' : undefined"
      :aria-describedby="head.describedBy"
      :tabindex="head.tabindex"
      :style="{ '--k-tree-depth': String(props.row.level - 1) }"
      @pointerenter="props.c.onCellEnter(node, head.column, $event)"
      @pointerover="props.c.onCellOver($event)"
      @pointerleave="props.c.onCellLeave(props.index, '', $event)"
    >
      <span class="k-tree-grid__twisty" aria-hidden="true">
        <Icon
          v-if="hasChildren(node)"
          name="chevron-right"
          class="k-chevron"
          :data-open="props.c.isExpanded(props.row.key) ? 'true' : undefined"
        />
      </span>
      <Icon v-if="node.icon" :name="node.icon" class="k-tree-grid__icon" />
      <div
        v-if="head.editing"
        class="k-field-frame k-tree-grid__editor"
        data-size="md"
        :data-invalid="head.message ? 'true' : undefined"
      >
        <CellSelect
          v-if="props.c.editorOptions.value"
          :options="props.c.editorOptions.value"
          :value="draft"
          :label="props.c.editorLabel(node, head.column)"
          :invalid="!!head.message"
          :described-by="head.errorId"
          @update="(value: string) => (draft = value)"
          @done="props.c.choose"
        />
        <input
          v-else
          v-model="draft"
          class="k-field-frame__input"
          type="text"
          spellcheck="false"
          :aria-label="props.c.editorLabel(node, head.column)"
          :aria-invalid="head.message ? 'true' : undefined"
          :aria-describedby="head.errorId"
        />
      </div>
      <template v-else>
        <span class="k-tree-grid__label">{{ node.label }}</span>
        <code v-if="node.objectKey" class="k-tree-grid__key">{{ node.objectKey }}</code>
      </template>
      <Icon v-if="move === 'pending'" name="loader-circle" class="k-spinner k-tree-grid__mark" />
      <Icon v-else-if="refused" name="circle-x" class="k-tree-grid__mark k-tree-grid__mark--ko" />
      <Icon v-else-if="move === 'cut'" name="scissors" class="k-tree-grid__mark" />
      <Icon v-if="state === 'draft'" name="draft" class="k-tree-grid__mark" />
      <Icon v-else-if="state === 'moved'" name="move" class="k-tree-grid__mark" />
      <Icon v-else-if="state === 'preview'" name="circle-dashed" class="k-tree-grid__mark" />
      <span v-if="words" class="k-visually-hidden">({{ words }})</span>
      <span v-if="refused" class="k-tree-grid__refusal">
        <span>{{ refused.reason }}</span>
        <CodeBadge v-if="refused.code" :code="refused.code" />
      </span>
      <IconButton
        v-if="addLabel"
        icon="plus"
        size="sm"
        tabindex="-1"
        class="k-tree-grid__add"
        :label="addLabel"
        v-bind="props.c.addBlocked(node) ?? {}"
        @click.stop="props.c.add(props.row.key)"
      />
      <CellError
        v-if="head.message"
        :id="head.errorId!"
        :error="head.message.error"
        :code="head.message.code"
        :anchor="head.anchor"
        @leave="props.c.errorLeave()"
        @dismiss="props.c.errorDismiss()"
        @back="props.c.errorBack({ row: props.row.key, column: '' })"
        @blur="props.c.errorBlur()"
      />
    </div>
    <div
      v-for="cell in cells"
      :key="cell.column.id"
      role="gridcell"
      class="k-tree-grid__cell"
      :class="{ 'k-mono': cell.column.mono }"
      :data-row="props.row.key"
      :data-column="cell.column.id"
      :data-align="cell.column.align"
      :data-priority="props.c.priority(cell.column)"
      :data-width="cell.column.width ?? 'md'"
      :data-element="cell.column.element ? 'true' : undefined"
      :data-state="cell.state"
      :data-editing="cell.editing ? 'true' : undefined"
      :aria-colindex="props.c.colIndex(cell.column)"
      :aria-readonly="cell.readonly ? 'true' : undefined"
      :aria-busy="cell.state === 'pending' ? 'true' : undefined"
      :aria-invalid="cell.message ? 'true' : undefined"
      :aria-describedby="cell.describedBy"
      :tabindex="cell.tabindex"
      @pointerenter="props.c.onCellEnter(node, cell.column, $event)"
      @pointerleave="props.c.onCellLeave(props.index, cell.column.id, $event)"
    >
      <div
        v-if="cell.editing"
        class="k-field-frame k-tree-grid__editor"
        data-size="md"
        :data-invalid="cell.message ? 'true' : undefined"
      >
        <CellSelect
          v-if="props.c.editorOptions.value"
          :options="props.c.editorOptions.value"
          :value="draft"
          :label="props.c.editorLabel(node, cell.column)"
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
          :aria-label="props.c.editorLabel(node, cell.column)"
          :aria-invalid="cell.message ? 'true' : undefined"
          :aria-describedby="cell.errorId"
        />
      </div>
      <template v-else>
        <span class="k-tree-grid__value"
          ><ExtensionCell
            v-if="cell.column.element"
            :key="props.row.key"
            :tag="cell.column.element"
            :row="node"
            :column="cell.column"
            :extra="props.c.extraOf(node, cell.column)"
            ><template v-if="props.c.extension()" #default="given"
              ><SlotOut :render="props.c.extension()!" :given="given" /></template></ExtensionCell
          ><NoValue v-else-if="props.c.cellOf(node, cell.column) === null" /><template v-else>{{
            props.c.cellOf(node, cell.column)
          }}</template></span
        >
        <Icon
          v-if="cell.state === 'pending'"
          name="loader-circle"
          class="k-spinner k-tree-grid__mark"
        />
        <Icon
          v-else-if="cell.state === 'error'"
          name="circle-x"
          class="k-tree-grid__mark k-tree-grid__mark--ko"
        />
        <Icon v-else-if="cell.state === 'review'" name="eye" class="k-tree-grid__mark" />
        <Icon v-else-if="cell.state === 'draft'" name="draft" class="k-tree-grid__mark" />
        <span v-if="cell.word" class="k-visually-hidden">({{ cell.word }})</span>
      </template>
      <CellError
        v-if="cell.message"
        :id="cell.errorId!"
        :error="cell.message.error"
        :code="cell.message.code"
        :anchor="cell.anchor"
        @leave="props.c.errorLeave()"
        @dismiss="props.c.errorDismiss()"
        @back="props.c.errorBack({ row: props.row.key, column: cell.column.id })"
        @blur="props.c.errorBlur()"
      />
    </div>
  </div>
</template>
