<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A tree with columns (APG Treegrid, written by kartenn as `DataGrid`; design
// « Inventaire », line 21, « Contrat d'édition », « Contrat de déplacement »; REQ-020-19,
// 31, 32). The rows of the model, depth first, with `aria-level`, `aria-setsize`,
// `aria-posinset`, `aria-expanded`, `aria-rowindex`; a list with pages left counts
// `-1` and ends with a row that asks the host for its next page (`load`) when it comes
// into view or is activated; a node opened with its children still to read asks for the
// first page the same way. Virtualised beyond 200 rows (`internal/virtualRows.ts`): the
// row of the focus and the one of the editor stay mounted, the keys move over the
// flattened index (no press lost).
// Keyboard: one tab stop. On the node cell of a row, Up and Down, Home and End, Page Up
// and Page Down move between rows; Right opens a closed node, else goes to the next cell
// of the row (an open node without one: its first child); Left closes an open node, else
// goes to the parent. On another cell, the arrows move by one cell (Left from the first
// one back to the node cell), Home and End to the ends of the row, Ctrl+Home and Ctrl+End
// to the first and last rows (APG Treegrid). Space selects the row. Enter or F2 edits an
// editable cell (`editable`; Escape cancels, Enter or Tab commits, Ctrl+Enter commits
// marked `now`); else, on the node cell, Enter activates the row (`activate`: its record,
// as in the other grids, 021/A20) and Tab or F2 moves into its « + » (from there Tab
// leaves the tree, Shift+Tab, Escape or F2 come back); on another cell, Enter or F2 moves
// into its first widget, else Enter activates the row. The cell of the focus is a model
// (`active`), kept when the focus leaves, as in `DataGrid`.
// Direct addition (024/M10): the « + » of a node (`addLabel`) emits `add`; refused
// (`addBlocked`: the permission missing, or another reason), it stays visible and
// focusable, `aria-disabled`, its reason in its tooltip and description (REQ-021-37).
// Moves (`parentMode`): `derived`, none; `declared`, drag and drop, Ctrl+X then Ctrl+V,
// and « Déplacer vers… » (`openMove`, for the host's action on the chosen node), every
// target checked by `canMove` first; an intent only (`move`), the node shown at its place
// with the state the host gives (`moveState`) until its data change. The tree never moves
// a node: it renders the parent of its model, checked at each render outside production.
// Adaptation to the width (design « Adaptation à la largeur »), as `DataGrid`: the root is
// a container, the tree column sticks, columns of priority 2 and 3 hide under a container
// `md` and `lg`, a column chooser shows them again; with no hover, the « + » shows on the
// selected row only.
// A cell whose value is `null` is absent, said by `NoValue` (a dash shown, « non
// renseigné » read); a cell not given (a domain row) and `""` stay blank, as before.
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref,
  useAttrs,
  useId,
  watch,
} from "vue";
import { t } from "../../i18n.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import CellError from "../DataGrid/CellError.vue";
import {
  columnTemplates,
  priorityOf,
  renderedColumns,
  type CellRef,
  type CellState,
  type ColumnVisibility,
  type GridColumn,
  type GridCommit,
} from "../DataGrid/grid.ts";
import Icon from "../Icon/Icon.vue";
import IconButton from "../IconButton/IconButton.vue";
import Skeleton from "../Skeleton/Skeleton.vue";
import { useCellEditor, useCellErrors, useCommitWatch } from "../internal/cellEditor.ts";
import ColumnChooser from "../internal/ColumnChooser.vue";
import { isRtl, moveInGrid } from "../internal/gridKeys.ts";
import MoveDialog from "../internal/MoveDialog.vue";
import NoValue from "../NoValue/NoValue.vue";
import { useTreeStates } from "../internal/treeStates.ts";
import { rowsInView, useVirtualRows } from "../internal/virtualRows.ts";
import {
  flatten,
  guardRendered,
  hasChildren,
  indexNodes,
  landingRow,
  treeStep,
  type MoveCheck,
  type MoveIntent,
  type MoveState,
  type ParentMode,
  type TreeNode,
  type TreeNodeState,
  type TreeRow,
} from "./tree.ts";

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    label: string;
    /** The header of the tree column. */
    treeLabel: string;
    nodes: TreeNode[];
    columns?: GridColumn[];
    /** The first level has pages left (a row after it reads the next one). */
    more?: boolean;
    /** The next page of the first level is coming. */
    moreLoading?: boolean;
    nodeState?: ((node: TreeNode) => TreeNodeState | undefined) | undefined;
    /** The name of the « + » of a node (« Ajouter dans 192.0.2.0/24 »); none, no button. */
    addLabel?: ((node: TreeNode) => string | undefined) | undefined;
    /**
     * Why the « + » of a node is refused: the permission the user lacks (`forbidden`,
     * « Il faut … »), or another sentence (`reason`); nothing, it is allowed.
     */
    addBlocked?:
      ((node: TreeNode) => { forbidden?: string; reason?: string } | undefined) | undefined;
    /**
     * The nodes are coming: skeleton rows of the height of a row, in the cells, the tree
     * busy (REQ-020-23); as many as the host expects, so that the nodes take their place.
     */
    loading?: boolean;
    loadingRows?: number;
    /** How the parent of a node is known: deduced (no move offered) or declared. */
    parentMode?: ParentMode;
    /** Whether `node` may go into `target` (`null`: the first level), for each target. */
    canMove?: ((node: TreeNode, target: TreeNode | null) => MoveCheck) | undefined;
    /** Where the move of a node stands, as the host says it. */
    moveState?: ((node: TreeNode) => MoveState | undefined) | undefined;
    /**
     * Which cells the user may edit (the tree column has the id `""`): the tree never
     * decides it (REQ-020-19). Without it, the treegrid is read only.
     */
    editable?: ((node: TreeNode, column: GridColumn) => boolean) | undefined;
    /** The local check of the text of an editor: its error, or nothing. */
    validate?:
      ((node: TreeNode, column: GridColumn, value: string) => string | undefined) | undefined;
    cellState?: ((node: TreeNode, column: GridColumn) => CellState | undefined) | undefined;
  }>(),
  {
    columns: () => [],
    nodeState: undefined,
    addLabel: undefined,
    addBlocked: undefined,
    loadingRows: 5,
    parentMode: "derived",
    canMove: undefined,
    moveState: undefined,
    editable: undefined,
    validate: undefined,
    cellState: undefined,
  },
);
const emit = defineEmits<{
  add: [id: string];
  /** The next page of the children of a node (`null`: of the first level). */
  load: [parent: string | null];
  /** A move accepted by `canMove`: the host moves the node in its data, or refuses. */
  move: [intent: MoveIntent];
  /** An editor commits its text (Enter, Tab, or the focus leaving it), checked locally. */
  commit: [commit: GridCommit];
  /** Enter on the node cell of a row (not editable), or on a cell without a widget. */
  activate: [id: string];
}>();
/** The ids of the open nodes. */
const expanded = defineModel<string[]>("expanded", { default: () => [] });
/** The id of the selected node (Space or a click selects). */
const selected = defineModel<string | undefined>("selected", { default: undefined });
/** The cell whose editor is open (`row`: the node id; `column`: `""` for the tree). */
const editing = defineModel<CellRef | undefined>("editing", { default: undefined });
/** The user's choice of columns (column chooser). */
const visibility = defineModel<ColumnVisibility>("columnVisibility", { default: () => ({}) });

