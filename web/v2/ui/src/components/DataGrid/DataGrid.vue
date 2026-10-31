<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts" generic="R extends Record<string, unknown>">
// The data grid of kartenn (APG Grid, written by kartenn: Reka has none; design
// « Inventaire », line 20; « Structure avant style »; REQ-020-18, 19). One CSS grid: the
// columns are set once on the grid, every row (header included) is a subgrid of it, and
// the header sticks to the top of the same scroll container: head and cells align by
// construction, whatever the scroll bar and the elastic columns (REQ-020-34).
// Keyboard of the pattern over a roving focus: arrows, Home, End, Ctrl+Home, Ctrl+End,
// Page Up and Page Down (a screen of rows); on a header, Enter or Space sorts (`aria-sort`)
// and Alt+Down asks for the filter of the column (the host opens it; one filter model with
// its bar, decision A7). Selection (`selectable`): Space, Shift+Space, Shift+Up and
// Shift+Down (a range from the anchor), Ctrl+A; several rows get a column of checkboxes
// (the pointer's way), one row a click.
// Virtualised beyond `VIRTUAL_ROWS` rows (`@tanstack/vue-virtual`): only the rows near the
// view are in the document, between gaps of the height of the others, measured; the row
// of the focus and the one of the editor stay mounted whatever the scroll; the keys move
// over the indexes of the rows, never over the rendered ones (no press lost). The window
// is emitted (`window`: `end` among the loaded rows, as the shell's `useNextPage` reads it).
// In-place editing (design « Contrat d'édition »): the host says which cells are editable
// and checks the text (`editable`, `validate`); Enter or F2 opens the editor, Escape
// cancels, Enter or Tab commits (`commit`), and the host renders the cell's state then
// (pending, draft, review, error with its code, its message in a floating layer that
// never resizes the cell: `CellError`; an error that follows a commit is said by an alert).
// Columns resizable by the keyboard (`resizable`: F2 on a header reaches its handle, a
// Window Splitter: arrows, Home, End) and the pointer (drag, or a click for the next
// token width, WCAG 2.5.7). A column of an extension holds a custom element per row.
// Adaptation to the width (design « Adaptation à la largeur »): the root is a container;
// the grid scrolls in its own region (`scrollbar-gutter: stable`), its key column sticks
// at the inline start, and under a container `md` (`lg`) the columns of priority 2 (3)
// hide, a column chooser above the grid showing them again. The arrows skip what is
// hidden. The class and style of the host go to the root, other attributes to the grid.
// With `stack` (a simple list), under a container `sm` the rows stack instead: the key on
// a line with the row's actions, then one line per attribute, the column's label before
// the value (generated content, its alternative text empty: the column header names the
// cell already), labels and values aligned by a subgrid of the row; every column shows,
// the header is a strip of controls. Still a grid of rows and cells, same keys.
import { useVirtualizer } from "@tanstack/vue-virtual";
import {
  computed,
  h,
  nextTick,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref,
  useAttrs,
  useId,
  useSlots,
  watch,
  type ComponentPublicInstance,
  type FunctionalComponent,
} from "vue";
import { t } from "../../i18n.ts";
import Checkbox from "../Checkbox/Checkbox.vue";
import Icon from "../Icon/Icon.vue";
import ColumnChooser from "../internal/ColumnChooser.vue";
import CellError from "./CellError.vue";
import { useCellEditor, useCommitWatch } from "../internal/cellEditor.ts";
import { isRtl, moveInGrid } from "../internal/gridKeys.ts";
import { bodyParts, keptIndexes, rowsInView, type BodyPart } from "../internal/virtualRows.ts";
import Skeleton from "../Skeleton/Skeleton.vue";
import {
  columnTemplates,
  nextPresetWidth,
  nextSort,
  priorityOf,
  renderedColumns,
  selectRange,
  SELECT_COLUMN,
  WIDTH_MAX,
  WIDTH_STEP,
  WIDTH_STEP_LARGE,
  type CellRef,
  type CellState,
  type ColumnVisibility,
  type ColumnWidths,
  type GridColumn,
  type GridCommit,
  type GridSort,
  type GridWindow,
  type RowState,
} from "./grid.ts";

defineOptions({ inheritAttrs: false });

const props = withDefaults(
  defineProps<{
    label: string;
    columns: GridColumn[];
    /** The rows the grid holds (the loaded ones of a paged list). */
    rows: R[];
    /** The field of a row that identifies it. */
    rowKey: keyof R & string;
    /**
     * Rows can be selected (`aria-selected`): `true` several (`aria-multiselectable`, a
     * column of checkboxes; Space or Shift+Space toggles the row of the focus, Shift+Up and
     * Shift+Down extend from the anchor, Ctrl+A selects all or none), `"single"` one (Space
     * or a click selects it).
     */
    selectable?: boolean | "single";
    cellState?: ((row: R, column: GridColumn) => CellState | undefined) | undefined;
    rowState?: ((row: R) => RowState | undefined) | undefined;
    /** The columns whose filter is set (the host keeps the filters). */
    filtered?: string[];
    /** The rows are coming: skeleton rows in the cells, the grid busy. */
    loading?: boolean;
    loadingRows?: number;
    /**
     * A simple list (a key, two or three attributes): under a container `sm`, its rows
     * stack, the key on top, each attribute on a line of its own after its column's label;
     * the header becomes a strip of sort and filter controls.
     */
    stack?: boolean;
    /**
     * Which cells the user may edit (rights, present, `as_of`): the grid never decides it
     * (REQ-020-19). Without it, the grid is read only (`aria-readonly`), and an editor the
     * host opens (`editing`) keeps all its keys: the host drives it.
     */
    editable?: ((row: R, column: GridColumn) => boolean) | undefined;
    /** The local check of the text of an editor: its error, or nothing (contract types). */
    validate?: ((row: R, column: GridColumn, value: string) => string | undefined) | undefined;
    /**
     * The rows of the whole list when the grid holds only some (a paged list), the header
     * left out: `aria-rowcount` is this plus one; `-1` while unknown.
     */
    rowCount?: number | undefined;
    /**
     * Where the first row given stands in the whole list (a paged list whose first pages
     * the cache dropped): `aria-rowindex` counts from it; the `window` event does not
     * (its indexes are among the rows given, as `useNextPage` reads them).
     */
    rowOffset?: number;
    /** The columns take a width from the keyboard and the pointer (`columnWidths`). */
    resizable?: boolean;
    /** The properties of the elements of the extension columns, beside `row`. */
    elementProps?: ((row: R, column: GridColumn) => Record<string, unknown>) | undefined;
  }>(),
  {
    selectable: false,
    cellState: undefined,
    rowState: undefined,
    filtered: () => [],
    loadingRows: 5,
    editable: undefined,
    validate: undefined,
    rowCount: undefined,
    rowOffset: 0,
    elementProps: undefined,
  },
);
/** The sort of the grid; a sortable header changes it. */
const sort = defineModel<GridSort | undefined>("sort", { default: undefined });
/** The cell that holds the roving focus (the first header when unset). */
const active = defineModel<CellRef | undefined>("active", { default: undefined });
/** The keys of the selected rows (`selectable`). */
const selected = defineModel<string[]>("selected", { default: () => [] });
/** The cell whose editor is open (Enter or F2 opens it; the host may open one too). */
const editing = defineModel<CellRef | undefined>("editing", { default: undefined });
/** The widths the user gave the columns (`resizable`). */
const widths = defineModel<ColumnWidths>("columnWidths", { default: () => ({}) });
const emit = defineEmits<{
  /** Alt+Down or a click on the funnel of a filterable header. */
  filter: [column: string];
  /** Enter on a cell of the body that is neither editable nor holds a widget. */
  activate: [cell: CellRef];
  /** An editor commits its text (Enter, Tab, or the focus leaving it), checked locally. */
  commit: [commit: GridCommit];
  /** The rows in the document changed (virtualised or not). */
  window: [window: GridWindow];
}>();
const slots = useSlots();

