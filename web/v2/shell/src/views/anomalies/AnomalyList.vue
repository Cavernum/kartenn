<script setup lang="ts">
// The open anomalies the reader may see (REQ-021-9; design 021 « Catalogue des écrans » :
// Anomalies et lacunes): filtered by rule, domain and entity in the URL, page by page, and
// acknowledged with a motive. Three states.
import { EmptyState, ErrorState, FilterPicker, Stack } from "@kartenn/ui";
import { useTemplateRef } from "vue";
import { useI18n } from "vue-i18n";
import { useSettings } from "../../frame/settings.ts";
import { useViewAction } from "../../shortcuts/view.ts";
import { problemOf } from "../data.ts";
import { useFilteredList } from "../list.ts";
import AnomalyGrid from "./AnomalyGrid.vue";
import type { Anomaly } from "./anomaly.ts";

const { t } = useI18n();
const settings = useSettings();
const { model, filters, onFilters, list, rows, failed, onWindow } = useFilteredList<Anomaly>(
  () => "/v1/anomalies",
);
const picker = useTemplateRef<InstanceType<typeof FilterPicker>>("picker");
useViewAction("filter", {
  label: () => t("views.list.filterAction", { name: t("nav.anomalies") }),
  run: () => picker.value?.openAdd(),
});
</script>

<template>
  <Stack gap="3">
    <FilterPicker
      ref="picker"
      :model-value="filters"
      :model="model"
      :label="t('views.list.filters', { name: t('nav.anomalies') })"
      :shortcut="settings.singleKeys.value ? ['/'] : undefined"
      @update:model-value="onFilters"
    />
    <ErrorState
      v-if="failed"
      :title="t('views.anomalies.error')"
      :problem="problemOf(list.error.value)"
      :retry-label="t('views.retry')"
      @retry="list.refetch()"
    />
    <EmptyState
      v-else-if="list.isSuccess.value && rows.length === 0"
      icon="circle-check"
      :title="t('views.anomalies.none')"
    >
      {{ t("views.anomalies.noneText") }}
    </EmptyState>
    <AnomalyGrid
      v-else
      :label="t('nav.anomalies')"
      :anomalies="rows"
      :loading="list.isPending.value"
      with-object
      @window="onWindow"
    />
  </Stack>
</template>
