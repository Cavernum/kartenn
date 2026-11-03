<script setup lang="ts">
// The gaps (REQ-021-9; 016): the properties an object should carry and lacks, filtered by
// property, domain and entity in the URL, page by page. Three states.
import type { components } from "@kartenn/sdk";
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
import { useKeyLabel } from "../data.ts";
import { useFilteredList } from "../list.ts";
import ObjectName from "../record/ObjectName.vue";

type Gap = components["schemas"]["Gap"];

const { t, locale } = useI18n();
const settings = useSettings();
const keyLabel = useKeyLabel();
const { model, filters, onFilters, list, rows, failed, nextFailed, onWindow } =
  useFilteredList<Gap>(() => "/v1/property-gaps");
const picker = useTemplateRef<InstanceType<typeof FilterPicker>>("picker");
useViewAction("filter", {
  label: () => t("views.list.filterAction", { name: t("views.anomalies.gaps") }),
  run: () => picker.value?.openAdd(),
});
const columns = computed<GridColumn[]>(() => [
  { id: "property", label: t("views.record.property"), width: "md" },
  { id: "message", label: t("views.anomalies.message"), width: "fill", wrap: true },
  { id: "object", label: t("views.anomalies.object"), width: "md", priority: 2 },
  { id: "opened_at", label: t("views.anomalies.opened"), width: "md", mono: true, priority: 3 },
]);
const when = (iso: string) =>
  new Intl.DateTimeFormat(locale.value, { dateStyle: "short", timeStyle: "short" }).format(
    new Date(iso),
  );
const gapOf = (row: Record<string, unknown>) => row as unknown as Gap;
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
        <KeyLabel
          v-if="column.id === 'property' && gapOf(row).property"
          :value="gapOf(row).property!"
          :label="keyLabel('property', gapOf(row).property!)"
          show-key
        />
        <NoValue v-else-if="column.id === 'property'" />
        <template v-else-if="column.id === 'message'">{{ gapOf(row).message }}</template>
        <ObjectName
          v-else-if="column.id === 'object'"
          :id="gapOf(row).object.id"
          :kind="gapOf(row).object.kind"
          :href="gapOf(row).object.href"
        />
        <time v-else-if="column.id === 'opened_at'" :datetime="gapOf(row).opened_at">{{
          when(gapOf(row).opened_at)
        }}</time>
      </template>
    </DataGrid>
  </Stack>
</template>