/** The user's choice of columns (column chooser). */
const visibility = defineModel<ColumnVisibility>("columnVisibility", { default: () => ({}) });
const attrs = useAttrs();
const rootAttrs = computed(() => ({ class: attrs.class, style: attrs.style }));
const gridAttrs = computed(() =>
  Object.fromEntries(Object.entries(attrs).filter(([key]) => key !== "class" && key !== "style")),
);

const grid = ref<HTMLElement | null>(null);
const root = ref<HTMLElement | null>(null);
/** Several rows selectable: a column of checkboxes first. */
const checkboxes = computed(() => props.selectable === true);
/** The columns rendered (the user's hidden ones left out), in order. */
const shown = computed(() => renderedColumns(props.columns, visibility.value));
const template = computed(() =>
  columnTemplates(props.columns, visibility.value, widths.value, checkboxes.value),
);
/** The priority a cell carries for the container queries; none for an always shown one. */
const priority = (column: GridColumn) => {
  const p = priorityOf(props.columns, column, visibility.value);
  return p > 1 ? p : undefined;
};
/** The index of a column among all of them (`aria-colindex`), hidden ones counted. */
const colIndex = (column: GridColumn) =>
  props.columns.indexOf(column) + 1 + (checkboxes.value ? 1 : 0);
/** The columns on screen now: the rendered ones whose header takes room. */
function onScreen(): GridColumn[] {
  const visible = shown.value.filter((column) => {
    const header = headerOf(column.id);
    return !!header && header.getClientRects().length > 0;
  });
  return visible.length > 0 ? visible : shown.value;
}
/** The columns the focus moves over: the checkboxes first, then the ones on screen. */
const navColumns = (): string[] => [
  ...(checkboxes.value ? [SELECT_COLUMN] : []),
  ...onScreen().map((c) => c.id),
];
const headerOf = (id: string) =>
  grid.value?.querySelector<HTMLElement>(
    `[role="columnheader"][data-column="${CSS.escape(id)}"]`,
  ) ?? null;
const keyOf = (row: R) => String(row[props.rowKey]);
/** Where each key stands among the rows. */
const indexOfKey = computed(() => new Map(props.rows.map((row, i) => [keyOf(row), i])));
const rowOf = (key: string) => {
  const i = indexOfKey.value.get(key);
  return i === undefined ? undefined : props.rows[i];
};
const columnOf = (id: string) => props.columns.find((c) => c.id === id);
const selectedSet = computed(() => new Set(selected.value));
const isSelected = (row: R) => selectedSet.value.has(keyOf(row));
const allState = computed<boolean | "mixed">(() =>
  props.rows.length > 0 && props.rows.every(isSelected)
    ? true
    : selected.value.length > 0
      ? "mixed"
      : false,
);
const ariaSort = (column: GridColumn) =>
  !column.sortable ? undefined : sort.value?.column === column.id ? sort.value.direction : "none";
const isFiltered = (column: GridColumn) => props.filtered.includes(column.id);
/** A header with a control (filter, handle) is named by its label and state, not by them. */
const headerName = (column: GridColumn) =>
  isFiltered(column) ? `${column.label} (${t("filtered")})` : column.label;
const namedHeader = (column: GridColumn) => column.filterable || props.resizable;
/** The width a cell holds: its token, or none once the user gave the column a width. */
const widthOf = (column: GridColumn) =>
  widths.value[column.id] !== undefined ? "set" : (column.width ?? "md");
/** The width a cell of a resized column holds (CSSOM, a property of its own). */
const widthStyle = (column: GridColumn) =>
  widths.value[column.id] !== undefined
    ? { "--k-data-grid-width": `${Math.round(widths.value[column.id]!)}px` }
    : undefined;

/** The keys of the rows the focus moves over: the header (""), then the body. */
const rowKeys = computed(() => ["", ...(props.loading ? [] : props.rows.map(keyOf))]);
const current = computed<CellRef>(
  () => active.value ?? { row: "", column: props.columns[0]?.id ?? "" },
);
const tabindex = (row: string, column: string) =>
  current.value.row === row && current.value.column === column ? 0 : -1;
const isEditing = (row: R, column: GridColumn) =>
  editing.value?.row === keyOf(row) && editing.value.column === column.id;
const canEdit = (row: R, column: GridColumn) => !column.element && !!props.editable?.(row, column);

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

// ——— Virtualisation ———

/** Rows beyond which the grid is virtualised (as `DiffList`, design). */
const VIRTUAL_ROWS = 200;
const OVERSCAN = 10;
const virtual = computed(() => !props.loading && props.rows.length > VIRTUAL_ROWS);
/** The height of a row before it is measured: the row height of the density. */
const estimate = ref(28);
/** The height of the header, above the rows in the scroll container. */
const headHeight = ref(30);
const virtualizer = useVirtualizer<HTMLElement, HTMLElement>(
  computed(() => ({
    count: virtual.value ? props.rows.length : 0,
    getScrollElement: () => grid.value,
    estimateSize: () => estimate.value,
    getItemKey: (index: number) => (props.rows[index] ? keyOf(props.rows[index]) : index),
    overscan: OVERSCAN,
    scrollMargin: headHeight.value,
  })),
);
function measure(el: Element | ComponentPublicInstance | null): void {
  if (virtual.value && el instanceof HTMLElement) virtualizer.value.measureElement(el);
}
/** The rows and gaps of the body: every row, or the window and the rows kept mounted. */
type Part = { index: number; row: R; key: string } | { gap: number; key: string };
const parts = computed<Part[]>(() => {
  const withRow = (part: BodyPart): Part => {
    if (!("index" in part)) return part;
    const row = props.rows.at(part.index)!;
    return { index: part.index, row, key: keyOf(row) };
  };
  if (!virtual.value) return props.rows.map((_, index) => withRow({ index }));
  const v = virtualizer.value;
  const keep = keptIndexes(
    v.getVirtualItems().map((item) => item.index),
    [current.value.row, editing.value?.row],
    (key) => indexOfKey.value.get(key),
  );
  const sizes = v.measurementsCache;
  const at = (i: number) => sizes[i]?.start ?? headHeight.value + i * estimate.value;
  const end = (i: number) => sizes[i]?.end ?? at(i) + estimate.value;
  return bodyParts(keep, props.rows.length, at, end, at(0)).map(withRow);
});
/** The rows in view now, by their boxes (virtualised or not; the page may scroll too). */
const inView = () =>
  grid.value
    ? rowsInView(
        grid.value,
        grid.value.querySelectorAll<HTMLElement>(".k-data-grid__body > [role='row']"),
        headHeight.value,
      )
    : null;
