<script setup lang="ts">
// A list of the referential (REQ-021-9, 18; design 021 « Écrans » : Référentiel, « Filtres,
// libellés, codes »; maquette E3): one `DataGrid` of a collection of the core, read page by
// page through the cache at the dates of the page, the next page read before the window
// reaches the end (REQ-021-22), and the typed filters of its route in the URL — one model
// for the bar and the filters of the columns (020 decision A7). Three states: skeleton
// rows, an empty list with a sentence (and the way out of the filters), an error with its
// codes. A refused cursor starts the list again from its first page.
// In-place editing (task 12, REQ-021-15, 16): the cells the user may edit, to the draft by
// Enter, applied now by Ctrl+Enter with a motive asked in line at the foot of the list;
// the foot says the keys of the cell, or why it offers no editor. `c` creates and `e`
// modifies the object of the row by the edit panel of the kind (REQ-021-36).
// Columns of properties (task 12i, Q-021-18; `propertyColumns.ts`): none by default, each
// property declared for the kind offered by « Colonnes », the choice kept by view and by
// user; their cells edited by the rules of the record's tab.
import {
  Button,
  DataGrid,
  EmptyState,
  ErrorState,
  FilterPicker,
  KeyLabel,
  NoValue,
  StatusBadge,
  type CellRef,
  type GridColumn,
  type GridWindow,
} from "@kartenn/ui";
import { computed, nextTick, ref, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useSettings } from "../frame/settings.ts";
import ExtensionElement from "../plugins/ExtensionElement.vue";
import { cellExtension, gridColumns, stableObjectRefs, useExtensions } from "../plugins/slots.ts";
import KLink from "../prefetch/KLink.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { applyRight, editRight } from "./edit/fields.ts";
import { useInlineEdit, whyWords } from "./edit/inline.ts";
import { openEdit } from "./edit/panel.ts";
import EditHint from "./edit/EditHint.vue";
import QuickApply from "./edit/QuickApply.vue";
import { useSay, useStatus } from "./edit/say.ts";
import {
  fieldText,
  isSummary,
  nameOf,
  navOf,
  pageOfHref,
  referenceOf,
  type Collection,
  type CollectionColumn,
  type Item,
} from "./collections.ts";
import { useDeclarations, useKeyLabel } from "./data.ts";
import NetworkVlan from "./parts/NetworkVlan.vue";
import PropertyCell from "./parts/PropertyCell.vue";
import { propertyOfColumn, usePropertyColumns } from "./propertyColumns.ts";
import ObjectName from "./record/ObjectName.vue";
import NextPageAlert from "../problems/NextPageAlert.vue";
import ProblemState from "../problems/ProblemState.vue";
import { useProblems } from "../problems/index.ts";
import { useFilteredList } from "./list.ts";
import ViewPanel from "./parts/ViewPanel.vue";

const props = defineProps<{ collection: Collection }>();
const { t, locale } = useI18n();
const router = useRouter();
const settings = useSettings();

const title = computed(() => t(navOf(props.collection)));
const {
  model,
  words,
  filters,
  onFilters,
  applied,
  list,
  rows,
  failed,
  nextFailed,
  complete,
  blocked,
  declarationsError,
  restarted,
  onWindow,
} = useFilteredList<Item>(() => props.collection.path);

// The columns of the core the user may read (the VLAN of a network: `core.vlan.read`).
const session = useSession();
const shownColumns = computed(() =>
  props.collection.columns.filter(
    (column) => !column.read || session.value.rights.has(column.read),
  ),
);
// The columns of the plugins for the kind of the list, after the core's (REQ-021-13): each
// cell an element of the plugin, given its row and the object it stands for.
const extensions = useExtensions("column", () => props.collection.kind);
const columns = computed<GridColumn[]>(() => {
  const served = new Set(model.value.map((property) => property.key));
  return [
    ...shownColumns.value.map((column) => ({
      id: column.id,
      label: t(`views.field.${column.id}`),
      width: column.width,
      ...(column.mono ? { mono: true } : {}),
      ...(column.priority ? { priority: column.priority } : {}),
      // A column offers its filter only when the model serves its parameter (A7).
      ...(column.filter && served.has(column.filter) ? { filterable: true } : {}),
    })),
    ...gridColumns(extensions.value, (key) => t(key)),
    ...cells.columns.value,
  ];
});
// The same `object` for a row from one render to the next: an element is not created again
// as the focus moves (`ExtensionElement` creates it again for another row).
const objectRef = stableObjectRefs();
const elementProps = (row: Item) => ({
  object: objectRef(
    row,
    props.collection.kind,
    String(row.id),
    String((row.links as { self?: unknown } | undefined)?.self ?? ""),
  ),
});
const columnOf = (id: string): CollectionColumn | undefined =>
  props.collection.columns.find((column) => column.id === id);