const attrs = useAttrs();
const rootAttrs = computed(() => ({ class: attrs.class, style: attrs.style }));
const gridAttrs = computed(() =>
  Object.fromEntries(Object.entries(attrs).filter(([key]) => key !== "class" && key !== "style")),
);
const root = ref<HTMLElement | null>(null);
const grid = ref<HTMLElement | null>(null);
const uid = useId();

// ——— Model and rows ———

const open = computed(() => new Set(expanded.value));
const index = computed(() => indexNodes(props.nodes));
const nodeOf = (id: string) => index.value.get(id)?.node;
const rows = computed(() => flatten(props.nodes, open.value, props.more));
const rowIndex = computed(() => new Map(rows.value.map((row, i) => [row.key, i])));
const isExpanded = (id: string) => open.value.has(id);
const anyMore = computed(() => rows.value.some((row) => row.kind === "more"));

// ——— Columns ———

const treeColumn = computed<GridColumn>(() => ({ id: "", label: props.treeLabel, width: "fill" }));
/** The tree column, then the columns of the host. */
const all = computed<GridColumn[]>(() => [treeColumn.value, ...props.columns]);
/** The columns of the host rendered (the user's hidden ones left out). */
const shown = computed(() => renderedColumns(all.value, visibility.value).slice(1));
const template = computed(() => columnTemplates(all.value, visibility.value));
const priority = (column: GridColumn) => {
  const p = priorityOf(all.value, column, visibility.value);
  return p > 1 ? p : undefined;
};
const colIndex = (column: GridColumn) => all.value.indexOf(column) + 1;
const columnOf = (id: string) => all.value.find((column) => column.id === id);
/** The columns the focus moves over: the tree column, then those of the host on screen. */
function navColumns(): string[] {
  const visible = shown.value.filter((column) => {
    const header = grid.value?.querySelector(
      `[role="columnheader"][data-column="${CSS.escape(column.id)}"]`,
    );
    return !!header && header.getClientRects().length > 0;
  });
  return ["", ...visible.map((column) => column.id)];
}
const depth = (row: TreeRow) => ({ "--k-tree-depth": String(row.level - 1) });

// ——— Focus ———

/** The cell of the focus (`row`: the node id; `column`: `""` for the tree), kept on leaving. */
const active = defineModel<CellRef | undefined>("active", { default: undefined });
/** The cell that holds the roving focus: the one last focused, the selected row, the first. */
const current = computed<CellRef | undefined>(() => {
  const there = (key: string | undefined) => key !== undefined && rowIndex.value.has(key);
  if (active.value && there(active.value.row)) return active.value;
  if (there(selected.value)) return { row: selected.value!, column: "" };
  const first = rows.value[0];
  return first ? { row: first.key, column: "" } : undefined;
});
const isStop = (row: string, column: string) =>
  current.value?.row === row && current.value.column === column;
const cellElement = (cell: CellRef) =>
  grid.value?.querySelector<HTMLElement>(
    `[data-row="${CSS.escape(cell.row)}"][data-column="${CSS.escape(cell.column)}"]`,
  ) ?? null;