/** The rows of one screen (Page Up, Page Down): the rows wholly in view, less one. */
function pageRows(): number {
  const seen = inView();
  if (seen && seen.count > 1) return seen.count - 1;
  const el = grid.value;
  if (!el) return 10;
  const view = Math.min(el.clientHeight, window.innerHeight) - headHeight.value;
  return Math.max(1, Math.floor(view / estimate.value) - 1);
}
/**
 * The window of the grid (`window`): the rows in view, among the rows given; read again
 * when anything scrolls (the grid, the page, a panel), the window resizes or the rows
 * change, once per frame.
 */
let last = "";
let frame = 0;
function emitWindow(): void {
  frame = 0;
  if (props.loading) return;
  const seen = inView();
  const view: GridWindow = seen
    ? { start: seen.first, end: seen.last + 1, size: Math.max(1, seen.count) }
    : { start: 0, end: 0, size: 0 };
  const key = `${view.start} ${view.end} ${view.size}`;
  if (key === last) return;
  last = key;
  emit("window", view);
}
function scheduleWindow(): void {
  if (!frame) frame = requestAnimationFrame(emitWindow);
}
onMounted(() => {
  document.addEventListener("scroll", scheduleWindow, { capture: true, passive: true });
  window.addEventListener("resize", scheduleWindow, { passive: true });
  scheduleWindow();
});
onBeforeUnmount(() => {
  document.removeEventListener("scroll", scheduleWindow, { capture: true });
  window.removeEventListener("resize", scheduleWindow);
  if (frame) cancelAnimationFrame(frame);
});
watch(
  () => [props.rows, props.loading],
  () => void nextTick(scheduleWindow),
);

// ——— Errors of cells ———

// The error of a cell opens in a floating layer (`CellError`) while the cell is hovered or
// holds the focus, the pointer first, and while its editor is open; Escape closes it until
// the cell has neither (WCAG 1.4.13). The cell keeps its size; its message is named by
// `aria-describedby` (of the cell, and of its editor).
const uid = useId();
const errorId = (r: number, c: number) => `k-data-grid-${uid}-error-${r}-${c}`;
const cellKey = (row: string, column: string) => `${row}\n${column}`;
const hovered = ref<string | null>(null);
const focused = ref<string | null>(null);
const dismissed = ref<string | null>(null);
/** Whether the cell of key `k` has an error now (it may come after the focus, by a commit). */
function errorAt(k: string): boolean {
  const [r, c] = k.split("\n");
  const row = rowOf(r!);
  const column = columnOf(c!);
  return !!row && !!column && hasError(row, column);
}
const shownError = computed(
  () =>
    [hovered.value, focused.value].find((k) => k !== null && k !== dismissed.value && errorAt(k)) ??
    null,
);
/** The error a cell shows: its editor's (the host's check, `validate`), else the host's. */
function messageOf(row: R, column: GridColumn): { error: string; code?: string } | undefined {
  if (isEditing(row, column) && draftError.value) return { error: draftError.value };
  return errorOf(row, column);
}
const hasError = (row: R, column: GridColumn) => !!messageOf(row, column);
const errorShown = (row: R, column: GridColumn) =>
  hasError(row, column) &&
  (isEditing(row, column) || shownError.value === cellKey(keyOf(row), column.id));
/** The cell under an open error, for its layer to stand against. */
function errorAnchor(row: R, column: GridColumn): HTMLElement | null {
  if (!errorShown(row, column)) return null;
  return cellElement({ row: keyOf(row), column: column.id });
}
const cellElement = (cell: CellRef) =>
  grid.value?.querySelector<HTMLElement>(
    `[data-row="${CSS.escape(cell.row)}"][data-column="${CSS.escape(cell.column)}"]`,
  ) ?? null;
watch([hovered, focused], ([h, f]) => {
  if (dismissed.value !== null && h !== dismissed.value && f !== dismissed.value)
    dismissed.value = null;
});
/** A touch pointer leaves its target when the finger lifts: the focus speaks for it. */
function onCellEnter(row: R, column: GridColumn, event: PointerEvent): void {
  if (event.pointerType !== "touch" && hasError(row, column))
    hovered.value = cellKey(keyOf(row), column.id);
}
function onCellLeave(r: number, c: number, event: PointerEvent): void {
  if (event.pointerType === "touch") return;
  // Onto its message (adjacent): still hovered.
  const to = event.relatedTarget as Node | null;
  if (to && document.getElementById(errorId(r, c))?.contains(to)) return;
  hovered.value = null;
}
function onFocusout(event: FocusEvent): void {
  const to = event.relatedTarget as HTMLElement | null;
  // Into the message of the cell (its copy button, by Enter or F2): still focused.
  if (to?.closest(".k-cell-error")) return;
  if (!grid.value?.contains(to)) focused.value = null;
  editor.onFocusout(event);
}
/** The widgets of the open message of a cell (the copy button of its code). */
function errorWidgetsOf(cell: HTMLElement): HTMLElement[] {
  const id = cell.dataset.errorOpen ? cell.getAttribute("aria-describedby") : null;
  const message = id ? document.getElementById(id) : null;
  return message ? [...message.querySelectorAll<HTMLElement>("button")] : [];
}
/** Back from the message to its cell. */
function focusCell(r: number, c: number): void {
  grid.value
    ?.querySelector<HTMLElement>(`[aria-describedby="${CSS.escape(errorId(r, c))}"]`)
    ?.focus();
}

/**
 * The alert of the grid: the error the host gives a cell after its commit, or the one of
 * the local check, said once (a region present empty before, then filled).
 */
const announcement = ref("");
async function announce(text: string): Promise<void> {
  announcement.value = "";
  await nextTick();
  announcement.value = text;
}
const sayError = (column: GridColumn, key: string, error: { error: string; code?: string }) =>
  `${column.label}, ${key} : ${error.error}${error.code ? ` (${error.code})` : ""}`;
/** The commits not yet settled by the host: an error it gives then is said once. */
const commits = useCommitWatch(
  (cell) => {
    const row = rowOf(cell.row);
    const column = columnOf(cell.column);
    return row && column ? props.cellState?.(row, column) : undefined;
  },
  (cell) => (rowOf(cell.row) ? textOf(cell) : undefined),
  (cell, error) => {
    const column = columnOf(cell.column);
    if (column) void announce(sayError(column, cell.row, error));
  },
);

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
  () =>
    props.rowState ? props.rows.map((row) => [keyOf(row), props.rowState!(row)] as const) : [],
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

/** The focus to the cell of the roving stop, rendered by then (kept mounted). */
async function focusActive(): Promise<void> {
  await nextTick();
  cellElement(current.value)?.focus();
}

/** The cell under `target`, as the grid keys it. */
function cellOf(target: EventTarget | null): CellRef | null {
  const cell = (target as HTMLElement | null)?.closest<HTMLElement>("[data-column]");
  return cell ? { row: cell.dataset.row ?? "", column: cell.dataset.column! } : null;
}

function onFocusin(event: FocusEvent): void {
  const target = event.target as HTMLElement;
  // The checkbox of a row is the pointer's target: the focus stays on its cell, whose
  // keys select (Space).
  const box = target.closest<HTMLElement>(".k-data-grid__select")?.querySelector("input");
  if (box && box === target) {
    target.closest<HTMLElement>("[data-column]")?.focus();
    return;
  }
  const cell = cellOf(target);
  if (cell && (cell.row !== current.value.row || cell.column !== current.value.column))
    active.value = cell;
  focused.value = cell && cell.row !== "" ? cellKey(cell.row, cell.column) : null;
}

