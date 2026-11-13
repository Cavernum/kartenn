<script setup lang="ts">
// The gaps (REQ-021-9; 016): the properties an object should carry and lacks, filtered by
// property, domain and entity in the URL, page by page. Three states. Each gap composed by
// the screen from its members (021/A36): the expected and resolved values by their labels,
// the source and its revision, the rule by its code; the server's sentence is not shown.
// `resolved: null` alone says nothing: a gap evaluated since its values are stored always
// carries `source` and `expected`; masked, both values are left out (`expected` too);
// stored before them (migration 0075), `source` as well. Only a resolved `null` beside an
// expected value is « no value ».
import {
  DataGrid,
  EmptyState,
  FilterPicker,
  KeyLabel,
  NoValue,
  Stack,
  type GridColumn,
} from "@kartenn/ui";
import { computed, useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { useSettings } from "../../frame/settings.ts";
import { useViewAction } from "../../shortcuts/view.ts";
import NextPageAlert from "../../problems/NextPageAlert.vue";
import ProblemState from "../../problems/ProblemState.vue";
import { useDeclarations, useKeyLabel } from "../data.ts";
import { useFilteredList } from "../list.ts";
import ObjectName from "../record/ObjectName.vue";
import PropertyValue from "../record/PropertyValue.vue";
import { gapDetail as detailOf, noValue as empty, type Gap } from "./gap.ts";

const { t, locale } = useI18n();
const settings = useSettings();
const keyLabel = useKeyLabel();
const { declarations } = useDeclarations();
const { model, filters, onFilters, list, rows, failed, nextFailed, onWindow } =
  useFilteredList<Gap>(() => "/v1/property-gaps");
const picker = useTemplateRef<InstanceType<typeof FilterPicker>>("picker");
useViewAction("filter", {
  label: () => t("views.list.filterAction", { name: t("views.anomalies.gaps") }),
  run: () => picker.value?.openAdd(),
});
const columns = computed<GridColumn[]>(() => [
  { id: "property", label: t("views.record.property"), width: "md" },
  { id: "expected", label: t("views.anomalies.expected"), width: "lg", wrap: true },
  { id: "resolved", label: t("views.anomalies.resolved"), width: "lg", wrap: true },
  { id: "source", label: t("views.anomalies.source"), width: "md", priority: 2 },
  { id: "object", label: t("views.anomalies.object"), width: "fill", priority: 2 },
  { id: "rule", label: t("views.anomalies.rule"), width: "md", priority: 3 },
  { id: "opened_at", label: t("views.anomalies.opened"), width: "md", mono: true, priority: 3 },
]);
const when = (iso: string) =>
  new Intl.DateTimeFormat(locale.value, { dateStyle: "short", timeStyle: "short" }).format(
    new Date(iso),
  );
const gapOf = (row: Record<string, unknown>) => row as unknown as Gap;
const valueOf = (gap: Gap, column: string): unknown =>
  column === "expected" ? gap.expected : gap.resolved;
const byKey = computed(() => new Map(declarations.value.map((d) => [d.key, d])));
const declarationOf = (key: string | null | undefined) => (key ? byKey.value.get(key) : undefined);
const targetOf = (key: string | null | undefined) => {
  const target = declarationOf(key)?.params?.target;
  return typeof target === "string" ? target : undefined;
};
</script>

<template>
  <Stack gap="3">
    <FilterPicker
      ref="picker"
      :model-value="filters"
      :model="model"
      :label="t('views.list.filters', { name: t('views.anomalies.gaps') })"
      :shortcut="settings.singleKeys.value ? ['/'] : undefined"
      @update:model-value="onFilters"
    />
    <NextPageAlert v-if="nextFailed" :error="list.error.value" :retry="list.fetchNextPage" />
    <ProblemState
      v-if="failed"
      :title="t('views.anomalies.gapsError')"
      :error="list.error.value"
      :retry="list.refetch"
    />
    <EmptyState
      v-else-if="list.isSuccess.value && rows.length === 0"
      icon="circle-check"
      :title="t('views.anomalies.noGaps')"
    >
      {{ t("views.anomalies.noGapsText") }}
    </EmptyState>
    <DataGrid
      v-else
      :label="t('views.anomalies.gaps')"
      :columns="columns"
      :rows="rows as unknown as Record<string, unknown>[]"
      row-key="id"
      :loading="list.isPending.value"
      :loading-rows="4"
      @window="onWindow"
    >
      <template #cell="{ row, column }">
        <template v-if="column.id === 'property'">
          <KeyLabel
            v-if="gapOf(row).property"
            :value="gapOf(row).property!"
            :label="keyLabel('property', gapOf(row).property!)"
            show-key
          />
          <NoValue v-else />
        </template>
        <template v-else-if="column.id === 'expected' || column.id === 'resolved'">
          <span v-if="detailOf(gapOf(row)) === 'masked'">{{ t("views.record.masked") }}</span>
          <span v-else-if="detailOf(gapOf(row)) === 'unevaluated'">{{
            t("views.anomalies.unevaluated")
          }}</span>
          <NoValue v-else-if="empty(valueOf(gapOf(row), column.id))" />
          <PropertyValue
            v-else
            :property="gapOf(row).property ?? ''"
            :value="valueOf(gapOf(row), column.id)"
            :type="declarationOf(gapOf(row).property)?.type"
            :target="targetOf(gapOf(row).property)"
          />
        </template>
        <template v-else-if="column.id === 'source'">
          <template v-if="gapOf(row).source">
            <KeyLabel :value="gapOf(row).source!" />
            <template v-if="gapOf(row).revision">
              {{ ` ${t("views.anomalies.revision")} `
              }}<span class="k-mono">{{ gapOf(row).revision }}</span>
            </template>
          </template>
          <NoValue v-else />
        </template>
        <ObjectName
          v-else-if="column.id === 'object'"
          :id="gapOf(row).object.id"
          :kind="gapOf(row).object.kind"
          :href="gapOf(row).object.href"
        />
        <KeyLabel
          v-else-if="column.id === 'rule'"
          :value="gapOf(row).rule"
          :label="keyLabel('rule', gapOf(row).rule)"
          show-key
        />
        <time v-else-if="column.id === 'opened_at'" :datetime="gapOf(row).opened_at">{{
          when(gapOf(row).opened_at)
        }}</time>
      </template>
    </DataGrid>
  </Stack>
</template>
