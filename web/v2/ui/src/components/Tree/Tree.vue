<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A tree (APG Tree View; design « Inventaire », line 21, « Vue et Reka UI », « Contrat de
// déplacement »; REQ-020-19, 31, 32, 33): Reka's `TreeRoot` and `TreeItem` (roles, levels,
// open state, selection by Space and the pointer, Right and Left kept from the pattern),
// with the fixes of the prototype (spike 005):
// - the selection is an id (`selected`): Reka stores a copy of the node, never read; the
//   nodes given to Reka are raw (`markRaw`), out of its deep reactivity;
// - the keys of the pattern are handled before Reka's (a capture listener on the tree that
//   stops them): Up and Down, Home and End over the flattened index (a virtualised tree
//   loses no press), Right opens then goes to the first child, Left closes then goes to the
//   parent, `*` opens the siblings, Enter renames an editable node (else selects and
//   activates it), F2 renames, the search by typing takes printable characters only (Shift
//   for capitals; Shift, Control or an arrow never enter it);
// - virtualised beyond 200 rows as `TreeGrid` (not Reka's `TreeVirtualizer`, which unmounts
//   the focused row and keys its rows by index): the focused row and the renamed one stay
//   mounted;
// - each row is named by its label alone, from its content (no `aria-label`, which axe
//   would find shorter than the text shown): its key, detail, states and reasons are in
//   a part hidden from the name (`aria-hidden`) that is its description
//   (`aria-describedby`); the copy buttons of a reason are the pointer's (out of the tab
//   order).
// Children read page by page, moves (`parentMode`, `canMove`, `moveState`, an intent only;
// a node whose move is pending is neither dragged nor cut) and the display guard on the
// rows rendered, as `TreeGrid`; a notice (`TreeNode.notice`, a level capped) as there too,
// out of the set of its siblings, activated by Enter or a click. The tree consumes the
// printable keys (`data-k-typeahead`: the one-key shortcuts of the shell leave them to it).
import { TreeItem, TreeRoot } from "reka-ui";
import {
  computed,
  markRaw,
  nextTick,
  onMounted,
  onUpdated,
  ref,
  toRaw,
  useAttrs,
  useId,
  watch,
} from "vue";
import { t } from "../../i18n.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import type { CellRef, CellState, GridCommit } from "../DataGrid/grid.ts";
import Icon from "../Icon/Icon.vue";
import { useCellEditor, useCommitWatch } from "../internal/cellEditor.ts";
import { isRtl } from "../internal/gridKeys.ts";
import MoveDialog from "../internal/MoveDialog.vue";
import { useTreeStates } from "../internal/treeStates.ts";
import { useVirtualRows } from "../internal/virtualRows.ts";
import {
  closedSiblings,
  flatten,
  guardRendered,
  hasChildren,
  indexNodes,
  landingRow,
  searchRow,
  treeStep,
  typesAhead,
  type MoveCheck,
  type MoveIntent,
  type MoveState,
  type ParentMode,
  type TreeNode,
  type TreeNodeState,
  type TreeRow,
} from "../TreeGrid/tree.ts";

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    label: string;
    /** The nodes, plain data (a `shallowRef` of the host): the tree marks them raw. */
    nodes: TreeNode[];
    /** The first level has pages left (a row after it reads the next one). */
    more?: boolean;
    /** The next page of the first level is coming. */
    moreLoading?: boolean;
    nodeState?: ((node: TreeNode) => TreeNodeState | undefined) | undefined;
    /** The nodes are coming: skeleton rows, the tree busy (REQ-020-23). */
    loading?: boolean;
    loadingRows?: number;
    parentMode?: ParentMode;
    canMove?: ((node: TreeNode, target: TreeNode | null) => MoveCheck) | undefined;
    moveState?: ((node: TreeNode) => MoveState | undefined) | undefined;
    /** Which nodes the user may rename (Enter or F2): the tree never decides it. */
    editable?: ((node: TreeNode) => boolean) | undefined;
    /** The local check of a new label: its error, or nothing. */
    validate?: ((node: TreeNode, value: string) => string | undefined) | undefined;
    /** The state of the label of a node after a rename (pending, draft, review, error). */
    labelState?: ((node: TreeNode) => CellState | undefined) | undefined;
  }>(),
  {
    nodeState: undefined,
    loadingRows: 5,
    parentMode: "derived",
    canMove: undefined,
    moveState: undefined,
    editable: undefined,
    validate: undefined,
    labelState: undefined,
  },
);
const emit = defineEmits<{
  /** The next page of the children of a node (`null`: of the first level). */
  load: [parent: string | null];
  move: [intent: MoveIntent];
  /** A new label (`row`: the node id; `column`: `""`), checked locally. */
  commit: [commit: GridCommit];
  /** Enter on a node that is not renamed. */
  activate: [id: string];
}>();
const expanded = defineModel<string[]>("expanded", { default: () => [] });
const selected = defineModel<string | undefined>("selected", { default: undefined });
/** The node being renamed. */
const editing = defineModel<string | undefined>("editing", { default: undefined });