/** The widgets inside a cell (a link, a copy button): out of the tab order, reached by keys. */
const WIDGETS = "a[href], button, input, select, textarea, [tabindex], [contenteditable]";
function widgetsOf(cell: HTMLElement): HTMLElement[] {
  return [...cell.querySelectorAll<HTMLElement>(WIDGETS)].filter(
    (el) => !el.closest(".k-data-grid__editor"),
  );
}
/** A widget of a cell (never the cell itself, nor an editor) leaves the tab order. */
function tame(el: Element): void {
  if (!(el instanceof HTMLElement) || !el.matches(WIDGETS) || el.matches("[data-column]")) return;
  if (!el.parentElement?.closest("[data-column]") || el.closest(".k-data-grid__editor")) return;
  if (el.tabIndex !== -1) el.tabIndex = -1;
}
/** The grid keeps one tab stop: the widgets of its cells leave the tab order (APG Grid). */
function tameWidgets(root: ParentNode | null = grid.value): void {
  for (const el of root?.querySelectorAll(WIDGETS) ?? []) tame(el);
}
onMounted(() => tameWidgets());
onUpdated(() => tameWidgets());
// The element of an extension column renders when it likes (its plugin, after a read):
// its controls leave the tab order as they come or turn focusable, in the light DOM (ADR
// 0038, § 4), each mutation looked at in its own subtree.
let mutations: MutationObserver | undefined;
function onMutations(records: MutationRecord[]): void {
  for (const record of records) {
    if (record.type === "attributes") tame(record.target as Element);
    for (const node of record.addedNodes) {
      if (!(node instanceof Element)) continue;
      tame(node);
      tameWidgets(node);
    }
  }
}
const extended = computed(() => props.columns.some((column) => column.element));
function observeExtensions(): void {
  mutations?.disconnect();
  mutations = undefined;
  if (!extended.value || !grid.value || typeof MutationObserver === "undefined") return;
  mutations = new MutationObserver(onMutations);
  mutations.observe(grid.value, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["tabindex", "href", "contenteditable"],
  });
}
onMounted(observeExtensions);
watch(extended, observeExtensions);
onBeforeUnmount(() => mutations?.disconnect());

// ——— Selection ———

/** The row a range starts from (Shift with an arrow), and what was selected then. */
let anchor: { key: string; base: string[] } | null = null;
function toggleRow(key: string): void {
  const on = selected.value.includes(key);
  if (props.selectable === "single") selected.value = [key];
  else selected.value = on ? selected.value.filter((k) => k !== key) : [...selected.value, key];
  anchor = { key, base: selected.value };
}
function selectAll(on: boolean): void {
  selected.value = on ? props.rows.map(keyOf) : [];
  anchor = null;
}
/** A click on the checkbox of a row: Shift extends from the anchor. */
let shiftClick = false;
/** Whether the click that reaches a checkbox holds Shift (heard before the checkbox). */
function noteShift(event: MouseEvent): void {
  shiftClick = event.shiftKey;
}
async function onBox(key: string): Promise<void> {
  if (shiftClick && anchor) {
    selected.value = selectRange(rowKeys.value.slice(1), anchor.base, anchor.key, key);
  } else toggleRow(key);
  // The native box follows the model: a range that keeps the clicked row selected leaves
  // its model unchanged, while the click turned the box off (no patch would turn it on).
  await nextTick();
  for (const box of grid.value?.querySelectorAll<HTMLInputElement>(
    ".k-data-grid__body .k-data-grid__select input",
  ) ?? []) {
    const row = box.closest<HTMLElement>("[data-row]")?.dataset.row;
    if (row !== undefined) box.checked = selectedSet.value.has(row);
  }
}
/** A click on a row of a single selection selects it (not on a widget or an editor). */
function onBodyClick(event: MouseEvent): void {
  if (props.selectable !== "single") return;
  const target = event.target as HTMLElement;
  if (target.closest(".k-data-grid__editor, a, button, input")) return;
  const cell = cellOf(target);
  if (cell && cell.row !== "") toggleRow(cell.row);
}

// ——— Editing ———

// The contract (`internal/cellEditor.ts`): the grid's cells, focus and alert.
const textOf = (cell: CellRef) => String(rowOf(cell.row)?.[cell.column] ?? "");
const editor = useCellEditor({
  editing,
  textOf,
  driven: () => !!props.editable,
  check: (cell, value) => {
    const row = rowOf(cell.row);
    const column = columnOf(cell.column);
    return row && column ? props.validate?.(row, column, value) : undefined;
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
    const column = columnOf(cell.column);
    if (column) void announce(`${sayError(column, cell.row, { error })} ${t("editEscape")}`);
  },
});
const draft = editor.draft;
const draftError = editor.error;
const editorLabel = (row: R, column: GridColumn) => `${column.label} — ${keyOf(row)}`;
async function startEditing(cell: CellRef): Promise<void> {
  active.value = cell;
  await editor.start(cell);
}
function onDblclick(event: MouseEvent): void {
  const target = event.target as HTMLElement;
  if (target.closest(".k-data-grid__editor")) return;
  const cell = cellOf(target);
  const row = cell ? rowOf(cell.row) : undefined;
  const column = cell ? columnOf(cell.column) : undefined;
  if (cell && row && column && canEdit(row, column)) void startEditing(cell);
}

// ——— Width of the columns ———

/** The measured width of each header, and the narrowest it can be (its min-content). */
const measured = ref<Record<string, { now: number; min: number }>>({});
function minWidthOf(header: HTMLElement): number {
  const style = getComputedStyle(header);
  // The children in the flow (an icon is an SVG element: its box, not `offsetWidth`).
  const inFlow = [...header.children].filter((el) => getComputedStyle(el).position !== "absolute");
  const gap = parseFloat(style.columnGap) || 0;
  const content = inFlow.reduce(
    (sum, el) =>
      sum +
      (el.classList.contains("k-data-grid__label")
        ? el.scrollWidth
        : el.getBoundingClientRect().width),
    0,
  );
  const padding =
    (parseFloat(style.paddingInlineStart) || 0) + (parseFloat(style.paddingInlineEnd) || 0);
  return Math.ceil(content + gap * Math.max(0, inFlow.length - 1) + padding);
}
function measureHeaders(): void {
  if (!props.resizable || !grid.value) return;
  const next: Record<string, { now: number; min: number }> = {};
  for (const column of shown.value) {
    const header = headerOf(column.id);
    if (header && header.getClientRects().length > 0)
      next[column.id] = { now: Math.round(header.offsetWidth), min: minWidthOf(header) };
  }
  measured.value = next;
}
async function setWidth(column: GridColumn, width: number): Promise<void> {
  const min = measured.value[column.id]?.min ?? 0;
  const asked = Math.round(Math.max(min, Math.min(WIDTH_MAX, width)));
  if (!Number.isFinite(asked)) return;
  widths.value = { ...widths.value, [column.id]: asked };
  await nextTick();
  measureHeaders();
  // Narrower than its header can be: the header holds it, and that is its least width.
  const now = measured.value[column.id]?.now;
  if (now !== undefined && now > asked) {
    widths.value = { ...widths.value, [column.id]: now };
    measured.value = { ...measured.value, [column.id]: { now, min: now } };
  }
  measureKey();
}
function onHandleKeydown(column: GridColumn, event: KeyboardEvent): void {
  if (!measured.value[column.id]) measureHeaders();
  const now = measured.value[column.id]?.now ?? 0;
  const rtl = isRtl(grid.value!);
  const step = event.shiftKey ? WIDTH_STEP_LARGE : WIDTH_STEP;
  const wider = rtl ? "ArrowLeft" : "ArrowRight";
  const narrower = rtl ? "ArrowRight" : "ArrowLeft";
  const next =
    event.key === wider
      ? now + step
      : event.key === narrower
        ? now - step
        : event.key === "Home"
          ? 0
          : event.key === "End"
            ? WIDTH_MAX
            : null;
  if (next === null) return;
  event.preventDefault();
  event.stopPropagation();
  void setWidth(column, next);
}
/** The token widths of the columns, as the grid resolves them (a click's steps). */
function presets(): number[] {
  const style = grid.value ? getComputedStyle(grid.value) : null;
  return ["xs", "sm", "md", "lg"].map(
    (size) => parseFloat(style?.getPropertyValue(`--k-size-col-${size}`) ?? "") || 0,
  );
}
let drag: { column: GridColumn; x: number; width: number; sign: number; moved: boolean } | null =
  null;