async function focusActive(): Promise<void> {
  await nextTick();
  if (current.value) cellElement(current.value)?.focus();
}
/** A column hidden under the focus, a row gone: the stop back to a cell still there. */
function keepStop(): void {
  const cell = active.value;
  if (!cell) return;
  if (!rowIndex.value.has(cell.row)) active.value = undefined;
  else if (cell.column !== "" && !navColumns().includes(cell.column))
    active.value = { row: cell.row, column: "" };
}
watch([rows, shown], () => void nextTick(keepStop));
// The focused row leaves the rows shown (the host's new data): the focus to where it
// lands, never to the page. Read before the render, while the row is still there.
watch(
  rows,
  (now, before) => {
    const key = active.value?.row;
    const at = key ? before.findIndex((row) => row.key === key) : -1;
    if (!key || at < 0 || now.some((row) => row.key === key)) return;
    const el = grid.value?.querySelector(`[role="row"][data-node="${CSS.escape(key)}"]`);
    if (!el?.contains(document.activeElement)) return;
    const next = landingRow(key, before, now, (k) => index.value.get(k)?.parent);
    if (next) {
      active.value = { row: next, column: "" };
      void focusActive();
    }
  },
  { flush: "pre" },
);

/**
 * The nodes arrive while the focus is on the cell that said the tree was loading: that
 * cell goes, the focus goes to the row of the stop (WCAG 2.4.3), not to the page. Read
 * before the render (`flush: "pre"`), while the skeleton is still there.
 */
watch(
  () => props.loading,
  (now, was) => {
    const skeleton = grid.value?.querySelector("[data-loading]");
    if (was && !now && skeleton?.contains(document.activeElement)) void focusActive();
  },
  { flush: "pre" },
);

// ——— Virtualisation ———

const headHeight = ref(30);
const virtual = useVirtualRows({
  count: () => rows.value.length,
  keyAt: (i) => rows.value[i]?.key ?? String(i),
  indexOf: (key) => rowIndex.value.get(key),
  scroller: grid,
  kept: () => [current.value?.row, editing.value?.row],
  top: () => headHeight.value,
  off: () => !!props.loading,
});
/** The rows rendered and their gaps; the display guard runs on them (REQ-020-32). */
const parts = computed(() => {
  const list = virtual.parts.value;
  guardRendered(
    "TreeGrid",
    rows.value,
    list.flatMap((part) => ("index" in part ? [part.index] : [])),
    (key) => index.value.get(key)?.parent,
  );
  return list.map((part) =>
    "index" in part ? { index: part.index, row: rows.value[part.index]! } : part,
  );
});
/** The rows of one screen (Page Up, Page Down): the rows wholly in view, less one. */
function pageRows(): number {
  const el = grid.value;
  const seen = el
    ? rowsInView(el, el.querySelectorAll<HTMLElement>("[role='row'][data-index]"), headHeight.value)
    : null;
  return seen && seen.count > 1 ? seen.count - 1 : 10;
}

// ——— Pages of children ———

// The states shared with `Tree` (`internal/treeStates.ts`): moves, regions, pages.
const {
  status,
  alerts,
  moves,
  declared,
  moveOf,
  refusal,
  dropOf,
  rowWords,
  movingOpen,
  movingLabel,
  openMove,
  moveTargets,
  onMoveTo,
  pages,
  pageLoading,
} = useTreeStates(props, rows, index, grid, {
  load: (parent) => emit("load", parent),
  move: (intent) => emit("move", intent),
});
defineExpose({ openMove });
function openNode(id: string): void {
  if (!isExpanded(id)) expanded.value = [...expanded.value, id];
  if (!nodeOf(id)?.children) pages.request(id);
}
function closeNode(id: string): void {
  expanded.value = expanded.value.filter((key) => key !== id);
}
function toggle(id: string): void {
  if (isExpanded(id)) closeNode(id);
  else openNode(id);
}

// ——— Editing ———

const isEditing = (key: string, column: string) =>
  editing.value?.row === key && editing.value.column === column;
const canEdit = (node: TreeNode, column: GridColumn) => !!props.editable?.(node, column);
/** The value of a cell: `null` absent (`NoValue`), a cell not given blank. */
function cellOf(node: TreeNode, column: GridColumn): string | null {
  const value = node.cells?.[column.id];
  return value === undefined ? "" : value;
}
const textOf = (cell: CellRef) => {
  const node = nodeOf(cell.row);
  return cell.column === "" ? (node?.label ?? "") : (node?.cells?.[cell.column] ?? "");
};
const alert = alerts.say;
const sayError = (cell: CellRef, error: { error: string; code?: string }) =>
  `${columnOf(cell.column)?.label ?? ""}, ${nodeOf(cell.row)?.label ?? cell.row} : ${error.error}${
    error.code ? ` (${error.code})` : ""
  }`;
const commits = useCommitWatch(
  (cell) => {
    const node = nodeOf(cell.row);
    const column = columnOf(cell.column);
    return node && column ? props.cellState?.(node, column) : undefined;
  },
  (cell) => (nodeOf(cell.row) ? textOf(cell) : undefined),
  (cell, error) => void alert(sayError(cell, error)),
);
const editor = useCellEditor({
  editing,
  textOf,
  driven: () => !!props.editable,
  check: (cell, value) => {
    const node = nodeOf(cell.row);
    const column = columnOf(cell.column);
    return node && column ? props.validate?.(node, column, value) : undefined;
  },
  cellElement,
  moveFocus(cell, by) {
    const columns = navColumns();
    const at = columns.indexOf(cell.column);
    const to = columns[Math.max(0, Math.min(columns.length - 1, at + by))] ?? cell.column;
    active.value = { row: cell.row, column: to };
    void focusActive();
  },
  commit(commit) {
    emit("commit", commit);
    commits.follow(commit);
  },
  refuse(cell, error) {
    void alert(`${sayError(cell, { error })} ${t("editEscape")}`);
  },
  editorClass: "k-tree-grid__editor",
});
const draft = editor.draft;
const editorLabel = (node: TreeNode, column: GridColumn) => `${column.label} — ${node.label}`;
async function startEditing(cell: CellRef): Promise<void> {
  active.value = cell;
  await editor.start(cell);
}

