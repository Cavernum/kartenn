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
import {
  Button,
  CodeBadge,
  DataGrid,
  EmptyState,
  ErrorState,
  FilterPicker,
  Kbd,
  KeyLabel,
  type CellRef,
  type GridColumn,
} from "@kartenn/ui";
import { computed, nextTick, ref, useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useSettings } from "../frame/settings.ts";
import { gridColumns, objectRef, useExtensions } from "../plugins/slots.ts";
import KLink from "../prefetch/KLink.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { applyRight, editRight } from "./edit/fields.ts";
import { useInlineEdit } from "./edit/inline.ts";
import { openEdit } from "./edit/panel.ts";
import QuickApply from "./edit/QuickApply.vue";
import { useSay } from "./edit/say.ts";
import {
  fieldText,
  isSummary,
  nameOf,
  pageOfHref,
  type Collection,
  type CollectionColumn,
  type Item,
} from "./collections.ts";
import { problemOf } from "./data.ts";
import { useFilteredList } from "./list.ts";
import ViewPanel from "./parts/ViewPanel.vue";

const props = defineProps<{ collection: Collection }>();
const { t, locale } = useI18n();
const router = useRouter();
const settings = useSettings();

const title = computed(() => t(`nav.${props.collection.id}`));
const {
  model,
  words,
  filters,
  onFilters,
  applied,
  list,
  rows,
  failed,
  complete,
  blocked,
  declarationsError,
  restarted,
  onWindow,
} = useFilteredList<Item>(() => props.collection.path);

