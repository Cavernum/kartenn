<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A tree with columns (APG Treegrid; design « Inventaire », line 21). Visual version (task
// 12): the rows of the model, depth first, with `aria-level`, `aria-setsize`,
// `aria-posinset`, `aria-expanded`; the states the host decides (draft, children loading,
// drop target accepted or refused, move pending or refused, with reason and code). The
// tree never moves a node itself: it renders the parent of its model, checked at each
// render outside production (REQ-020-32). Direct addition (024/M10): when the host names
// the « + » of a node (`addLabel`), the node cell ends with a button that emits `add` with
// the node id; it shows on the selected row, on hover and on focus within the row, and is
// reached from the keyboard with the arrows of the complete version (the shortcut `c` of
// the screen creates meanwhile). Virtualisation, keyboard, editing and moves come with
// the complete version (task 26). Adaptation to the width (design « Adaptation à la
// largeur »), as `DataGrid`: the root is a container, the tree column sticks, the columns
// of priority 2 and 3 hide under a container `md` and `lg`, a column chooser shows them
// again; with no hover (touch), the « + » shows on the selected row only.
// Keyboard (relecture du 2026-10-26, the part of APG Treegrid the tree column needs): one
// tab stop on the node cell of a row; Up and Down, Home and End move between rows; Right
// opens a closed node, or goes to the first child of an open one; Left closes an open node,
// or goes to the parent. Enter or F2 moves into the « + » of the row, Escape or F2 back.
// The open nodes are the model `expanded`. Columns, editing and moves remain to come.
import { computed, nextTick, ref, useAttrs, watch } from "vue";
import { t } from "../../i18n.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import {
  columnTemplates,
  priorityOf,
  renderedColumns,
  type ColumnVisibility,
  type GridColumn,
} from "../DataGrid/grid.ts";
import Icon from "../Icon/Icon.vue";
import IconButton from "../IconButton/IconButton.vue";
import ColumnChooser from "../internal/ColumnChooser.vue";

defineOptions({ inheritAttrs: false });
import { flatten, misplaced, type TreeNode, type TreeNodeState, type TreeRow } from "./tree.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    /** The header of the tree column. */
    treeLabel: string;
    nodes: TreeNode[];
    columns?: GridColumn[];
    selected?: string | undefined;
    nodeState?: ((node: TreeNode) => TreeNodeState | undefined) | undefined;
    /** The name of the « + » of a node (« Ajouter dans 192.0.2.0/24 »); none, no button. */
    addLabel?: ((node: TreeNode) => string | undefined) | undefined;
  }>(),
  {
    columns: () => [],
    selected: undefined,
    nodeState: undefined,
    addLabel: undefined,
  },
);
const emit = defineEmits<{ add: [id: string] }>();
/** The ids of the open nodes. */
const expanded = defineModel<string[]>("expanded", { default: () => [] });

const rows = computed(() => {
  const list = flatten(props.nodes, new Set(expanded.value));
  if (import.meta.env.MODE !== "production") {
    const wrong = misplaced(list);
    if (wrong.length) throw new Error(`TreeGrid: rows under a parent not theirs: ${wrong}`);
  }
  return list;
});
/** The user's choice of columns (column chooser). */
const visibility = defineModel<ColumnVisibility>("columnVisibility", { default: () => ({}) });
const attrs = useAttrs();
const rootAttrs = computed(() => ({ class: attrs.class, style: attrs.style }));
const gridAttrs = computed(() =>
  Object.fromEntries(Object.entries(attrs).filter(([key]) => key !== "class" && key !== "style")),
);
const grid = ref<HTMLElement | null>(null);
/** The tree column, the key, then the columns of the host. */
const all = computed<GridColumn[]>(() => [
  { id: "", label: props.treeLabel, width: "fill" },
  ...props.columns,
]);
/** The columns of the host rendered (the user's hidden ones left out). */
const shown = computed(() => renderedColumns(all.value, visibility.value).slice(1));
const template = computed(() => columnTemplates(all.value, visibility.value));
const priority = (column: GridColumn) => {
  const p = priorityOf(all.value, column, visibility.value);
  return p > 1 ? p : undefined;
};
const colIndex = (column: GridColumn) => all.value.indexOf(column) + 1;
const depth = (row: TreeRow) => ({ "--k-tree-depth": String(row.level - 1) });
const hasChildren = (node: TreeNode) => Boolean(node.children?.length || node.hasChildren);
const isExpanded = (node: TreeNode) => expanded.value.includes(node.id);