// ——— States and errors of cells ———

function stateOf(node: TreeNode, column: GridColumn): string | undefined {
  const state = props.cellState?.(node, column);
  return state === undefined ? undefined : typeof state === "string" ? state : "error";
}
const CELL_WORDS = { pending: "pending", draft: "inDraft", review: "inReview" } as const;
function stateWord(node: TreeNode, column: GridColumn): string | undefined {
  const state = stateOf(node, column);
  return state && state in CELL_WORDS ? t(CELL_WORDS[state as keyof typeof CELL_WORDS]) : undefined;
}
/** The error a cell shows: its editor's (the host's check), else the host's. */
function messageOf(
  node: TreeNode,
  column: GridColumn,
): { error: string; code?: string } | undefined {
  if (isEditing(node.id, column.id) && editor.error.value) return { error: editor.error.value };
  const state = props.cellState?.(node, column);
  return typeof state === "object" ? state : undefined;
}
const cellKey = (row: string, column: string) => `${row}\n${column}`;
const errors = useCellErrors((key) => {
  const [row, column] = key.split("\n");
  const node = nodeOf(row!);
  const col = columnOf(column!);
  return !!node && !!col && !!messageOf(node, col);
});
const errorId = (row: number, column: string) =>
  `k-tree-grid-${uid}-error-${row}-${colIndex(columnOf(column)!)}`;
const errorShown = (node: TreeNode, column: GridColumn) =>
  !!messageOf(node, column) &&
  (isEditing(node.id, column.id) || errors.shown.value === cellKey(node.id, column.id));
/** The pointer leaves a cell, unless for its message (adjacent). */
const onCellLeave = (row: number, column: string, event: PointerEvent) =>
  errors.leave(event, document.getElementById(errorId(row, column)));
function errorAnchor(node: TreeNode, column: GridColumn): HTMLElement | null {
  return errorShown(node, column) ? cellElement({ row: node.id, column: column.id }) : null;
}

// ——— Events ———

function onKeydown(event: KeyboardEvent): void {
  const target = event.target as HTMLElement;
  if (target.closest(".k-tree-grid__editor")) {
    editor.onKeydown(event);
    return;
  }
  const cell = target.closest<HTMLElement>("[data-column]");
  if (!cell || !grid.value?.contains(cell)) return;
  // A widget of a cell (the « + », the copy button of a code; reached by Enter or F2, the
  // « + » of the node cell by Tab or F2) keeps its keys; F2 goes on to the next one, then
  // back to the cell; Escape back to the cell.
  if (cell !== target) {
    if (event.key === "Escape" || event.key === "F2") {
      event.preventDefault();
      const widgets = widgetsOf(cell);
      const next = event.key === "F2" ? widgets[widgets.indexOf(target) + 1] : undefined;
      (next ?? cell).focus();
    }
    return;
  }
  // Escape closes the error of the cell only, not the layer around the tree (1.4.13).
  if (event.key === "Escape" && errors.shown.value !== null) {
    event.preventDefault();
    errors.dismiss();
    return;
  }
  const ref = { row: cell.dataset.row!, column: cell.dataset.column! };
  const at = rowIndex.value.get(ref.row);
  const row = at === undefined ? undefined : rows.value[at];
  if (!row || at === undefined) return;
  const ctrl = event.ctrlKey || event.metaKey;
  const letter = event.key.toLowerCase();
  if (ctrl && !event.altKey && !event.shiftKey && (letter === "x" || letter === "v")) {
    if (row.kind !== "node") return;
    if (letter === "x" ? moves.cutNode(row.key) : moves.pasteInto(row.key)) event.preventDefault();
    return;
  }
  if (event.key === "Escape") {
    if (moves.cancelCut()) event.preventDefault();
    return;
  }
  if (row.kind === "more" && (event.key === "Enter" || event.key === " ")) {
    event.preventDefault();
    pages.request(row.parent, true);
    return;
  }
  // The node cell (021/A20): Enter activates the row, Tab goes into its « + », if any;
  // from there Tab leaves the tree (the « + » is out of the tab order).
  const nodeCell = ref.column === "";
  if (row.kind === "node" && nodeCell && event.key === "Tab" && !event.shiftKey && !ctrl) {
    const add = cell.querySelector<HTMLElement>(".k-tree-grid__add");
    if (add && !event.altKey) {
      event.preventDefault();
      add.focus();
    }
    return;
  }
  if (row.kind === "node" && (event.key === "Enter" || event.key === "F2")) {
    event.preventDefault();
    const column = columnOf(ref.column);
    if (column && canEdit(row.node, column)) void startEditing(ref);
    else if (nodeCell && event.key === "Enter") emit("activate", row.key);
    else {
      const widget = widgetsOf(cell)[0];
      if (widget) widget.focus();
      else if (event.key === "Enter") emit("activate", row.key);
    }
    return;
  }
  if (row.kind === "node" && event.key === " ") {
    event.preventDefault();
    selected.value = row.key;
    return;
  }
  if (event.altKey) return;
  const rtl = isRtl(cell);
  const next =
    ref.column === "" ? fromNodeCell(row, at, event, rtl) : fromCell(ref, at, event, rtl);
  if (next === null) return;
  event.preventDefault();
  if (next) {
    active.value = next;
    void focusActive();
  }
}

