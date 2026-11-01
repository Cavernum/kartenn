<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The filters of a column, for the catalogue of FilterPicker (020 decision A7): the bar and
// the funnels of the grid's headers read one model; Alt+Down on a header (or a click on its
// funnel) opens the bar's picker on that column's property (`openFor`), its filter to edit
// if applied, and the focus comes back to the header once it closes. Only the columns whose
// parameter the model serves have a funnel.
import { computed, ref, useTemplateRef } from "vue";
import DataGrid from "../components/DataGrid/DataGrid.vue";
import type { GridColumn } from "../components/DataGrid/grid.ts";
import FilterPicker from "../components/FilterPicker/FilterPicker.vue";
import type { Filter, FilterModel } from "../components/FilterPicker/filters.ts";
import Stack from "../components/Stack/Stack.vue";

const props = defineProps<{ model: FilterModel }>();
const filters = ref<Filter[]>([]);
const picker = useTemplateRef<InstanceType<typeof FilterPicker>>("picker");

/** The property of the model each column stands for. */
const COLUMN_FILTERS: Record<string, string> = { value: "value", usage: "usage" };
const columns: GridColumn[] = [
  { id: "value", label: "Adresse", width: "md", mono: true, filterable: true },
  { id: "usage", label: "Usage", width: "sm", filterable: true },
  { id: "machine", label: "Machine", width: "fill", mono: true },
];
const rows = [
  { id: "a1", value: "192.0.2.10", usage: "host", machine: "srv-app-01" },
  { id: "a2", value: "192.0.2.11", usage: "vip", machine: "—" },
  { id: "a3", value: "198.51.100.7", usage: "host", machine: "srv-rec-03" },
];
const filtered = computed(() =>
  Object.keys(COLUMN_FILTERS).filter((column) =>
    filters.value.some((f) => !f.draft && f.key === COLUMN_FILTERS[column]),
  ),
);
function onFilter(column: string): void {
  const from = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const key = COLUMN_FILTERS[column];
  if (key) picker.value?.openFor(key, from);
}
</script>

<template>
  <Stack gap="3">
    <FilterPicker
      ref="picker"
      v-model="filters"
      :model="props.model"
      label="Filtres des adresses"
    />
    <DataGrid
      label="Adresses"
      :columns="columns"
      :rows="rows"
      row-key="id"
      :filtered="filtered"
      @filter="onFilter"
    />
  </Stack>
</template>
