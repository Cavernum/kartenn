<script setup lang="ts">
// The imports of the user (spec 006, REQ-006-17 to 19; REQ-007-19, REQ-021-12; space
// `changes`): one `DataGrid` of `GET /v1/imports`, the most recent first, page by page, at
// the present (the list takes neither filter nor date): its source, its state, its
// elements and its batches. An import is opened by the API (`POST /v1/imports`), chunk by
// chunk; a row chosen opens its follow-up in the Détail tab of the right panel
// (`/imports/:id`). Three states.
import { DataGrid, EmptyState, StatusBadge, type CellRef, type GridColumn } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { revealDetail } from "../frame/detail.ts";
import NextPageAlert from "../problems/NextPageAlert.vue";
import ProblemState from "../problems/ProblemState.vue";
import { useFilteredList } from "../views/list.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import { importBadge, type ImportItem } from "./imports.ts";

const props = defineProps<{ selected?: string | undefined }>();
const { t, locale } = useI18n();
const router = useRouter();

const title = computed(() => t("nav.imports"));
const { list, rows, failed, nextFailed, refreshFailed, complete, onWindow } =
  useFilteredList<ImportItem>(() => "/v1/imports", { dated: false });

const number = computed(() => new Intl.NumberFormat(locale.value));
const columns = computed<GridColumn[]>(() => [
  { id: "source", label: t("admin.imports.source"), width: "fill", mono: true },
  { id: "state", label: t("admin.imports.state"), width: "md" },
  {
    id: "elements",
    label: t("admin.imports.elements"),
    width: "sm",
    align: "end",
    mono: true,
    priority: 2,
  },
  { id: "batches", label: t("admin.imports.batches"), width: "lg", priority: 2 },
]);
const stateWords = (state: string) => t(`admin.imports.states.${state}`);
const batchWords = (row: ImportItem) =>
  t("admin.imports.applied", {
    applied: number.value.format(row.batches.applied),
    remaining: number.value.format(row.batches.remaining),
  });

/** Opens an import; the one shown already, the panel reopened on it. */
function open(id: string): void {
  if (id === props.selected) revealDetail();
  else void router.push(`/imports/${encodeURIComponent(id)}`);
}
function onSelected(keys: string[]): void {
  const id = keys[0] ?? props.selected;
  if (id) open(id);
}
const onActivate = (cell: CellRef) => open(cell.row);
const total = computed(() => (complete.value ? number.value.format(rows.value.length) : undefined));
</script>

<template>
  <h1 class="k-visually-hidden">{{ title }}</h1>
  <ViewPanel :title="title" :detail="total" main flush>
    <NextPageAlert v-if="nextFailed" :error="list.error.value" :retry="list.fetchNextPage" />
    <NextPageAlert
      v-else-if="refreshFailed"
      :title="t('problem.readAgain')"
      :error="list.error.value"
      :retry="list.refetch"
    />
    <ProblemState
      v-if="failed"
      :title="t('admin.imports.error')"
      :error="list.error.value"
      :retry="list.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="list.isSuccess.value && rows.length === 0"
      :title="t('admin.imports.none')"
    >
      {{ t("admin.imports.noneText") }}
    </EmptyState>
    <DataGrid
      v-else
      :label="title"
      :columns="columns"
      :rows="rows"
      row-key="id"
      selectable="single"
      :selected="props.selected ? [props.selected] : []"
      :loading="list.isPending.value"
      :loading-rows="6"
      :row-count="complete ? rows.length : undefined"
      @update:selected="onSelected"
      @activate="onActivate"
      @window="onWindow"
    >
      <template #cell="{ row, column }">
        <StatusBadge
          v-if="column.id === 'state'"
          :state="importBadge(row.state)"
          :label="stateWords(row.state)"
        />
        <template v-else-if="column.id === 'elements'">{{ number.format(row.elements) }}</template>
        <template v-else-if="column.id === 'batches'">{{ batchWords(row) }}</template>
        <template v-else>{{ row.source }}</template>
      </template>
    </DataGrid>
  </ViewPanel>
</template>