function onHandleDown(column: GridColumn, event: PointerEvent): void {
  if (event.button !== 0) return;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  measureHeaders();
  drag = {
    column,
    x: event.clientX,
    width: measured.value[column.id]?.now ?? 0,
    sign: isRtl(grid.value!) ? -1 : 1,
    moved: false,
  };
}
function onHandleMove(event: PointerEvent): void {
  if (!drag) return;
  const dx = (event.clientX - drag.x) * drag.sign;
  if (Math.abs(dx) >= 3) drag.moved = true;
  if (drag.moved) void setWidth(drag.column, drag.width + dx);
}
function onHandleUp(event: PointerEvent): void {
  if (!drag) return;
  (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  // A click without a drag: the next token width (WCAG 2.5.7, no path needed). A pointer
  // the browser cancels (a scroll, a gesture) is no click.
  if (!drag.moved && event.type === "pointerup")
    void setWidth(drag.column, nextPresetWidth(drag.width, presets()));
  drag = null;
}

/** The custom element of an extension column, its properties set as such (not attributes). */
const ExtensionCell: FunctionalComponent<{
  tag: string;
  row: unknown;
  extra?: Record<string, unknown> | undefined;
}> = (p) =>
  h(p.tag, {
    ".row": p.row,
    ...Object.fromEntries(Object.entries(p.extra ?? {}).map(([k, v]) => [`.${k}`, v])),
  });
ExtensionCell.props = ["tag", "row", "extra"];

// ——— Keys ———

function onKeydown(event: KeyboardEvent): void {
  const target = event.target as HTMLElement;
  // The editor of a cell keeps its keys, but Escape, Enter and Tab (the contract); a host
  // that opens editors without `editable` keeps those too (it drives its editing).
  if (target.closest(".k-data-grid__editor")) {
    editor.onKeydown(event);
    return;
  }
  // A widget inside a cell keeps its keys (Enter on a copy button copies); Escape or F2
  // gives the focus back to its cell (APG Grid). In a header, F2 goes on from the funnel
  // to the handle, then back to the header; Enter on the handle comes back too.
  const owner = target.closest<HTMLElement>("[data-column]");
  if (owner && owner !== target) {
    if (event.key === "F2" && owner.getAttribute("role") === "columnheader") {
      const widgets = headerWidgets(owner);
      event.preventDefault();
      (widgets[widgets.indexOf(target) + 1] ?? owner).focus();
    } else if (
      event.key === "Escape" ||
      event.key === "F2" ||
      (event.key === "Enter" && isHandle(target))
    ) {
      event.preventDefault();
      owner.focus();
    }
    return;
  }
  // Escape closes the error of the cell only, not the layer around the grid (1.4.13).
  if (event.key === "Escape" && shownError.value !== null) {
    event.preventDefault();
    dismissed.value = shownError.value;
    return;
  }
  const cell = current.value;
  const column = columnOf(cell.column);
  const row = cell.row === "" ? undefined : rowOf(cell.row);
  if (cell.column === SELECT_COLUMN && (event.key === " " || event.key === "Enter")) {
    if (cell.row === "") selectAll(allState.value !== true);
    else toggleRow(cell.row);
    event.preventDefault();
    return;
  }
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
    if (event.key === "F2") {
      // Into the controls of the header: its funnel, then its handle.
      if (owner) headerWidgets(owner)[0]?.focus();
      event.preventDefault();
      return;
    }
  } else if (
    (event.key === "Enter" || event.key === "F2") &&
    row &&
    column &&
    canEdit(row, column)
  ) {
    void startEditing(cell);
    event.preventDefault();
    return;
  } else if (event.key === "Enter" || event.key === "F2") {
    // Into the first widget of the cell, if any; Enter on a plain cell activates it.
    const first = owner ? [...widgetsOf(owner), ...errorWidgetsOf(owner)][0] : undefined;
    if (first) first.focus();
    else if (event.key === "Enter") emit("activate", cell);
    event.preventDefault();
    return;
  } else if (props.selectable && event.key === " ") {
    toggleRow(cell.row);
    event.preventDefault();
    return;
  }
  if (
    props.selectable === true &&
    (event.ctrlKey || event.metaKey) &&
    event.key.toLowerCase() === "a"
  ) {
    selectAll(!props.rows.every(isSelected));
    event.preventDefault();
    return;
  }
  // The arrows move over the columns on screen only, by the indexes of the rows.
  const columns = navColumns();
  const from = {
    row: cell.row === "" ? 0 : (indexOfKey.value.get(cell.row) ?? -1) + 1,
    column: Math.max(0, columns.indexOf(cell.column)),
  };
  const to = moveInGrid(
    from,
    event,
    rowKeys.value.length,
    columns.length,
    isRtl(grid.value!),
    pageRows(),
  );
  if (!to) return;
  event.preventDefault();
  const next = { row: rowKeys.value[to.row]!, column: columns[to.column]! };
  // Shift with Up or Down extends the selection from the anchor (several rows).
  const vertical = event.key === "ArrowDown" || event.key === "ArrowUp";
  if (
    props.selectable === true &&
    event.shiftKey &&
    vertical &&
    next.row !== "" &&
    cell.row !== ""
  ) {
    anchor ??= { key: cell.row, base: selected.value };
    selected.value = selectRange(rowKeys.value.slice(1), anchor.base, anchor.key, next.row);
  } else if (!event.shiftKey) anchor = null;
  active.value = next;
  void focusActive();
}
const isHandle = (el: HTMLElement) => el.classList.contains("k-data-grid__resize");
/** The controls of a header, in order: its funnel, its handle. */
const headerWidgets = (header: HTMLElement) => [
  ...header.querySelectorAll<HTMLElement>(".k-data-grid__filter, .k-data-grid__resize"),
];

// A container query or the column chooser may hide the column of the cell that holds the
// roving focus, a filter may drop its row: the stop goes back to the key column of its row
// (or to the first header), so that Tab still reaches the grid.
function keepStop(): void {
  const cell = active.value;
  if (!cell) return;
  const key = props.columns[0]?.id ?? "";
  if (cell.row !== "" && (props.loading || !indexOfKey.value.has(cell.row)))
    active.value = { row: "", column: key };
  else if (!navColumns().includes(cell.column)) active.value = { row: cell.row, column: key };
}
watch([shown, rowKeys], () => void nextTick(keepStop));
// A column shown again, a width the host resets: the handles say the widths of now.
watch([shown, widths], () => void nextTick(measureHeaders));