const attrs = useAttrs();
const rootAttrs = computed(() => ({ class: attrs.class, style: attrs.style }));
const treeAttrs = computed(() =>
  Object.fromEntries(Object.entries(attrs).filter(([key]) => key !== "class" && key !== "style")),
);
const scroller = ref<HTMLElement | null>(null);
const uid = useId();

// ——— Model and rows ———

const open = computed(() => new Set(expanded.value));
const index = computed(() => indexNodes(props.nodes));
const nodeOf = (id: string) => index.value.get(id)?.node;
const rows = computed(() => flatten(props.nodes, open.value, props.more));
const rowIndex = computed(() => new Map(rows.value.map((row, i) => [row.key, i])));
const isExpanded = (id: string) => open.value.has(id);

/** What Reka holds: raw nodes, and for a row that reads a page a node of its own. */
const sentinels = new Map<string, TreeNode>();
function rekaValue(row: TreeRow): TreeNode {
  if (row.kind !== "more") return markRaw(toRaw(row.node));
  let node = sentinels.get(row.key);
  if (!node) sentinels.set(row.key, (node = markRaw({ id: row.key, label: t("moreRows") })));
  return node;
}
const getKey = (node: TreeNode) => node.id;
const getChildren = (node: TreeNode) =>
  sentinels.get(node.id) ? undefined : (node.children ?? (hasChildren(node) ? [] : undefined));
/** The selection by id: what Reka emits is a copy, only its id is read. */
const NONE: TreeNode = markRaw({ id: "", label: "" });
const selectedNode = computed(() => {
  const node = selected.value ? nodeOf(selected.value) : undefined;
  return node ? markRaw(toRaw(node)) : NONE;
});
function onSelect(value: unknown): void {
  const id = (value as TreeNode | undefined)?.id;
  if (id !== undefined && !sentinels.has(id) && nodeOf(id) && !nodeOf(id)!.notice)
    selected.value = id;
}

// ——— Focus ———

/** The row of the tab stop: the one last focused, the selected one, the first. */
const focused = ref<string | undefined>(undefined);
const stop = computed(() => {
  const there = (key: string | undefined) => key !== undefined && rowIndex.value.has(key);
  if (there(focused.value)) return focused.value!;
  if (there(selected.value)) return selected.value!;
  return rows.value[0]?.key;
});
const itemOf = (key: string) =>
  scroller.value?.querySelector<HTMLElement>(`[data-node="${CSS.escape(key)}"]`) ?? null;
async function focusRow(key: string): Promise<void> {
  focused.value = key;
  await nextTick();
  itemOf(key)?.focus();
}
// The focused row leaves the rows shown (the host's new data): the focus to where it
// lands, never to the page. Read before the render, while the row is still there.
watch(
  rows,
  (now, before) => {
    const key = focused.value;
    if (!key || now.some((row) => row.key === key)) return;
    if (!itemOf(key)?.contains(document.activeElement)) return;
    const next = landingRow(key, before, now, (k) => index.value.get(k)?.parent);
    if (next) void focusRow(next);
  },
  { flush: "pre" },
);
function onFocusin(event: FocusEvent): void {
  const key = (event.target as HTMLElement).closest<HTMLElement>("[data-node]")?.dataset.node;
  if (key && key !== focused.value) focused.value = key;
}