/** The object a cell of a column of references names (a group, an anchor), if any. */
const refOf = (id: string, value: unknown) => {
  const column = columnOf(id);
  return column ? referenceOf(column, value) : null;
};
const filtered = computed(() =>
  props.collection.columns
    .filter((column) => column.filter && applied.value.has(column.filter))
    .map((column) => column.id),
);

const picker = useTemplateRef<InstanceType<typeof FilterPicker>>("picker");
/** The funnel of a header, or Alt+Down on it: the bar's picker, on that column's filter. */
function onColumnFilter(id: string): void {
  const filter = columnOf(id)?.filter;
  const from = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  if (filter) picker.value?.openFor(filter, from);
}
useViewAction("filter", {
  label: () => t("views.list.filterAction", { name: title.value }),
  run: () => picker.value?.openAdd(),
});

// In-place editing, and the edit panel behind `c` and `e`.
// The status of the list (`useStatus`); « restarted » gives way to a sentence.
const { said, status } = useStatus();
const say = useSay((text) => {
  restarted.value = false;
  status(text);
});
const edit = useInlineEdit({
  collection: () => props.collection,
  rows: () => rows.value,
  // The words of a closed list, its cell's list showing them (task 12g).
  words: (field, key) => choiceWords(field, key),
  t: (key, values) => t(key, values ?? {}),
  sentence: useProblems().sentence,
  say,
});
const active = ref<CellRef | undefined>();
const editing = ref<CellRef | undefined>();
// Those of the list's kind, the very read its filters take (`carrier`).
const { declarations } = useDeclarations(() => props.collection.kind);
const keyLabel = useKeyLabel();
const valueLabel = (key: string, value: string) => keyLabel("value", `${key}/${value}`);
const cells = usePropertyColumns({
  collection: () => props.collection,
  rows: () => rows.value,
  inline: edit,
  declarations: () => declarations.value,
  active: () => active.value,
  // The words of its filter: the dictionary's, else the declaration's own label.
  label: (key) => words.property(key) ?? null,
  valueLabel,
  t: (key, values) => t(key, values ?? {}),
  sentence: useProblems().sentence,
  say,
});
const { visibility: columnVisibility } = cells;
watch(active, (cell) => cells.focus(cell), { immediate: true });
// The column of the focus hidden by « Colonnes »: the focus to the key of its row, never on
// a cell no longer shown (the foot would speak of it).
watch(columnVisibility, (visibility) => {
  const cell = active.value;
  if (cell && visibility[cell.column] === false) {
    active.value = { row: cell.row, column: props.collection.columns[0]!.id };
  }
});
// Said when the editor of a property opens too, or once its origin is read: heard before
// the value is sent, not only seen (as in the record's tab).
watch(
  () => (editing.value ? cells.noteOf(editing.value.column, editing.value.row) : undefined),
  (note) => {
    if (note) say.status(note);
  },
);
const mayEdit = () => session.value.rights.has(editRight(props.collection.type));
/** Why `c` and `e` are refused here: the permission they lack (the list has no « Créer »). */
const forbidden = () =>
  whyWords({ kind: "forbidden", permission: editRight(props.collection.type) }, t);