// The key column sticks, and so does the header: what takes the focus scrolls out from
// under them (WCAG 2.4.11), the inline end of the key column measured into the scroll
// padding, the header's height into the virtualiser's offset of the rows.
function measureKey(): void {
  const key = headerOf(props.columns[0]?.id ?? "");
  if (key)
    grid.value!.style.setProperty("--k-data-grid-key", `${key.offsetLeft + key.offsetWidth}px`);
  const head = grid.value?.querySelector<HTMLElement>(".k-data-grid__head");
  if (head && head.offsetHeight > 0) headHeight.value = head.offsetHeight;
}
let observer: ResizeObserver | undefined;
/** The row height of the density, read again when the density changes (rows resize). */
function readEstimate(): void {
  const row = grid.value
    ? parseFloat(getComputedStyle(grid.value).getPropertyValue("--k-row-height"))
    : NaN;
  if (Number.isFinite(row) && row !== estimate.value) estimate.value = row;
}
onMounted(() => {
  readEstimate();
  measureKey();
  measureHeaders();
  if (typeof ResizeObserver === "undefined" || !root.value) return;
  observer = new ResizeObserver(() => {
    readEstimate();
    keepStop();
    measureKey();
    measureHeaders();
    scheduleWindow();
  });
  observer.observe(root.value);
});
onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <div
    ref="root"
    class="k-data-grid-frame"
    v-bind="rootAttrs"
    :data-stack="props.stack ? 'true' : undefined"
    :data-select="checkboxes ? 'true' : undefined"
  >
    <ColumnChooser v-model="visibility" :columns="props.columns" :grid="grid" />
    <div
      ref="grid"
      v-bind="gridAttrs"
      class="k-data-grid"
      role="grid"
      :aria-label="props.label"
      :aria-rowcount="
        props.rowCount === -1
          ? -1
          : (props.rowCount ?? (props.loading ? props.loadingRows : props.rows.length)) + 1
      "
      :aria-colcount="props.columns.length + (checkboxes ? 1 : 0)"
      :aria-multiselectable="props.selectable === true ? 'true' : undefined"
      :aria-readonly="props.editable ? undefined : 'true'"
      :aria-busy="props.loading ? 'true' : undefined"
      :data-virtual="virtual ? 'true' : undefined"
      :data-select="checkboxes ? 'true' : undefined"
      :style="template"
      @keydown="onKeydown"
      @focusin="onFocusin"
      @focusout="onFocusout"
      @click="onBodyClick"
      @dblclick="onDblclick"
    >
      <div role="rowgroup" class="k-data-grid__head">
        <div role="row" aria-rowindex="1" class="k-data-grid__row">
          <div
            v-if="checkboxes"
            role="columnheader"
            class="k-data-grid__header k-data-grid__select"
            :data-column="SELECT_COLUMN"
            data-row=""
            aria-colindex="1"
            :tabindex="tabindex('', SELECT_COLUMN)"
          >
            <Checkbox
              :label="t('selectAll')"
              hide-label
              size="sm"
              :model-value="allState"
              @update:model-value="selectAll($event === true)"
            />
          </div>
          <div
            v-for="column in shown"
            :key="column.id"
            role="columnheader"
            class="k-data-grid__header"
            :data-column="column.id"
            data-row=""
            :data-align="column.align"
            :data-priority="priority(column)"
            :data-width="column.width ?? 'md'"
            :data-sortable="column.sortable ? 'true' : undefined"
            :data-actions="column.actions ? 'true' : undefined"
            :aria-colindex="colIndex(column)"
            :aria-sort="ariaSort(column)"
            :aria-label="namedHeader(column) ? headerName(column) : undefined"
            :aria-keyshortcuts="column.filterable ? 'Alt+ArrowDown' : undefined"
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
            <button
              v-if="column.filterable"
              type="button"
              tabindex="-1"
              class="k-data-grid__filter"
              :aria-label="`${t('filterColumn')} ${column.label}`"
              :data-active="isFiltered(column) ? 'true' : undefined"
              @click.stop="emit('filter', column.id)"
            >
              <Icon name="funnel" />
            </button>
            <span v-if="isFiltered(column)" class="k-visually-hidden">({{ t("filtered") }})</span>
            <span
              v-if="props.resizable"
              class="k-data-grid__resize"
              role="separator"
              tabindex="-1"
              aria-orientation="vertical"
              :aria-label="`${t('columnResize')} ${column.label}`"
              :aria-valuenow="measured[column.id]?.now"
              :aria-valuemin="measured[column.id]?.min"
              :aria-valuemax="WIDTH_MAX"
              :aria-valuetext="measured[column.id] ? `${measured[column.id]!.now} px` : undefined"
              @keydown="onHandleKeydown(column, $event)"
              @pointerdown="onHandleDown(column, $event)"
              @pointermove="onHandleMove"
              @pointerup="onHandleUp"
              @pointercancel="onHandleUp"
              @click.stop
            ></span>
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
          <div v-if="checkboxes" role="gridcell" class="k-data-grid__cell" aria-colindex="1">
            <Skeleton variant="cell" :rows="r" :columns="0" />
          </div>
          <div
            v-for="(column, c) in shown"
            :key="column.id"
            role="gridcell"
            class="k-data-grid__cell"
            :data-align="column.align"
            :data-priority="priority(column)"
            :data-width="column.width ?? 'md'"
            :data-actions="column.actions ? 'true' : undefined"
            :aria-colindex="colIndex(column)"
          >
            <span v-if="r === 1 && c === 0" class="k-visually-hidden">{{ t("loading") }}</span>
            <Skeleton variant="cell" :rows="r" :columns="c" />
          </div>
        </div>
      </div>
      <div v-else role="rowgroup" class="k-data-grid__body">
        <template v-for="part in parts" :key="part.key">
          <div
            v-if="'gap' in part"
            class="k-data-grid__gap"
            aria-hidden="true"
            :style="{ '--k-data-grid-gap': `${part.gap}px` }"
          ></div>
          <div
            v-else
            :ref="measure"
            role="row"
            class="k-data-grid__row"
            :data-index="part.index"
            :aria-rowindex="props.rowOffset + part.index + 2"
            :aria-selected="props.selectable ? isSelected(part.row) : undefined"
            :data-row-state="props.rowState?.(part.row)"
            :data-fresh="fresh.has(part.key) ? 'true' : undefined"
            @animationend="onAnimationend(part.key, $event)"
          >
            <div
              v-if="checkboxes"
              role="gridcell"
              class="k-data-grid__cell k-data-grid__select"
              :data-row="part.key"
              :data-column="SELECT_COLUMN"
              aria-colindex="1"
              :tabindex="tabindex(part.key, SELECT_COLUMN)"
              @click.capture="noteShift"
            >
              <Checkbox
                :label="`${t('selectRow')} ${part.key}`"
                hide-label
                size="sm"
                :model-value="isSelected(part.row)"
                @update:model-value="onBox(part.key)"
              />
            </div>
            <div
              v-for="(column, c) in shown"
              :key="column.id"
              role="gridcell"
              class="k-data-grid__cell"
              :class="{ 'k-mono': column.mono }"
              :data-row="part.key"
              :data-column="column.id"
              :data-align="column.align"
              :data-priority="priority(column)"
              :data-width="widthOf(column)"
              :style="widthStyle(column)"
              :data-wrap="column.wrap ? 'true' : undefined"
              :data-label="props.stack && c > 0 && !column.actions ? column.label : undefined"
              :data-actions="column.actions ? 'true' : undefined"
              :data-state="stateOf(part.row, column)"
              :data-editing="isEditing(part.row, column) ? 'true' : undefined"
              :aria-colindex="colIndex(column)"
              :aria-readonly="props.editable && !canEdit(part.row, column) ? 'true' : undefined"
              :aria-busy="stateOf(part.row, column) === 'pending' ? 'true' : undefined"
              :aria-invalid="messageOf(part.row, column) ? 'true' : undefined"
              :aria-describedby="hasError(part.row, column) ? errorId(part.index, c) : undefined"
              :data-error-open="errorShown(part.row, column) ? 'true' : undefined"
              :tabindex="isEditing(part.row, column) ? undefined : tabindex(part.key, column.id)"
              @pointerenter="onCellEnter(part.row, column, $event)"
              @pointerleave="onCellLeave(part.index, c, $event)"
            >
              <div v-if="isEditing(part.row, column) && slots.editor" class="k-data-grid__editor">
                <slot
                  name="editor"
                  :row="part.row"
                  :column="column"
                  :value="draft"
                  :update="(value: string) => (draft = value)"
                  :label="editorLabel(part.row, column)"
                  :invalid="!!messageOf(part.row, column)"
                  :described-by="hasError(part.row, column) ? errorId(part.index, c) : undefined"
                />
              </div>
              <div
                v-else-if="isEditing(part.row, column)"
                class="k-field-frame k-data-grid__editor"
                data-size="md"
                :data-invalid="messageOf(part.row, column) ? 'true' : undefined"
              >
                <input
                  v-model="draft"
                  class="k-field-frame__input"
                  :class="{ 'k-mono': column.mono }"
                  type="text"
                  spellcheck="false"
                  :aria-label="editorLabel(part.row, column)"
                  :aria-invalid="messageOf(part.row, column) ? 'true' : undefined"
                  :aria-describedby="
                    hasError(part.row, column) ? errorId(part.index, c) : undefined
                  "
                />
              </div>
              <template v-else>
                <template v-if="c === 0 && props.rowState?.(part.row)">
                  <Icon :name="markOf(part.row).icon" class="k-data-grid__row-mark" />
                  <span class="k-visually-hidden">({{ t(markOf(part.row).word) }})</span>
                </template>
                <span class="k-data-grid__value">
                  <ExtensionCell
                    v-if="column.element"
                    :tag="column.element"
                    :row="part.row"
                    :extra="props.elementProps?.(part.row, column)"
                  />
                  <slot
                    v-else
                    name="cell"
                    :row="part.row"
                    :column="column"
                    :value="part.row[column.id]"
                    >{{ part.row[column.id] }}</slot
                  >
                </span>
                <Icon
                  v-if="stateOf(part.row, column) === 'pending'"
                  name="loader-circle"
                  class="k-spinner k-data-grid__mark"
                />
                <Icon
                  v-else-if="stateOf(part.row, column) === 'error'"
                  name="circle-x"
                  class="k-data-grid__mark"
                />
                <Icon
                  v-else-if="stateOf(part.row, column) === 'review'"
                  name="eye"
                  class="k-data-grid__mark"
                />
                <span v-if="stateWord(part.row, column)" class="k-visually-hidden"
                  >({{ stateWord(part.row, column) }})</span
                >
              </template>
              <CellError
                v-if="hasError(part.row, column)"
                :id="errorId(part.index, c)"
                :error="messageOf(part.row, column)!.error"
                :code="messageOf(part.row, column)!.code"
                :anchor="errorAnchor(part.row, column)"
                @leave="hovered = null"
                @dismiss="dismissed = shownError ?? dismissed"
                @back="focusCell(part.index, c)"
                @blur="focused = null"
              />
            </div>
          </div>
        </template>
      </div>
    </div>
    <div class="k-visually-hidden" role="alert">{{ announcement }}</div>
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
    --k-data-grid-key: var(--k-space-0);

    position: relative;
    display: grid;
    grid-template-columns: var(--k-grid-columns);
    align-content: start;
    min-inline-size: 0;
    overflow: auto;
    scrollbar-gutter: stable;
    /* The sticky header and key column never hide a focused cell (WCAG 2.4.11). */
    scroll-padding-block-start: var(--k-header-height);
    scroll-padding-inline-start: var(--k-data-grid-key);
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  /* Virtualised: the rows scroll in the grid, a list's height at most (a host's bound on
   * the root holds it lower). */
  .k-data-grid[data-virtual] {
    max-block-size: var(--k-size-list-max);
  }

  .k-data-grid__head,
  .k-data-grid__body,
  .k-data-grid__row {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
  }

  /* The height of the rows left out of the document (virtualised). */
  .k-data-grid__gap {
    grid-column: 1 / -1;
    block-size: var(--k-data-grid-gap);
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

  /* The key column sticks at the inline start while the grid scrolls sideways; after the
   * checkboxes, which stick too. */
  .k-data-grid__row > :first-child,
  .k-data-grid[data-select] .k-data-grid__row > :nth-child(2) {
    position: sticky;
    inset-inline-start: 0;
    z-index: var(--k-z-sticky);
    background-color: inherit;
  }

  .k-data-grid[data-select] .k-data-grid__row > :nth-child(2) {
    inset-inline-start: calc(var(--k-control-height-sm) + 2 * var(--k-cell-padding-inline));
  }

  .k-data-grid__row[aria-selected="true"] > :first-child {
    box-shadow: var(--k-shadow-inset-accent);
  }

  /* The checkbox of a row (several selectable), and the one of all rows in the header. */
  .k-data-grid__select.k-data-grid__header,
  .k-data-grid__select.k-data-grid__cell {
    justify-content: center;
    min-inline-size: 0;
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

  /* A simple list under a container `sm` (`stack`): its rows stack. One track for the
   * grid; the header a wrapping strip of its controls, in the flow; a row a grid of its
   * own (label, value, end), the key and the actions on its first line, each attribute
   * below as a subgrid of it: labels and values align down the row. Every column shows
   * (nothing to hide, no chooser), the key column no longer sticks. With checkboxes, theirs
   * is a first track of the row, on its first line. */
  @container (width < 480px) {
    .k-data-grid-frame[data-stack] > .k-column-chooser:not([data-chosen]) {
      display: none;
    }

    .k-data-grid-frame[data-stack] .k-data-grid {
      grid-template-columns: minmax(0, 1fr);
      scroll-padding: 0;
    }

    .k-data-grid-frame[data-stack] .k-data-grid__head {
      position: static;
    }

    .k-data-grid-frame[data-stack] .k-data-grid__head .k-data-grid__row {
      display: flex;
      flex-wrap: wrap;
      column-gap: var(--k-space-1);
    }

    .k-data-grid-frame[data-stack] .k-data-grid__header {
      display: flex;
      position: relative;
    }

    /* The box of all rows keeps the track of the boxes: the controls, wrapped lines
     * included, start after it, as the keys of the rows do. */
    .k-data-grid-frame[data-stack][data-select] .k-data-grid__head .k-data-grid__row {
      position: relative;
      padding-inline-start: calc(var(--k-control-height-sm) + 2 * var(--k-cell-padding-inline));
    }

    .k-data-grid-frame[data-stack][data-select] .k-data-grid__header.k-data-grid__select {
      position: absolute;
      inset-block-start: 0;
      inset-inline-start: 0;
      block-size: var(--k-header-height);
    }

    .k-data-grid-frame[data-stack] .k-data-grid__body .k-data-grid__row {
      grid-template-columns: max-content minmax(0, 1fr) auto;
      column-gap: 0;
      padding-block: var(--k-space-1);
    }

    .k-data-grid-frame[data-stack] [data-select] .k-data-grid__row {
      grid-template-columns: auto max-content minmax(0, 1fr) auto;
    }

    .k-data-grid-frame[data-stack] .k-data-grid__cell {
      position: relative;
      display: grid;
      min-inline-size: 0;
      grid-column: 1 / -1;
      grid-template-columns: subgrid;
      min-block-size: var(--k-control-height-sm);
    }

    .k-data-grid-frame[data-stack] [data-select] .k-data-grid__cell {
      grid-column: 2 / -1;
    }

    .k-data-grid-frame[data-stack] .k-data-grid__cell:first-child {
      display: flex;
      grid-column: 1 / 3;
      font-weight: var(--k-weight-medium);
    }

    .k-data-grid-frame[data-stack] [data-select] .k-data-grid__cell.k-data-grid__select {
      grid-row: 1;
      grid-column: 1;
    }

    .k-data-grid-frame[data-stack] [data-select] .k-data-grid__cell:nth-child(2) {
      display: flex;
      grid-row: 1;
      grid-column: 2 / 4;
      font-weight: var(--k-weight-medium);
    }

    .k-data-grid-frame[data-stack] .k-data-grid__cell[data-actions] {
      display: flex;
      grid-row: 1;
      grid-column: 3;
    }

    .k-data-grid-frame[data-stack] [data-select] .k-data-grid__cell[data-actions] {
      grid-column: 4;
    }

    .k-data-grid-frame[data-stack] .k-data-grid__cell[data-label]::before {
      grid-column: 1;
      padding-inline-end: var(--k-space-3);
      font-family: var(--k-font-sans);
      font-size: var(--k-text-sm);
      line-height: var(--k-text-sm-line);
      color: var(--k-text-muted);
      content: attr(data-label) / "";
    }

    .k-data-grid-frame[data-stack] .k-data-grid__cell[data-label] > .k-data-grid__value {
      grid-column: 2;
    }
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

  /* A column is as wide as its token, and never narrower than its header: title, sort and
   * filter whole (the body cells hold the token, the header its min-content; the template
   * is `minmax(auto, token)`, grid.ts). */
  .k-data-grid__header {
    min-inline-size: min-content;
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text-muted);
  }

  .k-data-grid__cell[data-width="xs"] {
    min-inline-size: var(--k-size-col-xs);
  }

  .k-data-grid__cell[data-width="sm"] {
    min-inline-size: var(--k-size-col-sm);
  }

  .k-data-grid__cell[data-width="md"] {
    min-inline-size: var(--k-size-col-md);
  }

  .k-data-grid__cell[data-width="lg"] {
    min-inline-size: var(--k-size-col-lg);
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

  /* The actions of a row at the end of their column, under its header. */
  .k-data-grid__header[data-actions],
  .k-data-grid__cell[data-actions] {
    justify-content: end;
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

  /* The funnel: a button of its own inside a sortable header, out of the tab order (Alt+Down
   * on the header asks for the filter), at the floor of 24 px for any pointer (WCAG 2.5.8).
   * Beside the title and the sort, never at the far end of a wide column (relecture du
   * 2026-10-26). */
  .k-data-grid__filter {
    display: inline-flex;
    flex: none;
    justify-content: center;
    align-items: center;
    min-inline-size: var(--k-control-height-sm);
    min-block-size: var(--k-control-height-sm);
    padding: 0;
    font: inherit;
    color: var(--k-text-muted);
    cursor: pointer;
    background-color: transparent;
    border: 0;
    border-radius: var(--k-radius-sm);
  }

  .k-data-grid__filter:hover {
    color: var(--k-text);
    background-color: var(--k-hover);
  }

  .k-data-grid__filter[data-active] {
    color: var(--k-accent);
  }

  /* The handle of a column's width: a band of 24 px astride the end of the header (WCAG
   * 2.5.8), a hairline shown under the pointer, at the focus and while dragged. */
  .k-data-grid__resize {
    position: absolute;
    inset-block: 0;
    inset-inline-end: calc(var(--k-control-height-sm) / -2);
    /* Under the sticky headers (key, checkboxes): a column scrolled under them gives
     * them its handle's room. */
    z-index: calc(var(--k-z-sticky) - 1);
    inline-size: var(--k-control-height-sm);
    cursor: col-resize;
    touch-action: none;
  }

  .k-data-grid__resize::after {
    position: absolute;
    inset-block: var(--k-space-1);
    inset-inline-start: calc(50% - var(--k-border-width));
    inline-size: calc(var(--k-border-width) * 2);
    content: "";
    background-color: transparent;
    border-radius: var(--k-radius-sm);
  }

  .k-data-grid__header:hover > .k-data-grid__resize::after {
    background-color: var(--k-border-strong);
  }

  .k-data-grid__resize:hover::after,
  .k-data-grid__resize:focus-visible::after {
    background-color: var(--k-accent);
  }

  .k-data-grid__resize:focus-visible {
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  /* The last header: its handle inside the grid, at its end. */
  .k-data-grid__header:last-child > .k-data-grid__resize {
    inset-inline-end: 0;
  }

  /* Clipped to the cell, its focus ring kept: a widget of the cell (a copy pastille, a
   * link) takes the focus with a ring outside its box (WCAG 2.4.7), which a plain `hidden`
   * cut away. A plain length: the engines refuse a `calc()` there. */
  .k-data-grid__value {
    min-inline-size: 0;
    overflow: clip;
    overflow-clip-margin: var(--k-space-2);
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

  /* An invalid cell, at rest: its tint, a danger edge and its icon (never colour alone),
   * at the size of its neighbours. Its message is a floating layer (`CellError`), open
   * while it is hovered or focused; open, the layer's outline takes the place of the edge.
   * The edge is drawn inside (a pseudo-element), leaving the outline to the focus ring,
   * itself drawn inside the edge. */
  .k-data-grid__cell[data-state="error"] {
    background-color: var(--k-state-ko-bg);
  }

  .k-data-grid__cell[data-state="error"]::after {
    position: absolute;
    inset: var(--k-space-0);
    pointer-events: none;
    content: "";
    border: var(--k-border-width) solid var(--k-state-ko-fg);
  }

  .k-data-grid__cell[data-error-open]::after {
    border-color: transparent;
  }

  .k-data-grid__cell[data-state="error"]:focus-visible {
    outline-offset: calc((var(--k-focus-width) + var(--k-border-width)) * -1);
  }

  .k-data-grid__cell[data-state="error"] .k-data-grid__mark {
    color: var(--k-state-ko-fg);
  }

  .k-data-grid__mark {
    flex: none;
    margin-inline-start: auto;
  }

  /* The editor takes the whole cell, at the height of the row. */
  .k-data-grid__cell[data-editing] {
    padding-inline: 0;
  }

  /* As tall as the cell, never taller: the row keeps its height (the frame of a field is
   * as tall as a control, a pixel more than a cell). */
  .k-data-grid__cell > .k-data-grid__editor {
    flex: 1;
    align-self: stretch;
    min-inline-size: 0;
    min-block-size: 0;
  }

  /* A width the user gave: the cells hold it (the track's least, its header bounds it). */
  .k-data-grid__cell[data-width="set"] {
    min-inline-size: var(--k-data-grid-width);
  }
}
</style>