// ——— Virtualisation ———

const virtual = useVirtualRows({
  count: () => rows.value.length,
  keyAt: (i) => rows.value[i]?.key ?? String(i),
  indexOf: (key) => rowIndex.value.get(key),
  scroller,
  kept: () => [stop.value, editing.value],
  off: () => !!props.loading,
});
/** The rows rendered and their gaps; the display guard runs on them (REQ-020-32). */
const parts = computed(() => {
  const list = virtual.parts.value;
  guardRendered(
    "Tree",
    rows.value,
    list.flatMap((part) => ("index" in part ? [part.index] : [])),
    (key) => index.value.get(key)?.parent,
  );
  return list.map((part) =>
    "index" in part ? { index: part.index, row: rows.value[part.index]! } : part,
  );
});
onMounted(() => virtual.readEstimate());
/** The copy buttons of the reasons stay out of the tab order: one tab stop, the row's. */
function tame(): void {
  for (const button of scroller.value?.querySelectorAll<HTMLElement>(".k-tree__detail button") ??
    [])
    button.tabIndex = -1;
}
onMounted(tame);
onUpdated(tame);

// ——— Pages of children ———

// The states shared with `TreeGrid` (`internal/treeStates.ts`): moves, regions, pages.
const {
  status,
  alerts,
  moves,
  declared,
  moveOf,
  refusal,
  dropOf,
  rowWords: stateWords,
  movingOpen,
  movingLabel,
  openMove,
  moveTargets,
  onMoveTo,
  pages,
  pageLoading,
} = useTreeStates(props, rows, index, scroller, {
  load: (parent) => emit("load", parent),
  move: (intent) => emit("move", intent),
});
defineExpose({ openMove });
// A node opened (by the keys, Reka's click or the host) with its children still to read
// asks for their first page.
watch(expanded, (now, before) => {
  for (const id of now) if (!before.includes(id) && !nodeOf(id)?.children) pages.request(id);
});

// ——— Renaming ———

const editingCell = computed<CellRef | undefined>({
  get: () => (editing.value === undefined ? undefined : { row: editing.value, column: "" }),
  set: (cell) => (editing.value = cell?.row),
});
const alert = alerts.say;
const sayError = (id: string, error: { error: string; code?: string }) =>
  `${nodeOf(id)?.label ?? id} : ${error.error}${error.code ? ` (${error.code})` : ""}`;
const commits = useCommitWatch(
  (cell) => {
    const node = nodeOf(cell.row);
    return node ? props.labelState?.(node) : undefined;
  },
  (cell) => nodeOf(cell.row)?.label,
  (cell, error) => void alert(sayError(cell.row, error)),
);
const editor = useCellEditor({
  editing: editingCell,
  textOf: (cell) => nodeOf(cell.row)?.label ?? "",
  driven: () => !!props.editable,
  check: (cell, value) => {
    const node = nodeOf(cell.row);
    return node ? props.validate?.(node, value) : undefined;
  },
  cellElement: (cell) => itemOf(cell.row),
  moveFocus: (cell) => void focusRow(cell.row),
  commit(commit) {
    emit("commit", commit);
    commits.follow(commit);
  },
  refuse: (cell, error) => void alert(`${sayError(cell.row, { error })} ${t("editEscape")}`),
  editorClass: "k-tree__editor",
});
const draft = editor.draft;
/** The error a label shows: its editor's (the host's check), else the host's. */
function labelError(node: TreeNode): { error: string; code?: string } | undefined {
  if (editing.value === node.id && editor.error.value) return { error: editor.error.value };
  const state = props.labelState?.(node);
  return typeof state === "object" ? state : undefined;
}
const LABEL_WORDS = { pending: "pending", draft: "inDraft", review: "inReview" } as const;
/** The states of a row in words: the host's, its move, its target state, its label's. */
function rowWords(node: TreeNode): string {
  const label = props.labelState?.(node);
  return stateWords(node, typeof label === "string" ? [t(LABEL_WORDS[label])] : []);
}
const describedBy = (row: TreeRow) => `k-tree-${uid}-${rowIndex.value.get(row.key)}`;