/** A key on the node cell of a row: the cell it goes to, `undefined` (done in place), null. */
function fromNodeCell(
  row: TreeRow,
  at: number,
  event: KeyboardEvent,
  rtl: boolean,
): CellRef | undefined | null {
  const list = rows.value;
  if (event.key === "PageDown" || event.key === "PageUp") {
    const by = event.key === "PageDown" ? pageRows() : -pageRows();
    return { row: list[Math.max(0, Math.min(list.length - 1, at + by))]!.key, column: "" };
  }
  // Right on an open node or a leaf: into the next cell of the row, if there is one.
  const forward = rtl ? "ArrowLeft" : "ArrowRight";
  if (
    event.key === forward &&
    row.kind === "node" &&
    (!hasChildren(row.node) || isExpanded(row.key))
  ) {
    const columns = navColumns();
    if (columns.length > 1) return { row: row.key, column: columns[1]! };
  }
  const step = treeStep(list, at, event.key, open.value, rtl);
  if (!step) return null;
  if (step.open) openNode(step.open);
  if (step.close) closeNode(step.close);
  return step.to === undefined ? undefined : { row: list[step.to]!.key, column: "" };
}

/** A key on another cell: the APG Grid moves, over the rows and the columns on screen. */
function fromCell(cell: CellRef, at: number, event: KeyboardEvent, rtl: boolean): CellRef | null {
  const columns = navColumns();
  const to = moveInGrid(
    { row: at, column: Math.max(0, columns.indexOf(cell.column)) },
    event,
    rows.value.length,
    columns.length,
    rtl,
    pageRows(),
  );
  if (!to) return null;
  const row = rows.value[to.row]!;
  return { row: row.key, column: row.kind === "more" ? "" : columns[to.column]! };
}

function onFocusin(event: FocusEvent): void {
  const cell = (event.target as HTMLElement).closest<HTMLElement>("[data-column]");
  if (!cell?.dataset.row) return;
  const ref = { row: cell.dataset.row, column: cell.dataset.column! };
  if (active.value?.row !== ref.row || active.value.column !== ref.column) active.value = ref;
  errors.focused.value = cellKey(ref.row, ref.column);
}
function onFocusout(event: FocusEvent): void {
  const to = event.relatedTarget as HTMLElement | null;
  // Into the message of the cell (its copy button): still focused.
  if (to?.closest(".k-cell-error")) return;
  if (!grid.value?.contains(to)) errors.focused.value = null;
  editor.onFocusout(event);
}

/** A click on a row selects it; on its chevron, opens or closes it. */
function onRowClick(row: TreeRow, event: MouseEvent): void {
  const target = event.target as HTMLElement;
  if (target.closest(".k-tree-grid__editor, button, a, input")) return;
  if (row.kind === "more") {
    pages.request(row.parent, true);
    return;
  }
  if (target.closest(".k-tree-grid__twisty")) toggle(row.key);
  selected.value = row.key;
}
function onDblclick(event: MouseEvent): void {
  const cell = (event.target as HTMLElement).closest<HTMLElement>("[data-column]");
  if (!cell?.dataset.row || (event.target as HTMLElement).closest(".k-tree-grid__editor")) return;
  const node = nodeOf(cell.dataset.row);
  const column = columnOf(cell.dataset.column!);
  if (node && column && canEdit(node, column))
    void startEditing({ row: node.id, column: column.id });
}

// ——— Measures ———