// `c` and `e` open the edit panel: under `valid_at` alone too, to the dated draft only
// (REQ-021-5; `action.ts`), as « Modifier » of the record; never under `as_of`. Refused,
// they say why by the status of the list.
useViewAction("create", {
  label: () => t("edit.createAction", { kind: kindWords.value }),
  allowed: mayEdit,
  why: forbidden,
  say: say.status,
  run: () => openEdit({ mode: "create", collection: props.collection.id }),
  dated: true,
});
/** The row of the cell that has the focus, if a row (not the header). */
const activeRow = computed(() =>
  active.value && active.value.row !== ""
    ? rows.value.find((r) => String(r.id) === active.value!.row)
    : undefined,
);
useViewAction("edit", {
  label: () => t("edit.editAction", { kind: kindWords.value }),
  allowed: () => mayEdit() && activeRow.value !== undefined,
  why: () => (activeRow.value === undefined ? undefined : forbidden()),
  say: say.status,
  run: () => {
    if (activeRow.value) {
      openEdit({ mode: "edit", collection: props.collection.id, id: String(activeRow.value.id) });
    }
  },
  dated: true,
});
const kindWords = computed(() => t(`labels.kind.${props.collection.type}`));
/** Why the cell of the focus offers no editor, said at the foot of the list. */
const activeWhy = computed(() =>
  active.value && active.value.row !== "" ? cells.why(active.value.column, active.value.row) : null,
);
/** Why the cell of the focus is held in review, said at the foot too (seen, not only read). */
const activeHeld = computed(() => {
  const cell = active.value;
  if (!cell || cell.row === "") return undefined;
  const row = edit.shownRows.value.find((item) => String(item.id) === cell.row);
  return row ? cells.heldOf(row, cell.column) : undefined;
});
/** What an edit of the cell of the focus writes, said at the foot and read with the cell. */
const activeNote = computed(() =>
  active.value && active.value.row !== "" && activeWhy.value === null
    ? cells.noteOf(active.value.column, active.value.row)
    : undefined,
);
const noteHere = (row: Item, column: string) =>
  active.value?.row === String(row.id) && active.value.column === column
    ? activeNote.value
    : undefined;
/** The words of a key of a closed list, for the line of the motive. */
const choiceWords = (field: string, key: string) =>
  words.value(field, key) ?? valueLabel(field, key);
const targetOf = (key: string) => {
  const target = cells.declarationOf(key)?.params?.target;
  return typeof target === "string" ? target : undefined;
};
/** The line of the motive closed: the focus back to its cell. */
async function afterAsk(cell: CellRef): Promise<void> {
  active.value = cell;
  await nextTick();
  document
    .querySelector<HTMLElement>(
      `[data-k-list] [data-row="${CSS.escape(cell.row)}"][data-column="${CSS.escape(cell.column)}"]`,
    )
    ?.focus();
}
async function onConfirm(motive: string, value: string): Promise<void> {
  const cell = cells.cellOfAsk();
  if (cell && (await cells.confirm(motive, value))) void afterAsk(cell);
}
function onCancelAsk(): void {
  const cell = cells.cellOfAsk();
  cells.cancel();
  if (cell) void afterAsk(cell);
}
function onGridWindow(window: GridWindow): void {
  onWindow(window);
  cells.onWindow(window);
}

const recordOf = (row: Item) =>
  pageOfHref((row.links as { self?: unknown } | undefined)?.self) ??
  `/${props.collection.id}/${String(row.id)}`;
/** Enter on a cell without a link opens the record of its row. */
function onActivate(cell: CellRef): void {
  const row = rows.value.find((r) => r.id === cell.row);
  if (row) void router.push(recordOf(row));
}

const number = computed(() => new Intl.NumberFormat(locale.value));
/** The whole count beside the title once every page is read; the foot says where it stands. */
const total = computed(() => (complete.value ? number.value.format(rows.value.length) : undefined));
const countText = computed(() =>
  t(
    complete.value ? "views.list.count" : "views.list.loaded",
    { n: number.value.format(rows.value.length) },
    rows.value.length,
  ),
);
const hasFilters = computed(() => filters.value.some((filter) => !filter.draft));
/** The way out of the filters: its button leaves with the empty state, the focus goes to the bar. */
function clearFilters(): void {
  onFilters(filters.value.filter((filter) => filter.draft));
  void nextTick(() => picker.value?.focusAdd());
}
</script>