// ——— Keys ———

let search = "";
let searchTimer: ReturnType<typeof setTimeout> | undefined;
/** The pause after which a search by typing starts again (APG: about half a second). */
const SEARCH_PAUSE = 500;

function onKeydown(event: KeyboardEvent): void {
  const target = event.target as HTMLElement;
  if (target.closest(".k-tree__editor")) {
    editor.onKeydown(event);
    if (event.defaultPrevented) event.stopPropagation();
    return;
  }
  const item = target.closest<HTMLElement>("[data-node]");
  const key = item?.dataset.node;
  const at = key === undefined ? undefined : rowIndex.value.get(key);
  const row = at === undefined ? undefined : rows.value[at];
  if (!row || at === undefined || item !== target) return;
  const handled = () => {
    event.preventDefault();
    event.stopPropagation();
  };
  const ctrl = event.ctrlKey || event.metaKey;
  const letter = event.key.toLowerCase();
  if (ctrl && !event.altKey && !event.shiftKey && (letter === "x" || letter === "v")) {
    if (row.kind === "node" && (letter === "x" ? moves.cutNode(row.key) : moves.pasteInto(row.key)))
      handled();
    return;
  }
  if (event.key === "Escape") {
    if (moves.cancelCut()) handled();
    return;
  }
  if (row.kind === "more" && (event.key === "Enter" || event.key === " ")) {
    handled();
    pages.request(row.parent, true);
    return;
  }
  // A notice is activated, never selected: Space does nothing on it (nor reaches Reka).
  if (row.kind === "notice" && (event.key === "Enter" || event.key === " ")) {
    handled();
    if (event.key === "Enter") emit("activate", row.key);
    return;
  }
  if (row.kind === "node" && (event.key === "Enter" || event.key === "F2")) {
    if (props.editable?.(row.node)) {
      handled();
      void editor.start({ row: row.key, column: "" });
    } else if (event.key === "Enter") {
      handled();
      selected.value = row.key;
      emit("activate", row.key);
    }
    return;
  }
  if (event.key === "*" && !ctrl && !event.altKey) {
    handled();
    const ids = closedSiblings(rows.value, at, open.value);
    if (ids.length) expanded.value = [...expanded.value, ...ids];
    return;
  }
  if (event.key === " " || event.altKey || ctrl) return;
  const step = treeStep(rows.value, at, event.key, open.value, isRtl(item));
  if (step) {
    handled();
    if (step.open) expanded.value = [...expanded.value, step.open];
    if (step.close) expanded.value = expanded.value.filter((id) => id !== step.close);
    if (step.to !== undefined) void focusRow(rows.value[step.to]!.key);
    return;
  }
  // Keys the pattern leaves alone (Shift, Tab, an arrow it does not use): never searched.
  if (!typesAhead(event)) {
    if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key))
      handled();
    return;
  }
  handled();
  search += event.key;
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => (search = ""), SEARCH_PAUSE);
  const found = searchRow(rows.value, at, search);
  if (found >= 0) void focusRow(rows.value[found]!.key);
}
</script>

