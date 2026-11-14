<script setup lang="ts">
// « À instruire » (REQ-021-10; design 021 « Catalogue des écrans » : `/proposals`; maquette
// E4): the pending proposals the reader may validate (`GET /v1/proposals`, oldest first),
// each with the permissions its author lacks, leading to its record and its review
// actions. Its count is the badge of the entry in the sidebar, read again on the event
// `changeset`, never by polling.
import { CodeBadge, DataGrid, EmptyState, type CellRef, type GridColumn } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import KLink from "../prefetch/KLink.vue";
import NextPageAlert from "../problems/NextPageAlert.vue";
import ProblemState from "../problems/ProblemState.vue";
import { useFilteredList } from "../views/list.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import type { ProposalItem } from "./model.ts";
import { useChangesetWords } from "./words.ts";

const { t, locale } = useI18n();
const router = useRouter();
const words = useChangesetWords();

const title = computed(() => t("nav.proposals"));
const { list, rows, failed, nextFailed, refreshFailed, complete, restarted, onWindow } =
  useFilteredList<ProposalItem>(() => "/v1/proposals");

const columns = computed<GridColumn[]>(() => [
  { id: "motive", label: t("changesets.field.motive"), width: "fill", wrap: true },
  { id: "author", label: t("views.field.author"), width: "md", mono: true },
  { id: "required", label: t("changesets.field.required"), width: "lg", priority: 2 },
  { id: "submitted_at", label: t("changesets.field.submitted"), width: "md", mono: true },
]);

const recordOf = (row: ProposalItem) => `/changesets/${encodeURIComponent(row.id)}`;
function onActivate(cell: CellRef): void {
  const row = rows.value.find((r) => r.id === cell.row);
  if (row) void router.push(recordOf(row));
}
/** The permissions an author lacks, each once. */
const required = (row: ProposalItem) => [...new Set(row.required.map((r) => r.permission))];

const number = computed(() => new Intl.NumberFormat(locale.value));
const total = computed(() => (complete.value ? number.value.format(rows.value.length) : undefined));
const countText = computed(() =>
  t(
    complete.value ? "views.list.count" : "views.list.loaded",
    { n: number.value.format(rows.value.length) },
    rows.value.length,
  ),
);
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
      :title="t('views.list.error', { name: title })"
      :error="list.error.value"
      :retry="list.refetch"
    >
      {{ t("views.errorText") }}
    </ProblemState>
    <EmptyState
      v-else-if="list.isSuccess.value && rows.length === 0"
      :title="t('changesets.proposalsNone')"
    >
      {{ t("changesets.proposalsNoneText") }}
    </EmptyState>
    <DataGrid
      v-else
      :label="title"
      :columns="columns"
      :rows="rows"
      row-key="id"
      :loading="list.isPending.value"
      :loading-rows="6"
      :row-count="complete ? rows.length : undefined"
      @activate="onActivate"
      @window="onWindow"
    >
      <template #cell="{ row, column }">
        <KLink v-if="column.id === 'motive'" :to="recordOf(row)">{{ row.motive }}</KLink>
        <template v-else-if="column.id === 'author'">{{ words.person(row.author) }}</template>
        <span v-else-if="column.id === 'required'" class="k-proposals__required">
          <CodeBadge v-for="permission in required(row)" :key="permission" :code="permission" />
        </span>
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

<style>
@layer components {
  /* The permissions lacking, side by side, wrapping in their cell. */
  .k-proposals__required {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    min-inline-size: 0;
  }
}
</style>
