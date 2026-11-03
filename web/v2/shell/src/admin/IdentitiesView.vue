<script setup lang="ts">
// The identities (005; spec 025, REQ-025-11; REQ-021-12; design 021 « Administration »;
// maquette E7, vue `identite`): one `DataGrid` of `GET /v1/identities`, by name, its kind
// (person, service account, break-glass account), its state and its issuer; no column of
// the last login (not served by the list). A row chosen opens the identity in the Détail
// tab of the right panel (`/admin/identities/:id`); one whose state the draft changes is
// marked. Three states.
import {
  DataGrid,
  EmptyState,
  NoValue,
  StatusBadge,
  type CellRef,
  type GridColumn,
  type GridSort,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { revealDetail } from "../frame/detail.ts";
import ProblemState from "../problems/ProblemState.vue";
import ViewPanel from "../views/parts/ViewPanel.vue";
import { draftedStates } from "./access.ts";
import { useAdminDraft } from "./draft.ts";
import type { IdentityItem } from "./model.ts";
import { useList } from "./reads.ts";

const props = defineProps<{ selected?: string | undefined }>();
const { t, locale } = useI18n();
const router = useRouter();
const draft = useAdminDraft();

const title = computed(() => t("nav.identities"));
const { query, rows } = useList("/v1/identities");
type Row = IdentityItem & { sort: string; state: string };
const kindOf = (kind: string) => t(`admin.members.kind.${kind}`);
const shown = computed<Row[]>(() =>
  rows.value.map((i) => ({ ...i, sort: kindOf(i.kind), state: i.disabled ? "1" : "0" })),
);
const sort = ref<GridSort>({ column: "name", direction: "ascending" });
const columns = computed<GridColumn[]>(() => [
  { id: "name", label: t("admin.identities.identity"), width: "lg", sortable: true },
  { id: "sort", label: t("admin.identities.kind"), width: "md", sortable: true, priority: 3 },
  { id: "state", label: t("admin.field.state"), width: "md", sortable: true },
  {
    id: "issuer",
    label: t("admin.members.issuer"),
    width: "fill",
    mono: true,
    sortable: true,
    priority: 2,
  },
]);
const sorted = computed(() => {
  const { column, direction } = sort.value;
  const sign = direction === "descending" ? -1 : 1;
  const value = (i: Row) => String(i[column as keyof Row] ?? "");
  return [...shown.value].sort((a, b) => sign * value(a).localeCompare(value(b), locale.value));
});
const rowState = (row: Row) =>
  draftedStates(draft.operations.value, row.id).length > 0 ? ("draft" as const) : undefined;

/** Opens an identity; the one shown already, the panel reopened on it. */
function open(id: string): void {
  if (id === props.selected) revealDetail();
  else void router.push(`/admin/identities/${encodeURIComponent(id)}`);
}
function onSelected(keys: string[]): void {
  const id = keys[0] ?? props.selected;
  if (id) open(id);
}
const onActivate = (cell: CellRef) => open(cell.row);
const total = computed(() =>
  query.isSuccess.value ? new Intl.NumberFormat(locale.value).format(rows.value.length) : undefined,
);
</script>

<template>
  <h1 class="k-visually-hidden">{{ title }}</h1>
  <ViewPanel :title="title" :detail="total" main flush>
    <ProblemState
      v-if="query.isError.value"
      :title="t('admin.identities.error')"
      :error="query.error.value"
      :retry="query.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="query.isSuccess.value && rows.length === 0"
      :title="t('admin.identities.none')"
    >
      {{ t("admin.identities.noneText") }}
    </EmptyState>
    <DataGrid
      v-else
      v-model:sort="sort"
      :label="title"
      :columns="columns"
      :rows="sorted"
      row-key="id"
      selectable="single"
      :selected="props.selected ? [props.selected] : []"
      :row-state="rowState"
      :loading="query.isPending.value"
      :loading-rows="6"
      stack
      @update:selected="onSelected"
      @activate="onActivate"
    >
      <template #cell="{ row, column, value }">
        <StatusBadge
          v-if="column.id === 'state'"
          :state="row.disabled ? 'neutral' : 'ok'"
          :label="row.disabled ? t('admin.identities.disabled') : t('admin.identities.enabled')"
        />
        <NoValue v-else-if="value === null || value === undefined" />
        <template v-else>{{ value }}</template>
      </template>
    </DataGrid>
  </ViewPanel>
</template>