<template>
  <h1 class="k-visually-hidden">{{ title }}</h1>
  <ViewPanel :title="title" :detail="total" main flush>
    <template #toolbar>
      <FilterPicker
        ref="picker"
        :model-value="filters"
        :model="model"
        :label="t('views.list.filters', { name: title })"
        :shortcut="settings.singleKeys.value ? ['/'] : undefined"
        @update:model-value="onFilters"
      />
    </template>
    <NextPageAlert v-if="nextFailed" :error="list.error.value" :retry="list.fetchNextPage" />
    <!-- A property filter of the URL that no declaration types: no list read without it. -->
    <ProblemState
      v-if="blocked?.reason === 'failed'"
      :title="t('views.list.declarationsFailed', { names: blocked.names.join(', ') })"
      :error="declarationsError"
    >
      {{ t("views.list.notRead") }}
    </ProblemState>
    <ErrorState
      v-else-if="blocked"
      :title="t('views.list.untyped', { names: blocked.names.join(', ') })"
    >
      {{ t("views.list.notRead") }}
    </ErrorState>
    <ProblemState
      v-else-if="failed"
      :title="t('views.list.error', { name: title })"
      :error="list.error.value"
      :retry="list.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="list.isSuccess.value && rows.length === 0"
      :title="hasFilters ? t('views.list.noneFiltered') : t('views.list.none', { name: title })"
    >
      {{ hasFilters ? t("views.list.noneFilteredText") : t("views.list.noneText") }}
      <template v-if="hasFilters" #action>
        <Button variant="ghost" icon="x" @click="clearFilters">{{
          t("views.list.clearFilters")
        }}</Button>
      </template>
    </EmptyState>
    <DataGrid
      v-else
      v-model:active="active"
      v-model:editing="editing"
      v-model:column-visibility="columnVisibility"
      data-k-list
      :label="title"
      :columns="columns"
      :rows="cells.shownRows.value"
      row-key="id"
      :loading="list.isPending.value"
      :loading-rows="8"
      :filtered="filtered"
      :row-count="complete ? rows.length : undefined"
      :element-props="elementProps"
      :editable="cells.editable"
      :readonly-reason="cells.readonlyReason"
      :validate="cells.validate"
      :options="cells.options"
      :cell-state="cells.cellState"
      :state-reason="cells.stateReason"
      :row-state="cells.rowState"
      :row-reason="cells.rowReason"
      @filter="onColumnFilter"
      @activate="onActivate"
      @commit="cells.onCommit"
      @window="onGridWindow"
    >
      <template #cell="{ row, column, value }">
        <KLink v-if="column.id === props.collection.columns[0]!.id" :to="recordOf(row)">{{
          (columnOf(column.id)?.ownValue && fieldText(value)) || nameOf(row)
        }}</KLink>
        <PropertyCell
          v-else-if="propertyOfColumn(column.id) !== null"
          :property="propertyOfColumn(column.id)!"
          :state="cells.cellOf(row, propertyOfColumn(column.id)!)"
          :type="cells.declarationOf(propertyOfColumn(column.id)!)?.type"
          :target="targetOf(propertyOfColumn(column.id)!)"
          :note="noteHere(row, column.id)"
        />
        <StatusBadge
          v-else-if="columnOf(column.id)?.states?.[String(value)]"
          :state="columnOf(column.id)!.states![String(value)]!"
          :label="words.value(column.id, String(value)) ?? String(value)"
        />
        <KeyLabel
          v-else-if="columnOf(column.id)?.enumerated && typeof value === 'string'"
          :value="value"
          :label="words.value(column.id, value) ?? null"
          plain
        />
        <KLink
          v-else-if="columnOf(column.id)?.summary && isSummary(value) && pageOfHref(value.href)"
          :to="pageOfHref(value.href)!"
          >{{ fieldText(value) }}</KLink
        >
        <ObjectName
          v-else-if="refOf(column.id, value)"
          :id="refOf(column.id, value)!.id"
          :kind="refOf(column.id, value)!.kind"
          :with-kind="columnOf(column.id)!.reference === 'anchor'"
        />
        <NetworkVlan
          v-else-if="columnOf(column.id)?.networkVlan"
          :id="String(row.id)"
          :network="row.nature === 'network'"
        />
        <template v-else-if="typeof value === 'boolean'">{{
          t(`views.value.boolean.${value}`)
        }}</template>
        <NoValue v-else-if="fieldText(value) === null" />
        <template v-else>{{ fieldText(value) }}</template>
      </template>
      <!-- A column of a plugin: its element hosted by the shell, an error kept in its cell. -->
      <template #extension="given">
        <ExtensionElement cell v-bind="cellExtension(extensions, given)" />
      </template>
    </DataGrid>
    <template #footer>
      <QuickApply
        v-if="cells.asking.value"
        :key="cells.asking.value.row + cells.asking.value.column"
        :ask="cells.asking.value"
        :words="choiceWords"
        @confirm="onConfirm"
        @cancel="onCancelAsk"
      />
      <template v-else>
        <span class="k-num">{{ list.isPending.value ? "" : countText }}</span>
        <EditHint
          v-if="active && active.row !== ''"
          :why="activeWhy"
          :apply-lacks="cells.applyLacks(active.column, applyRight(props.collection.type))"
          :held="activeHeld"
          :note="activeNote"
        />
      </template>
      <span role="status">{{ restarted ? t("views.list.restarted") : said }}</span>
    </template>
  </ViewPanel>
</template>
