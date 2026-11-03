<script setup lang="ts">
// The audit journal (spec 006, REQ-006-4; 012/REQ-012-17; REQ-007-19, REQ-021-12; design
// 021 « Administration »): one `DataGrid` of `GET /v1/audit-events`, newest first, page by
// page, at the present whatever the dates of the page (the journal takes none), its typed
// filters in the URL (period by days, event, actor, object). « Exporter (NDJSON) » saves
// the same search as a file, written as it comes (`audit.ts`). A row chosen opens the event
// in the Détail tab of the right panel (`/admin/audit/:id`, the filters kept). Three states.
import {
  Button,
  DataGrid,
  EmptyState,
  FilterPicker,
  InlineAlert,
  StatusBadge,
  injectToasts,
  type CellRef,
  type GridColumn,
} from "@kartenn/ui";
import { computed, nextTick, ref, shallowRef, useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { revealDetail } from "../frame/detail.ts";
import { useSettings } from "../frame/settings.ts";
import NextPageAlert from "../problems/NextPageAlert.vue";
import ProblemState from "../problems/ProblemState.vue";
import { useProblems } from "../problems/index.ts";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useFilteredList } from "../views/list.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import { holdsGlobally, IDENTITIES_READ } from "./access.ts";
import { ExportCut, ExportStorage, exportAudit, outcomeBadge, type AuditEvent } from "./audit.ts";
import { useAuditWords } from "./auditWords.ts";

const props = defineProps<{ selected?: string | undefined }>();
const { t, te, locale } = useI18n();
const route = useRoute();
const router = useRouter();
const settings = useSettings();
const session = useSession();
const toasts = injectToasts();
const problems = useProblems();

const title = computed(() => t("nav.audit"));
/** Whether the reader may list the identities: their names, their suggestions. */
const readsIdentities = () => holdsGlobally(session.value.me, IDENTITIES_READ);
const {
  model,
  filters,
  onFilters,
  applied,
  sent,
  list,
  rows,
  failed,
  nextFailed,
  complete,
  restarted,
  onWindow,
} = useFilteredList<AuditEvent>(() => "/v1/audit-events", {
  dated: false,
  field: (name) => (te(`admin.audit.filter.${name}`) ? t(`admin.audit.filter.${name}`) : undefined),
  // Who may not list the identities types the actor (an identifier pasted): no read of
  // `/v1/identities` refused, and journaled as such, in the journal he reads.
  refine: (model) =>
    readsIdentities()
      ? model
      : model.map((p) =>
          p.key === "actor" ? { key: p.key, label: p.label, type: "text", operators: ["eq"] } : p,
        ),
});
const words = useAuditWords(readsIdentities);

const columns = computed<GridColumn[]>(() => [
  { id: "occurred_at", label: t("admin.audit.field.occurred"), width: "lg", mono: true },
  {
    id: "name",
    label: t("admin.audit.field.name"),
    width: "fill",
    mono: true,
    filterable: true,
  },
  // Below `md`, the event and its outcome stay; the actor and the objects in « Colonnes ».
  {
    id: "actor",
    label: t("admin.audit.field.actor"),
    width: "md",
    filterable: true,
    priority: 2,
  },
  { id: "outcome", label: t("admin.audit.field.outcome"), width: "md" },
  { id: "target", label: t("admin.audit.field.targets"), width: "md", priority: 3 },
]);
const filtered = computed(() => ["name", "actor"].filter((key) => applied.value.has(key)));

const picker = useTemplateRef<InstanceType<typeof FilterPicker>>("picker");
function onColumnFilter(id: string): void {
  const from = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  picker.value?.openFor(id, from);
}
useViewAction("filter", {
  label: () => t("views.list.filterAction", { name: title.value }),
  run: () => picker.value?.openAdd(),
});

/** Opens an event, the filters of the list kept; the one shown already, the panel again. */
function open(id: string): void {
  if (id === props.selected) revealDetail();
  else void router.push({ path: `/admin/audit/${encodeURIComponent(id)}`, query: route.query });
}
function onSelected(keys: string[]): void {
  const id = keys[0] ?? props.selected;
  if (id) open(id);
}
const onActivate = (cell: CellRef) => open(cell.row);

// The export of the search the list shows, saved as a file.
const exporting = ref(false);
const exportError = shallowRef<unknown>(null);
async function onExport(): Promise<void> {
  if (exporting.value) return;
  exporting.value = true;
  exportError.value = null;
  try {
    const name = await exportAudit(sent.value);
    toasts?.show({ tone: "ok", title: t("admin.audit.exported", { name }) });
  } catch (error) {
    exportError.value = error;
  } finally {
    exporting.value = false;
  }
}
const exportLines = computed(() => {
  if (exportError.value instanceof ExportCut) return [{ message: t("admin.audit.exportCut") }];
  if (exportError.value instanceof ExportStorage) {
    return [{ message: t("admin.audit.exportStorage") }];
  }
  return problems.lines(exportError.value);
});

const number = computed(() => new Intl.NumberFormat(locale.value));
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
    <template #actions>
      <Button variant="ghost" :loading="exporting" @click="onExport">{{
        t("admin.audit.export")
      }}</Button>
    </template>
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
    <InlineAlert
      v-if="exportError !== null"
      tone="ko"
      :title="t('admin.audit.exportFailed')"
      :items="exportLines"
    />
    <NextPageAlert v-if="nextFailed" :error="list.error.value" :retry="list.fetchNextPage" />
    <ProblemState
      v-if="failed"
      :title="t('admin.audit.error')"
      :error="list.error.value"
      :retry="list.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="list.isSuccess.value && rows.length === 0"
      :title="hasFilters ? t('views.list.noneFiltered') : t('admin.audit.none')"
    >
      {{ hasFilters ? t("views.list.noneFilteredText") : t("admin.audit.noneText") }}
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
      selectable="single"
      :selected="props.selected ? [props.selected] : []"
      :loading="list.isPending.value"
      :loading-rows="8"
      :filtered="filtered"
      :row-count="complete ? rows.length : undefined"
      @filter="onColumnFilter"
      @update:selected="onSelected"
      @activate="onActivate"
      @window="onWindow"
    >
      <template #cell="{ row, column }">
        <time v-if="column.id === 'occurred_at'" :datetime="row.occurred_at">{{
          words.when(row.occurred_at)
        }}</time>
        <template v-else-if="column.id === 'name'">{{ row.name }}</template>
        <span v-else-if="column.id === 'actor'" :class="{ 'k-mono': !words.actorNamed(row) }">{{
          words.actor(row)
        }}</span>
        <StatusBadge
          v-else-if="column.id === 'outcome'"
          :state="outcomeBadge(row.outcome)"
          :label="words.outcome(row.outcome)"
        />
        <template v-else-if="column.id === 'target'">{{ words.targets(row) }}</template>
      </template>
    </DataGrid>
    <template #footer>
      <span class="k-num">{{ list.isPending.value ? "" : countText }}</span>
      <span role="status">{{ restarted ? t("views.list.restarted") : "" }}</span>
    </template>
  </ViewPanel>
</template>
