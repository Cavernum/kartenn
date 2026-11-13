<script setup lang="ts">
// The history of an object (REQ-021-9; design 021 « Catalogue des écrans » : Historique;
// maquette E2): its bitemporal versions, the latest first, each its period of validity, the
// instant it was known, its value, the motive of the changeset that wrote it (a link to
// it) and its author (task 021-11p5). Read page by page; three states.
// The author is named as the server serves it (`Version.author.name`, read from the
// identities at each request: « personne #… » once pseudonymised, Q-021-29), never from a
// name kept by the client; the history is read again at each use, at every date (the SDK
// never freezes it, `isRevalidated`). Under a narrow container the rows stack, each
// attribute on a line of its own.
import { reads, type components } from "@kartenn/sdk";
import { DataGrid, EmptyState, type GridColumn, type GridWindow } from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useNextPage, type ListWindow } from "../../prefetch/nextPage.ts";
import { useTime } from "../../time/index.ts";
import { nameOf, type Item } from "../collections.ts";
import NextPageAlert from "../../problems/NextPageAlert.vue";
import ProblemState from "../../problems/ProblemState.vue";
import { rowsOf } from "../data.ts";

type Version = components["schemas"]["Version"];
interface Row extends Record<string, unknown> {
  id: string;
  value: string;
  valid: string;
  known: string;
  motive: string;
  author: string;
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
    motive: version.motive,
    // As served: a pseudonymised person reads « personne #… », never a name of the cache.
    author: version.author.name,
    changeset: version.changeset.id,
  })),
);
// The motive and its author first after the dates (the subject of a version); the instant
// it was known leaves first as the container narrows, then the author; the identifier of
// the changeset is one choice away (the motive links to it).
const columns = computed<GridColumn[]>(() => [
  { id: "value", label: t("views.record.versionValue"), width: "md", mono: true },
  { id: "valid", label: t("views.record.valid"), width: "lg", mono: true },
  { id: "known", label: t("views.record.known"), width: "md", mono: true, priority: 3 },
  { id: "motive", label: t("views.record.versionMotive"), width: "fill", wrap: true },
  { id: "author", label: t("views.record.versionAuthor"), width: "md", priority: 2 },
  { id: "changeset", label: t("views.record.changeset"), width: "md", mono: true, priority: 3 },
]);
</script>

<template>
  <NextPageAlert
    v-if="history.isFetchNextPageError.value"
    :error="history.error.value"
    :retry="history.fetchNextPage"
  />
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
    stack
    @window="onWindow"
  >
    <template #cell="{ row, column, value }">
      <!-- The motive names the changeset that wrote the version: its link (« Changeset » and
           its identifier when the motive is blank, never an empty link). -->
      <KLink v-if="column.id === 'motive'" :to="`/changesets/${row.changeset}`">
        <template v-if="String(value).trim()">{{ value }}</template>
        <template v-else
          >{{ t("views.record.changeset") }}
          <span class="k-mono">{{ row.changeset }}</span></template
        >
      </KLink>
      <template v-else>{{ value }}</template>
    </template>
  </DataGrid>
</template>
