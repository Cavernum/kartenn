<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A grid of interfaces of any machine (task 18cc): the search of the view `interfaces` and
// the « Interfaces » tab of an address and of a VLAN. Each row: the interface, a link to its
// record; its machine, cited by its name; its MAC; its 802.1Q mode, its VLAN by their VID;
// its state, a `StatusBadge`. Enter on a cell without link opens the record of its row.
import { DataGrid, StatusBadge, type CellRef, type GridColumn, type GridWindow } from "@kartenn/ui";
import { computed } from "vue";
import CitedName from "./CitedName.vue";
import { arrival, followLink, hrefOf, PAGES } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import { modeOf, type Interface, type Names } from "./reads.ts";

const props = defineProps<{
  label: string;
  interfaces: readonly Interface[];
  names: () => Names;
  /** The first page is coming: skeleton rows. */
  loading: boolean;
  /** Pages remain to read: the count of the rows is unknown. */
  more: boolean;
}>();
const emit = defineEmits<{ window: [window: GridWindow] }>();

const t = useTexts();
const { context, element } = useHosted();

const columns = computed<GridColumn[]>(() => [
  { id: "name", label: t("interfaces.name"), width: "md", mono: true },
  { id: "machine", label: t("interfaces.machine"), width: "fill" },
  { id: "mac", label: t("interfaces.mac"), width: "md", mono: true },
  { id: "mode", label: t("interfaces.mode"), width: "md", priority: 2 },
  { id: "state", label: t("interfaces.state"), width: "sm", priority: 3 },
]);
const rows = computed(() =>
  props.interfaces.map((i) => ({
    id: i.id,
    name: i.name,
    machine: i.machine.id,
    mac: i.mac ?? null,
    mode: modeOf(i, props.names()),
    enabled: i.enabled,
  })),
);

/** The page of the record of an interface, at the dates of the context (W9). */
function recordOf(id: string): string {
  void context.value.time;
  return hrefOf(PAGES.interface(id));
}
/** Enter on a cell: the record of its row, its heading focused. */
function onActivate(cell: CellRef): void {
  if (!rows.value.some((r) => r.id === cell.row)) return;
  arrival.byLink = true;
  followLink(element, recordOf(cell.row));
}
</script>

<template>
  <DataGrid
    :label="props.label"
    :columns="columns"
    :rows="rows"
    row-key="id"
    :loading="props.loading"
    :loading-rows="4"
    :row-count="props.loading || props.more ? -1 : rows.length"
    @activate="onActivate"
    @window="(w) => emit('window', w)"
  >
    <template #cell="{ row, column }">
      <a v-if="column.id === 'name'" :href="recordOf(row.id)" class="k-mono">{{ row.name }}</a>
      <CitedName
        v-else-if="column.id === 'machine'"
        :id="row.machine"
        :names="props.names"
        type="machines.machine"
        :page="PAGES.machine(row.machine)"
      />
      <template v-else-if="column.id === 'mac'">
        <template v-if="row.mac">{{ row.mac }}</template>
        <span v-else
          ><span aria-hidden="true">—</span
          ><span class="k-visually-hidden">{{ t("none") }}</span></span
        >
      </template>
      <StatusBadge
        v-else-if="column.id === 'state'"
        :state="row.enabled ? 'ok' : 'neutral'"
        :label="t(row.enabled ? 'interfaces.enabled' : 'interfaces.disabled')"
      />
      <template v-else>{{ row.mode }}</template>
    </template>
  </DataGrid>
</template>