<template>
  <div class="k-tree-frame" v-bind="rootAttrs">
    <div
      ref="scroller"
      class="k-tree"
      data-k-typeahead
      :data-virtual="virtual.virtual.value ? 'true' : undefined"
      @keydown.capture="onKeydown"
      @focusin="onFocusin"
      @focusout="editor.onFocusout"
      @dragover="moves.onDragElsewhere"
    >
      <div
        v-if="props.loading"
        role="tree"
        class="k-tree__list"
        :aria-label="props.label"
        aria-busy="true"
        v-bind="treeAttrs"
      >
        <!-- Bars of a node's width, as the row that reads a page draws its own. -->
        <div v-for="r in props.loadingRows" :key="r" class="k-tree__row" aria-hidden="true">
          <span class="k-tree__twisty"></span>
          <span class="k-tree__bar k-skeleton-fill"></span>
        </div>
        <span class="k-visually-hidden">{{ t("loading") }}</span>
      </div>
      <TreeRoot
        v-else
        v-bind="treeAttrs"
        as="div"
        class="k-tree__list"
        selection-behavior="replace"
        :aria-label="props.label"
        :get-key="getKey"
        :get-children="getChildren"
        :expanded="expanded"
        :model-value="selectedNode"
        @update:expanded="(next: string[]) => (expanded = next)"
        @update:model-value="onSelect"
      >
        <template v-for="part in parts" :key="'gap' in part ? part.key : part.row.key">
          <div
            v-if="'gap' in part"
            class="k-tree__gap"
            aria-hidden="true"
            :style="{ '--k-tree-gap': `${part.gap}px` }"
          ></div>
          <TreeItem
            v-else-if="part.row.kind === 'more'"
            :ref="virtual.measure"
            as="div"
            class="k-tree__row k-tree__more"
            :value="rekaValue(part.row)"
            :level="part.row.level"
            :data-node="part.row.key"
            :data-index="part.index"
            :data-more="part.row.parent ?? ''"
            :data-active="part.row.key === stop ? '' : undefined"
            :tabindex="part.row.key === stop ? 0 : -1"
            :aria-label="pageLoading(part.row.parent) ? t('childrenLoading') : t('moreRows')"
            :aria-busy="pageLoading(part.row.parent) ? 'true' : undefined"
            :style="{ '--k-tree-depth': String(part.row.level - 1) }"
            @click="pages.request(part.row.parent, true)"
          >
            <span class="k-tree__twisty" aria-hidden="true"></span>
            <span
              v-if="pageLoading(part.row.parent)"
              class="k-tree__bar k-skeleton-fill"
              aria-hidden="true"
            ></span>
            <span v-else class="k-tree__more-label">{{ t("moreRows") }}</span>
          </TreeItem>
          <!-- A notice after its siblings: out of their set, never selected, activated. -->
          <TreeItem
            v-else-if="part.row.kind === 'notice'"
            :ref="virtual.measure"
            as="div"
            class="k-tree__row k-tree__notice"
            :value="rekaValue(part.row)"
            :level="part.row.level"
            :data-node="part.row.key"
            :data-index="part.index"
            :data-active="part.row.key === stop ? '' : undefined"
            :tabindex="part.row.key === stop ? 0 : -1"
            :style="{ '--k-tree-depth': String(part.row.level - 1) }"
            @click="emit('activate', part.row.key)"
          >
            <span class="k-tree__twisty" aria-hidden="true"></span>
            <Icon v-if="part.row.node.icon" :name="part.row.node.icon" class="k-tree__icon" />
            <span class="k-tree__more-label">{{ part.row.node.label }}</span>
          </TreeItem>
          <TreeItem
            v-else
            :ref="virtual.measure"
            as="div"
            class="k-tree__row"
            :value="rekaValue(part.row)"
            :level="part.row.level"
            :data-node="part.row.key"
            :data-index="part.index"
            :data-active="part.row.key === stop ? '' : undefined"
            :tabindex="part.row.key === stop ? 0 : -1"
            :aria-setsize="part.row.setSize"
            :aria-posinset="part.row.posInSet"
            :aria-describedby="describedBy(part.row)"
            :aria-busy="
              moveOf(part.row.node) === 'pending' ||
              props.nodeState?.(part.row.node) === 'loading-children'
                ? 'true'
                : undefined
            "
            :data-state="props.nodeState?.(part.row.node)"
            :data-move="moveOf(part.row.node)"
            :data-drop="dropOf(part.row.node)"
            :draggable="moves.movable(part.row.key) ? 'true' : undefined"
            :style="{ '--k-tree-depth': String(part.row.level - 1) }"
            @dragstart="moves.onDragStart(part.row.key, $event)"
            @dragover="moves.onDragOver(part.row.key, $event)"
            @dragenter="moves.onDragOver(part.row.key, $event)"
            @drop="moves.onDrop(part.row.key, $event)"
            @dragend="moves.onDragEnd()"
          >
            <span class="k-tree__twisty" aria-hidden="true">
              <Icon
                v-if="hasChildren(part.row.node)"
                name="chevron-right"
                class="k-chevron"
                :data-open="isExpanded(part.row.key) ? 'true' : undefined"
              />
            </span>
            <Icon v-if="part.row.node.icon" :name="part.row.node.icon" class="k-tree__icon" />
            <div
              v-if="editing === part.row.key"
              class="k-field-frame k-tree__editor"
              data-size="md"
              :data-invalid="labelError(part.row.node) ? 'true' : undefined"
              @click.stop
            >
              <input
                v-model="draft"
                class="k-field-frame__input"
                type="text"
                spellcheck="false"
                :aria-label="part.row.node.label"
                :aria-invalid="labelError(part.row.node) ? 'true' : undefined"
                :aria-describedby="
                  labelError(part.row.node) ? `${describedBy(part.row)}-error` : undefined
                "
              />
            </div>
            <span v-else class="k-tree__label">{{ part.row.node.label }}</span>
            <!-- Out of the name (the label alone), the row's description: key, detail,
                 states, reasons. Its copy buttons are the pointer's (tamed). -->
            <span :id="describedBy(part.row)" class="k-tree__detail" aria-hidden="true">
              <code v-if="part.row.node.objectKey" class="k-tree__key">{{
                part.row.node.objectKey
              }}</code>
              <span v-if="part.row.node.detail" class="k-tree__muted">{{
                part.row.node.detail
              }}</span>
              <!-- The marks of the states, in the order of TreeGrid's, before the reason. -->
              <Icon
                v-if="
                  moveOf(part.row.node) === 'pending' ||
                  props.labelState?.(part.row.node) === 'pending'
                "
                name="loader-circle"
                class="k-spinner k-tree__mark"
              />
              <Icon
                v-else-if="refusal(part.row.node) || labelError(part.row.node)"
                name="circle-x"
                class="k-tree__mark k-tree__mark--ko"
              />
              <Icon
                v-else-if="moveOf(part.row.node) === 'cut'"
                name="scissors"
                class="k-tree__mark"
              />
              <Icon
                v-if="
                  props.nodeState?.(part.row.node) === 'draft' ||
                  props.labelState?.(part.row.node) === 'draft'
                "
                name="draft"
                class="k-tree__mark"
              />
              <Icon
                v-else-if="props.labelState?.(part.row.node) === 'review'"
                name="eye"
                class="k-tree__mark"
              />
              <Icon
                v-if="props.nodeState?.(part.row.node) === 'moved'"
                name="move"
                class="k-tree__mark"
              />
              <Icon
                v-else-if="props.nodeState?.(part.row.node) === 'preview'"
                name="circle-dashed"
                class="k-tree__mark"
              />
              <span v-if="rowWords(part.row.node)" class="k-visually-hidden"
                >({{ rowWords(part.row.node) }})</span
              >
              <span
                v-if="labelError(part.row.node)"
                :id="`${describedBy(part.row)}-error`"
                class="k-tree__refusal"
              >
                {{ labelError(part.row.node)!.error }}
                <CodeBadge
                  v-if="labelError(part.row.node)!.code"
                  :code="labelError(part.row.node)!.code!"
                />
              </span>
              <span v-if="refusal(part.row.node)" class="k-tree__refusal">
                {{ refusal(part.row.node)!.reason }}
                <CodeBadge
                  v-if="refusal(part.row.node)!.code"
                  :code="refusal(part.row.node)!.code!"
                />
              </span>
            </span>
          </TreeItem>
        </template>
      </TreeRoot>
    </div>
    <div class="k-visually-hidden" role="status">{{ status.text.value }}</div>
    <div class="k-visually-hidden" role="alert">{{ alerts.text.value }}</div>
    <MoveDialog
      v-if="declared"
      v-model:open="movingOpen"
      :node="movingLabel"
      :targets="moveTargets"
      @move="onMoveTo"
    />
  </div>