// The columns of the plugins for the kind of the list, after the core's (REQ-021-13): each
// cell an element of the plugin, given its row and the object it stands for.
const extensions = useExtensions("column", () => props.collection.kind);
const columns = computed<GridColumn[]>(() => {
  const served = new Set(model.value.map((property) => property.key));
  return [
    ...props.collection.columns.map((column) => ({
      id: column.id,
      label: t(`views.field.${column.id}`),
      width: column.width,
      ...(column.mono ? { mono: true } : {}),
      ...(column.priority ? { priority: column.priority } : {}),
      // A column offers its filter only when the model serves its parameter (A7).
      ...(column.filter && served.has(column.filter) ? { filterable: true } : {}),
    })),
    ...gridColumns(extensions.value, (key) => t(key)),
  ];
});
const elementProps = (row: Item) => ({
  object: objectRef(
    props.collection.kind,
    String(row.id),
    String((row.links as { self?: unknown } | undefined)?.self ?? ""),
  ),
});
const columnOf = (id: string): CollectionColumn | undefined =>
  props.collection.columns.find((column) => column.id === id);
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
const session = useSession();
const said = ref("");
// Emptied first: the same sentence twice is said twice.
const say = useSay((text) => {
  said.value = "";
  void nextTick(() => (said.value = text));
});
const edit = useInlineEdit({
  collection: () => props.collection,
  rows: () => rows.value,
  t: (key, values) => t(key, values ?? {}),
  say,
});
const active = ref<CellRef | undefined>();
const mayEdit = () => session.value.rights.has(editRight(props.collection.type));
useViewAction("create", {
  label: () => t("edit.createAction", { kind: kindWords.value }),
  allowed: mayEdit,
  run: () => openEdit({ mode: "create", collection: props.collection.id }),
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
  run: () => {
    if (activeRow.value) {
      openEdit({ mode: "edit", collection: props.collection.id, id: String(activeRow.value.id) });
    }
  },
});
const kindWords = computed(() => t(`labels.kind.${props.collection.type}`));
/** Why the cell of the focus offers no editor, said at the foot of the list. */
const activeWhy = computed(() =>
  active.value && active.value.row !== "" ? edit.why(active.value.column) : null,
);
/** The same reason as one sentence: the description of a cell without an editor. */
function readonlyReason(_row: Item, column: GridColumn): string | undefined {
  const why = edit.why(column.id);
  if (!why || why.kind === "none") return undefined;
  if (why.kind === "forbidden") return `${t("edit.why.forbidden")} ${why.permission}`;
  return t(why.kind === "past" ? "edit.why.past" : `edit.why.${why.reason}`);
}
const activeEditable = computed(
  () => active.value !== undefined && active.value.row !== "" && activeWhy.value === null,
);
/** The words of a key of a closed list, for the line of the motive. */
const choiceWords = (field: string, key: string) => words.value(field, key) ?? null;
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
  const ask = edit.asking.value;
  if (ask && (await edit.confirm(motive, value))) void afterAsk(edit.cellOf(ask));
}
function onCancelAsk(): void {
  const ask = edit.asking.value;
  edit.cancel();
  if (ask) void afterAsk(edit.cellOf(ask));
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
  void nextTick(() =>
    (picker.value?.$el as HTMLElement | undefined)
      ?.querySelector<HTMLElement>(".k-filter-picker__add")
      ?.focus(),
  );
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
    <!-- A property filter of the URL that no declaration types: no list read without it. -->
    <ErrorState
      v-if="blocked"
      :title="
        t(blocked.reason === 'failed' ? 'views.list.declarationsFailed' : 'views.list.untyped', {
          names: blocked.names.join(', '),
        })
      "
      :problem="blocked.reason === 'failed' ? problemOf(declarationsError) : undefined"
    >
      {{ t("views.list.notRead") }}
    </ErrorState>
    <ErrorState
      v-else-if="failed"
      :title="t('views.list.error', { name: title })"
      :problem="problemOf(list.error.value)"
      :retry-label="t('views.retry')"
      @retry="list.refetch()"
    >
      {{ t("views.errorText") }}
    </ErrorState>
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
      data-k-list
      :label="title"
      :columns="columns"
      :rows="edit.shownRows.value"
      row-key="id"
      :loading="list.isPending.value"
      :loading-rows="8"
      :filtered="filtered"
      :row-count="complete ? rows.length : undefined"
      :element-props="elementProps"
      :editable="edit.editable"
      :readonly-reason="readonlyReason"
      :validate="edit.validate"
      :cell-state="edit.cellState"
      :row-state="edit.rowState"
      @filter="onColumnFilter"
      @activate="onActivate"
      @commit="edit.onCommit"
      @window="onWindow"
    >
      <template #cell="{ row, column, value }">
        <KLink v-if="column.id === props.collection.columns[0]!.id" :to="recordOf(row)">{{
          nameOf(row)
        }}</KLink>
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
        <template v-else-if="typeof value === 'boolean'">{{
          t(`views.value.boolean.${value}`)
        }}</template>
        <template v-else>{{ fieldText(value) }}</template>
      </template>
    </DataGrid>
    <template #footer>
      <QuickApply
        v-if="edit.asking.value"
        :key="edit.asking.value.row + edit.asking.value.column"
        :ask="edit.asking.value"
        :words="choiceWords"
        @confirm="onConfirm"
        @cancel="onCancelAsk"
      />
      <template v-else>
        <span class="k-num">{{ list.isPending.value ? "" : countText }}</span>
        <span v-if="activeEditable" class="k-keyboard-hint">
          <Kbd :keys="['Enter']" /> {{ t("edit.hintDraft") }} ·
          <template v-if="edit.canApply.value">
            <Kbd :keys="['Control', 'Enter']" /> {{ t("edit.hintApply") }}
          </template>
          <template v-else>
            <Kbd :keys="['Control', 'Enter']" /> {{ t("edit.hintApplyForbidden") }}
            <CodeBadge :code="applyRight(props.collection.type)" />
          </template>
          · <Kbd :keys="['Escape']" /> {{ t("edit.hintCancel") }}
        </span>
        <span v-else-if="activeWhy && activeWhy.kind !== 'none'" class="k-keyboard-hint">
          <template v-if="activeWhy.kind === 'forbidden'">
            {{ t("edit.why.forbidden") }} <CodeBadge :code="activeWhy.permission" />
          </template>
          <template v-else-if="activeWhy.kind === 'past'">{{ t("edit.why.past") }}</template>
          <template v-else>{{ t(`edit.why.${activeWhy.reason}`) }}</template>
        </span>
      </template>
      <span role="status">{{ restarted ? t("views.list.restarted") : said }}</span>
    </template>
  </ViewPanel>
</template>
