<script setup lang="ts">
// The changesets (REQ-021-10; design 021 « Écrans » : Changesets; maquette E4): one
// `DataGrid` of `GET /v1/changesets`, newest first, page by page, its typed filters in the
// URL (state, author, object, submission, retroactive) and « Mes changesets », the filter of
// the author set to the reader. Each row leads to its record. Three states, as every list.
import {
  Button,
  DataGrid,
  EmptyState,
  FilterPicker,
  KeyLabel,
  StatusBadge,
  type CellRef,
  type GridColumn,
} from "@kartenn/ui";
import { computed, nextTick, useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useSettings } from "../frame/settings.ts";
import KLink from "../prefetch/KLink.vue";
import NextPageAlert from "../problems/NextPageAlert.vue";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useFilteredList } from "../views/list.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import { stateBadge, type ChangesetItem } from "./model.ts";
import { useChangesetWords } from "./words.ts";

const { t, locale } = useI18n();
const router = useRouter();
const settings = useSettings();
const session = useSession();
const words = useChangesetWords();

const title = computed(() => t("nav.changesets"));
const {
  model,
  filters,
  onFilters,
  applied,
  list,
  rows,
  failed,
  nextFailed,
  complete,
  restarted,
  onWindow,
} = useFilteredList<ChangesetItem>(() => "/v1/changesets");

const columns = computed<GridColumn[]>(() => [
  { id: "motive", label: t("changesets.field.motive"), width: "fill", wrap: true },
  { id: "state", label: t("views.field.state"), width: "md", filterable: true },
  { id: "kind", label: t("changesets.field.kind"), width: "md", priority: 2 },
  { id: "author", label: t("views.field.author"), width: "md", mono: true, filterable: true },
  { id: "submitted_at", label: t("changesets.field.submitted"), width: "md", mono: true },
]);
const filtered = computed(() => ["state", "author"].filter((key) => applied.value.has(key)));

const picker = useTemplateRef<InstanceType<typeof FilterPicker>>("picker");
function onColumnFilter(id: string): void {
  const from = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  picker.value?.openFor(id, from);
}
useViewAction("filter", {
  label: () => t("views.list.filterAction", { name: title.value }),
  run: () => picker.value?.openAdd(),
});

// « Mes changesets »: the filter of the author, set to the reader or taken off.
const me = computed(() => session.value.me?.identity ?? null);
const mine = computed(() =>
  filters.value.some(
    (f) => !f.draft && f.key === "author" && f.values.length === 1 && f.values[0] === me.value,
  ),
);
function toggleMine(): void {
  const others = filters.value.filter((f) => f.key !== "author");
  onFilters(
    mine.value || me.value === null
      ? others
      : [...others, { key: "author", operator: "eq", values: [me.value] }],
  );
}

const recordOf = (row: ChangesetItem) => `/changesets/${encodeURIComponent(row.id)}`;
function onActivate(cell: CellRef): void {
  const row = rows.value.find((r) => r.id === cell.row);
  if (row) void router.push(recordOf(row));
}

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
      <Button v-if="me" variant="ghost" icon="user" :pressed="mine" @click="toggleMine">{{
        t("changesets.mine")
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
    <NextPageAlert v-if="nextFailed" :error="list.error.value" :retry="list.fetchNextPage" />
    <ProblemState
      v-if="failed"
      :title="t('views.list.error', { name: title })"
      :error="list.error.value"
      :retry="list.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="list.isSuccess.value && rows.length === 0"
      :title="hasFilters ? t('views.list.noneFiltered') : t('changesets.none')"
    >
      {{ hasFilters ? t("views.list.noneFilteredText") : t("changesets.noneText") }}
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
      @filter="onColumnFilter"
      @activate="onActivate"
      @window="onWindow"
    >
      <template #cell="{ row, column }">
        <KLink v-if="column.id === 'motive'" :to="recordOf(row)">{{ row.motive }}</KLink>
        <StatusBadge
          v-else-if="column.id === 'state'"
          :state="stateBadge(row.state)"
          :label="words.state(row.state) ?? row.state"
        />
        <KeyLabel
          v-else-if="column.id === 'kind'"
          :value="row.kind"
          :label="words.kind(row.kind)"
          plain
        />
        <template v-else-if="column.id === 'author'">{{ words.person(row.author) }}</template>
        <time v-else-if="column.id === 'submitted_at'" :datetime="row.submitted_at">{{
          words.when(row.submitted_at)
        }}</time>
      </template>
    </DataGrid>
    <template #footer>
      <span class="k-num">{{ list.isPending.value ? "" : countText }}</span>
      <span role="status">{{ restarted ? t("views.list.restarted") : "" }}</span>
    </template>
  </ViewPanel>
</template>