/** The row whose node cell holds the roving focus: the selected one, else the first. */
const focusRow = ref<string | undefined>(undefined);
const stop = computed(() => {
  const ids = rows.value.map((row) => row.node.id);
  if (focusRow.value && ids.includes(focusRow.value)) return focusRow.value;
  return props.selected && ids.includes(props.selected) ? props.selected : ids[0];
});
// A node closed above the focus: the stop goes back to a row still shown.
watch(rows, () => {
  if (focusRow.value && !rows.value.some((row) => row.node.id === focusRow.value))
    focusRow.value = undefined;
});

async function moveTo(id: string): Promise<void> {
  focusRow.value = id;
  await nextTick();
  grid.value
    ?.querySelector<HTMLElement>(`[role="row"][data-node="${CSS.escape(id)}"] > .k-tree-grid__node`)
    ?.focus();
}

function onKeydown(event: KeyboardEvent): void {
  const target = event.target as HTMLElement;
  const cell = target.closest<HTMLElement>(".k-tree-grid__node");
  if (!cell) return;
  // The « + » keeps its keys; Escape or F2 gives the focus back to its cell.
  if (cell !== target) {
    if (event.key === "Escape" || event.key === "F2") {
      event.preventDefault();
      cell.focus();
    }
    return;
  }
  const list = rows.value;
  const at = list.findIndex((row) => row.node.id === cell.parentElement?.dataset.node);
  const row = list[at];
  if (!row || event.altKey) return;
  const rtl = getComputedStyle(cell).direction === "rtl";
  const open = rtl ? "ArrowLeft" : "ArrowRight";
  const close = rtl ? "ArrowRight" : "ArrowLeft";
  let next: string | undefined;
  if (event.key === "ArrowDown") next = list[Math.min(list.length - 1, at + 1)]?.node.id;
  else if (event.key === "ArrowUp") next = list[Math.max(0, at - 1)]?.node.id;
  else if (event.key === "Home") next = list[0]?.node.id;
  else if (event.key === "End") next = list.at(-1)?.node.id;
  else if (event.key === open && hasChildren(row.node)) {
    if (!isExpanded(row.node)) expanded.value = [...expanded.value, row.node.id];
    else if (list[at + 1]?.parent === row.node.id) next = list[at + 1]!.node.id;
  } else if (event.key === close) {
    if (hasChildren(row.node) && isExpanded(row.node))
      expanded.value = expanded.value.filter((id) => id !== row.node.id);
    else if (row.parent) next = row.parent;
  } else if (event.key === "Enter" || event.key === "F2") {
    cell.querySelector<HTMLElement>(".k-tree-grid__add")?.focus();
  } else return;
  event.preventDefault();
  if (next) void moveTo(next);
}

function onFocusin(event: FocusEvent): void {
  const id = (event.target as HTMLElement).closest<HTMLElement>("[data-node]")?.dataset.node;
  if (id && id !== focusRow.value) focusRow.value = id;
}

function stateOf(node: TreeNode): string | undefined {
  const state = props.nodeState?.(node);
  if (state === undefined || typeof state === "string") return state;
  return "drop" in state ? "drop-refused" : "move-refused";
}
function refusal(node: TreeNode): { reason: string; code?: string } | undefined {
  const state = props.nodeState?.(node);
  if (typeof state !== "object") return undefined;
  return "drop" in state ? state : state.refused;
}
const WORDS = {
  draft: "inDraft",
  "loading-children": "childrenLoading",
  "drop-ok": "dropOk",
  "drop-refused": "dropRefused",
  "move-pending": "movePending",
  "move-refused": "moveRefused",
} as const;
const word = (node: TreeNode) => {
  const state = stateOf(node);
  return state ? t(WORDS[state as keyof typeof WORDS]) : undefined;
};
</script>

