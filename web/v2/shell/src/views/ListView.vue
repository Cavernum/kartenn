<script setup lang="ts">
// A list of the referential (REQ-021-9, 18; design 021 « Écrans » : Référentiel, « Filtres,
// libellés, codes »; maquette E3): one `DataGrid` of a collection of the core, read page by
// page through the cache at the dates of the page, the next page read before the window
// reaches the end (REQ-021-22), and the typed filters of its route in the URL — one model
// for the bar and the filters of the columns (020 decision A7). Three states: skeleton
// rows, an empty list with a sentence (and the way out of the filters), an error with its
// codes. A refused cursor starts the list again from its first page.
import {
  Button,
  DataGrid,
  EmptyState,
  ErrorState,
  FilterPicker,
  KeyLabel,
  type CellRef,
  type GridColumn,
} from "@kartenn/ui";
import { computed, nextTick, useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useSettings } from "../frame/settings.ts";
import { gridColumns, objectRef, useExtensions } from "../plugins/slots.ts";
import KLink from "../prefetch/KLink.vue";
import { useViewAction } from "../shortcuts/view.ts";
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
    <ErrorState
      v-if="failed"
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
      :label="title"
      :columns="columns"
      :rows="rows"
      row-key="id"
      :loading="list.isPending.value"
      :loading-rows="8"
      :filtered="filtered"
      :row-count="complete ? rows.length : undefined"
      :element-props="elementProps"
      @filter="onColumnFilter"
      @activate="onActivate"
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
      <span class="k-num">{{ list.isPending.value ? "" : countText }}</span>
      <span role="status">{{ restarted ? t("views.list.restarted") : "" }}</span>
    </template>
  </ViewPanel>
</template>
