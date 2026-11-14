<script setup lang="ts">
// The declared properties (REQ-021-9; design 021 « Catalogue des écrans » : Propriétés
// déclarées; 003, 016): each its label and key, its type, its cardinality, the kinds that
// carry it, how it is inherited, whether it is expected or sensitive, and the values of a
// closed list on demand. Their gaps are a tab of the anomalies. Three states. Read page by
// page as the grid nears its end, like the other lists (task 021-11p7), never capped.
import { reads } from "@kartenn/sdk";
import {
  Cluster,
  DataGrid,
  EmptyState,
  IconButton,
  KeyLabel,
  NoValue,
  Popover,
  type GridColumn,
  type GridWindow,
} from "@kartenn/ui";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useInfiniteQuery } from "../data/query.ts";
import { useNextPage, type ListWindow } from "../prefetch/nextPage.ts";
import NextPageAlert from "../problems/NextPageAlert.vue";
import ProblemState from "../problems/ProblemState.vue";
import { rowsOf, useKeyLabel } from "./data.ts";
import type { DeclarationOfModel } from "./filters.ts";
import ViewPanel from "./parts/ViewPanel.vue";
import DeclarationValues from "./properties/DeclarationValues.vue";

type Row = DeclarationOfModel & {
  id: string;
  cardinality?: string;
  inherit?: string | null;
  sensitive?: boolean;
  expected?: { mode: string; source: string } | null;
};

const { t, te, locale } = useI18n();
const router = useRouter();
const keyLabel = useKeyLabel();
const list = useInfiniteQuery(() => reads.list("/v1/property-declarations", null, null));
const view = ref<ListWindow>({ end: 0, size: 0 });
useNextPage(list, view);
const onWindow = (window: GridWindow) => (view.value = { end: window.end, size: window.size });
const rows = computed(() => rowsOf(list.data.value) as unknown as Row[]);
const failed = computed(() => list.isError.value && !list.isFetchNextPageError.value);
const complete = computed(() => list.isSuccess.value && !list.hasNextPage.value);
/** Whole once every page is read; while pages remain, those loaded so far. */
const count = computed(() => {
  if (list.data.value === undefined) return undefined;
  const n = new Intl.NumberFormat(locale.value).format(rows.value.length);
  return complete.value ? n : t("views.list.loaded", { n }, rows.value.length);
});

const columns = computed<GridColumn[]>(() => [
  { id: "key", label: t("views.record.property"), width: "fill" },
  { id: "type", label: t("views.properties.type"), width: "sm", mono: true },
  { id: "cardinality", label: t("views.properties.cardinality"), width: "sm", priority: 2 },
  { id: "carriers", label: t("views.properties.carriers"), width: "md", priority: 2 },
  { id: "inherit", label: t("views.properties.inherit"), width: "sm", priority: 3 },
  { id: "flags", label: t("views.properties.flags"), width: "md", priority: 3 },
  { id: "actions", label: t("views.actions"), width: "sm", actions: true },
]);
/** A value of the contract in words (`cardinalities`, `inheritance`), else the value itself. */
const words = (group: string, value: string) => {
  const key = `views.properties.${group}.${value}`;
  return te(key) ? t(key) : value;
};
/** The words of a type, `null` for a type the catalogs lack: its key alone, then. */
const typeWords = (type: string) => {
  const said = words("types", type);
  return said === type ? null : said;
};
const carriers = (row: Row) =>
  row.carriers.length === 0
    ? t("views.properties.everyKind")
    : row.carriers.map((kind) => keyLabel("kind", kind) ?? kind).join(", ");
const flags = (row: Row) =>
  [
    row.expected ? t("views.properties.expected") : null,
    row.sensitive ? t("views.properties.sensitive") : null,
  ]
    .filter(Boolean)
    .join(", ") || null;
const gaps = (key: string) =>
  void router.push({ path: "/anomalies", query: { tab: "gaps", property: key } });
</script>

<template>
  <h1 class="k-visually-hidden">{{ t("nav.properties") }}</h1>
  <ViewPanel :title="t('nav.properties')" :detail="count" main flush>
    <!-- A next page refused: the rows read stay shown, that page retried alone. -->
    <NextPageAlert
      v-if="list.isFetchNextPageError.value"
      :error="list.error.value"
      :retry="list.fetchNextPage"
    />
    <ProblemState
      v-if="failed"
      :title="t('views.properties.error')"
      :error="list.error.value"
      :retry="list.refetch"
    />
    <EmptyState v-else-if="complete && rows.length === 0" :title="t('views.properties.none')" />
    <DataGrid
      v-else
      :label="t('nav.properties')"
      :columns="columns"
      :rows="rows as unknown as Record<string, unknown>[]"
      row-key="id"
      :loading="list.isPending.value"
      :loading-rows="8"
      :row-count="complete ? rows.length : undefined"
      @window="onWindow"
    >
      <template #cell="{ row, column }">
        <KeyLabel
          v-if="column.id === 'key'"
          :value="(row as Row).key"
          :label="keyLabel('property', (row as Row).key)"
          show-key
        />
        <!-- The type in words (mock-up E13), its key at hand as in the key column. -->
        <KeyLabel
          v-else-if="column.id === 'type'"
          :value="(row as Row).type"
          :label="typeWords((row as Row).type)"
        />
        <template v-else-if="column.id === 'cardinality' && (row as Row).cardinality">{{
          words("cardinalities", String((row as Row).cardinality ?? ""))
        }}</template>
        <template v-else-if="column.id === 'carriers'">{{ carriers(row as Row) }}</template>
        <template v-else-if="column.id === 'inherit' && (row as Row).inherit">{{
          words("inheritance", String((row as Row).inherit))
        }}</template>
        <template v-else-if="column.id === 'flags' && flags(row as Row)">{{
          flags(row as Row)
        }}</template>
        <NoValue v-else-if="['cardinality', 'inherit', 'flags'].includes(column.id)" />
        <Cluster v-else gap="1" :wrap="false">
          <Popover
            v-if="(row as Row).type === 'enum'"
            :title="t('views.properties.valuesOf', { name: (row as Row).key })"
          >
            <template #trigger>
              <IconButton
                icon="tag"
                :label="t('views.properties.valuesOf', { name: (row as Row).key })"
                size="sm"
                variant="ghost"
              />
            </template>
            <DeclarationValues :property="(row as Row).key" />
          </Popover>
          <IconButton
            v-if="(row as Row).expected"
            icon="triangle-alert"
            :label="t('views.properties.gapsOf', { name: (row as Row).key })"
            size="sm"
            variant="ghost"
            @click="gaps((row as Row).key)"
          />
        </Cluster>
      </template>
    </DataGrid>
  </ViewPanel>
</template>
