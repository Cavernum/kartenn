<script setup lang="ts">
// The content of a domain or of a prefix (REQ-021-9; maquette E2, onglet Contenu): the
// prefixes at the top of a domain (`roots`), the direct children of a prefix (prefixes,
// ranges, addresses: `children`), each a link to its record, read page by page at the
// dates of the page; three states.
import { reads, type components } from "@kartenn/sdk";
import {
  DataGrid,
  EmptyState,
  ErrorState,
  Icon,
  KeyLabel,
  type GridColumn,
  type GridWindow,
  type IconName,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import KLink from "../../prefetch/KLink.vue";
import { useNextPage, type ListWindow } from "../../prefetch/nextPage.ts";
import { useTime } from "../../time/index.ts";
import { pageOfHref, pageOfObject } from "../collections.ts";
import { problemOf, rowsOf, useKeyLabel } from "../data.ts";

type TreeNode = components["schemas"]["TreeNode"];
interface Row extends Record<string, unknown> {
  id: string;
  kind: "prefix" | "range" | "address";
  value: string;
  detail: string | null;
  to: string | null;
}

const props = defineProps<{ kind: "domain" | "prefix"; id: string; name: string }>();
const { t, te } = useI18n();
const time = useTime();
const keyLabel = useKeyLabel();

const content = useInfiniteQuery(() =>
  reads.subPages<TreeNode>(
    props.kind,
    props.id,
    props.kind === "domain" ? "roots" : "children",
    null,
    {
      validAt: time.value.validAt,
      asOf: time.value.asOf,
    },
  ),
);
const view = ref<ListWindow>({ end: 0, size: 0 });
useNextPage(content, view);
const onWindow = (window: GridWindow) => (view.value = { end: window.end, size: window.size });

const ICONS: Record<Row["kind"], IconName> = {
  prefix: "prefix",
  range: "range",
  address: "address",
};
const rows = computed<Row[]>(() =>
  rowsOf(content.data.value).map((node) => ({
    id: node.id,
    kind: node.kind,
    value: node.kind === "range" ? `${node.first} – ${node.last}` : node.value,
    detail: node.kind === "prefix" ? node.nature : node.kind === "range" ? node.usage : null,
    to: pageOfHref(node.href) ?? pageOfObject(node.kind, node.id),
  })),
);
const columns = computed<GridColumn[]>(() => [
  { id: "value", label: t("views.record.object"), width: "fill", mono: true },
  { id: "detail", label: t("views.record.nature"), width: "md", priority: 2 },
]);
const kindWords = (kind: string) => keyLabel("kind", `core.${kind}`) ?? kind;
const detailWords = (row: Row) => {
  const key = `views.value.${row.kind === "prefix" ? "nature" : "usage"}.${row.detail}`;
  return te(key) ? t(key) : null;
};
</script>

<template>
  <ErrorState
    v-if="content.isError.value && !content.isFetchNextPageError.value"
    :title="t('views.record.contentError')"
    :problem="problemOf(content.error.value)"
    :retry-label="t('views.retry')"
    @retry="content.refetch()"
  />
  <EmptyState
    v-else-if="content.isSuccess.value && rows.length === 0"
    :title="t('views.record.noContent')"
  >
    {{ t("views.record.noContentText") }}
  </EmptyState>
  <DataGrid
    v-else
    :label="t('views.record.contentOf', { name: props.name })"
    :columns="columns"
    :rows="rows"
    row-key="id"
    :loading="content.isPending.value"
    :loading-rows="4"
    @window="onWindow"
  >
    <template #cell="{ row, column }">
      <template v-if="column.id === 'value'">
        <Icon :name="ICONS[row.kind]" meaningful :label="kindWords(row.kind)" />
        <KLink v-if="row.to" :to="row.to">{{ row.value }}</KLink>
        <template v-else>{{ row.value }}</template>
      </template>
      <KeyLabel v-else-if="row.detail" :value="row.detail" :label="detailWords(row)" plain />
    </template>
  </DataGrid>
</template>