<template>
  <div class="k-tree-grid-frame" v-bind="rootAttrs">
    <ColumnChooser v-model="visibility" :columns="all" :grid="grid" />
    <div
      ref="grid"
      v-bind="gridAttrs"
      class="k-tree-grid"
      role="treegrid"
      :aria-label="props.label"
      :aria-colcount="props.columns.length + 1"
      :style="template"
      @keydown="onKeydown"
      @focusin="onFocusin"
    >
      <div role="rowgroup" class="k-tree-grid__head">
        <div role="row" class="k-tree-grid__row">
          <div role="columnheader" class="k-tree-grid__header" aria-colindex="1">
            {{ props.treeLabel }}
          </div>
          <div
            v-for="column in shown"
            :key="column.id"
            role="columnheader"
            class="k-tree-grid__header"
            :data-column="column.id"
            :data-align="column.align"
            :data-priority="priority(column)"
            :aria-colindex="colIndex(column)"
          >
            {{ column.label }}
          </div>
        </div>
      </div>
      <div role="rowgroup">
        <template v-for="row in rows" :key="row.node.id">
          <div
            role="row"
            class="k-tree-grid__row"
            :data-node="row.node.id"
            :aria-level="row.level"
            :aria-setsize="row.setSize"
            :aria-posinset="row.posInSet"
            :aria-expanded="hasChildren(row.node) ? isExpanded(row.node) : undefined"
            :aria-selected="
              props.selected === undefined ? undefined : row.node.id === props.selected
            "
            :aria-busy="
              stateOf(row.node) === 'move-pending' || stateOf(row.node) === 'loading-children'
                ? 'true'
                : undefined
            "
            :data-state="stateOf(row.node)"
            :data-drop="
              stateOf(row.node) === 'drop-ok'
                ? 'ok'
                : stateOf(row.node) === 'drop-refused'
                  ? 'refused'
                  : undefined
            "
          >
            <div
              role="gridcell"
              class="k-tree-grid__cell k-tree-grid__node"
              aria-colindex="1"
              :tabindex="row.node.id === stop ? 0 : -1"
              :style="depth(row)"
            >
              <Icon
                v-if="hasChildren(row.node)"
                name="chevron-right"
                class="k-tree-grid__twisty k-chevron"
                :data-open="isExpanded(row.node) ? 'true' : undefined"
              />
              <span v-else class="k-tree-grid__twisty" aria-hidden="true"></span>
              <Icon v-if="row.node.icon" :name="row.node.icon" class="k-tree-grid__icon" />
              <span class="k-tree-grid__label">{{ row.node.label }}</span>
              <code v-if="row.node.objectKey" class="k-tree-grid__key">{{
                row.node.objectKey
              }}</code>
              <Icon
                v-if="stateOf(row.node) === 'move-pending'"
                name="loader-circle"
                class="k-spinner k-tree-grid__mark"
              />
              <Icon
                v-else-if="
                  stateOf(row.node) === 'move-refused' || stateOf(row.node) === 'drop-refused'
                "
                name="circle-x"
                class="k-tree-grid__mark k-tree-grid__mark--ko"
              />
              <span v-if="word(row.node)" class="k-visually-hidden">({{ word(row.node) }})</span>
              <span v-if="refusal(row.node)" class="k-tree-grid__refusal" role="status">
                <span>{{ refusal(row.node)!.reason }}</span>
                <CodeBadge v-if="refusal(row.node)!.code" :code="refusal(row.node)!.code!" />
              </span>
              <IconButton
                v-if="props.addLabel?.(row.node)"
                icon="plus"
                size="sm"
                tabindex="-1"
                class="k-tree-grid__add"
                :label="props.addLabel(row.node)!"
                @click.stop="emit('add', row.node.id)"
              />
            </div>
            <div
              v-for="column in shown"
              :key="column.id"
              role="gridcell"
              class="k-tree-grid__cell"
              :class="{ 'k-mono': column.mono }"
              :data-align="column.align"
              :data-priority="priority(column)"
              :aria-colindex="colIndex(column)"
              tabindex="-1"
            >
              {{ row.node.cells?.[column.id] ?? "" }}
            </div>
          </div>
          <div
            v-if="stateOf(row.node) === 'loading-children' && isExpanded(row.node)"
            role="row"
            class="k-tree-grid__row k-tree-grid__placeholder"
            :aria-level="row.level + 1"
          >
            <div
              role="gridcell"
              class="k-tree-grid__cell k-tree-grid__node"
              :style="{ '--k-tree-depth': String(row.level) }"
            >
              <span class="k-tree-grid__twisty" aria-hidden="true"></span>
              <span class="k-tree-grid__bar" aria-hidden="true"></span>
              <span class="k-visually-hidden">{{ t("childrenLoading") }}</span>
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style>
@layer components {
  /* The root: a container, the chooser above the tree, the tree in the room left. */
  .k-tree-grid-frame {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
    min-block-size: 0;
    container-type: inline-size;
  }

  .k-tree-grid-frame > .k-column-chooser {
    padding-block-end: var(--k-space-1);
  }

  .k-tree-grid-frame > .k-tree-grid {
    grid-row: 2;
  }

  .k-tree-grid {
    --k-grid-columns: var(--k-size-col-md);
    --k-grid-columns-below-lg: var(--k-grid-columns);
    --k-grid-columns-below-md: var(--k-grid-columns);

    overflow: auto;
    scrollbar-gutter: stable;
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-tree-grid__head {
    position: sticky;
    inset-block-start: 0;
    z-index: calc(var(--k-z-sticky) + 1);
    background-color: var(--k-surface-sunken);
  }

  /* The tree column sticks at the inline start; opaque rows lend it their background. */
  .k-tree-grid__head .k-tree-grid__row {
    background-color: inherit;
  }

  .k-tree-grid__row {
    background-color: var(--k-surface);
  }

  .k-tree-grid__row > :first-child {
    position: sticky;
    inset-inline-start: 0;
    z-index: var(--k-z-sticky);
    background-color: inherit;
  }

  .k-tree-grid__row[aria-selected="true"] > :first-child,
  .k-tree-grid__row[data-state="draft"] > :first-child {
    box-shadow: var(--k-shadow-inset-accent);
  }

  /* Columns of priority 3 (2) leave a container under `lg` (`md`). */
  @container (width < 960px) {
    .k-tree-grid .k-tree-grid__row {
      grid-template-columns: var(--k-grid-columns-below-lg);
    }

    .k-tree-grid [data-priority="3"] {
      display: none;
    }
  }

  @container (width < 640px) {
    .k-tree-grid .k-tree-grid__row {
      grid-template-columns: var(--k-grid-columns-below-md);
    }

    .k-tree-grid [data-priority="2"] {
      display: none;
    }
  }

  .k-tree-grid__row {
    display: grid;
    grid-template-columns: var(--k-grid-columns);
    min-block-size: var(--k-row-height);
    border-block-end: var(--k-border-width) solid var(--k-border);

    /* Hovered or selected: a colour fade, nothing moves (design « Mouvement »). */
    transition-property: background-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-tree-grid__head .k-tree-grid__row {
    min-block-size: var(--k-header-height);
  }

  .k-tree-grid__header {
    display: flex;
    align-items: center;
    padding-inline: var(--k-cell-padding-inline);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text-muted);
  }

  .k-tree-grid__cell {
    position: relative;
    display: flex;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
    padding-inline: var(--k-cell-padding-inline);
    white-space: nowrap;
  }

  .k-tree-grid__header[data-align="end"],
  .k-tree-grid__cell[data-align="end"] {
    justify-content: end;
    font-variant-numeric: tabular-nums;
  }

  /* The indent of a level: `--k-tree-depth`, set on the cell by CSSOM. */
  .k-tree-grid__node {
    padding-inline-start: calc(var(--k-space-1) + var(--k-tree-indent) * var(--k-tree-depth, 0));
  }

  .k-tree-grid__cell:focus-visible {
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-tree-grid__twisty {
    flex: none;
    inline-size: var(--k-icon-size);
    color: var(--k-text-muted);
  }

  .k-tree-grid__icon,
  .k-tree-grid__key {
    color: var(--k-text-muted);
  }

  .k-tree-grid__label {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .k-tree-grid__row:hover {
    background-color: var(--k-hover);
  }

  .k-tree-grid__row[aria-selected="true"] {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-tree-grid__row[data-state="draft"] {
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-tree-grid__row[data-drop="ok"] {
    background-color: var(--k-accent-soft);
    outline: var(--k-focus-width) solid var(--k-accent);
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-tree-grid__row[data-drop="refused"] {
    outline: var(--k-border-width) dashed var(--k-state-ko-fg);
    outline-offset: calc(var(--k-border-width) * -1);
  }

  .k-tree-grid__row[data-state="move-pending"] {
    color: var(--k-text-muted);
  }

  .k-tree-grid__mark {
    flex: none;
    color: var(--k-text-muted);
  }

  .k-tree-grid__mark--ko {
    color: var(--k-state-ko-fg);
  }

  .k-tree-grid__refusal {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-state-ko-fg);
  }

  /* The « + » of a node, at the end of its cell: shown where the user is. */
  .k-tree-grid__add {
    flex: none;
    margin-inline-start: auto;
    opacity: 0;
  }

  .k-tree-grid__row:focus-within .k-tree-grid__add,
  .k-tree-grid__row[aria-selected="true"] .k-tree-grid__add {
    opacity: 1;
  }

  /* On hover only where a pointer hovers: on touch, a tap selects the row, which shows
   * it; nothing is kept for a hover that never comes. */
  @media (hover: hover) {
    .k-tree-grid__row:hover .k-tree-grid__add {
      opacity: 1;
    }
  }

  .k-tree-grid__bar {
    inline-size: var(--k-size-col-md);
    block-size: var(--k-space-2);
    background-color: var(--k-surface-sunken);
    border-radius: var(--k-radius-sm);
  }
}
</style>
