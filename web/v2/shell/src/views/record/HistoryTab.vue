<script setup lang="ts">
// The history of an object (REQ-021-9; design 021 « Catalogue des écrans » : Historique;
// maquette E2): its bitemporal versions, the latest first, each its period of validity, the
// instant it was known, its value and the changeset that wrote it (a link). Read page by
// page; three states.
import { reads, type components } from "@kartenn/sdk";
import { DataGrid, EmptyState, type GridColumn, type GridWindow } from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useNextPage, type ListWindow } from "../../prefetch/nextPage.ts";
import { useTime } from "../../time/index.ts";
import { nameOf, type Item } from "../collections.ts";
import ProblemState from "../../problems/ProblemState.vue";
import { rowsOf } from "../data.ts";

type Version = components["schemas"]["Version"];
interface Row extends Record<string, unknown> {
  id: string;
  value: string;
  valid: string;
  known: string;
  changeset: string;
}

const props = defineProps<{ kind: string; id: string; name: string }>();
const { t } = useI18n();
const time = useTime();

const history = useInfiniteQuery(() =>
  reads.subPages<Version>(props.kind, props.id, "history", null, {
    validAt: time.value.validAt,
    asOf: time.value.asOf,
  }),
);
const view = ref<ListWindow>({ end: 0, size: 0 });
useNextPage(history, view);
const onWindow = (window: GridWindow) => (view.value = { end: window.end, size: window.size });

/** An instant as the API writes it, to the minute, in fixed pitch (UTC). */
const instant = (value: string | null | undefined) =>
  value ? value.replace("T", " ").replace(/:\d{2}(\.\d+)?Z$/, "Z") : "";
const rows = computed<Row[]>(() =>
  rowsOf(history.data.value).map((version) => ({
    // A version: what a changeset wrote, valid from, known from (stable as pages arrive).
    id: `${version.changeset.id}|${version.valid_from}|${version.asserted_from}`,
    value: nameOf(version.object as unknown as Item),
    valid: `${instant(version.valid_from)} → ${instant(version.valid_to)}`,
    known: instant(version.asserted_from),
    changeset: version.changeset.id,
  })),
);
const columns = computed<GridColumn[]>(() => [
  { id: "value", label: t("views.record.version"), width: "md", mono: true },
  { id: "valid", label: t("views.record.valid"), width: "lg", mono: true },
  { id: "known", label: t("views.record.known"), width: "md", mono: true, priority: 2 },
  { id: "changeset", label: t("views.record.changeset"), width: "fill", mono: true },
]);
</script>

<template>
  <ProblemState
    v-if="history.isError.value && !history.isFetchNextPageError.value"
    :title="t('views.record.historyError')"
    :error="history.error.value"
    :retry="history.refetch"
  />
  <EmptyState
    v-else-if="history.isSuccess.value && rows.length === 0"
    :title="t('views.record.noHistory')"
  />
  <DataGrid
    v-else
    :label="t('views.record.historyOf', { name: props.name })"
    :columns="columns"
    :rows="rows"
    row-key="id"
    :loading="history.isPending.value"
    :loading-rows="4"
    @window="onWindow"
  >
    <template #cell="{ row, column, value }">
      <KLink v-if="column.id === 'changeset'" :to="`/changesets/${row.changeset}`">{{
        value
      }}</KLink>
      <template v-else>{{ value }}</template>
    </template>
  </DataGrid>
</template>