/** The header sticks: its height into the virtualiser's offset and the scroll padding. */
function measureHead(): void {
  const head = grid.value?.querySelector<HTMLElement>(".k-tree-grid__head");
  if (head && head.offsetHeight > 0) headHeight.value = head.offsetHeight;
  // The tree column sticks too: a cell taking the focus scrolls out from under it (2.4.11).
  const key = head?.querySelector<HTMLElement>('[data-column=""]');
  if (key) grid.value!.style.setProperty("--k-tree-grid-key", `${key.offsetWidth}px`);
}
let observer: ResizeObserver | undefined;
/** The widgets of a cell, in order: out of the tab order, reached by Enter or F2. */
const widgetsOf = (cell: HTMLElement) => [...cell.querySelectorAll<HTMLElement>("button")];
function tame(): void {
  for (const button of grid.value?.querySelectorAll<HTMLElement>(".k-tree-grid__cell button") ?? [])
    button.tabIndex = -1;
}
onMounted(tame);
onUpdated(tame);
onMounted(() => {
  virtual.readEstimate();
  measureHead();
  if (typeof ResizeObserver === "undefined" || !root.value) return;
  observer = new ResizeObserver(() => {
    virtual.readEstimate();
    measureHead();
    keepStop();
  });
  observer.observe(root.value);
});
onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <div ref="root" class="k-tree-grid-frame" v-bind="rootAttrs">
    <ColumnChooser v-model="visibility" :columns="all" :grid="grid" />
    <div
      ref="grid"
      v-bind="gridAttrs"
      class="k-tree-grid"
      role="treegrid"
      :aria-label="props.label"
      :aria-colcount="props.columns.length + 1"
      :aria-rowcount="props.loading ? props.loadingRows + 1 : anyMore ? -1 : rows.length + 1"
      :aria-readonly="props.editable ? undefined : 'true'"
      :aria-busy="props.loading ? 'true' : undefined"
      :data-virtual="virtual.virtual.value ? 'true' : undefined"
      :style="template"
      @keydown="onKeydown"
      @focusin="onFocusin"
      @focusout="onFocusout"
      @dblclick="onDblclick"
      @dragover="moves.onDragElsewhere"
    >
      <div role="rowgroup" class="k-tree-grid__head">
        <div role="row" class="k-tree-grid__row" aria-rowindex="1">
          <div role="columnheader" class="k-tree-grid__header" data-column="" aria-colindex="1">
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
            :data-width="column.width ?? 'md'"
            :aria-colindex="colIndex(column)"
          >
            {{ column.label }}
          </div>
        </div>
      </div>
      <div v-if="props.loading" role="rowgroup" data-loading>
        <div
          v-for="r in props.loadingRows"
          :key="r"
          role="row"
          class="k-tree-grid__row"
          aria-level="1"
          :aria-rowindex="r + 1"
        >
          <!-- The tree keeps its tab stop while it loads: the cell that says it. -->
          <div
            role="gridcell"
            class="k-tree-grid__cell k-tree-grid__node"
            aria-colindex="1"
            :tabindex="r === 1 ? 0 : undefined"
          >
            <span class="k-tree-grid__twisty" aria-hidden="true"></span>
            <span v-if="r === 1" class="k-visually-hidden">{{ t("loading") }}</span>
            <Skeleton variant="cell" :rows="r" :columns="0" />
          </div>
          <div
            v-for="(column, c) in shown"
            :key="column.id"
            role="gridcell"
            class="k-tree-grid__cell"
            :data-align="column.align"
            :data-priority="priority(column)"
            :data-width="column.width ?? 'md'"
            :aria-colindex="colIndex(column)"
          >
            <Skeleton variant="cell" :rows="r" :columns="c + 1" />
          </div>
        </div>
      </div>
      <div v-else role="rowgroup" class="k-tree-grid__body">
        <template v-for="part in parts" :key="'gap' in part ? part.key : part.row.key">
          <div
            v-if="'gap' in part"
            class="k-tree-grid__gap"
            aria-hidden="true"
            :style="{ '--k-tree-grid-gap': `${part.gap}px` }"
          ></div>
          <!-- The row that reads the next page of a list. -->
          <div
            v-else-if="part.row.kind === 'more'"
            :ref="virtual.measure"
            role="row"
            class="k-tree-grid__row k-tree-grid__more"
            :data-index="part.index"
            :data-node="part.row.key"
            :data-more="part.row.parent ?? ''"
            :aria-level="part.row.level"
            :aria-rowindex="part.index + 2"
            :aria-busy="pageLoading(part.row.parent) ? 'true' : undefined"
            @click="onRowClick(part.row, $event)"
          >
            <div
              role="gridcell"
              class="k-tree-grid__cell k-tree-grid__node"
              :data-row="part.row.key"
              data-column=""
              aria-colindex="1"
              :aria-colspan="shown.length + 1"
              :tabindex="isStop(part.row.key, '') ? 0 : -1"
              :style="depth(part.row)"
            >
              <span class="k-tree-grid__twisty" aria-hidden="true"></span>
              <template v-if="pageLoading(part.row.parent)">
                <span class="k-tree-grid__bar k-skeleton-fill" aria-hidden="true"></span>
                <span class="k-visually-hidden">{{ t("childrenLoading") }}</span>
              </template>
              <span v-else class="k-tree-grid__more-label">{{ t("moreRows") }}</span>
            </div>
          </div>
          <div
            v-else
            :ref="virtual.measure"
            role="row"
            class="k-tree-grid__row"
            :data-index="part.index"
            :data-node="part.row.key"
            :aria-level="part.row.level"
            :aria-setsize="part.row.setSize"
            :aria-posinset="part.row.posInSet"
            :aria-rowindex="part.index + 2"
            :aria-expanded="hasChildren(part.row.node) ? isExpanded(part.row.key) : undefined"
            :aria-selected="selected === undefined ? undefined : part.row.key === selected"
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
            @click="onRowClick(part.row, $event)"
            @dragstart="moves.onDragStart(part.row.key, $event)"
            @dragover="moves.onDragOver(part.row.key, $event)"
            @dragenter="moves.onDragOver(part.row.key, $event)"
            @drop="moves.onDrop(part.row.key, $event)"
            @dragend="moves.onDragEnd()"
          >
            <div
              role="gridcell"
              class="k-tree-grid__cell k-tree-grid__node"
              :data-row="part.row.key"
              data-column=""
              aria-colindex="1"
              :data-editing="isEditing(part.row.key, '') ? 'true' : undefined"
              :aria-readonly="
                props.editable && !canEdit(part.row.node, treeColumn) ? 'true' : undefined
              "
              :aria-invalid="messageOf(part.row.node, treeColumn) ? 'true' : undefined"
              :aria-describedby="
                messageOf(part.row.node, treeColumn) ? errorId(part.index, '') : undefined
              "
              :tabindex="
                isEditing(part.row.key, '') ? undefined : isStop(part.row.key, '') ? 0 : -1
              "
              :style="depth(part.row)"
              @pointerenter="errors.enter(cellKey(part.row.key, ''), $event)"
              @pointerleave="onCellLeave(part.index, '', $event)"
            >
              <span class="k-tree-grid__twisty" aria-hidden="true">
                <Icon
                  v-if="hasChildren(part.row.node)"
                  name="chevron-right"
                  class="k-chevron"
                  :data-open="isExpanded(part.row.key) ? 'true' : undefined"
                />
              </span>
              <Icon
                v-if="part.row.node.icon"
                :name="part.row.node.icon"
                class="k-tree-grid__icon"
              />
              <div
                v-if="isEditing(part.row.key, '')"
                class="k-field-frame k-tree-grid__editor"
                data-size="md"
                :data-invalid="messageOf(part.row.node, treeColumn) ? 'true' : undefined"
              >
                <input
                  v-model="draft"
                  class="k-field-frame__input"
                  type="text"
                  spellcheck="false"
                  :aria-label="editorLabel(part.row.node, treeColumn)"
                  :aria-invalid="messageOf(part.row.node, treeColumn) ? 'true' : undefined"
                  :aria-describedby="
                    messageOf(part.row.node, treeColumn) ? errorId(part.index, '') : undefined
                  "
                />
              </div>
              <template v-else>
                <span class="k-tree-grid__label">{{ part.row.node.label }}</span>
                <code v-if="part.row.node.objectKey" class="k-tree-grid__key">{{
                  part.row.node.objectKey
                }}</code>
              </template>
              <Icon
                v-if="moveOf(part.row.node) === 'pending'"
                name="loader-circle"
                class="k-spinner k-tree-grid__mark"
              />
              <Icon
                v-else-if="refusal(part.row.node)"
                name="circle-x"
                class="k-tree-grid__mark k-tree-grid__mark--ko"
              />
              <Icon
                v-else-if="moveOf(part.row.node) === 'cut'"
                name="scissors"
                class="k-tree-grid__mark"
              />
              <Icon
                v-if="props.nodeState?.(part.row.node) === 'draft'"
                name="draft"
                class="k-tree-grid__mark"
              />
              <Icon
                v-else-if="props.nodeState?.(part.row.node) === 'moved'"
                name="move"
                class="k-tree-grid__mark"
              />
              <Icon
                v-else-if="props.nodeState?.(part.row.node) === 'preview'"
                name="circle-dashed"
                class="k-tree-grid__mark"
              />
              <span v-if="rowWords(part.row.node)" class="k-visually-hidden"
                >({{ rowWords(part.row.node) }})</span
              >
              <span v-if="refusal(part.row.node)" class="k-tree-grid__refusal">
                <span>{{ refusal(part.row.node)!.reason }}</span>
                <CodeBadge
                  v-if="refusal(part.row.node)!.code"
                  :code="refusal(part.row.node)!.code!"
                />
              </span>
              <IconButton
                v-if="props.addLabel?.(part.row.node)"
                icon="plus"
                size="sm"
                tabindex="-1"
                class="k-tree-grid__add"
                :label="props.addLabel(part.row.node)!"
                v-bind="props.addBlocked?.(part.row.node) ?? {}"
                @click.stop="emit('add', part.row.key)"
              />
              <CellError
                v-if="messageOf(part.row.node, treeColumn)"
                :id="errorId(part.index, '')"
                :error="messageOf(part.row.node, treeColumn)!.error"
                :code="messageOf(part.row.node, treeColumn)!.code"
                :anchor="errorAnchor(part.row.node, treeColumn)"
                @leave="errors.hovered.value = null"
                @dismiss="errors.dismiss()"
                @back="cellElement({ row: part.row.key, column: '' })?.focus()"
                @blur="errors.focused.value = null"
              />
            </div>
            <div
              v-for="column in shown"
              :key="column.id"
              role="gridcell"
              class="k-tree-grid__cell"
              :class="{ 'k-mono': column.mono }"
              :data-row="part.row.key"
              :data-column="column.id"
              :data-align="column.align"
              :data-priority="priority(column)"
              :data-width="column.width ?? 'md'"
              :data-state="stateOf(part.row.node, column)"
              :data-editing="isEditing(part.row.key, column.id) ? 'true' : undefined"
              :aria-colindex="colIndex(column)"
              :aria-readonly="
                props.editable && !canEdit(part.row.node, column) ? 'true' : undefined
              "
              :aria-busy="stateOf(part.row.node, column) === 'pending' ? 'true' : undefined"
              :aria-invalid="messageOf(part.row.node, column) ? 'true' : undefined"
              :aria-describedby="
                messageOf(part.row.node, column) ? errorId(part.index, column.id) : undefined
              "
              :tabindex="
                isEditing(part.row.key, column.id)
                  ? undefined
                  : isStop(part.row.key, column.id)
                    ? 0
                    : -1
              "
              @pointerenter="errors.enter(cellKey(part.row.key, column.id), $event)"
              @pointerleave="onCellLeave(part.index, column.id, $event)"
            >
              <div
                v-if="isEditing(part.row.key, column.id)"
                class="k-field-frame k-tree-grid__editor"
                data-size="md"
                :data-invalid="messageOf(part.row.node, column) ? 'true' : undefined"
              >
                <input
                  v-model="draft"
                  class="k-field-frame__input"
                  :class="{ 'k-mono': column.mono }"
                  type="text"
                  spellcheck="false"
                  :aria-label="editorLabel(part.row.node, column)"
                  :aria-invalid="messageOf(part.row.node, column) ? 'true' : undefined"
                  :aria-describedby="
                    messageOf(part.row.node, column) ? errorId(part.index, column.id) : undefined
                  "
                />
              </div>
              <template v-else>
                <span class="k-tree-grid__value"
                  ><NoValue v-if="cellOf(part.row.node, column) === null" /><template v-else>{{
                    cellOf(part.row.node, column)
                  }}</template></span
                >
                <Icon
                  v-if="stateOf(part.row.node, column) === 'pending'"
                  name="loader-circle"
                  class="k-spinner k-tree-grid__mark"
                />
                <Icon
                  v-else-if="stateOf(part.row.node, column) === 'error'"
                  name="circle-x"
                  class="k-tree-grid__mark k-tree-grid__mark--ko"
                />
                <Icon
                  v-else-if="stateOf(part.row.node, column) === 'review'"
                  name="eye"
                  class="k-tree-grid__mark"
                />
                <Icon
                  v-else-if="stateOf(part.row.node, column) === 'draft'"
                  name="draft"
                  class="k-tree-grid__mark"
                />
                <span v-if="stateWord(part.row.node, column)" class="k-visually-hidden"
                  >({{ stateWord(part.row.node, column) }})</span
                >
              </template>
              <CellError
                v-if="messageOf(part.row.node, column)"
                :id="errorId(part.index, column.id)"
                :error="messageOf(part.row.node, column)!.error"
                :code="messageOf(part.row.node, column)!.code"
                :anchor="errorAnchor(part.row.node, column)"
                @leave="errors.hovered.value = null"
                @dismiss="errors.dismiss()"
                @back="cellElement({ row: part.row.key, column: column.id })?.focus()"
                @blur="errors.focused.value = null"
              />
            </div>
          </div>
        </template>
      </div>
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
    scroll-padding-block-start: var(--k-header-height);
    scroll-padding-inline-start: var(--k-tree-grid-key, 0);
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  /* Virtualised: the rows scroll in the tree, a list's height at most (a host's bound on
   * the root holds it lower). */
  .k-tree-grid[data-virtual] {
    max-block-size: var(--k-size-list-max);
  }

  /* The height of the rows left out of the document (virtualised). */
  .k-tree-grid__gap {
    block-size: var(--k-tree-grid-gap);
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
  .k-tree-grid__row[data-state="draft"] > :first-child,
  .k-tree-grid__row[data-state="moved"] > :first-child,
  .k-tree-grid__row[data-state="preview"] > :first-child {
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

  /* The row that reads a page: one cell over the whole row. */
  .k-tree-grid__more > .k-tree-grid__node {
    grid-column: 1 / -1;
  }

  .k-tree-grid__more-label {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
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

  /* Each row is a grid of its own: every cell holds its column's token, so that the
   * `minmax(auto, token)` tracks (grid.ts) keep the same width in every row. */
  .k-tree-grid [data-width="xs"] {
    min-inline-size: var(--k-size-col-xs);
  }

  .k-tree-grid [data-width="sm"] {
    min-inline-size: var(--k-size-col-sm);
  }

  .k-tree-grid [data-width="md"] {
    min-inline-size: var(--k-size-col-md);
  }

  .k-tree-grid [data-width="lg"] {
    min-inline-size: var(--k-size-col-lg);
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

  /* The chevron: the pointer's target of opening, 24 px around a 16 px icon, laid out at
   * the icon's size (WCAG 2.5.8). */
  .k-tree-grid__twisty {
    display: inline-flex;
    flex: none;
    box-sizing: content-box;
    inline-size: var(--k-icon-size);
    padding: var(--k-space-1);
    margin: calc(var(--k-space-1) * -1);
    color: var(--k-text-muted);
  }

  .k-tree-grid__icon,
  .k-tree-grid__key {
    color: var(--k-text-muted);
  }

  .k-tree-grid__label,
  .k-tree-grid__value {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* The key of a node keeps its room: the label gives way first. */
  .k-tree-grid__key {
    flex: none;
  }

  .k-tree-grid__cell > .k-tree-grid__editor {
    flex: 1;
    align-self: stretch;
    min-inline-size: 0;
    min-block-size: 0;
  }

  .k-tree-grid__row:hover {
    background-color: var(--k-hover);
  }

  .k-tree-grid__row[aria-selected="true"] {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-tree-grid__row[data-state="draft"],
  .k-tree-grid__row[data-state="moved"],
  .k-tree-grid__row[data-state="preview"] {
    box-shadow: var(--k-shadow-inset-accent);
  }

  /* A node a draft moves: at its present place, muted (it leaves); at its place after the
   * draft, a dashed outline (not there yet). Each also has its mark and its words. */
  .k-tree-grid__row[data-state="moved"] {
    color: var(--k-text-muted);
  }

  /* A target during a drag, or a preview: an outline over the whole row, the sticky tree
   * cell included (a layer of the row above its cells, the header still above it). */
  .k-tree-grid__row[data-drop],
  .k-tree-grid__row[data-state="preview"] {
    position: relative;
  }

  /* Never over the focus ring of one of its cells (as in `Tree`); the outline of a target
   * during a drag wins over it (`:where`: no more weight than the row's class). */
  .k-tree-grid__row:where([data-state="preview"]:not(:has(:focus-visible)))::after {
    outline: var(--k-border-width) dashed var(--k-accent);
    outline-offset: calc(var(--k-border-width) * -1);
  }

  .k-tree-grid__row[data-drop]::after,
  .k-tree-grid__row[data-state="preview"]::after {
    position: absolute;
    inset: 0;
    z-index: var(--k-z-sticky);
    pointer-events: none;
    content: "";
  }

  .k-tree-grid__row[data-drop="ok"] {
    background-color: var(--k-accent-soft);
  }

  .k-tree-grid__row[data-drop="ok"]::after {
    outline: var(--k-focus-width) solid var(--k-accent);
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-tree-grid__row[data-drop="refused"]::after {
    outline: var(--k-border-width) dashed var(--k-state-ko-fg);
    outline-offset: calc(var(--k-border-width) * -1);
  }

  .k-tree-grid__row[data-move="pending"],
  .k-tree-grid__row[data-move="cut"] {
    color: var(--k-text-muted);
  }

  .k-tree-grid__row[data-move="cut"] .k-tree-grid__label {
    text-decoration: underline dashed;
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
    block-size: var(--k-space-3);
    border-radius: var(--k-radius-sm);
  }
}
</style>