</template>

<style>
@layer components {
  /* The width it is given, whatever the nodes open or renamed. */
  .k-tree-frame {
    display: grid;
    inline-size: 100%;
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
    min-block-size: 0;
  }

  .k-tree {
    min-inline-size: 0;
    overflow: auto;
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-tree[data-virtual] {
    max-block-size: var(--k-size-list-max);
  }

  .k-tree__list {
    padding-block: var(--k-space-1);
  }

  .k-tree__gap {
    block-size: var(--k-tree-gap);
  }

  .k-tree__row {
    display: flex;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-row-height);
    padding-inline: calc(var(--k-space-1) + var(--k-tree-indent) * var(--k-tree-depth, 0))
      var(--k-cell-padding-inline);
    white-space: nowrap;
    cursor: default;

    /* Hovered or selected: a colour fade, nothing moves (design « Mouvement »). */
    transition-property: background-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-tree__row:hover {
    background-color: var(--k-hover);
  }

  /* A notice says where the rest is: it wraps rather than lose its end. */
  .k-tree__notice {
    padding-block: var(--k-space-1);
    white-space: normal;
  }

  .k-tree__row:focus-visible {
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-tree__row[aria-selected="true"] {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-tree__row[data-state="draft"],
  .k-tree__row[data-state="moved"],
  .k-tree__row[data-state="preview"] {
    box-shadow: var(--k-shadow-inset-accent);
  }

  /* A node a draft moves: at its present place, muted (it leaves); at its place after the
   * draft, a dashed outline (not there yet). Each also has its mark and its words. */
  .k-tree__row[data-state="moved"] {
    color: var(--k-text-muted);
  }

  /* Under the focus ring, never in its place; the outline of a target during a drag wins
   * over it (`:where`: no more weight than the row's class). */
  .k-tree__row:where([data-state="preview"]:not(:focus-visible)) {
    outline: var(--k-border-width) dashed var(--k-accent);
    outline-offset: calc(var(--k-border-width) * -1);
  }

  .k-tree__row[data-drop="ok"] {
    background-color: var(--k-accent-soft);
    outline: var(--k-focus-width) solid var(--k-accent);
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-tree__row[data-drop="refused"] {
    outline: var(--k-border-width) dashed var(--k-state-ko-fg);
    outline-offset: calc(var(--k-border-width) * -1);
  }

  .k-tree__row[data-move="pending"],
  .k-tree__row[data-move="cut"] {
    color: var(--k-text-muted);
  }

  .k-tree__row[data-move="cut"] .k-tree__label {
    text-decoration: underline dashed;
  }

  /* The chevron: 24 px around a 16 px icon, laid out at the icon's size (WCAG 2.5.8). */
  .k-tree__twisty {
    display: inline-flex;
    flex: none;
    box-sizing: content-box;
    inline-size: var(--k-icon-size);
    padding: var(--k-space-1);
    margin: calc(var(--k-space-1) * -1);
    color: var(--k-text-muted);
  }

  .k-tree__icon,
  .k-tree__key,
  .k-tree__muted,
  .k-tree__more-label {
    color: var(--k-text-muted);
  }

  .k-tree__label {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .k-tree__detail {
    display: inline-flex;
    flex: none;
    gap: var(--k-space-1);
    align-items: center;
  }

  .k-tree__muted,
  .k-tree__more-label {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
  }

  .k-tree__row > .k-tree__editor {
    flex: 1;
    min-inline-size: 0;
  }

  .k-tree__mark {
    flex: none;
    color: var(--k-text-muted);
  }

  .k-tree__mark--ko {
    color: var(--k-state-ko-fg);
  }

  .k-tree__refusal {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-state-ko-fg);
  }

  .k-tree__bar {
    inline-size: var(--k-size-col-md);
    block-size: var(--k-space-3);
    border-radius: var(--k-radius-sm);
  }
}
</style>
